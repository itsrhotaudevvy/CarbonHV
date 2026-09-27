#include "presentation.h"

#include <android/hardware_buffer.h>
#include <android/hardware_buffer_jni.h>
#include <android/native_window.h>
#include <android/native_window_jni.h>
#include <android/surface_control.h>
#include <dlfcn.h>
#include <unistd.h>

#include <cstdint>
#include <mutex>
#include <unordered_map>
#include <unordered_set>

namespace carbonhv::graphics {
namespace {

using SetBufferWithReleaseFn = void (*)(
    ASurfaceTransaction*,
    ASurfaceControl*,
    AHardwareBuffer*,
    int,
    void*,
    ASurfaceTransaction_OnBufferRelease);

SetBufferWithReleaseFn resolve_set_buffer_with_release() noexcept {
    static SetBufferWithReleaseFn function = []() -> SetBufferWithReleaseFn {
        void* handle = dlopen("libandroid.so", RTLD_NOW | RTLD_LOCAL);
        if (handle == nullptr) {
            return nullptr;
        }
        return reinterpret_cast<SetBufferWithReleaseFn>(
            dlsym(handle, "ASurfaceTransaction_setBufferWithRelease"));
    }();
    return function;
}

void close_if_valid(int fd) noexcept {
    if (fd >= 0) {
        close(fd);
    }
}

}  // namespace

struct PresentationSession::Impl {
    struct ReleaseContext {
        Impl* impl;
        std::int64_t frame_id;
    };

    std::mutex mutex;
    ANativeWindow* window = nullptr;
    ASurfaceControl* layer = nullptr;
    std::int32_t width = 0;
    std::int32_t height = 0;
    std::int64_t generation = 0;
    std::unordered_set<std::int64_t> pending_frames;
    std::unordered_map<std::int64_t, int> release_fences;

    static void on_buffer_release(void* context, int release_fence_fd) noexcept {
        auto* release = static_cast<ReleaseContext*>(context);
        if (release == nullptr || release->impl == nullptr) {
            close_if_valid(release_fence_fd);
            delete release;
            return;
        }

        Impl* impl = release->impl;
        {
            std::lock_guard lock(impl->mutex);
            if (impl->pending_frames.erase(release->frame_id) == 0) {
                close_if_valid(release_fence_fd);
            } else {
                const auto existing = impl->release_fences.find(release->frame_id);
                if (existing != impl->release_fences.end()) {
                    close_if_valid(existing->second);
                }
                impl->release_fences[release->frame_id] = release_fence_fd;
            }
        }
        delete release;
    }

    void destroy_surface_locked() noexcept {
        if (layer != nullptr) {
            ASurfaceTransaction* transaction = ASurfaceTransaction_create();
            if (transaction != nullptr) {
                ASurfaceTransaction_setVisibility(
                    transaction, layer, ASURFACE_TRANSACTION_VISIBILITY_HIDE);
                ASurfaceTransaction_apply(transaction);
                ASurfaceTransaction_delete(transaction);
            }
            ASurfaceControl_release(layer);
            layer = nullptr;
        }
        if (window != nullptr) {
            ANativeWindow_release(window);
            window = nullptr;
        }
        width = 0;
        height = 0;
    }

    bool replace_surface_locked(
        JNIEnv* env,
        jobject surface,
        std::int32_t new_width,
        std::int32_t new_height,
        std::int64_t new_generation) noexcept {
        if (env == nullptr || surface == nullptr || new_width <= 0 || new_height <= 0) {
            return false;
        }

        ANativeWindow* new_window = ANativeWindow_fromSurface(env, surface);
        if (new_window == nullptr) {
            return false;
        }
        ASurfaceControl* new_layer =
            ASurfaceControl_createFromWindow(new_window, "CarbonHV guest framebuffer");
        if (new_layer == nullptr) {
            ANativeWindow_release(new_window);
            return false;
        }

        destroy_surface_locked();
        window = new_window;
        layer = new_layer;
        width = new_width;
        height = new_height;
        generation = new_generation;
        return true;
    }

