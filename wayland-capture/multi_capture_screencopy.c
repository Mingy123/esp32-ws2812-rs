// Comment out to use shm buffers instead.
#define USE_DMABUF
// The advantages of using dmabuf are not certain. However the general idea
// is that the copying of the frame is done on VRAM, and only specific
// portions of the screen will be copied over through PCIe.

#include <fcntl.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>
#include "wlr-screencopy-unstable-v1.h"

#ifdef USE_DMABUF
#include "linux-dmabuf-v1.h"
#include <gbm.h>
#include <xf86drm.h>
#endif

#define MAX_OUTPUTS 8

static struct wl_shm                     *shm        = NULL;
static struct zwlr_screencopy_manager_v1 *screencopy = NULL;
#ifdef USE_DMABUF
static struct zwp_linux_dmabuf_v1 *dmabuf_iface = NULL;
static uint32_t map_stride[MAX_OUTPUTS];
static void *map_data[MAX_OUTPUTS];
static void *gbm_map[MAX_OUTPUTS];
static struct gbm_bo *gbm_bos[MAX_OUTPUTS];
#endif

struct output_info {
    struct wl_output *output;
    uint32_t          registry_name;
    int               index;
    int               width, height;
    char              name[64];
};

struct capture_session {
    struct wl_buffer *buffer;
    void             *data;
    int               width, height, stride;
    bool              done;
    int               output_index;
};

static struct output_info outputs[MAX_OUTPUTS];
static int                output_count = 0;
static struct capture_session     caps[MAX_OUTPUTS];

// --- Output listener ---

static void output_geometry(void *data, struct wl_output *wl_output,
        int32_t x, int32_t y, int32_t pw, int32_t ph,
        int32_t subpixel, const char *make, const char *model,
        int32_t transform) {}

static void output_mode(void *data, struct wl_output *wl_output,
        uint32_t flags, int32_t width, int32_t height, int32_t refresh)
{
    if (flags & WL_OUTPUT_MODE_CURRENT) {
        struct output_info *info = data;
        info->width  = width;
        info->height = height;
        printf("[%d] mode: %dx%d @ %dHz\n",
               info->index, width, height, refresh / 1000);
    }
}

static void output_done(void *data, struct wl_output *o) {}
static void output_scale(void *data, struct wl_output *o, int32_t f) {}

static void output_name(void *data, struct wl_output *o, const char *name) {
    struct output_info *info = data;
    strncpy(info->name, name, sizeof(info->name) - 1);
    printf("[%d] name: %s\n", info->index, info->name);
}

static void output_description(void *data, struct wl_output *o, const char *d) {}

static const struct wl_output_listener output_listener = {
    .geometry    = output_geometry,
    .mode        = output_mode,
    .done        = output_done,
    .scale       = output_scale,
    .name        = output_name,
    .description = output_description,
};

// --- Shared memory ---

static int open_shm(int output_index) {
    char name[64];
    snprintf(name, sizeof(name), "/wl-capture-%d", output_index);
    int fd = shm_open(name, O_RDWR | O_CREAT, 0600);
    if (fd < 0) perror("shm_open");
    return fd;
}

// --- Frame listener ---

// Provides information about wl_shm buffer parameters that need to be
// used for this frame. This event is sent once after the frame is created
// if wl_shm buffers are supported.
// "Hey, you can make a shm buffer"
static void frame_buffer(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t format, uint32_t width, uint32_t height, uint32_t stride)
{
    #ifdef USE_DMABUF
    return;
    #endif

    struct capture_session *cap = data;
    if (cap->buffer) {
        fprintf(stderr, "[%d] warning: multiple buffer events, ignoring\n",
                cap->output_index);
        return;
    }

    cap->width  = width;
    cap->height = height;
    cap->stride = stride;

    int size = stride * height;
    int fd = open_shm(cap->output_index);
    ftruncate(fd, size);
    cap->data = mmap(NULL, size, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);

    struct wl_shm_pool *pool = wl_shm_create_pool(shm, fd, size);
    cap->buffer = wl_shm_pool_create_buffer(pool, 0,
                      width, height, stride, format);
    wl_shm_pool_destroy(pool);
    close(fd);

    printf("[%d] shm created: %dx%d stride=%d\n",
           cap->output_index, width, height, stride);
}

