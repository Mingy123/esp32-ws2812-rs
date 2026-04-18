#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>
#include <stdbool.h>
#include <wayland-client.h>
#include "wlr-screencopy-unstable-v1.h"

static struct wl_shm                     *shm        = NULL;
static struct wl_output                  *output     = NULL;
static struct zwlr_screencopy_manager_v1 *screencopy = NULL;

struct capture {
    struct wl_buffer *buffer;
    void             *data;
    int               width, height, stride;
    bool              done;
};

static int anon_shm_open(void) {
    char name[32] = "/wl-capture";
    //snprintf(name, sizeof(name), "/wl-capture-%d", getpid());
    int fd = shm_open(name, O_RDWR | O_CREAT, 0600);
    return fd;
}



static void frame_buffer(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t format, uint32_t width, uint32_t height, uint32_t stride)
{
    struct capture *cap = data;
    if (cap->buffer) return; // already set up from a previous buffer event

    cap->width  = width;
    cap->height = height;
    cap->stride = stride;

    int size = stride * height;
    int fd = anon_shm_open();
    ftruncate(fd, size);
    cap->data = mmap(NULL, size, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);

    struct wl_shm_pool *pool = wl_shm_create_pool(shm, fd, size);
    cap->buffer = wl_shm_pool_create_buffer(pool, 0,
                      width, height, stride, format);
    wl_shm_pool_destroy(pool);
    close(fd);
}

static void frame_flags(void *data,
        struct zwlr_screencopy_frame_v1 *frame, uint32_t flags) {}

static void frame_ready(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t tv_sec_hi, uint32_t tv_sec_lo, uint32_t tv_nsec)
{
    ((struct capture *)data)->done = true;
}

static void frame_failed(void *data,
        struct zwlr_screencopy_frame_v1 *frame) {
    fprintf(stderr, "capture failed\n");
    exit(1);
}

static void frame_damage(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t x, uint32_t y, uint32_t width, uint32_t height) {}

static void frame_linux_dmabuf(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t format, uint32_t width, uint32_t height) {}

static void frame_buffer_done(void *data,
        struct zwlr_screencopy_frame_v1 *frame)
{
    // All buffer format offers are done — now safe to copy
    struct capture *cap = data;
    zwlr_screencopy_frame_v1_copy(frame, cap->buffer);
}

static const struct zwlr_screencopy_frame_v1_listener frame_listener = {
    .buffer       = frame_buffer,
    .flags        = frame_flags,
    .ready        = frame_ready,
    .failed       = frame_failed,
    .damage       = frame_damage,
    .linux_dmabuf = frame_linux_dmabuf,
    .buffer_done  = frame_buffer_done,
};



static void registry_global(void *data, struct wl_registry *reg,
        uint32_t name, const char *interface, uint32_t version)
{
    if (!strcmp(interface, wl_shm_interface.name))
        shm = wl_registry_bind(reg, name, &wl_shm_interface, 1);
    else if (!strcmp(interface, wl_output_interface.name))
        output = wl_registry_bind(reg, name, &wl_output_interface, 1);
    else if (!strcmp(interface, zwlr_screencopy_manager_v1_interface.name))
        screencopy = wl_registry_bind(reg, name, &zwlr_screencopy_manager_v1_interface, 3);
}

static void registry_global_remove(void *d, struct wl_registry *r, uint32_t n) {}

static const struct wl_registry_listener registry_listener = {
    .global        = registry_global,
    .global_remove = registry_global_remove,
};



int main(void) {
    struct wl_display  *display  = wl_display_connect(NULL);
    struct wl_registry *registry = wl_display_get_registry(display);

    wl_registry_add_listener(registry, &registry_listener, NULL);
    wl_display_roundtrip(display);
    wl_display_roundtrip(display);

    struct capture cap = {0};

    struct zwlr_screencopy_frame_v1 *frame =
        zwlr_screencopy_manager_v1_capture_output(screencopy, 0, output);

    zwlr_screencopy_frame_v1_add_listener(frame, &frame_listener, &cap);

    //while (!cap.done)
    while (true)
        wl_display_dispatch(display);

    //printf("Captured %dx%d, first pixel: R=%02x G=%02x B=%02x\n",
    //       cap.width, cap.height,
    //       ((uint8_t*)cap.data)[2],
    //       ((uint8_t*)cap.data)[1],
    //       ((uint8_t*)cap.data)[0]);
    //FILE* outfile = fopen("outfile", "w");
    //fwrite(cap.data, 1, cap.stride * cap.height, outfile);
    //fclose(outfile);
}