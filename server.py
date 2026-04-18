#!/usr/bin/env python3
"""
Video to LED Strip Server
Captures video frames from shared memory, processes them, and sends RGB data to LED strip.
"""

import numpy as np
import serial
import struct
import time


SERIAL_PORT = '/dev/mcu0'
BAUDRATE = 115200
FRAMES_PER_SECOND = 45
NUM_LEDS = 88  # Using 88 LEDs for the strip

# Shared memory capture
SHAREDMEM_FILE = "/dev/shm/wl-capture"
CAPTURE_WIDTH = 1920
CAPTURE_HEIGHT = 1080
CAPTURE_CHANNELS = 4  # B G R A


class LEDStripController:
    """Controller for sending manual color data to LED strip."""

    def __init__(self, port, baudrate=115200):
        """Initialize serial connection."""
        self.serial = serial.Serial(port, baudrate=baudrate, timeout=1)

    def close(self):
        """Close the serial connection."""
        self.serial.close()

    SOF = 0xAA  # Start of frame
    ACTION_CONTROL_ONOFF = 0x01
    ACTION_SET_VALUE = 0x02
    ACTION_SET_STRIPSETTING = 0x03
    ACTION_MANUAL_COLOR_INPUT = 0x04

    # Value IDs for ACTION_SET_VALUE (0x02)
    VALUE_BRIGHTNESS = 0x00
    VALUE_PHASE_STEP = 0x01
    VALUE_NUM_LEDS = 0x02
    VALUE_FPS = 0x03
    VALUE_REVERSE = 0x04

    def set_frames_per_second(self, fps):
        """
        Set frames per second.

        Args:
            fps: Frames per second (integer 0-255)
        """
        if not (0 <= fps <= 255):
            raise ValueError("Frames per second must be between 0 and 255")
        payload = bytes([self.VALUE_FPS, fps])
        self.send_frame(self.ACTION_SET_VALUE, payload)

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

        self.serial.write(frame)
        self.serial.flush()


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

    def send_colors(self, colors, start_index=0, reverse=False):
        """
        Send manual color data to LED strip.

        Args:
            colors: List of (R, G, B) tuples, each value 0-255
            start_index: Starting LED index (default 0)
        """
        # Build payload: 2 bytes for start index + RGB data
        payload = struct.pack('>H', start_index)
        if reverse:
            colors = colors[::-1]
        for r, g, b in colors:
            payload += bytes([r, g, b])

        # Check payload size limit
        if len(payload) > 1024:
            raise ValueError(f"Payload too large: {len(payload)} bytes (max 1024)")

        # Build frame
        length = len(payload)
        length_bytes = struct.pack('>H', length)

        # Calculate CRC over action + length + payload
        crc_data = bytes([self.ACTION_MANUAL_COLOR_INPUT]) + length_bytes + payload
        crc = self.crc16_ccitt(crc_data)
        crc_bytes = struct.pack('>H', crc)

        # Construct and send complete frame
        frame = bytes([self.SOF, self.ACTION_MANUAL_COLOR_INPUT]) + length_bytes + payload + crc_bytes
        self.serial.write(frame)
        self.serial.flush()

    def wait_for_response(self):
        """
        Wait for the 4-byte response frame sent by the MCU after each render.

        Returns:
            True if the expected response [0xBB, 0x00, 0xDF, 0xF8] was received,
            False on timeout or unexpected data.
        """
        data = self.serial.read(4)
        return data == bytes([0xBB, 0x00, 0xDF, 0xF8])


