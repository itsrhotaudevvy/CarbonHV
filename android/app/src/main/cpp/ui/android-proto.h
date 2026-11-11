/*
 * CarbonHV: frame socket wire format shared with the `:vm` service.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 *
 * This header is the C producer of the format that `crates/carbon-emu/src/frame_proto.rs`
 * parses. It is deliberately free of QEMU and Android headers: the two files that make the
 * protocol hard to get right (byte layout and the buffer ring) are compiled and exercised by
 * `qualification/emu/frame-proto/` on the build host, against the Rust decoder.
 *
 * Every message is fixed size, little-endian, and carries a 12-byte header:
 *
 *     u32 magic ('CHVF'), u16 version, u16 kind, u32 payload length
 *
 * A `FRAME` is followed by one extra datagram holding the AHardwareBuffer handle (see
 * `AHardwareBuffer_sendHandleToUnixSocket`) and, when ANDROID_FRAME_FLAG_HAS_FENCE is set, an
 * `SCM_RIGHTS` sync_file descriptor. The socket is SOCK_SEQPACKET, so message boundaries and
 * order are preserved.
 */
#ifndef UI_ANDROID_PROTO_H
#define UI_ANDROID_PROTO_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define ANDROID_PROTO_VERSION 1
#define ANDROID_PROTO_MAGIC 0x43485646u /* "CHVF" */
#define ANDROID_PROTO_HEADER_BYTES 12
#define ANDROID_PROTO_MAX_PAYLOAD 4096
/* Bounded so a corrupt producer cannot make the reader allocate or trust a huge size. */
#define ANDROID_PROTO_MAX_DIMENSION 16384u

#define ANDROID_MSG_HELLO 1
#define ANDROID_MSG_MODE 2
#define ANDROID_MSG_FRAME 3
#define ANDROID_MSG_CURSOR 4
#define ANDROID_MSG_RELEASE 0x81
#define ANDROID_MSG_RESIZE 0x82

/* FRAME.flags bit: a sync_file descriptor follows the buffer handle. */
#define ANDROID_FRAME_FLAG_HAS_FENCE (1u << 0)

/* AHardwareBuffer_Format / AHardwareBuffer_UsageFlags values, copied from the NDK so this
 * header stays host-compilable. ui/android.c asserts they still match the NDK. */
#define ANDROID_AHB_FORMAT_R8G8B8A8_UNORM 1u
#define ANDROID_AHB_USAGE_CPU_READ_RARELY (2u << 0)
#define ANDROID_AHB_USAGE_CPU_WRITE_OFTEN (3u << 4)
#define ANDROID_AHB_USAGE_GPU_SAMPLED_IMAGE (1u << 8)
/* What a frame is allocated as: written often by the CPU (QEMU's readback), sampled by the GPU
 * (the app's renderer), and readable rarely by the CPU. The read flag costs nothing on this
 * hardware and is what makes screenshots and on-device diagnostics possible without a second
 * buffer: a buffer allocated with CPU_READ_NEVER cannot be locked for reading at all. */
#define ANDROID_FRAME_USAGE \
    ((uint64_t)(ANDROID_AHB_USAGE_CPU_READ_RARELY | ANDROID_AHB_USAGE_CPU_WRITE_OFTEN | \
                ANDROID_AHB_USAGE_GPU_SAMPLED_IMAGE))

/* The ring depth QEMU keeps per display mode. Mirrors the app's backpressure: never reuse a
 * buffer that has not been RELEASEd, and drop the frame rather than block a vCPU. */
#define ANDROID_RING_SLOTS 3

/* Message sizes, payload only. */
#define ANDROID_HELLO_BYTES 4
#define ANDROID_MODE_BYTES 12
#define ANDROID_FRAME_BYTES 28
#define ANDROID_CURSOR_BYTES 12
#define ANDROID_RELEASE_BYTES 4
#define ANDROID_RESIZE_BYTES 12

static inline void android_put_u16(uint8_t *out, uint16_t value)
{
    out[0] = (uint8_t)(value & 0xff);
    out[1] = (uint8_t)((value >> 8) & 0xff);
}