static void frame_flags(void *data,
        struct zwlr_screencopy_frame_v1 *frame, uint32_t flags) {}

// Called once the frame is copied, indicating it is available for reading.
static void frame_ready(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t tv_sec_hi, uint32_t tv_sec_lo, uint32_t tv_nsec)
{
    struct capture_session *cap = data;
    cap->done = true;
    // TODO: if using dmabuf, read from cap->data and send to ws2812
    // or something else idk
    // NOTE: consider using map_stride to optimise choice of locations to read
    // NOTE: consider using damage events to minimise reads

    // Don't think I need this
    // // Re-arm immediately for continuous capture
    // cap->done   = false;
    // cap->buffer = NULL;  // will be re-allocated next frame_buffer event
    // struct zwlr_screencopy_frame_v1 *next =
    //     zwlr_screencopy_manager_v1_capture_output(
    //         screencopy, 0, outputs[cap->output_index].output);
    // zwlr_screencopy_frame_v1_add_listener(next, &frame_listener, cap);
}

// This event indicates that the attempted frame copy has failed.
static void frame_failed(void *data,
        struct zwlr_screencopy_frame_v1 *frame) {
    struct capture_session *cap = data;
    fprintf(stderr, "[%d] capture failed\n", cap->output_index);
}

// carries the coordinates of the damaged region
// Called right before the ready event when copy_with_damage is requested.
// It may be generated multiple times for each copy_with_damage request.
static void frame_damage(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t x, uint32_t y, uint32_t width, uint32_t height) {}

// Provides information about linux-dmabuf buffer parameters that
// need to be used for this frame.
// Called after the frame is created if linux-dmabuf is supported.
static void frame_linux_dmabuf(void *data,
        struct zwlr_screencopy_frame_v1 *frame,
        uint32_t format, uint32_t width, uint32_t height)
{
#ifdef USE_DMABUF
    struct capture_session *cap = data;
    if (cap->buffer) {
        fprintf(stderr, "[%d] warning: multiple buffer events, ignoring\n",
                cap->output_index);
        return;
    }

    int drm_fd = open("/dev/dri/renderD128", O_RDWR);
    struct gbm_device *gbm = gbm_create_device(drm_fd);
    gbm_bos[cap->output_index] = gbm_bo_create(
        gbm, width, height, format,
        GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR
    );

    int dmabuf_fd = gbm_bo_get_fd(gbm_bos[cap->output_index]);
    uint32_t stride = gbm_bo_get_stride(gbm_bos[cap->output_index]);
    uint64_t modifier = gbm_bo_get_modifier(gbm_bos[cap->output_index]);

    struct zwp_linux_buffer_params_v1 *params =
        zwp_linux_dmabuf_v1_create_params(dmabuf_iface);

    zwp_linux_buffer_params_v1_add(params, dmabuf_fd, 0, 0, stride,
        modifier >> 32, modifier & 0xffffffff);

    cap->width = width;
    cap->height = height;
    cap->stride = stride;

    cap->buffer = zwp_linux_buffer_params_v1_create_immed(params,
        width, height, format, 0);

    gbm_map[cap->output_index] = gbm_bo_map(
        gbm_bos[cap->output_index],
        0,
        0,
        width,
        height,
        GBM_BO_TRANSFER_READ,
        &map_stride[cap->output_index],
        &map_data[cap->output_index]
    );

    zwp_linux_buffer_params_v1_destroy(params);

    printf("[%d] dmabuf created: %dx%d stride=%d\n",
        cap->output_index, width, height, stride);
#endif
}

