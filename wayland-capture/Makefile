CC = gcc
WLFLAGS = $(shell pkg-config --cflags --libs wayland-client)
DRMFLAGS = $(shell pkg-config --cflags --libs gbm libdrm)

all: capture multi_capture

capture: capture.c wlr-screencopy-unstable-v1.c
	$(CC) capture.c wlr-screencopy-unstable-v1.c $(WLFLAGS) -o capture

multi_capture: multi_capture.c wlr-screencopy-unstable-v1.c
	$(CC) multi_capture.c wlr-screencopy-unstable-v1.c linux-dmabuf-v1.c $(WLFLAGS) $(DRMFLAGS) -o multi_capture

clean:
	rm -f capture multi_capture