static inline void android_put_u32(uint8_t *out, uint32_t value)
{
    android_put_u16(out, (uint16_t)(value & 0xffff));
    android_put_u16(out + 2, (uint16_t)((value >> 16) & 0xffff));
}

static inline void android_put_u64(uint8_t *out, uint64_t value)
{
    android_put_u32(out, (uint32_t)(value & 0xffffffffu));
    android_put_u32(out + 4, (uint32_t)((value >> 32) & 0xffffffffu));
}

static inline uint16_t android_get_u16(const uint8_t *in)
{
    return (uint16_t)(in[0] | ((uint16_t)in[1] << 8));
}

static inline uint32_t android_get_u32(const uint8_t *in)
{
    return (uint32_t)android_get_u16(in) |
           ((uint32_t)android_get_u16(in + 2) << 16);
}

/* Writes the header and returns the total message size; `payload` is written after it. */
static inline size_t android_proto_header(uint8_t *out, uint16_t kind, size_t payload_bytes)
{
    android_put_u32(out, ANDROID_PROTO_MAGIC);
    android_put_u16(out + 4, ANDROID_PROTO_VERSION);
    android_put_u16(out + 6, kind);
    android_put_u32(out + 8, (uint32_t)payload_bytes);
    return ANDROID_PROTO_HEADER_BYTES + payload_bytes;
}

static inline size_t android_proto_hello(uint8_t *out)
{
    android_put_u16(out + ANDROID_PROTO_HEADER_BYTES, ANDROID_PROTO_VERSION);
    android_put_u16(out + ANDROID_PROTO_HEADER_BYTES + 2, 0);
    return android_proto_header(out, ANDROID_MSG_HELLO, ANDROID_HELLO_BYTES);
}

static inline size_t android_proto_mode(uint8_t *out, uint32_t width, uint32_t height,
                                        uint32_t format)
{
    uint8_t *payload = out + ANDROID_PROTO_HEADER_BYTES;

    android_put_u32(payload, width);
    android_put_u32(payload + 4, height);
    android_put_u32(payload + 8, format);
    return android_proto_header(out, ANDROID_MSG_MODE, ANDROID_MODE_BYTES);
}

static inline size_t android_proto_frame(uint8_t *out, uint32_t seq, uint32_t width,
                                         uint32_t height, uint32_t format, uint64_t usage,
                                         bool has_fence)
{
    uint8_t *payload = out + ANDROID_PROTO_HEADER_BYTES;

    android_put_u32(payload, seq);
    android_put_u32(payload + 4, width);
    android_put_u32(payload + 8, height);
    android_put_u32(payload + 12, format);
    android_put_u64(payload + 16, usage);
    android_put_u32(payload + 24, has_fence ? ANDROID_FRAME_FLAG_HAS_FENCE : 0);
    return android_proto_header(out, ANDROID_MSG_FRAME, ANDROID_FRAME_BYTES);
}

static inline size_t android_proto_cursor(uint8_t *out, uint32_t x, uint32_t y, bool visible)
{
    uint8_t *payload = out + ANDROID_PROTO_HEADER_BYTES;

    android_put_u32(payload, x);
    android_put_u32(payload + 4, y);
    android_put_u32(payload + 8, visible ? 1 : 0);
    return android_proto_header(out, ANDROID_MSG_CURSOR, ANDROID_CURSOR_BYTES);
}

/*
 * Validates a datagram received from the app and returns its payload, or NULL. Only the two
 * app -> QEMU kinds are accepted; anything else (including a FRAME arriving here) is dropped.
 */
static inline const uint8_t *android_proto_expect(const uint8_t *bytes, size_t len,
                                                  uint16_t kind, size_t payload_bytes)
{
    if (!bytes || len != ANDROID_PROTO_HEADER_BYTES + payload_bytes) {
        return NULL;
    }
    if (android_get_u32(bytes) != ANDROID_PROTO_MAGIC) {
        return NULL;
    }
    if (android_get_u16(bytes + 4) != ANDROID_PROTO_VERSION) {
        return NULL;
    }
    if (android_get_u16(bytes + 6) != kind) {
        return NULL;
    }
    if (android_get_u32(bytes + 8) != (uint32_t)payload_bytes) {
        return NULL;
    }
    return bytes + ANDROID_PROTO_HEADER_BYTES;
}

