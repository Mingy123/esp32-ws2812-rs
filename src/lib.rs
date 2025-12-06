#![no_std]

use esp_hal::gpio::Level;
use esp_hal::rmt::{PulseCode, TxChannelConfig};

pub const NUM_LEDS: usize = 280;
pub const PULSE_DATA_SIZE: usize = NUM_LEDS * 24 + 1;

// WS2812B timing (in RMT ticks at 80MHz clock with divider 1)
// T0H = 0.4us = 32 ticks, T0L = 0.85us = 68 ticks
// T1H = 0.8us = 64 ticks, T1L = 0.45us = 36 ticks
const WS2812_T0H: u16 = 32;
const WS2812_T0L: u16 = 68;
const WS2812_T1H: u16 = 64;
const WS2812_T1L: u16 = 36;

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

pub fn hsv_to_rgb(h: u16, s: u8, v: u8) -> RGBPixel {
  // Normalize h to 0-359 range
  let h = h % 360;
  
  // c = chroma = v * s
  let c = (v as u32 * s as u32) / 255;
  
  // h' = h / 60 (which sector of the color wheel)
  // x = c * (1 - |h' mod 2 - 1|)
  // We compute this using fixed-point math to avoid issues
  let h_prime = h as u32; // 0-359
  let sector = h_prime / 60; // 0-5
  let h_mod = h_prime % 60; // position within sector (0-59)
  
  // |h' mod 2 - 1| ranges from 0 to 1 as h_mod goes 0->60 or 60->0
  // For even sectors (0,2,4): h_mod goes 0->59, so factor = h_mod/60
  // For odd sectors (1,3,5): h_mod goes 0->59, so factor = 1 - h_mod/60
  let x = if sector % 2 == 0 {
    // Rising edge: x goes from 0 to c as h_mod goes 0 to 59
    (c * h_mod) / 60
  } else {
    // Falling edge: x goes from c to 0 as h_mod goes 0 to 59
    (c * (60 - h_mod)) / 60
  };
  
  let m = v as u32 - c;

  let (r1, g1, b1) = match sector {
    0 => (c, x, 0),     // Red to Yellow
    1 => (x, c, 0),     // Yellow to Green
    2 => (0, c, x),     // Green to Cyan
    3 => (0, x, c),     // Cyan to Blue
    4 => (x, 0, c),     // Blue to Magenta
    _ => (c, 0, x),     // Magenta to Red (sector 5)
  };

  RGBPixel {
    r: (r1 + m) as u8,
    g: (g1 + m) as u8,
    b: (b1 + m) as u8,
  }
}

/// Convert a single byte to 8 PulseCodes for WS2812B
fn byte_to_pulses(byte: u8, pulses: &mut [PulseCode]) {
  for i in 0..8 {
    let bit = (byte >> (7 - i)) & 1;
    pulses[i] = if bit == 1 {
      PulseCode::new(Level::High, WS2812_T1H, Level::Low, WS2812_T1L)
    } else {
      PulseCode::new(Level::High, WS2812_T0H, Level::Low, WS2812_T0L)
    };
  }
}

/// Convert RGB color to WS2812B pulse data (GRB order)
fn rgb_to_pulses(pixel: &RGBPixel, pulses: &mut [PulseCode]) {
  byte_to_pulses(pixel.g, &mut pulses[0..8]);
  byte_to_pulses(pixel.r, &mut pulses[8..16]);
  byte_to_pulses(pixel.b, &mut pulses[16..24]);
}

pub struct LEDStrip<const N: usize = NUM_LEDS> {
  pixels: [RGBPixel; N],
  setting: StripSetting,
  frame: u32,
}

impl<const N: usize> LEDStrip<N> {
  pub fn new() -> Self {
    Self {
      pixels: [RGBPixel::off(); N],
      setting: StripSetting::Off,
      frame: 0,
    }
  }

  pub fn set_pixel(&mut self, index: usize, pixel: RGBPixel) {
    if index < N {
      self.pixels[index] = pixel;
    }
  }

  pub fn get_pixel(&self, index: usize) -> Option<&RGBPixel> {
    self.pixels.get(index)
  }

  pub fn set_setting(&mut self, setting: StripSetting) {
    self.setting = setting;
  }

  pub fn len(&self) -> usize {
    N
  }

  /// Fill pulse data buffer with current pixel state
  /// Returns the number of pulse codes written (including end marker)
  pub fn fill_pulse_data(&self, pulse_data: &mut [PulseCode]) -> usize {
    let required_size = N * 24 + 1;
    assert!(pulse_data.len() >= required_size, "pulse_data buffer too small");

    for (i, pixel) in self.pixels.iter().enumerate() {
      rgb_to_pulses(pixel, &mut pulse_data[i * 24..(i + 1) * 24]);
    }
    pulse_data[N * 24] = PulseCode::end_marker();
    required_size
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

impl<const N: usize> Default for LEDStrip<N> {
  fn default() -> Self {
    Self::new()
  }
}

/// Helper to create the TX channel configuration for WS2812B
pub fn ws2812_tx_config() -> TxChannelConfig {
  TxChannelConfig::default()
    .with_clk_divider(1)
    .with_idle_output_level(Level::Low)
    .with_idle_output(true)
}