#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
  holding buffers for the duration of a data transfer."
)]

use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::rmt::{PulseCode, Rmt, TxChannelCreator};
use esp_hal::time::{Duration, Instant, Rate};
use rgb_led::{LEDStrip, PULSE_DATA_SIZE, RGBPixel, StripSetting, ws2812_tx_config};

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

  // Configure TX channel on GPIO3
  let mut channel = rmt
    .channel0
    .configure_tx(peripherals.GPIO3, ws2812_tx_config())
    .unwrap();

  let mut strip: LEDStrip = LEDStrip::new();
  // strip.set_setting(StripSetting::RainbowCycle {
  //   cycles: 2.0,
  //   brightness: 0.1,    
  // });
  strip.set_setting(StripSetting::Custom);
  strip.set_pixel(0, RGBPixel::blue());
  strip.set_pixel(90, RGBPixel::blue());
  strip.set_pixel(120, RGBPixel::blue());
  strip.set_pixel(220, RGBPixel::blue());
  strip.set_pixel(278, RGBPixel::blue());

  let mut pulse_data: [PulseCode; PULSE_DATA_SIZE] = [PulseCode::default(); PULSE_DATA_SIZE];

  // Main loop: update pixels, fill pulse data, and transmit
  loop {
    // Recompute pixel data
    strip.update_pixels();
    // Send data to LED strip
    strip.fill_pulse_data(&mut pulse_data);
    let transaction = channel.transmit(&pulse_data).unwrap();
    channel = transaction.wait().unwrap();
    let delay_start = Instant::now();
    while delay_start.elapsed() < Duration::from_millis(20) {}
  }
}
