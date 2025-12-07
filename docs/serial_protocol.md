# Serial Protocol Specification

This project expects to receive data from a host PC over a serial connection
via USB to control the LED strip.  
The data should be sent in frames in the format specified below.

## WARNING

If you send data too quickly (e.g. 60 FPS * 300 LED), the
microcontroller will likely be unable to keep up.  
The way this project is implemented, the ISR (interrupt service routine) in
charge of receiving data will drop incoming data if the buffer is full.  
This means that if data is constantly sent faster than the microcontroller can
process, new frames will constantly get malformed due to dropped bytes and the
LED strip will never update.

## Frame format

| Field      | Size (bytes) | Description                 |
|------------|--------------|-----------------------------|
| SOF (0xAA) | 1            | Start of frame              |
| Action     | 1            | Action to be taken          |
| Length     | 2            | Payload length (max 1024, big endian) |
| Payload    | N            | Message-specific data       |
| CRC16      | 2            | CRC-16-CCITT (big endian)   |

The `Length` field specifies the length of the `Payload` field in bytes, i.e. `N`.

## Actions

| Action | Description               | Payload Description                     |
|--------|---------------------------|-----------------------------------------|
| 0x01   | Control on/off            | 1 byte (0 = off, 1 = on)                |
| 0x02   | Set global brightness     | 4 bytes float (big endian)              |
| 0x03   | Set StripSetting          | Sets the StripSetting enum (see below)  |
| 0x04   | Manual color input        | Manually set the color of each pixel    |
| 0x05   | Set phase_step            | 4 bytes float (0 - 1) (big endian)      |
| 0x06   | Set num_leds_to_update    | 2 bytes unsigned integer (big endian)   |
| 0x07   | Set frames_per_second     | 1 byte unsigned integer                 |

## Payloads

### StripSetting Payload

StripSetting controls what happens in `update_pixels()`.  
Custom will do nothing.

| Setting ID | Description               | Additional Payload                   |
|------------|---------------------------|--------------------------------------|
| 0x00       | Custom (manual)           | None                                 |
| 0x01       | Breathing                 | 3 bytes (R, G, B)                    |
| 0x02       | Solid Color               | 3 bytes (R, G, B)                    |
| 0x03       | Rainbow Cycle             | 4 bytes (f32): N cycles in strip     |

### Manual Color Input Payload

The payload first starts with the index of the first LED to set (2 bytes, big
endian), followed by the color data for each LED in sequence.  
Each LED color is represented by 3 bytes (R, G, B).

The number of LEDs to set will be determined from the payload length.  
For example, if the payload length is 11 bytes, the first 2 bytes are the
starting index, and the remaining 9 bytes correspond to 3 LEDs (3 bytes each).

Note: The maximum payload length is 1024 bytes, so the maximum number of LEDs
that can be set in one command is limited by that.  
For example, to set LEDs 0-340 (341 LEDs), the payload length would be
2 + (341 * 3) = 1025 bytes, which exceeds the limit.  
To set more than 340 LEDs, you must split it into multiple commands.

## CRC-16-CCITT Calculation

The implementation can be found in `src/command.rs`.  
The CRC is calculated over the `Action`, `Length`, and `Payload` fields in that order.
`Length` is treated as big-endian when calculating the CRC, i.e. the high byte is processed first.