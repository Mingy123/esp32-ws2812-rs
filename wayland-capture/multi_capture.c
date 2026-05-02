// Comment out to use shm buffers instead.
// #define USE_DMABUF
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
#include "ext-image-copy-capture.h"
#include "ext-image-capture-source.h"

#ifdef USE_DMABUF
#include "linux-dmabuf-v1.h"
#include <gbm.h>
#include <xf86drm.h>
#endif

#define MAX_OUTPUTS 8

static struct wl_shm *shm = NULL;
static struct ext_output_image_capture_source_manager_v1 *capture_source_manager = NULL;
static struct ext_image_copy_capture_manager_v1 *copy_manager = NULL;
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
    struct ext_image_copy_capture_session_v1 *copy_session;
    struct wl_buffer *buffer;
    void             *data;
    uint32_t          shm_format, dmabuf_format;
    int               width, height, stride;
    bool              done;
    int               output_index;
};

static struct output_info outputs[MAX_OUTPUTS];
static int                output_count = 0;
static struct capture_session   caps[MAX_OUTPUTS];
static const struct ext_image_copy_capture_frame_v1_listener frame_listener;

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

// --- Session listener ---

static void buffer_size(void *data,
        struct ext_image_copy_capture_session_v1 *session, uint32_t width, uint32_t height)
{
    struct capture_session *cap = data;
    cap->width  = width;
    cap->height = height;
    cap->stride = width * 4;  // assume ARGB8888
}

static void shm_format(void *data,
        struct ext_image_copy_capture_session_v1 *session, uint32_t format)
{
    struct capture_session *cap = data;
    cap->shm_format = format;
}

// Gives a wl_array containing the device name that dma-buf buffers must be allocated on.
// `device` is a wl_array of dev_t values
static void dmabuf_device(void *data,
        struct ext_image_copy_capture_session_v1 *session, struct wl_array *device)
{}

static void dmabuf_format(void *data,
        struct ext_image_copy_capture_session_v1 *session, uint32_t format, struct wl_array *modifiers)
{
    struct capture_session *cap = data;
    cap->dmabuf_format = format;
    printf("[%d] dmabuf format: 0x%08x modifiers:", cap->output_index, format);
    for (size_t i = 0; i < modifiers->size / sizeof(uint64_t); i++) {
        uint64_t modifier = ((uint64_t *)modifiers->data)[i];
        printf(" 0x%016" PRIx64, modifier);
    }
    printf("\n");
}

static void done(void *data,
        struct ext_image_copy_capture_session_v1 *session)
{
    struct capture_session *cap = data;
    if (cap->buffer != NULL) {
        printf("[%d] warning: buffer already exists, ignoring\n", cap->output_index);
    }
#ifdef USE_DMABUF
        int drm_fd = open("/dev/dri/renderD128", O_RDWR);
        struct gbm_device *gbm = gbm_create_device(drm_fd);

        gbm_bos[cap->output_index] = gbm_bo_create(
            gbm, cap->width, cap->height, cap->dmabuf_format,
            GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR
        );

        int dmabuf_fd = gbm_bo_get_fd(gbm_bos[cap->output_index]);
        uint32_t stride = gbm_bo_get_stride(gbm_bos[cap->output_index]);
        uint64_t modifier = gbm_bo_get_modifier(gbm_bos[cap->output_index]);

        struct zwp_linux_buffer_params_v1 *params =
            zwp_linux_dmabuf_v1_create_params(dmabuf_iface);
        zwp_linux_buffer_params_v1_add(
            params, dmabuf_fd, 0, 0, stride, modifier >> 32, modifier & 0xffffffff
        );

        cap->stride = stride;
        cap->buffer = zwp_linux_buffer_params_v1_create_immed(
            params, cap->width, cap->height, cap->dmabuf_format, 0
        );

        gbm_map[cap->output_index] = gbm_bo_map(
            gbm_bos[cap->output_index],
            0,
            0,
            cap->width,
            cap->height,
            GBM_BO_TRANSFER_READ,
            &map_stride[cap->output_index],
            &map_data[cap->output_index]
        );

        zwp_linux_buffer_params_v1_destroy(params);
        printf("[%d] dmabuf created: %dx%d stride=%d\n",
            cap->output_index, cap->width, cap->height, stride);
#else
        int size = cap->stride * cap->height;
        int fd = open_shm(cap->output_index);
        ftruncate(fd, size);
        cap->data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);

        struct wl_shm_pool *pool = wl_shm_create_pool(shm, fd, size);
        cap->buffer = wl_shm_pool_create_buffer(
            pool, 0, cap->width, cap->height, cap->stride, cap->shm_format
        );
        wl_shm_pool_destroy(pool);
        close(fd);
        printf("[%d] shm created: %dx%d stride=%d\n",
               cap->output_index, cap->width, cap->height, cap->stride);
