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

This project in configured for ESP32-C3 and ESP32-S3.  
ESP32-S3 has not been tested thoroughly but it works with the onboard WS2812b.

The boards provide a built-in USB-Serial-JTAG peripheral, which is used for the serial communication to a host PC.  
A list of USB peripheral support for other ESP boards can be found here:  
https://docs.espressif.com/projects/esp-iot-solution/en/latest/usb/usb_overview/usb_overview.html  
ESP32-S2, ESP32-C2, and ESP8266 do not provide USB-Serial-JTAG. For ESP32-S2, USB-OTG may be implemented.

The serial protocol is documented in [docs/serial_protocol.md](docs/serial_protocol.md).

## Building and Flashing

Install the toolchains following [this guide](https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html).  
For ESP32-C3, install for RISC-V; For ESP32-S3, install for Xtensa.

You probably also need `espflash`:  
`cargo install espflash --locked`

Then, to build and flash:

#### ESP32-C3

```sh
cargo run --release --target riscv32imc-unknown-none-elf --features esp32c3
```

#### ESP32-S3

```sh
cargo run --release --target xtensa-esp32s3-none-elf --features esp32s3
```