    void release_frame_locked(std::int64_t frame_id) noexcept {
        pending_frames.erase(frame_id);
        const auto fence = release_fences.find(frame_id);
        if (fence != release_fences.end()) {
            close_if_valid(fence->second);
            release_fences.erase(fence);
        }
    }
};

PresentationSession::PresentationSession() : impl_(new Impl()) {}

PresentationSession::~PresentationSession() {
    if (impl_ == nullptr) {
        return;
    }
    std::lock_guard lock(impl_->mutex);
    for (const auto& [_, fd] : impl_->release_fences) {
        close_if_valid(fd);
    }
    impl_->release_fences.clear();
    impl_->pending_frames.clear();
    impl_->destroy_surface_locked();
    delete impl_;
    impl_ = nullptr;
}

bool PresentationSession::attach_surface(
    JNIEnv* env,
    jobject surface,
    std::int32_t width,
    std::int32_t height,
    std::int64_t generation) noexcept {
    std::lock_guard lock(impl_->mutex);
    return impl_->replace_surface_locked(env, surface, width, height, generation);
}

bool PresentationSession::resize_surface(
    JNIEnv* env,
    jobject surface,
    std::int32_t width,
    std::int32_t height,
    std::int64_t generation) noexcept {
    std::lock_guard lock(impl_->mutex);
    return impl_->replace_surface_locked(env, surface, width, height, generation);
}

void PresentationSession::detach_surface(std::int64_t generation) noexcept {
    std::lock_guard lock(impl_->mutex);
    impl_->generation = generation;
    impl_->destroy_surface_locked();
}

bool PresentationSession::present_frame(
    JNIEnv* env,
    std::int64_t frame_id,
    jobject hardware_buffer,
    std::int32_t width,
    std::int32_t height,
    std::int32_t format,
    std::uint64_t usage,
    std::int32_t acquire_fence_fd,
    std::int64_t generation) noexcept {
    if (env == nullptr || hardware_buffer == nullptr || frame_id < 0 || width <= 0 || height <= 0 ||
        format <= 0 || usage == 0 || acquire_fence_fd < -1) {
        return false;
    }

    AHardwareBuffer* buffer = AHardwareBuffer_fromHardwareBuffer(env, hardware_buffer);
    if (buffer == nullptr) {
        return false;
    }

    AHardwareBuffer_Desc descriptor{};
    AHardwareBuffer_describe(buffer, &descriptor);
    const bool descriptor_matches =
        descriptor.width == static_cast<std::uint32_t>(width) &&
        descriptor.height == static_cast<std::uint32_t>(height) &&
        descriptor.format == static_cast<std::uint32_t>(format) &&
        (descriptor.usage & usage) == usage &&
        (descriptor.usage & AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE) != 0;
    if (!descriptor_matches) {
        return false;
    }

    const SetBufferWithReleaseFn set_buffer_with_release = resolve_set_buffer_with_release();
    if (set_buffer_with_release == nullptr) {
        return false;
    }

    std::lock_guard lock(impl_->mutex);
    if (impl_->layer == nullptr || impl_->window == nullptr || generation != impl_->generation ||
        width != impl_->width || height != impl_->height || impl_->pending_frames.contains(frame_id) ||
        impl_->release_fences.contains(frame_id)) {
        return false;
    }

    ASurfaceTransaction* transaction = ASurfaceTransaction_create();
    if (transaction == nullptr) {
        return false;
    }

    auto* release_context = new Impl::ReleaseContext{impl_, frame_id};
    impl_->pending_frames.insert(frame_id);
    ASurfaceTransaction_setVisibility(
        transaction, impl_->layer, ASURFACE_TRANSACTION_VISIBILITY_SHOW);
    set_buffer_with_release(
        transaction,
        impl_->layer,
        buffer,
        acquire_fence_fd,
        release_context,
        &Impl::on_buffer_release);
    ASurfaceTransaction_apply(transaction);
    ASurfaceTransaction_delete(transaction);
    return true;
}

void PresentationSession::release_frame(std::int64_t frame_id) noexcept {
    std::lock_guard lock(impl_->mutex);
    impl_->release_frame_locked(frame_id);
}

PresentationSession& presentation_session() noexcept {
    static auto* session = new PresentationSession();
    return *session;
}

}  // namespace carbonhv::graphics
