#![no_std]

mod algo;

use esp_hal::rmt::PulseCode;

use crate::algo::{hsv_to_rgb, rgb_to_pulses};

pub const NUM_LEDS: usize = 280;

#[derive(Copy, Clone, Default)]
pub struct RGBPixel {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

impl RGBPixel {
  pub const fn new(r: u8, g: u8, b: u8) -> Self {
    Self { r, g, b }
  }

  pub const fn off() -> Self {
    Self { r: 0, g: 0, b: 0 }
  }

  pub const fn blue() -> Self {
    Self { r: 0, g: 0, b: 255 }
  }

  pub const fn red() -> Self {
    Self { r: 255, g: 0, b: 0 }
  }

  pub const fn green() -> Self {
    Self { r: 0, g: 255, b: 0 }
  }
}

#[derive(Copy, Clone)]
pub enum StripSetting {
  SolidColor { r: u8, g: u8, b: u8 },
  /// Rainbow cycle animation. `cycles` defines how many full rainbow cycles
  /// appear across the entire strip length (e.g., 1.0 = one rainbow, 2.0 = two rainbows)
  RainbowCycle { cycles: f32, brightness: f32 },
  Custom,
  Off,
}

pub struct LEDStrip {
  pixels: [RGBPixel; NUM_LEDS],
  pulse_data: [PulseCode; NUM_LEDS * 24 + 1],
  setting: StripSetting,
  frame: u32,
}

impl LEDStrip {
  pub fn new() -> Self {
    Self {
      pixels: [RGBPixel::off(); NUM_LEDS],
      pulse_data: [PulseCode::default(); NUM_LEDS * 24 + 1],
      setting: StripSetting::Off,
      frame: 0,
    }
  }

  pub fn set_pixel(&mut self, index: usize, pixel: RGBPixel) {
    if index < NUM_LEDS {
      self.pixels[index] = pixel;
    }
  }

  pub fn get_pixel(&self, index: usize) -> Option<&RGBPixel> {
    self.pixels.get(index)
  }

  pub fn get_pulse_data(&self) -> &[PulseCode] {
    &self.pulse_data
  }

  /// Get pulse data for `num` LEDs.
  /// Modifies its own `pulse_data` buffer to add end marker.
  pub fn get_pulse_data_limited(&mut self, num: usize) -> &[PulseCode] {
    let len = if num <= NUM_LEDS {
      num
    } else {
      NUM_LEDS
    };
    self.pulse_data[len * 24] = PulseCode::end_marker();
    &self.pulse_data[..len * 24 + 1]
  }

  pub fn set_setting(&mut self, setting: StripSetting) {
    self.setting = setting;
  }

  /// Fill `pulse_data` buffer with current pixel state
  pub fn fill_pulse_data(&mut self) {
    for (i, pixel) in self.pixels.iter().enumerate() {
      rgb_to_pulses(pixel, &mut self.pulse_data[i * 24..(i + 1) * 24]);
    }
    self.pulse_data[NUM_LEDS * 24] = PulseCode::end_marker();
  }

  pub fn update_pixels(&mut self) {
    match self.setting {
      StripSetting::SolidColor { r, g, b } => {
        for pixel in self.pixels.iter_mut() {
          pixel.r = r;
          pixel.g = g;
          pixel.b = b;
        }
      }
      StripSetting::RainbowCycle { cycles, brightness } => {
        let len = self.pixels.len() as f32;
        for (i, pixel) in self.pixels.iter_mut().enumerate() {
          // Calculate hue: position along strip * cycles * 360 degrees + animation offset
          let hue = ((i as f32 / len) * cycles * 360.0 + self.frame as f32) % 360.0;
          let value = brightness * 255.0;
          let rgb = hsv_to_rgb(hue as u16, 255, value as u8);
          pixel.r = rgb.r;
          pixel.g = rgb.g;
          pixel.b = rgb.b;
        }
      }
      StripSetting::Off => {
        self.clear();
      }
      StripSetting::Custom => {
        // Custom pattern logic can be implemented here
      }
    }
    // Advance frame for animations
    self.frame = (self.frame + 1) % 360;
  }

  pub fn clear(&mut self) {
    for pixel in self.pixels.iter_mut() {
      *pixel = RGBPixel::off();
    }
  }
}

impl Default for LEDStrip {
  fn default() -> Self {
    Self::new()
  }
}