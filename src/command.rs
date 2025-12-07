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


// 1. Find the next Header byte (0xAA) in the buffer
// 2. Try to parse the following bytes as a frame
// 3. If fail, retry (while there is data in the buffer)
// TODO: possible improvement: save the SerialCommand state between calls
//   (in case the serial connection is slow and we don't have a full frame yet)
/// Read bytes from the consumer buffer and parse into a SerialCommand
pub fn read_buffer_into_command(
  consumer: &mut Consumer<u8>,
) -> Result<SerialCommand, ()> {
  
  loop {
    let mut result = SerialCommand::new();

    // First byte is header
    let header = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };
    
    if header != 0xAA {
      continue; // Not a header, keep looking
    }
    
    // Second byte is action
    result.action = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };

    // Next two bytes are length (big-endian)
    let length_high = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };
    let length_low = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };
    let length = ((length_high as u16) << 8) | (length_low as u16);
    if length > 1024 {
      continue; // Invalid length, retry
    }
    result.length = length;
    
    // Read payload bytes
    let mut failed = false;
    for i in 0..length as usize {
      result.data[i] = match consumer.dequeue() {
        Some(byte) => byte,
        None => {
          failed = true;
          break;
        }
      };
    }
    if failed {
      return Err(()); // No data left to read
    }

    // Read CRC16 checksum (2 bytes, big-endian)
    let checksum_high = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };
    let checksum_low = match consumer.dequeue() {
      Some(byte) => byte,
      None => return Err(()), // No data left to read
    };
    result.checksum = ((checksum_high as u16) << 8) | (checksum_low as u16);

    // Verify checksum
    if !result.verify_checksum() {
      continue; // Invalid checksum, retry
    }

    return Ok(result); // Successfully parsed a valid frame
  }
}