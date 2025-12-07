#!/usr/bin/env python3
"""
Test script for Action 0x04 (Manual Color Input)
Creates an animation of a red pixel moving cyclically across the LED strip.
"""

import serial
import struct
import sys
import time


class LEDController:
    """Controller for RGB LED strip via serial protocol."""

    SOF = 0xAA  # Start of frame
    ACTION_MANUAL_COLOR_INPUT = 0x04
    NUM_LEDS = 88

    def __init__(self, port='/dev/mcu0', baudrate=115200, timeout=1):
        """
        Initialize the LED controller.

        Args:
            port: Serial port device path
            baudrate: Serial communication baud rate
            timeout: Serial read timeout in seconds
        """
        self.serial = serial.Serial(port, baudrate=baudrate, timeout=timeout)

    def close(self):
        """Close the serial connection."""
        self.serial.close()

    @staticmethod
    def crc16_ccitt(data):
        """
        Calculate CRC-16-CCITT checksum.

        Args:
            data: Bytes to calculate CRC over

        Returns:
            16-bit CRC value
        """
        crc = 0xFFFF
        for byte in data:
            crc ^= byte << 8
            for _ in range(8):
                if crc & 0x8000:
                    crc = (crc << 1) ^ 0x1021
                else:
                    crc = crc << 1
                crc &= 0xFFFF
        return crc

    def send_frame(self, action, payload):
        """
        Send a frame with the specified action and payload.

        Args:
            action: Action byte
            payload: Payload bytes
        """
        # Build frame components
        length = len(payload)
        if length > 1024:
            raise ValueError(f"Payload too large: {length} bytes (max 1024)")

        # Pack length as big-endian 16-bit integer
        length_bytes = struct.pack('>H', length)

        # Calculate CRC over action + length + payload
        crc_data = bytes([action]) + length_bytes + payload
        crc = self.crc16_ccitt(crc_data)
        crc_bytes = struct.pack('>H', crc)

        # Construct complete frame
        frame = bytes([self.SOF, action]) + length_bytes + payload + crc_bytes

        # Send frame
        self.serial.write(frame)
        self.serial.flush()

    def set_all_leds(self, colors):
        """
        Set colors for all LEDs starting from index 0.

        Args:
            colors: List of (r, g, b) tuples, one for each LED
        """
        if len(colors) > self.NUM_LEDS:
            raise ValueError(f"Too many LEDs: {len(colors)} (max {self.NUM_LEDS})")

        # Due to 1024 byte payload limit, we can only send up to 340 LEDs at once
        # (2 bytes for start index + 340*3 = 1022 bytes)
        # For 280 LEDs: 2 + 280*3 = 842 bytes, which is within the limit

        # Build payload: start_index (2 bytes) + RGB data (3 bytes per LED)
        payload = struct.pack('>H', 0)  # Start at index 0

        for r, g, b in colors:
            payload += bytes([r, g, b])

        self.send_frame(self.ACTION_MANUAL_COLOR_INPUT, payload)

    def set_led_range(self, start_index, colors):
        """
        Set colors for a range of LEDs starting from start_index.

        Args:
            start_index: Starting LED index
            colors: List of (r, g, b) tuples
        """
        if start_index + len(colors) > self.NUM_LEDS:
            raise ValueError(f"LED range exceeds strip length")

        # Check payload size limit
        payload_size = 2 + len(colors) * 3
        if payload_size > 1024:
            raise ValueError(f"Payload too large: {payload_size} bytes (max 1024)")

        # Build payload: start_index (2 bytes) + RGB data (3 bytes per LED)
        payload = struct.pack('>H', start_index)

        for r, g, b in colors:
            payload += bytes([r, g, b])

        self.send_frame(self.ACTION_MANUAL_COLOR_INPUT, payload)