#endif

    struct ext_image_copy_capture_frame_v1 *new_frame =
        ext_image_copy_capture_session_v1_create_frame(session);
    ext_image_copy_capture_frame_v1_attach_buffer(new_frame, cap->buffer);
    ext_image_copy_capture_frame_v1_add_listener(new_frame, &frame_listener, cap);
    // I don't need to do this but the docs said I should damage the buffer
    // I'm thinking the compositor takes in damage from both this code and the actual screen
    // or maybe I just don't have the damage tracking feature
    ext_image_copy_capture_frame_v1_damage_buffer(new_frame, 0, 0, cap->width, cap->height);
    ext_image_copy_capture_frame_v1_capture(new_frame);
    printf("[%d] capture started\n", cap->output_index);
}

static void stopped(void *data,
        struct ext_image_copy_capture_session_v1 *session)
{
    struct capture_session *cap = data;
    cap->copy_session = NULL;
    ext_image_copy_capture_session_v1_destroy(session);
}

static const struct ext_image_copy_capture_session_v1_listener session_listener = {
    .buffer_size        = buffer_size,
    .shm_format         = shm_format,
    .dmabuf_device      = dmabuf_device,
    .dmabuf_format      = dmabuf_format,
    .done               = done,
    .stopped            = stopped,
};


// --- Frame listener ---

static void transform(void *data,
        struct ext_image_copy_capture_frame_v1 *frame, uint32_t transform)
{}

static void damage(void *data,
        struct ext_image_copy_capture_frame_v1 *frame, int32_t x, int32_t y, int32_t width, int32_t height)
{}

static void presentation_time(void *data,
        struct ext_image_copy_capture_frame_v1 *frame, uint32_t tv_sec_hi, uint32_t tv_sec_lo, uint32_t tv_nsec)
{}

static void frame_ready(void *data,
        struct ext_image_copy_capture_frame_v1 *frame)
{
    struct capture_session *cap = data;

    ext_image_copy_capture_frame_v1_destroy(frame);
    struct ext_image_copy_capture_frame_v1 *new_frame =
        ext_image_copy_capture_session_v1_create_frame(cap->copy_session);
    ext_image_copy_capture_frame_v1_attach_buffer(new_frame, cap->buffer);
    ext_image_copy_capture_frame_v1_add_listener(new_frame, &frame_listener, data);
    // ext_image_copy_capture_frame_v1_damage_buffer(new_frame, 0, 0, cap->width, cap->height);
    ext_image_copy_capture_frame_v1_capture(new_frame);
}

static void frame_failed(void *data,
        struct ext_image_copy_capture_frame_v1 *frame, uint32_t reason)
{
    struct capture_session *cap = data;
    fprintf(stderr, "Frame capture failed with reason %u\n", reason);
    ext_image_copy_capture_frame_v1_destroy(frame);
    struct ext_image_copy_capture_frame_v1 *new_frame =
        ext_image_copy_capture_session_v1_create_frame(cap->copy_session);
    ext_image_copy_capture_frame_v1_attach_buffer(new_frame, cap->buffer);
    ext_image_copy_capture_frame_v1_add_listener(new_frame, &frame_listener, data);
    // ext_image_copy_capture_frame_v1_damage_buffer(new_frame, 0, 0, cap->width, cap->height);
    ext_image_copy_capture_frame_v1_capture(new_frame);
}

static const struct ext_image_copy_capture_frame_v1_listener frame_listener = {
    .transform          = transform,
    .damage             = damage,
    .presentation_time  = presentation_time,
    .ready              = frame_ready,
    .failed             = frame_failed,
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

    } else if (!strcmp(interface, ext_output_image_capture_source_manager_v1_interface.name)) {
        capture_source_manager = wl_registry_bind(reg, name, &ext_output_image_capture_source_manager_v1_interface, 1);

    } else if (!strcmp(interface, ext_image_copy_capture_manager_v1_interface.name)) {
        copy_manager = wl_registry_bind(reg, name, &ext_image_copy_capture_manager_v1_interface, 1);

    #ifdef USE_DMABUF
    } else if (!strcmp(interface, zwp_linux_dmabuf_v1_interface.name)) {
        dmabuf_iface = wl_registry_bind(reg, name, &zwp_linux_dmabuf_v1_interface, 3);
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
    // Roundtrip to get registry events and output info
    wl_display_roundtrip(display);
    wl_display_roundtrip(display);

    puts("-------------------------------");
    printf("Found %d output(s)\n", output_count);
    puts("===============================");

    // Kick off a capture for each output
    for (int i = 0; i < output_count; i++) {
        caps[i].output_index = i;

        struct ext_image_capture_source_v1 *source = NULL;
        source = ext_output_image_capture_source_manager_v1_create_source(capture_source_manager, outputs[i].output);

        caps[i].copy_session = ext_image_copy_capture_manager_v1_create_session(copy_manager, source, 0);
        ext_image_copy_capture_session_v1_add_listener(caps[i].copy_session, &session_listener, &caps[i]);

        printf("[%d] capture session created\n", i);
    }

    signal(SIGINT, signal_handler);
    signal(SIGTERM, signal_handler);

    // Single event loop drives all monitors
    while (true)
        wl_display_dispatch(display);
}
