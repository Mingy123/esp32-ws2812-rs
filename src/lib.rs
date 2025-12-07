#![no_std]

mod algo;
mod command;

use esp_hal::rmt::PulseCode;
use micromath::F32Ext;

use crate::algo::{hsv_to_rgb, rgb_to_pulses};
use crate::command::SerialCommand;

pub use crate::algo::print_elapsed_time;
pub use crate::command::SerialParser;

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
  Custom,
  Breathing { r: u8, g: u8, b: u8 },
  SolidColor { r: u8, g: u8, b: u8 },
  /// Rainbow cycle animation. `cycles` defines how many full rainbow cycles
  /// appear across the entire strip length (e.g., 1.0 = one rainbow, 2.0 = two rainbows)
  RainbowCycle { cycles: f32 },
}

pub struct LEDStrip {
  /// Whether update_pixels() should render anything
  is_on: bool,
  /// Buffer holding the RGB values for each LED
  pixels: [RGBPixel; NUM_LEDS],
  /// Buffer holding the RMT pulse data for the entire strip
  pulse_data: [PulseCode; NUM_LEDS * 24 + 1],
  /// Setting for rendering pixels in update_pixels()
  setting: StripSetting,
  /// Global brightness level, applied in update_pixels().
  /// Can be anything above 0.0, above 1.0 to brighten further.
  brightness: f32,
  /// Phase counter for animations, ranges from 0.0 to 1.0 per cycle
  phase: f32,
  /// How much to increment phase per update (speed of animation)
  phase_step: f32,
  /// Number of LEDs to update when filling pulse data
  num_leds_to_update: usize,
  /// Number of update + write to RMT per second
  frames_per_second: u8,
}

impl Default for LEDStrip {
  fn default() -> Self {
    Self::new()
  }
}

impl LEDStrip {
  pub fn new() -> Self {
    Self {
      is_on: true,
      pixels: [RGBPixel::off(); NUM_LEDS],
      pulse_data: [PulseCode::default(); NUM_LEDS * 24 + 1],
      setting: StripSetting::Custom,
      brightness: 1.0,
      phase: 0.0,
      phase_step: 0.01,
      num_leds_to_update: NUM_LEDS,
      frames_per_second: 25,
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

  pub fn set_setting(&mut self, setting: StripSetting) {
    self.setting = setting;
  }

  pub fn get_setting(&self) -> StripSetting {
    self.setting
  }

  pub fn set_brightness(&mut self, brightness: f32) {
    self.brightness = brightness;
  }

  pub fn get_brightness(&self) -> f32 {
    self.brightness
  }

  pub fn set_phase_step(&mut self, fpc: f32) {
    self.phase_step = fpc;
  }

  pub fn get_phase_step(&self) -> f32 {
    self.phase_step
  }

  pub fn get_frames_per_second(&self) -> u8 {
    self.frames_per_second
  }

  // Return a slice from the same one as the input buffer because if the buffer is bigger than necessary,
  // only the first part should be sent.
  // The last PulseCode needs to be the end marker.

  /// Wrapper to get pulse data based on num_leds_to_update
  pub fn get_pulse_data<'a>(&self, buffer: &'a mut [PulseCode]) -> &'a [PulseCode] {
    if self.num_leds_to_update >= NUM_LEDS {
      self.get_pulse_data_all(buffer)
    } else {
      self.get_pulse_data_limited(self.num_leds_to_update, buffer)
    }
  }

