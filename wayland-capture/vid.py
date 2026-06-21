import numpy as np
import cv2
import time
import os
import sys


filename = sys.argv[1]
if filename.split('/')[-1] == "wl-capture-0":
    width = 2560
    height = 1440
else:
    width = 1920
    height = 1080
print(filename, width, height)
channels = 4  # B G R A
expected_size = width * height * channels

cv2.namedWindow("Image", cv2.WINDOW_NORMAL)

while True:
    try:
        if not os.path.exists(filename):
            time.sleep(0.1)
            continue

        with open(filename, "rb") as f:
            raw_data = f.read()

        if len(raw_data) != expected_size:
            # Skip incomplete frames
            time.sleep(0.1)
            continue

        img = np.frombuffer(raw_data, dtype=np.uint8)
        img = img.reshape((height, width, 4))

        # Extract BGR channels
        bgr = img[:, :, :3]

        cv2.imshow("Image", bgr)

        # Press 'q' to quit
        if cv2.waitKey(1) & 0xFF == ord('q'):
            break

        time.sleep(0.1)

    except Exception as e:
        print("Error:", e)
        time.sleep(0.1)

cv2.destroyAllWindows()
