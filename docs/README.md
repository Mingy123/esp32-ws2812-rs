Here I talk about everything going on in this project yes

The entry point is the main function in `src/bin/main.rs`.  
There we initialise the stuff we need e.g. the heapless Queue, USB Serial JTAG, the GPIO pin, and my own LEDStrip struct.

The heapless queue is like a ring buffer.  
I could have just used it like a normal queue but there was an example showing that i could use it as
a SPSC (Single Provider Single Consumer) queue, which makes sense since only the ISR will
provide data and only the main loop, which is not multithreaded, will consume data.  
And we have to put the provider in a static mut because the ISR is some special function that is
called on a hardware interrupt and we cannot pass parameters to it like normal.
  
USB_SERIAL is in a static mutex so that both the ISR function and the main runtime can access it.  
Though as of now the main loop doesn't use it because printing to serial caused some issues.  
So if that remains I could put that in a static mut just like the provider.

## `struct LEDStrip`

This holds an array of RGBPixel, which is just 3 u8's, and other settings of the strip:

- `setting`  
  Stuff like SolidColor, Rainbow. Used in update_pixels() to determine how the colours are rendered.  
  Can be thought of as a hardcoded animation configuration setting.
- `phase` and `phase_step`  
  These hardcoded animations should change depending on time (i.e. the colours should actually change).  
  To do this, we keep a variable `phase` that goes from 0 to 1, and `phase_step` is the amount it steps
  every time update_pixels() is called.
- `num_leds_to_update`  
  I sometimes want to only light the first N LEDs in the strip to increase the FPS I can get out of the strip
  while maintaining the reliability (no flickering etc.)  
  Setting this value will make `get_pulse_data` return data for only N LEDs, though all LEDs will still be rendered.  
  Computational power shouldn't be the bottleneck though, unless some crazy animation is added later on.

Everything else should be self-explanatory.

## Serial interface

I defend my design choices here

As mentioned earlier, a heapless queue was used to store data from the ISR. The ISR receives raw bytes from
the serial connection and will throw them all on this queue.  
This queue is allocated 16kB which should be enough for at least 15 proper frames.

We should not trust the serial connection to provide reliable data. This is the first entry step of external data.  
Therefore we need a function to parse this data into a clear struct that we know is valid.

We should also assume that the ISR may run with any amount of data in the serial buffer, which may not
be enough for a complete frame. So perhaps somewhere between the app's code and our microcontroller,
the USB serial packet gets split up.

Therefore, we create our own buffer for this data.

We only need 1 + 1 + 2 + 1024 + 2 bytes for each proper frame. But just in case some weird stuff happens
it's always better to have more.

#### Parsing the data

Don't do this in the ISR because the ISR should be as lightweight and fast as possible.

We specified that each frame starts with a header 0xAA. For frames controlling raw RGB data, this byte can easily
show up, so we can't say that a frame will exist at any 0xAA byte we see.  
This is especially important considering some frames may get malformed. Imagine one where the length value
increases beyond what it actually is and we end up reading more than we should have from the queue.

Therefore we need another buffer to store recently read bytes. This is put in the `SerialParser` struct.  
We can use the `find_next_header_and_shift` function to find the next 0xAA byte in the buffer (if any)
to attempt to continue forming a frame from there. And if it fails, we throw away the data until the next
0xAA byte, and start again. This makes for a pretty robust error recovery process.