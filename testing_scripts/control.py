#!/usr/bin/env python3
"""
Serial Protocol Implementation for RGB LED Control
Implements actions 0x01 (Control on/off) and 0x02 (Set global brightness)
"""

import serial
import struct
import sys
import time
import random


class LEDController:
    """Controller for RGB LED strip via serial protocol."""

    SOF = 0xAA  # Start of frame
    ACTION_CONTROL_ONOFF = 0x01
    ACTION_SET_BRIGHTNESS = 0x02
    ACTION_SET_STRIPSETTING = 0x03
    ACTION_MANUAL_COLOR_INPUT = 0x04
    ACTION_SET_FRAME_PER_CYCLE = 0x05
    ACTION_SET_NUM_LEDS_TO_UPDATE = 0x06
    ACTION_SET_FRAMES_PER_SECOND = 0x07
    ACTION_SET_REVERSE_ANIMATION = 0x08
    def set_frames_per_second(self, fps, chunked=False):
        """
        Set frames per second.

        Args:
            fps: Frames per second (integer 0-255)
            chunked: If True, send data in small random chunks with delays
        """
        if not (0 <= fps <= 255):
            raise ValueError("Frames per second must be between 0 and 255")
        payload = bytes([fps])
        self.send_frame(self.ACTION_SET_FRAMES_PER_SECOND, payload, chunked=chunked)
        if not chunked:
            print(f"Frames per second set to {fps}")

    def set_reverse_animation(self, reverse, chunked=False):
        """
        Set reverse animation.

        Args:
            reverse: True/1 for reverse, False/0 for forward
            chunked: If True, send data in small random chunks with delays
        """
        payload = bytes([1 if reverse else 0])
        self.send_frame(self.ACTION_SET_REVERSE_ANIMATION, payload, chunked=chunked)
        if not chunked:
            print(f"Animation direction set to {'REVERSE' if reverse else 'FORWARD'}")

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

    def send_frame(self, action, payload, chunked=False):
        """
        Send a frame with the specified action and payload.

        Args:
            action: Action byte (0x01 or 0x02)
            payload: Payload bytes
            chunked: If True, send data in small random chunks with delays
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
        if chunked:
            print(f"Sending {len(frame)} bytes in random chunks (1-3 bytes)...")
            i = 0
            while i < len(frame):
                chunk_size = random.randint(1, min(3, len(frame) - i))
                chunk = frame[i:i+chunk_size]
                self.serial.write(chunk)
                self.serial.flush()
                print(f"  Sent {chunk_size} byte(s): {' '.join(f'{b:02X}' for b in chunk)}")
                i += chunk_size
                if i < len(frame):
                    time.sleep(0.2)
            print("Frame sent completely.")
        else:
            self.serial.write(frame)
            self.serial.flush()

    def control_onoff(self, state, chunked=False):
        """
        Control LED strip on/off state.

        Args:
            state: True/1 for on, False/0 for off
            chunked: If True, send data in small random chunks with delays
        """
        payload = bytes([1 if state else 0])
        self.send_frame(self.ACTION_CONTROL_ONOFF, payload, chunked=chunked)
        if not chunked:
            print(f"LED strip turned {'ON' if state else 'OFF'}")

    def set_brightness(self, brightness, chunked=False):
        """
        Set global brightness level.

        Args:
            brightness: Float value for brightness (typically 0.0 to 1.0)
            chunked: If True, send data in small random chunks with delays
        """
        # Pack as big-endian 32-bit float
        payload = struct.pack('>f', brightness)
        self.send_frame(self.ACTION_SET_BRIGHTNESS, payload, chunked=chunked)
        if not chunked:
            print(f"Brightness set to {brightness}")

    def set_strip_setting(self, setting_id, r=None, g=None, b=None, cycles=None, chunked=False):
        """
        Set strip setting (animation/pattern mode).

        Args:
            setting_id: Setting ID (0x00=Custom, 0x01=Breathing, 0x02=SolidColor, 0x03=RainbowCycle)
            r: Red value for Breathing/SolidColor (0-255)
            g: Green value for Breathing/SolidColor (0-255)
            b: Blue value for Breathing/SolidColor (0-255)
            cycles: Number of rainbow cycles for RainbowCycle (float)
            chunked: If True, send data in small random chunks with delays
        """
        payload = bytes([setting_id])

        if setting_id == 0x01:  # Breathing
            if r is None or g is None or b is None:
                raise ValueError("Breathing requires r, g, b values")
            payload += bytes([r, g, b])
            description = f"Breathing(R={r}, G={g}, B={b})"
        elif setting_id == 0x02:  # SolidColor
            if r is None or g is None or b is None:
                raise ValueError("SolidColor requires r, g, b values")
            payload += bytes([r, g, b])
            description = f"SolidColor(R={r}, G={g}, B={b})"
        elif setting_id == 0x03:  # RainbowCycle
            if cycles is None:
                raise ValueError("RainbowCycle requires cycles value")
            payload += struct.pack('>f', cycles)
            description = f"RainbowCycle(cycles={cycles})"
        elif setting_id == 0x00:
            description = "Custom"
        else:
            raise ValueError(f"Invalid setting_id: {setting_id}")

        self.send_frame(self.ACTION_SET_STRIPSETTING, payload, chunked=chunked)
        if not chunked:
            print(f"Strip setting set to {description}")

    def set_frame_per_cycle(self, frame_per_cycle, chunked=False):
        """
        Set frame per cycle (animation speed).

        Args:
            frame_per_cycle: Float value for frame increment per cycle (typically 0.0 to 1.0)
            chunked: If True, send data in small random chunks with delays
        """
        # Pack as big-endian 32-bit float
        payload = struct.pack('>f', frame_per_cycle)
        self.send_frame(self.ACTION_SET_FRAME_PER_CYCLE, payload, chunked=chunked)
        if not chunked:
            print(f"Frame per cycle set to {frame_per_cycle}")

    def set_num_leds_to_update(self, num_leds, chunked=False):
        """
        Set number of LEDs to update.

        Args:
            num_leds: Number of LEDs to update (0-280)
            chunked: If True, send data in small random chunks with delays
        """
        # Pack as big-endian 16-bit unsigned integer
        payload = struct.pack('>H', num_leds)
        self.send_frame(self.ACTION_SET_NUM_LEDS_TO_UPDATE, payload, chunked=chunked)
        if not chunked:
            print(f"Number of LEDs to update set to {num_leds}")

    def send_malformed_data(self, chunked=False):
        """
        Send intentionally malformed data to test error handling.
        Sends: malformed frame + valid frame to test recovery.
        """
        # Create a malformed frame (bad CRC)
        malformed = bytes([self.SOF, 0xFF, 0x00, 0x02, 0xDE, 0xAD, 0xBE, 0xEF])

        # Create a valid frame (LED ON)
        valid_payload = bytes([1])
        length_bytes = struct.pack('>H', len(valid_payload))
        crc_data = bytes([self.ACTION_CONTROL_ONOFF]) + length_bytes + valid_payload
        crc = self.crc16_ccitt(crc_data)
        crc_bytes = struct.pack('>H', crc)
        valid_frame = bytes([self.SOF, self.ACTION_CONTROL_ONOFF]) + length_bytes + valid_payload + crc_bytes

        # Combine both frames
        combined = malformed + valid_frame

        if chunked:
            print(f"Sending malformed + valid frame ({len(combined)} bytes) in random chunks...")
            print(f"  Malformed frame: {' '.join(f'{b:02X}' for b in malformed)}")
            print(f"  Valid frame: {' '.join(f'{b:02X}' for b in valid_frame)}")
            i = 0
            while i < len(combined):
                chunk_size = random.randint(1, min(3, len(combined) - i))
                chunk = combined[i:i+chunk_size]
                self.serial.write(chunk)
                self.serial.flush()
                print(f"  Sent {chunk_size} byte(s): {' '.join(f'{b:02X}' for b in chunk)}")
                i += chunk_size
                if i < len(combined):
                    time.sleep(0.2)
            print("Test data sent. Device should skip malformed frame and process valid frame.")
        else:
            print(f"Sending malformed frame: {' '.join(f'{b:02X}' for b in malformed)}")
            print(f"Sending valid frame: {' '.join(f'{b:02X}' for b in valid_frame)}")
            self.serial.write(combined)
            self.serial.flush()
            print("Test data sent. Device should skip malformed frame and process valid frame.")


def main():
    """Main function with interactive menu."""
    try:
        controller = LEDController('/dev/mcu0')

        print("RGB LED Controller - Interactive Mode")
        print("=" * 50)

        while True:
            print("\nAvailable Commands:")
            print("  1. Turn LED strip ON")
            print("  2. Turn LED strip OFF")
            print("  3. Set brightness")
            print("  4. Set strip setting to Custom")
            print("  5. Set strip setting to Breathing")
            print("  6. Set strip setting to SolidColor")
            print("  7. Set strip setting to RainbowCycle")
            print("  8. Turn LED strip ON (chunked - test buffering)")
            print("  9. Turn LED strip OFF (chunked - test buffering)")
            print(" 10. Set brightness (chunked - test buffering)")
            print(" 11. Send malformed data (test error recovery)")
            print(" 12. Send malformed data chunked (test buffered error recovery)")
            print(" 13. Set frame per cycle (phase step)")
            print(" 14. Set num_leds_to_update")
            print(" 15. Set frames per second")
            print(" 16. Set reverse animation (forward)")
            print(" 17. Set reverse animation (reverse)")
            print(" 18. Exit")

            choice = input("\nEnter your choice (1-18): ").strip()

            if choice == '1':
                controller.control_onoff(True)
            elif choice == '2':
                controller.control_onoff(False)
            elif choice == '3':
                try:
                    brightness = float(input("Enter brightness (0.0 to 1.0): ").strip())
                    if 0.0 <= brightness <= 1.0:
                        controller.set_brightness(brightness)
                    else:
                        print("Error: Brightness must be between 0.0 and 1.0")
                except ValueError:
                    print("Error: Invalid brightness value")
            elif choice == '4':
                controller.set_strip_setting(0x00)  # Custom
            elif choice == '5':
                try:
                    r = int(input("Enter red (0-255): ").strip())
                    g = int(input("Enter green (0-255): ").strip())
                    b = int(input("Enter blue (0-255): ").strip())
                    if 0 <= r <= 255 and 0 <= g <= 255 and 0 <= b <= 255:
                        controller.set_strip_setting(0x01, r=r, g=g, b=b)  # Breathing
                    else:
                        print("Error: RGB values must be between 0 and 255")
                except ValueError:
                    print("Error: Invalid RGB values")
            elif choice == '6':
                try:
                    r = int(input("Enter red (0-255): ").strip())
                    g = int(input("Enter green (0-255): ").strip())
                    b = int(input("Enter blue (0-255): ").strip())
                    if 0 <= r <= 255 and 0 <= g <= 255 and 0 <= b <= 255:
                        controller.set_strip_setting(0x02, r=r, g=g, b=b)  # SolidColor
                    else:
                        print("Error: RGB values must be between 0 and 255")
                except ValueError:
                    print("Error: Invalid RGB values")
            elif choice == '7':
                try:
                    cycles = float(input("Enter number of rainbow cycles (e.g., 1.0, 2.0): ").strip())
                    if cycles > 0:
                        controller.set_strip_setting(0x03, cycles=cycles)  # RainbowCycle
                    else:
                        print("Error: Cycles must be greater than 0")
                except ValueError:
                    print("Error: Invalid cycles value")
            elif choice == '8':
                controller.control_onoff(True, chunked=True)
            elif choice == '9':
                controller.control_onoff(False, chunked=True)
            elif choice == '10':
                try:
                    brightness = float(input("Enter brightness (0.0 to 1.0): ").strip())
                    if 0.0 <= brightness <= 1.0:
                        controller.set_brightness(brightness, chunked=True)
                    else:
                        print("Error: Brightness must be between 0.0 and 1.0")
                except ValueError:
                    print("Error: Invalid brightness value")
            elif choice == '11':
                controller.send_malformed_data(chunked=False)
            elif choice == '12':
                controller.send_malformed_data(chunked=True)
            elif choice == '13':
                try:
                    frame_per_cycle = float(input("Enter frame per cycle (e.g., 0.01, 0.05): ").strip())
                    if 0.0 <= frame_per_cycle <= 1.0:
                        controller.set_frame_per_cycle(frame_per_cycle)
                    else:
                        print("Warning: Frame per cycle is typically between 0.0 and 1.0, but continuing...")
                        controller.set_frame_per_cycle(frame_per_cycle)
                except ValueError:
                    print("Error: Invalid frame per cycle value")
            elif choice == '14':
                try:
                    num_leds = int(input("Enter number of LEDs to update (0-280): ").strip())
                    if 0 <= num_leds <= 280:
                        controller.set_num_leds_to_update(num_leds)
                    else:
                        print("Error: Number of LEDs must be between 0 and 280")
                except ValueError:
                    print("Error: Invalid number of LEDs")
            elif choice == '15':
                try:
                    fps = int(input("Enter frames per second (0-255): ").strip())
                    if 0 <= fps <= 255:
                        controller.set_frames_per_second(fps)
                    else:
                        print("Error: Frames per second must be between 0 and 255")
                except ValueError:
                    print("Error: Invalid frames per second value")
            elif choice == '16':
                controller.set_reverse_animation(False)
            elif choice == '17':
                controller.set_reverse_animation(True)
            elif choice == '18':
                print("Exiting...")
                break
            else:
                print("Error: Invalid choice. Please enter 1-18.")

        controller.close()

    except serial.SerialException as e:
        print(f"Error: Could not open serial port - {e}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\n\nInterrupted by user. Exiting...")
        sys.exit(0)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