// all buffer types reported
// This event is sent once after all buffer events have been sent.
static void frame_buffer_done(void *data,
        struct zwlr_screencopy_frame_v1 *frame)
{
    struct capture_session *cap = data;
    // TODO: possibly do FPS limiting by doing the copy every n frames
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

// --- Registry ---

static void registry_global(void *data, struct wl_registry *reg,
        uint32_t name, const char *interface, uint32_t version)
{
    if (!strcmp(interface, wl_shm_interface.name)) {
        shm = wl_registry_bind(reg, name, &wl_shm_interface, 1);

    } else if (!strcmp(interface, wl_output_interface.name)) {
        if (output_count >= MAX_OUTPUTS) return;
        struct output_info *info = &outputs[output_count];
        info->index         = output_count;
        info->registry_name = name;
        info->output        = wl_registry_bind(reg, name, &wl_output_interface, 4);
        wl_output_add_listener(info->output, &output_listener, info);
        output_count++;

    } else if (!strcmp(interface, zwlr_screencopy_manager_v1_interface.name)) {
        screencopy = wl_registry_bind(reg, name,
                         &zwlr_screencopy_manager_v1_interface, 3);

    #ifdef USE_DMABUF
    } else if (!strcmp(interface, zwp_linux_dmabuf_v1_interface.name)) {
        dmabuf_iface = wl_registry_bind(reg, name,
                         &zwp_linux_dmabuf_v1_interface, 3);
    #endif
    }
}

static void registry_global_remove(void *data,
        struct wl_registry *reg, uint32_t name)
{
    for (int i = 0; i < output_count; i++) {
        if (outputs[i].registry_name == name) {
            printf("[%d] output disconnected: %s\n", i, outputs[i].name);
            wl_output_destroy(outputs[i].output);
            memmove(&outputs[i], &outputs[i+1],
                    (output_count - i - 1) * sizeof(struct output_info));
            output_count--;
            break;
        }
    }
}

static const struct wl_registry_listener registry_listener = {
    .global        = registry_global,
    .global_remove = registry_global_remove,
};

// Unlink shm or close dmabuf fds
void signal_handler(int signum) {
#ifdef USE_DMABUF
    puts("Cleaning up dmabuf resources...\n");
    for (int i = 0; i < output_count; i++) {
        if (caps[i].buffer) {
            printf("[%d] cleaning up dmabuf\n", i);
            wl_buffer_destroy(caps[i].buffer);
            printf("[%d] destroying gbm bo\n", i);
            gbm_bo_destroy(gbm_bos[i]);
        }
    }
#else
    puts("Cleaning up shm resources...\n");
    for (int i = 0; i < output_count; i++) {
        if (caps[i].buffer) {
            munmap(caps[i].data, caps[i].stride * caps[i].height);
            wl_buffer_destroy(caps[i].buffer);
            char name[64];
            snprintf(name, sizeof(name), "/wl-capture-%d", i);
            shm_unlink(name);
        }
    }
#endif
    exit(0);
}

// --- Main ---

int main(void) {
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) {
        fprintf(stderr, "Failed to connect to Wayland display\n");
        return 1;
    }

    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &registry_listener, NULL);
    wl_display_roundtrip(display);
    wl_display_roundtrip(display);

    puts("-------------------------------");
    printf("Found %d output(s)\n", output_count);
    puts("===============================");

    // Kick off a capture for each output
    for (int i = 0; i < output_count; i++) {
        caps[i].output_index = i;

        struct zwlr_screencopy_frame_v1 *frame =
            zwlr_screencopy_manager_v1_capture_output(
                screencopy, 0, outputs[i].output
            );
        zwlr_screencopy_frame_v1_add_listener(frame, &frame_listener, &caps[i]);
    }

    signal(SIGINT, signal_handler);
    signal(SIGTERM, signal_handler);

    // Single event loop drives all monitors
    while (true)
        wl_display_dispatch(display);
}