def animate_moving_pixel(controller, fps=30, duration=None):
    """
    Animate a single red pixel moving across the LED strip.

    Args:
        controller: LEDController instance
        fps: Frames per second
        duration: Duration in seconds (None for infinite)
    """
    frame_delay = 1.0 / fps
    position = 0
    frame_count = 0
    start_time = time.time()

    print(f"Starting animation: Red pixel moving at {fps} FPS")
    print("Press Ctrl+C to stop")
    print("-" * 50)

    try:
        while True:
            # Create color array: all off except current position (red)
            colors = []
            for i in range(controller.NUM_LEDS):
                if i == position:
                    colors.append((255, 0, 0))  # Red
                else:
                    colors.append((0, 0, 0))    # Off

            # Send to controller
            controller.set_all_leds(colors)

            # Update position (cycle from 0 to 279)
            position = (position + 1) % controller.NUM_LEDS
            frame_count += 1

            # Print status every second
            if frame_count % fps == 0:
                elapsed = time.time() - start_time
                actual_fps = frame_count / elapsed if elapsed > 0 else 0
                print(f"Frame {frame_count:5d} | Position: {position:3d} | "
                      f"Elapsed: {elapsed:.1f}s | Actual FPS: {actual_fps:.1f}")

            # Check duration
            if duration is not None and (time.time() - start_time) >= duration:
                break

            # Delay for next frame
            time.sleep(frame_delay)

    except KeyboardInterrupt:
        print("\n\nAnimation stopped by user")
        elapsed = time.time() - start_time
        actual_fps = frame_count / elapsed if elapsed > 0 else 0
        print(f"Total frames: {frame_count}")
        print(f"Total time: {elapsed:.1f}s")
        print(f"Average FPS: {actual_fps:.1f}")


def test_partial_update(controller):
    """
    Test updating only a portion of the strip.
    Sets LEDs 100-110 to green.
    """
    print("Testing partial update: Setting LEDs 100-110 to green")

    # First, clear all LEDs
    colors_all_off = [(0, 0, 0)] * controller.NUM_LEDS
    controller.set_all_leds(colors_all_off)
    time.sleep(0.5)

    # Now set LEDs 100-110 to green
    colors_green = [(0, 255, 0)] * 11  # 11 LEDs (100-110 inclusive)
    controller.set_led_range(100, colors_green)

    print("Green segment should be visible at LEDs 100-110")


def test_multiple_segments(controller):
    """
    Test updating multiple segments of the strip.
    """
    print("Testing multiple segments")

    # Clear all
    colors_all_off = [(0, 0, 0)] * controller.NUM_LEDS
    controller.set_all_leds(colors_all_off)
    time.sleep(0.5)

    # Set first 10 LEDs to red
    print("Setting LEDs 0-9 to red")
    colors_red = [(255, 0, 0)] * 10
    controller.set_led_range(0, colors_red)
    time.sleep(0.5)

    # Set middle 10 LEDs to green
    print("Setting LEDs 135-144 to green")
    colors_green = [(0, 255, 0)] * 10
    controller.set_led_range(135, colors_green)
    time.sleep(0.5)

    # Set last 10 LEDs to blue
    print("Setting LEDs 270-279 to blue")
    colors_blue = [(0, 0, 255)] * 10
    controller.set_led_range(270, colors_blue)

    print("Three colored segments should be visible")


def main():
    """Main function."""
    try:
        controller = LEDController('/dev/mcu0')

        print("RGB LED Manual Color Input Test")
        print("=" * 50)
        print("\nTest Options:")
        print("  1. Animate moving red pixel (30 FPS)")
        print("  2. Animate moving red pixel (60 FPS)")
        print("  3. Animate moving red pixel (10 FPS)")
        print("  4. Test partial update (LEDs 100-110)")
        print("  5. Test multiple segments")
        print("  6. Clear all LEDs")
        print("  7. Exit")

        choice = input("\nEnter your choice (1-7): ").strip()

        if choice == '1':
            animate_moving_pixel(controller, fps=30)
        elif choice == '2':
            animate_moving_pixel(controller, fps=60)
        elif choice == '3':
            animate_moving_pixel(controller, fps=10)
        elif choice == '4':
            test_partial_update(controller)
            input("Press Enter to continue...")
        elif choice == '5':
            test_multiple_segments(controller)
            input("Press Enter to continue...")
        elif choice == '6':
            print("Clearing all LEDs...")
            colors_off = [(0, 0, 0)] * controller.NUM_LEDS
            controller.set_all_leds(colors_off)
            print("All LEDs cleared")
        elif choice == '7':
            print("Exiting...")
        else:
            print("Error: Invalid choice")

        controller.close()

    except serial.SerialException as e:
        print(f"Error: Could not open serial port - {e}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\n\nInterrupted by user. Exiting...")
        sys.exit(0)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()
