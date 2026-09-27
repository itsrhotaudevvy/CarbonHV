#pragma once

#include <jni.h>

#include <cstdint>

namespace carbonhv::graphics {

class PresentationSession {
  public:
    PresentationSession();
    ~PresentationSession();

    PresentationSession(const PresentationSession&) = delete;
    PresentationSession& operator=(const PresentationSession&) = delete;

    bool attach_surface(
        JNIEnv* env,
        jobject surface,
        std::int32_t width,
        std::int32_t height,
        std::int64_t generation) noexcept;

    bool resize_surface(
        JNIEnv* env,
        jobject surface,
        std::int32_t width,
        std::int32_t height,
        std::int64_t generation) noexcept;

    void detach_surface(std::int64_t generation) noexcept;

    bool present_frame(
        JNIEnv* env,
        std::int64_t frame_id,
        jobject hardware_buffer,
        std::int32_t width,
        std::int32_t height,
        std::int32_t format,
        std::uint64_t usage,
        std::int32_t acquire_fence_fd,
        std::int64_t generation) noexcept;

    void release_frame(std::int64_t frame_id) noexcept;

  private:
    struct Impl;
    Impl* impl_;
};

PresentationSession& presentation_session() noexcept;

}  // namespace carbonhv::graphics