def process_frame_to_colors(frame, num_leds):
    """
    Process video frame and compute RGB colors for LED strip.

    Analyzes the bottom 10% of the frame, dividing it into num_leds segments.
    For each segment, computes a weighted average where pixels closer to the
    bottom have higher importance (linearly from 0.5 at top of region to 1.0 at bottom).

    Args:
        frame: Numpy array (BGR format, height x width x 3)
        num_leds: Number of LEDs in the strip

    Returns:
        List of (R, G, B) tuples, one per LED
    """
    height, width = frame.shape[:2]

    # Extract bottom 10% of the frame
    bottom_10_percent_height = int(height * 0.1)
    if bottom_10_percent_height == 0:
        bottom_10_percent_height = 1

    bottom_region = frame[height - bottom_10_percent_height:height, :]
    region_height = bottom_region.shape[0]

    # Create linear weight array (0.5 at top to 1.0 at bottom)
    # Shape: (region_height, 1, 1) for broadcasting
    weights = np.linspace(0.5, 1.0, region_height).reshape(-1, 1, 1)

    colors = []

    # Divide width into num_leds segments
    for i in range(num_leds):
        # Calculate segment boundaries
        seg_start = int(i * width / num_leds)
        seg_end = int((i + 1) * width / num_leds)

        # Extract segment
        segment = bottom_region[:, seg_start:seg_end, :]

        # Apply weights and compute weighted average
        weighted_segment = segment * weights
        weighted_sum = np.sum(weighted_segment, axis=(0, 1))  # Sum over height and width
        weight_sum = np.sum(weights) * (seg_end - seg_start)  # Total weight for normalization

        avg_color = weighted_sum / weight_sum

        # Convert from BGR to RGB and to integers
        b, g, r = avg_color
        colors.append((int(r), int(g), int(b)))

    return colors


def main():
    """Main function to capture video from shared memory and send to LED strip."""
    try:
        # Initialize LED strip controller
        controller = LEDStripController(SERIAL_PORT, BAUDRATE)
        print(f"Connected to LED strip on {SERIAL_PORT}")
        controller.set_frames_per_second(FRAMES_PER_SECOND)

        # Verify shared memory file exists
        try:
            open(SHAREDMEM_FILE, "rb").close()
        except FileNotFoundError:
            print(f"Error: Shared memory file not found at {SHAREDMEM_FILE}")
            return

        print(f"Capturing from {SHAREDMEM_FILE}")
        print("Press Ctrl+C to quit")

        expected_size = CAPTURE_WIDTH * CAPTURE_HEIGHT * CAPTURE_CHANNELS

        while True:
            # (1) Read frame from shared memory
            t_grab_start = time.perf_counter()
            try:
                with open(SHAREDMEM_FILE, "rb") as f:
                    raw_data = f.read()
            except Exception as e:
                print(f"Error reading shared memory: {e}")
                break

            if len(raw_data) != expected_size:
                print(f"Warning: Unexpected frame size: {len(raw_data)} vs {expected_size}")
                continue

            # Reshape into 4-channel image (B G R A)
            img = np.frombuffer(raw_data, dtype=np.uint8)
            img = img.reshape((CAPTURE_HEIGHT, CAPTURE_WIDTH, 4))

            # Extract BGR channels and drop alpha
            frame = img[:, :, :3]
            t_grab_end = time.perf_counter()

            # (2) Process frame to get LED colors
            t_process_start = time.perf_counter()
            colors = process_frame_to_colors(frame, NUM_LEDS)
            t_process_end = time.perf_counter()

            # (3) Send colors and wait for MCU response frame [0xBB, 0x00, 0xDF, 0xF8]
            # Flush stale responses before sending so the ack we read is for this frame.
            controller.serial.reset_input_buffer()
            t_send_start = time.perf_counter()
            # Due to 1024 byte payload limit, we can send max 340 LEDs at once
            # (2 bytes start index + 340*3 = 1022 bytes)
            max_leds_per_packet = 340
            for i in range(0, len(colors), max_leds_per_packet):
                chunk = colors[i:i + max_leds_per_packet]
                controller.send_colors(chunk, start_index=i, reverse=True)
            controller.wait_for_response()
            t_send_end = time.perf_counter()

            grab_ms    = (t_grab_end    - t_grab_start)    * 1000
            process_ms = (t_process_end - t_process_start) * 1000
            send_ms    = (t_send_end    - t_send_start)    * 1000
            total_ms   = (t_send_end    - t_grab_start)    * 1000
            print(f"grab={grab_ms:.1f}ms  process={process_ms:.1f}ms  send+ack={send_ms:.1f}ms  total={total_ms:.1f}ms")

        # Cleanup
        controller.close()
        print("Cleanup complete. Exiting.")

    except serial.SerialException as e:
        print(f"Serial error: {e}")
    except KeyboardInterrupt:
        print("\nInterrupted by user")
    except Exception as e:
        print(f"Error: {e}")


if __name__ == '__main__':
    main()