static inline bool android_proto_decode_release(const uint8_t *bytes, size_t len,
                                                uint32_t *seq)
{
    const uint8_t *payload = android_proto_expect(bytes, len, ANDROID_MSG_RELEASE,
                                                  ANDROID_RELEASE_BYTES);
    uint32_t value;

    if (!payload) {
        return false;
    }
    value = android_get_u32(payload);
    if (value == 0) {
        return false;
    }
    *seq = value;
    return true;
}

static inline bool android_proto_decode_resize(const uint8_t *bytes, size_t len,
                                               uint32_t *width, uint32_t *height,
                                               uint32_t *dpi)
{
    const uint8_t *payload = android_proto_expect(bytes, len, ANDROID_MSG_RESIZE,
                                                  ANDROID_RESIZE_BYTES);
    uint32_t w, h, d;

    if (!payload) {
        return false;
    }
    w = android_get_u32(payload);
    h = android_get_u32(payload + 4);
    d = android_get_u32(payload + 8);
    if (w == 0 || h == 0 || w > ANDROID_PROTO_MAX_DIMENSION ||
        h > ANDROID_PROTO_MAX_DIMENSION || d == 0 || d > 10000) {
        return false;
    }
    *width = w;
    *height = h;
    *dpi = d;
    return true;
}

/*
 * The per-mode buffer ring. A slot is busy from the moment a FRAME is sent until the app
 * echoes its seq in a RELEASE, so QEMU never overwrites a buffer the app is still sampling.
 * `next_seq` starts at 1 because 0 means "no sequence" in the protocol.
 */
typedef struct AndroidRingSlot {
    uint32_t seq;
    bool busy;
} AndroidRingSlot;

typedef struct AndroidFrameRing {
    AndroidRingSlot slot[ANDROID_RING_SLOTS];
    uint32_t next_seq;
} AndroidFrameRing;

static inline void android_ring_init(AndroidFrameRing *ring)
{
    size_t i;

    for (i = 0; i < ANDROID_RING_SLOTS; i++) {
        ring->slot[i].seq = 0;
        ring->slot[i].busy = false;
    }
    ring->next_seq = 1;
}

/* A mode change invalidates every outstanding buffer; the app's late RELEASEs are ignored. */
static inline void android_ring_reset(AndroidFrameRing *ring)
{
    android_ring_init(ring);
}

static inline uint32_t android_ring_inflight(const AndroidFrameRing *ring)
{
    size_t i;
    uint32_t count = 0;

    for (i = 0; i < ANDROID_RING_SLOTS; i++) {
        if (ring->slot[i].busy) {
            count++;
        }
    }
    return count;
}

/* Marks a slot busy and gives it the next sequence number. False means "drop this frame". */
static inline bool android_ring_acquire(AndroidFrameRing *ring, uint32_t *index,
                                        uint32_t *seq)
{
    size_t i;

    for (i = 0; i < ANDROID_RING_SLOTS; i++) {
        if (!ring->slot[i].busy) {
            uint32_t value = ring->next_seq;

            ring->next_seq = value == UINT32_MAX ? 1 : value + 1;
            ring->slot[i].seq = value;
            ring->slot[i].busy = true;
            *index = (uint32_t)i;
            *seq = value;
            return true;
        }
    }
    return false;
}

/* Frees the slot a RELEASE names. False for an unknown or already-freed sequence. */
static inline bool android_ring_release(AndroidFrameRing *ring, uint32_t seq)
{
    size_t i;

    for (i = 0; i < ANDROID_RING_SLOTS; i++) {
        if (ring->slot[i].busy && ring->slot[i].seq == seq) {
            ring->slot[i].busy = false;
            ring->slot[i].seq = 0;
            return true;
        }
    }
    return false;
}

#endif /* UI_ANDROID_PROTO_H */
