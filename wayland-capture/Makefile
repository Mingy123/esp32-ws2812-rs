CC = gcc
CFLAGS = $(shell pkg-config --cflags wayland-client)
LDFLAGS = $(shell pkg-config --libs wayland-client) -lrt

TARGET = capture
SRC = capture.c wlr-screencopy-unstable-v1.c

all: $(TARGET)

$(TARGET): $(SRC)
	$(CC) $(SRC) $(CFLAGS) $(LDFLAGS) -o $(TARGET)

clean:
	rm -f $(TARGET)
