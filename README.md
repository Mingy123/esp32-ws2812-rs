# ESP RGB LED Controller

This project provides firmware for controlling a WS2812B LED strip (also known as NeoPixels)
using an ESP32 microcontroller (ESP32-C3 in particular) via a custom serial protocol over USB.

## Project Roadmap

- [x] Get it to work
- [x] Custom pixel control
- [x] Rainbow cycle animation
- [x] Global brightness control
- [x] Dynamic transmission size (sometimes we only want to light the first N LEDs)
- [x] USB interface for desktop control
- [ ] Additional animations (e.g. breathing, flashing)
- [ ] Power saving when PC goes to sleep

## Usage

This project was generated with `esp-generate`, configured for the ESP32-C3.  
If you want to build for a different board, you may need to:
1. Change the board name from "esp32c3" to your desired board in `Cargo.toml` and `.cargo/config.toml`.
2. Change the target from "riscv32imc-esp-espidf" to Xtensa or something in `.cargo/config.toml` and `rust-toolchain.toml`.
3. Adjust any board-specific configurations in the code e.g. GPIO pin assignments in `src/bin/main.rs`.

The ESP32-C3 board provides a built-in USB-Serial-JTAG peripheral, which is used for the serial communication to a host PC.  
A list of USB peripheral support for other ESP boards can be found here:  
https://docs.espressif.com/projects/esp-iot-solution/en/latest/usb/usb_overview/usb_overview.html  
ESP32-S2, ESP32-C2, and ESP8266 do not provide USB-Serial-JTAG. For ESP32-S2, USB-OTG may be implemented.

The serial protocol is documented in [docs/serial_protocol.md](docs/serial_protocol.md).

## Building and Flashing

Make sure you have the ESP-IDF and Rust toolchain set up for embedded development:  
https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html

You probably also need `espflash`.

Then, to build and flash:

```sh
cargo run --release
```