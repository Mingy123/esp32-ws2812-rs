#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
  holding buffers for the duration of a data transfer."
)]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level;
use esp_hal::main;
use esp_hal::rmt::{Rmt, TxChannelConfig, TxChannelCreator};
use esp_hal::time::{Instant, Rate};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use rgb_led::{LEDStrip, StripSetting, print_elapsed_time};

const FRAME_DURATION_MS: u64 = 20;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
  loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
  let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
  let peripherals = esp_hal::init(config);
  let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80)).unwrap();

  let mut usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
  usb_serial.write(b"LED Strip Example Starting...\n").unwrap();

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

  let delay = Delay::new();
  let mut strip: LEDStrip = LEDStrip::new();
  strip.set_frame_per_cycle(0.01);
  strip.set_brightness(0.2);

  strip.set_setting(StripSetting::RainbowCycle {
    cycles: 2.0,
  });

  // Main loop: update pixels, fill pulse data, and transmit
  loop {
    let now = Instant::now();
    strip.update_pixels();
    strip.fill_pulse_data();
    let pulse_data = strip.get_pulse_data_limited(88);
    let transaction = channel.transmit(pulse_data).unwrap();
    channel = transaction.wait().unwrap();
    let elapsed = now.elapsed();
    // print_elapsed_time(&mut usb_serial, elapsed);
    // wait such that FRAME_DURATION_MS per frame is maintained
    if elapsed.as_millis() < FRAME_DURATION_MS {
      delay.delay_millis((FRAME_DURATION_MS - elapsed.as_millis()) as u32);
    }
  }
}