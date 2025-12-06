use esp_hal::{gpio::Level, rmt::PulseCode};

use crate::RGBPixel;


// WS2812B timing (in RMT ticks at 80MHz clock with divider 1)
// T0H = 0.4us = 32 ticks, T0L = 0.85us = 68 ticks
// T1H = 0.8us = 64 ticks, T1L = 0.45us = 36 ticks
// In my testing changing T1L to 48 ticks (0.6us) reduces flickering at the end of the strip
const WS2812_T0H: u16 = 32;
const WS2812_T0L: u16 = 68;
const WS2812_T1H: u16 = 64;
const WS2812_T1L: u16 = 48;

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
pub fn rgb_to_pulses(pixel: &RGBPixel, pulses: &mut [PulseCode]) {
  byte_to_pulses(pixel.g, &mut pulses[0..8]);
  byte_to_pulses(pixel.r, &mut pulses[8..16]);
  byte_to_pulses(pixel.b, &mut pulses[16..24]);
}