  /// Copy pulse data into the provided buffer.
  fn get_pulse_data_all<'a>(&self, buffer: &'a mut [PulseCode]) -> &'a [PulseCode] {
    if buffer.len() < self.pulse_data.len() {
      panic!("Buffer too small for pulse data");
    }
    buffer.copy_from_slice(&self.pulse_data);
    &buffer[..self.pulse_data.len()]
  }

  /// Copy pulse data for `num` LEDs into the provided buffer.
  /// Adds end marker after the specified number of LEDs.
  fn get_pulse_data_limited<'a>(&self, num: usize, buffer: &'a mut [PulseCode]) -> &'a [PulseCode] {
    let len = if num <= NUM_LEDS { num } else { NUM_LEDS };
    let required_len = len * 24 + 1;
    if buffer.len() < required_len {
      panic!("Buffer too small for limited pulse data");
    }
    buffer[..len * 24].copy_from_slice(&self.pulse_data[..len * 24]);
    buffer[len * 24] = PulseCode::end_marker();
    &buffer[..required_len]
  }

  /// Fill `pulse_data` buffer with current pixel state
  pub fn fill_pulse_data(&mut self) {
    for (i, pixel) in self.pixels.iter().enumerate() {
      rgb_to_pulses(pixel, &mut self.pulse_data[i * 24..(i + 1) * 24]);
    }
    self.pulse_data[NUM_LEDS * 24] = PulseCode::end_marker();
  }

  pub fn update_pixels(&mut self) -> bool {
    let mut changed = false;

    if !self.is_on {
      changed = self.clear();
      return changed;
    }
    match self.setting {
      StripSetting::Breathing { r, g, b } => {
        // Calculate brightness factor using sine wave
        let brightness_factor = (0.5 + 0.5 * (self.phase * core::f32::consts::TAU).sin()) * self.brightness;
        let new_r = ((r as f32 * brightness_factor).clamp(0.0, 255.0)) as u8;
        let new_g = ((g as f32 * brightness_factor).clamp(0.0, 255.0)) as u8;
        let new_b = ((b as f32 * brightness_factor).clamp(0.0, 255.0)) as u8;
        for pixel in self.pixels.iter_mut() {
          if pixel.r != new_r || pixel.g != new_g || pixel.b != new_b {
            changed = true;
            pixel.r = new_r;
            pixel.g = new_g;
            pixel.b = new_b;
          }
        }
      }
      StripSetting::SolidColor { r, g, b } => {
        for pixel in self.pixels.iter_mut() {
          let new_r = ((r as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          let new_g = ((g as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          let new_b = ((b as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          if pixel.r != new_r || pixel.g != new_g || pixel.b != new_b {
            changed = true;
            pixel.r = new_r;
            pixel.g = new_g;
            pixel.b = new_b;
          }
        }
      }
      StripSetting::RainbowCycle { cycles } => {
        let len = self.pixels.len() as f32;
        for (i, pixel) in self.pixels.iter_mut().enumerate() {
          // Calculate hue: position along strip * cycles * 360 degrees + animation offset
          let hue = ((i as f32 / len) * cycles * 360.0 + self.phase * 360.0) % 360.0;
          let rgb = hsv_to_rgb(hue as u16, 255, 255);
          let new_r = ((rgb.r as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          let new_g = ((rgb.g as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          let new_b = ((rgb.b as f32 * self.brightness).clamp(0.0, 255.0)) as u8;
          if pixel.r != new_r || pixel.g != new_g || pixel.b != new_b {
            changed = true;
            pixel.r = new_r;
            pixel.g = new_g;
            pixel.b = new_b;
          }
        }
      }
      StripSetting::Custom => {
        // For the user to custom set pixels directly, do nothing here
        changed = true;
      }
    }
    // Advance phase for animations
    self.phase = (self.phase + self.phase_step) % 1.0;
    changed
  }

  pub fn clear(&mut self) -> bool {
    let mut changed = false;
    for pixel in self.pixels.iter_mut() {
      if pixel.r != 0 || pixel.g != 0 || pixel.b != 0 {
        changed = true;
        *pixel = RGBPixel::off();
      }
    }
    changed
  }

  /// Applies a SerialCommand modifying the LED strip settings or individual pixels.
  pub fn apply_command(&mut self, command: &SerialCommand) {
    match command.action {
      0x01 => { // Set on / off
        let state = command.data[0];
        self.is_on = state != 0;
      },
      0x02 => { // Set global brightness
        let brightness = f32::from_be_bytes([
          command.data[0],
          command.data[1],
          command.data[2],
          command.data[3],
        ]);
        self.set_brightness(brightness);
      },
      0x03 => { // Set StripSetting
        let setting_id = command.data[0];
        let setting = match setting_id {
          0x00 => StripSetting::Custom,
          0x01 => {
            StripSetting::Breathing {
              r: command.data[1],
              g: command.data[2],
              b: command.data[3],
            }
          },
          0x02 => {
            StripSetting::SolidColor {
              r: command.data[1],
              g: command.data[2],
              b: command.data[3],
            }
          },
          0x03 => {
            let cycles = f32::from_be_bytes([
              command.data[1],
              command.data[2],
              command.data[3],
              command.data[4],
            ]);
            StripSetting::RainbowCycle { cycles }
          },
          _ => return, // Unknown setting, ignore
        };
        self.set_setting(setting);
      },
      0x04 => { // Manual color input
        let start_index = u16::from_be_bytes([command.data[0], command.data[1]]) as usize;
        let color_data = &command.data[2..(command.length as usize)];
        let num_leds = color_data.len() / 3;

        self.set_setting(StripSetting::Custom);

        for i in 0..num_leds {
          let led_index = start_index + i;
          if led_index >= NUM_LEDS {
            break; // Don't exceed strip bounds
          }
          let offset = i * 3;
          self.set_pixel(led_index, RGBPixel::new(
            color_data[offset],
            color_data[offset + 1],
            color_data[offset + 2],
          ));
        }
      },
      0x05 => { // Set phase step
        let phase_step = f32::from_be_bytes([
          command.data[0],
          command.data[1],
          command.data[2],
          command.data[3],
        ]);
        self.set_phase_step(phase_step);
      },
      0x06 => { // Set num_leds_to_update
        let num_leds = u16::from_be_bytes([command.data[0], command.data[1]]) as usize;
        self.num_leds_to_update = num_leds.min(NUM_LEDS);
      },
      0x07 => { // Set frames_per_second
        let fps = command.data[0];
        self.frames_per_second = fps;
      },
      _ => {
        // Unknown command, ignore
      }
    }
  }
}