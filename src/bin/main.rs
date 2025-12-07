#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
  holding buffers for the duration of a data transfer."
)]

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level;
use esp_hal::{handler, main};
use esp_hal::rmt::{PulseCode, Rmt, TxChannelConfig, TxChannelCreator};
use esp_hal::time::{Instant, Rate};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use heapless::spsc::{Producer, Queue};
use rgb_led::{LEDStrip, NUM_LEDS, StripSetting, SerialParser};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
  loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static USB_SERIAL: Mutex<RefCell<Option<UsbSerialJtag<'static, esp_hal::Blocking>>>> =
  Mutex::new(RefCell::new(None));

static mut USB_QUEUE: Queue<u8, { 16*1024 }> = Queue::new();
static mut USB_PRODUCER: Option<Producer<'static, u8>> = None;

#[handler]
fn usb_serial_isr() {
  critical_section::with(|cs| {
    let mut usb_serial = USB_SERIAL.borrow_ref_mut(cs);
    if let Some(usb_serial) = usb_serial.as_mut() {
      // Read and store in buffer. Data will be processed in main loop.
      // I'd like to do "If buffer is full, discard oldest data."
      // But I made myself able to access only the Producer here (for performance gains hopefully)
      // So instead just discard new data if full.
      unsafe {
        #[allow(static_mut_refs)]
        if let Some(producer) = USB_PRODUCER.as_mut() {
          while let Ok(byte) = usb_serial.read_byte() {
            if producer.enqueue(byte).is_err() {
              break; // Buffer full, discard remaining data
            }
          }
        }
      }
      usb_serial.reset_rx_packet_recv_interrupt();
    }
  });
}

#[main]
fn main() -> ! {
  let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
  let peripherals = esp_hal::init(config);
  let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80)).unwrap();

  let consumer = unsafe {
    // These invariants have to be met to keep safety:
    // Only one mutable reference exists to the queue, producer, and consumer.
    // split() is only called once and nothing touches USB_QUEUE afterwards.
    // Don't touch USB_QUEUE after this.
    #[allow(static_mut_refs)]
    let (producer, consumer) = USB_QUEUE.split();
    USB_PRODUCER = Some(producer);
    consumer
  };

  let mut usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
  usb_serial.set_interrupt_handler(usb_serial_isr);
  usb_serial.listen_rx_packet_recv_interrupt(); // Enable RX interrupt
  critical_section::with(|cs| USB_SERIAL.borrow_ref_mut(cs).replace(usb_serial)); // Store in mutex

  // Configure TX channel on GPIO3
  let mut channel = rmt
    .channel0
    .configure_tx(peripherals.GPIO3,
      TxChannelConfig::default()
        .with_clk_divider(1)
        .with_idle_output_level(Level::Low)
        .with_idle_output(true)
    )
    .unwrap();

  let mut strip: LEDStrip = LEDStrip::new();
  strip.set_phase_step(0.01);
  strip.set_brightness(0.05);
  strip.set_setting(StripSetting::RainbowCycle {
    cycles: 2.0,
  });

  let mut pulse_buffer = [PulseCode::default(); NUM_LEDS * 24 + 1];
  let delay = Delay::new();
  let mut serial_parser = SerialParser::new(consumer);

  loop {
    let now = Instant::now();

    let frame_duration_ms = 1000.0 / (strip.get_frames_per_second() as f32);

    let command = serial_parser.read_buffer_into_command();
    if let Some(command) = &command {
      strip.apply_command(command);
    }

    let changed = strip.update_pixels();
    if changed {
      strip.fill_pulse_data();
      let pulse_data = strip.get_pulse_data(&mut pulse_buffer);
      let transaction = channel.transmit(pulse_data).unwrap();
      channel = transaction.wait().unwrap();
    }

    // For some reason if this runs and I disconnect serial monitor, the strip stops updating.
    // Probably hanging on the write?
    // critical_section::with(|cs| {
    //   if let Some(usb_serial) = USB_SERIAL.borrow_ref_mut(cs).as_mut() {
    //     print_elapsed_time(
    //       usb_serial,
    //       elapsed,
    //     );
    //   }
    // });

    // wait such that FRAME_DURATION_MS per frame is maintained
    let elapsed = now.elapsed();
    delay.delay_micros(((frame_duration_ms * 1000.0) as u32).saturating_sub(elapsed.as_micros() as u32));
  }
}