#!/usr/bin/env python3
"""
Serial Protocol Implementation for RGB LED Control
Implements actions 0x01 (Control on/off) and 0x02 (Set global brightness)
"""

import serial
import struct
import sys


class LEDController:
    """Controller for RGB LED strip via serial protocol."""
    
    SOF = 0xAA  # Start of frame
    ACTION_CONTROL_ONOFF = 0x01
    ACTION_SET_BRIGHTNESS = 0x02
    
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
            action: Action byte (0x01 or 0x02)
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
    
    def control_onoff(self, state):
        """
        Control LED strip on/off state.
        
        Args:
            state: True/1 for on, False/0 for off
        """
        payload = bytes([1 if state else 0])
        self.send_frame(self.ACTION_CONTROL_ONOFF, payload)
        print(f"LED strip turned {'ON' if state else 'OFF'}")
    
    def set_brightness(self, brightness):
        """
        Set global brightness level.
        
        Args:
            brightness: Float value for brightness (typically 0.0 to 1.0)
        """
        # Pack as big-endian 32-bit float
        payload = struct.pack('>f', brightness)
        self.send_frame(self.ACTION_SET_BRIGHTNESS, payload)
        print(f"Brightness set to {brightness}")


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
            print("  4. Exit")
            
            choice = input("\nEnter your choice (1-4): ").strip()
            
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
                print("Exiting...")
                break
            else:
                print("Error: Invalid choice. Please enter 1-4.")
        
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
