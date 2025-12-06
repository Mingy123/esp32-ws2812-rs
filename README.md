# ESP RGB LED Controller

This project provides firmware for controlling an RGB LED strip using an ESP32 microcontroller.

## Speed Benchmarks

### Microcontroller: ESP32-C3

| Frame Duration (ms) | 60 LEDs | 88 LEDs | 280 LEDs |
|---|---|---|---|
| Hardcoded algorithm | - | 7 | 13 |
| USB control | - | - | - |

## Project Roadmap

- [x] Get it to work
- [x] Custom pixel control
- [x] Rainbow cycle animation
- [x] Global brightness control
- [x] Dynamic transmission size (sometimes we only want to light the first N LEDs)
- [ ] USB interface for desktop control
- [ ] Additional animations (e.g. breathing, flashing)
- [ ] Power saving when PC goes to sleep

## Usage

This project was generated with `esp-generate`, configured for the ESP32-C3.  
If you want to build for a different board, you may need to:
1. Change the board name from "esp32c3" to your desired board in `Cargo.toml` and `.cargo/config.toml`.
2. Change the target from "riscv32imc-esp-espidf" to Xtensa or something in `.cargo/config.toml` and `rust-toolchain.toml`.
3. Adjust any board-specific configurations in the code e.g. GPIO pin assignments in `src/bin/main.rs`.

## Building and Flashing

Make sure you have the ESP-IDF and Rust toolchain set up for embedded development:  
https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html

You probably also need `espflash`.

Then, to build and flash:

```sh
cargo run --release
```