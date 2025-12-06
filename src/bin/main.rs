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
use rgb_led::{LEDStrip, NUM_LEDS, StripSetting, print_elapsed_time};

const FRAME_DURATION_MS: u64 = 20;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
  loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static USB_SERIAL: Mutex<RefCell<Option<UsbSerialJtag<'static, esp_hal::Blocking>>>> =
  Mutex::new(RefCell::new(None));

static LED_STRIP: Mutex<RefCell<Option<LEDStrip>>> =
  Mutex::new(RefCell::new(None));

#[handler]
fn usb_device() {
  critical_section::with(|cs| {
    let mut usb_serial = USB_SERIAL.borrow_ref_mut(cs);
    if let Some(usb_serial) = usb_serial.as_mut() {
      while let Ok(c) = usb_serial.read_byte() {
        // Parse bytes and update LED strip setting if valid
        // Rudimentary command parser. Will upgrade later.
        if c == b'0' {
          // Turn off strip
          critical_section::with(|cs| {
            if let Some(strip) = LED_STRIP.borrow_ref_mut(cs).as_mut() {
              strip.set_setting(StripSetting::Off);
            }
          });
        } else if c == b'1' {
          // Solid red
          critical_section::with(|cs| {
            if let Some(strip) = LED_STRIP.borrow_ref_mut(cs).as_mut() {
              strip.set_setting(StripSetting::SolidColor { r: 255, g: 0, b: 0 });
            }
          });
        } else if c == b'2' {
          // Rainbow cycle
          critical_section::with(|cs| {
            if let Some(strip) = LED_STRIP.borrow_ref_mut(cs).as_mut() {
              strip.set_setting(StripSetting::RainbowCycle { cycles: 2.0 });
            }
          });
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

  let mut usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
  usb_serial.write(b"LED Strip Example Starting...\n").unwrap();
  usb_serial.set_interrupt_handler(usb_device);
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
  strip.set_frame_per_cycle(0.01);
  strip.set_brightness(0.2);

  strip.set_setting(StripSetting::RainbowCycle {
    cycles: 2.0,
  });

  critical_section::with(|cs| LED_STRIP.borrow_ref_mut(cs).replace(strip));

  let mut pulse_buffer = [PulseCode::default(); NUM_LEDS * 24 + 1];
  let delay = Delay::new();

  loop {
    let now = Instant::now();
    let result = critical_section::with(|cs| {
      if let Some(strip) = LED_STRIP.borrow_ref_mut(cs).as_mut() {
        strip.update_pixels();
        strip.fill_pulse_data();
        Some(strip.get_pulse_data_limited(88, &mut pulse_buffer))
      } else {
        None
      }
    });

    match result {
      Some(pulse_data) => {
        let transaction = channel.transmit(pulse_data).unwrap();
        channel = transaction.wait().unwrap();
      },
      None => {
        // If we failed to get the LED strip data, just wait and try again
        let elapsed = now.elapsed();
        delay.delay_millis((FRAME_DURATION_MS - elapsed.as_millis()) as u32);
        continue;
      },
    }

    let elapsed = now.elapsed();
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
    if elapsed.as_millis() < FRAME_DURATION_MS {
      delay.delay_millis((FRAME_DURATION_MS - elapsed.as_millis()) as u32);
    }
  }
}