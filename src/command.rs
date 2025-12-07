use core::panic;

use heapless::spsc::Consumer;

/// One frame (command) received over serial
pub struct SerialCommand {
  /// Type of command
  pub action: u8,
  /// Max 1024
  pub length: u16,
  /// Just a buffer, only `length` bytes are valid
  pub data: [u8; 1024],
  // CRC-16-CCITT checksum
  pub checksum: u16,
}

impl SerialCommand {
  pub fn new() -> Self {
    SerialCommand {
      action: 0,
      length: 0,
      data: [0; 1024],
      checksum: 0,
    }
  }

  /// Calculate CRC-16-CCITT checksum for the command
  /// CRC is calculated over: action (1 byte) -> length (2 bytes) -> data (length bytes)
  pub fn calculate_checksum(&self) -> u16 {
    let mut crc: u16 = 0xFFFF; // Initial value for CRC-16-CCITT

    // Process action/ byte
    crc = Self::update_crc(crc, self.action);

    // Process length field (big-endian)
    crc = Self::update_crc(crc, ((self.length >> 8) & 0xFF) as u8);
    crc = Self::update_crc(crc, (self.length & 0xFF) as u8);

    // Process data field (only up to length bytes)
    let data_len = self.length.min(1024) as usize;
    for i in 0..data_len {
      crc = Self::update_crc(crc, self.data[i]);
    }

    crc
  }

  /// Update CRC-16-CCITT with one byte
  fn update_crc(crc: u16, byte: u8) -> u16 {
    let mut crc = crc;
    crc ^= (byte as u16) << 8;

    for _ in 0..8 {
      if (crc & 0x8000) != 0 {
        crc = (crc << 1) ^ 0x1021; // CRC-16-CCITT polynomial
      } else {
        crc <<= 1;
      }
    }

    crc
  }

  /// Verify that the checksum field matches the calculated checksum
  pub fn verify_checksum(&self) -> bool {
    self.checksum == self.calculate_checksum()
  }
}



pub struct SerialParser {
  buffer: [u8; 1024 + 512], // extra space in case
  buffer_len_in_use: usize,
  consumer: Consumer<'static, u8>,
}

impl SerialParser {

  pub fn new(consumer: Consumer<'static, u8>) -> Self {
    SerialParser {
      buffer: [0; 1024 + 512],
      buffer_len_in_use: 0,
      consumer,
    }
  }

  /// Add a byte to the buffer
  fn buffer_push(&mut self, byte: u8) {
    if self.buffer_len_in_use >= self.buffer.len() {
      panic!("Buffer overflow in SerialParser");
    }
    self.buffer[self.buffer_len_in_use] = byte;
    self.buffer_len_in_use += 1;
  }

  /// Find the next 0xAA header byte in the buffer and shift data to the beginning.
  /// Returns true if a header was found, false if no header exists in the buffer.
  fn find_next_header_and_shift(&mut self) -> bool {
    // Look for the next 0xAA starting from index 1 (skip the first byte)
    for i in 1..self.buffer_len_in_use {
      if self.buffer[i] == 0xAA {
        // Found a header, shift data to the beginning
        let shift_amount = i;
        let new_len = self.buffer_len_in_use - shift_amount;

        // Copy data to the beginning
        for j in 0..new_len {
          self.buffer[j] = self.buffer[j + shift_amount];
        }

        self.buffer_len_in_use = new_len;
        return true;
      }
    }

    // No header found, clear the buffer
    self.buffer_len_in_use = 0;
    false
  }

  // 1. Fill buffer from consumer until we have enough data or consumer is empty
  // 2. Try to parse a frame from the buffer
  // 3. If frame is malformed, find next header in buffer and retry
  // 4. If frame is valid, clear buffer and return the command
  /// Read bytes from the consumer buffer and parse into a SerialCommand
  pub fn read_buffer_into_command(
    &mut self
  ) -> Option<SerialCommand> {

    loop {
      // Fill buffer from consumer
      while let Some(byte) = self.consumer.dequeue() {
        self.buffer_push(byte);

        if self.buffer_len_in_use >= 1056 {
          break;
        }
      }

      if self.buffer_len_in_use == 0 {
        return None;
      }

      // Ensure the first byte is a header
      if self.buffer[0] != 0xAA {
        // Find the next header and shift
        if !self.find_next_header_and_shift() {
          return None;
        } else {
          continue;
        }
      }

      // Check if we have at least enough bytes for header + action + length
      if self.buffer_len_in_use < 4 {
        return None;
      }

      let action = self.buffer[1];
      // TODO: check that action is valid
      let length = ((self.buffer[2] as u16) << 8) | (self.buffer[3] as u16);

      if length > 1024 {
        // Invalid length, find next header
        if !self.find_next_header_and_shift() {
          return None;
        } else {
          continue;
        }
      }

      // Check if we have enough bytes for the complete frame
      let frame_size = 4 + (length as usize) + 2; // header + action + length_bytes + payload + checksum
      if self.buffer_len_in_use < frame_size {
        return None;
      }

      let mut result = SerialCommand::new();
      result.action = action;
      result.length = length;
      for i in 0..length as usize {
        result.data[i] = self.buffer[4 + i];
      }

      let checksum_offset = 4 + length as usize;
      result.checksum = ((self.buffer[checksum_offset] as u16) << 8) 
                      | (self.buffer[checksum_offset + 1] as u16);
      if !result.verify_checksum() {
        // Invalid checksum, find next header
        if !self.find_next_header_and_shift() {
          return None;
        } else {
          continue;
        }
      }

      // Valid frame, clear the buffer and return
      self.buffer_len_in_use = 0;
      return Some(result);
    }
  }

}