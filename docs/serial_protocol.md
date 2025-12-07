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

## Payloads

### StripSetting Payload

StripSetting controls what happens in `update_pixels()`.  
For example, "Off" will clear all LEDs, Custom will do nothing, RainbowCycle
will run the rainbow animation.

| Setting ID | Description               | Additional Payload                   |
|------------|---------------------------|--------------------------------------|
| 0x00       | Off                       | None                                 |
| 0x01       | Custom (manual)           | None                                 |
| 0x02       | Solid Color               | 3 bytes (R, G, B)                    |
| 0x03       | Rainbow Cycle             | 4 bytes (f32): N cycles in strip     |

## CRC-16-CCITT Calculation

The implementation can be found in `src/command.rs`.  
The CRC is calculated over the `Action`, `Length`, and `Payload` fields in that order.
`Length` is treated as big-endian when calculating the CRC, i.e. the high byte is processed first.