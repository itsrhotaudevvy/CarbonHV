#include "gpu_probe.h"
#include "presentation.h"

#include <jni.h>

#include <cstdint>
#include <optional>
#include <unistd.h>
#include <vector>

namespace {

jobject capability_state(JNIEnv* env, carbonhv::graphics::CapabilityState state) {
    jclass enum_class = env->FindClass("dev/carbonhv/session/NativeCapabilityState");
    if (enum_class == nullptr) {
        return nullptr;
    }

    const char* field_name = "UNKNOWN";
    switch (state) {
        case carbonhv::graphics::CapabilityState::Supported:
            field_name = "SUPPORTED";
            break;
        case carbonhv::graphics::CapabilityState::Unsupported:
            field_name = "UNSUPPORTED";
            break;
        case carbonhv::graphics::CapabilityState::Unknown:
            break;
    }

    const jfieldID field = env->GetStaticFieldID(
        enum_class, field_name, "Ldev/carbonhv/session/NativeCapabilityState;");
    if (field == nullptr) {
        return nullptr;
    }
    return env->GetStaticObjectField(enum_class, field);
}

jstring nullable_string(JNIEnv* env, const std::optional<std::string>& value) {
    if (!value.has_value()) {
        return nullptr;
    }
    return env->NewStringUTF(value->c_str());
}

jobject new_array_list(JNIEnv* env) {
    jclass list_class = env->FindClass("java/util/ArrayList");
    if (list_class == nullptr) {
        return nullptr;
    }
    const jmethodID constructor = env->GetMethodID(list_class, "<init>", "()V");
    return constructor == nullptr ? nullptr : env->NewObject(list_class, constructor);
}

bool list_add(JNIEnv* env, jobject list, jobject value) {
    if (list == nullptr || value == nullptr) {
        return false;
    }
    jclass list_class = env->GetObjectClass(list);
    const jmethodID add = env->GetMethodID(list_class, "add", "(Ljava/lang/Object;)Z");
    if (add == nullptr) {
        return false;
    }
    env->CallBooleanMethod(list, add, value);
    return !env->ExceptionCheck();
}

jobject buffer_support_list(
    JNIEnv* env,
    const std::vector<carbonhv::graphics::NativeBufferSupport>& values) {
    jobject list = new_array_list(env);
    if (list == nullptr) {
        return nullptr;
    }

    jclass support_class = env->FindClass("dev/carbonhv/session/NativeBufferSupport");
    if (support_class == nullptr) {
        return nullptr;
    }
    const jmethodID constructor = env->GetMethodID(support_class, "<init>", "(IJ)V");
    if (constructor == nullptr) {
        return nullptr;
    }

    for (const auto& value : values) {
        jobject item = env->NewObject(
            support_class,
            constructor,
            static_cast<jint>(value.format),
            static_cast<jlong>(value.usage));
        if (!list_add(env, list, item)) {
            return nullptr;
        }
        env->DeleteLocalRef(item);
    }
    return list;
}

jobject refresh_rate_list(JNIEnv* env, const std::vector<float>& values) {
    jobject list = new_array_list(env);
    if (list == nullptr) {
        return nullptr;
    }
    jclass float_class = env->FindClass("java/lang/Float");
    if (float_class == nullptr) {
        return nullptr;
    }
    const jmethodID value_of = env->GetStaticMethodID(float_class, "valueOf", "(F)Ljava/lang/Float;");
    if (value_of == nullptr) {
        return nullptr;
    }
    for (const float value : values) {
        jobject boxed = env->CallStaticObjectMethod(float_class, value_of, value);
        if (!list_add(env, list, boxed)) {
            return nullptr;
        }
        env->DeleteLocalRef(boxed);
    }
    return list;
}

}  // namespace

extern "C" JNIEXPORT jobject JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeProbe(JNIEnv* env, jobject) {
    const auto result = carbonhv::graphics::probe_gpu();

    jclass probe_class = env->FindClass("dev/carbonhv/session/NativeGpuProbe");
    if (probe_class == nullptr) {
        return nullptr;
    }
    const jmethodID constructor = env->GetMethodID(
        probe_class,
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;"
        "Ldev/carbonhv/session/NativeCapabilityState;"
        "Ldev/carbonhv/session/NativeCapabilityState;"
        "Ldev/carbonhv/session/NativeCapabilityState;"
        "Ldev/carbonhv/session/NativeCapabilityState;"
        "Ljava/util/List;Ljava/util/List;)V");
    if (constructor == nullptr) {
        return nullptr;
    }

    jstring gpu_name = nullable_string(env, result.gpu_name);
    jstring api_version = nullable_string(env, result.vulkan_api_version);
    jstring driver_version = nullable_string(env, result.vulkan_driver_version);
    jobject external_memory = capability_state(env, result.external_memory);
    jobject explicit_sync = capability_state(env, result.explicit_sync);
    jobject hardware_buffer = capability_state(env, result.hardware_buffer);
    jobject protected_memory = capability_state(env, result.protected_memory);
    jobject buffer_formats = buffer_support_list(env, result.native_buffer_formats);
    jobject refresh_rates = refresh_rate_list(env, result.display_refresh_rates_hz);

    if (env->ExceptionCheck() || external_memory == nullptr || explicit_sync == nullptr ||
        hardware_buffer == nullptr || protected_memory == nullptr || buffer_formats == nullptr ||
        refresh_rates == nullptr) {
        return nullptr;
    }

    return env->NewObject(
        probe_class,
        constructor,
        gpu_name,
        api_version,
        driver_version,
        external_memory,
        explicit_sync,
        hardware_buffer,
        protected_memory,
        buffer_formats,
        refresh_rates);
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeAttachSurface(
    JNIEnv* env,
    jobject,
    jobject surface,
    jint width,
    jint height,
    jlong generation) {
    return carbonhv::graphics::presentation_session().attach_surface(
        env, surface, width, height, generation)
        ? JNI_TRUE
        : JNI_FALSE;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeResizeSurface(
    JNIEnv* env,
    jobject,
    jobject surface,
    jint width,
    jint height,
    jlong generation) {
    return carbonhv::graphics::presentation_session().resize_surface(
        env, surface, width, height, generation)
        ? JNI_TRUE
        : JNI_FALSE;
}

extern "C" JNIEXPORT void JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeDetachSurface(
    JNIEnv*,
    jobject,
    jlong generation) {
    carbonhv::graphics::presentation_session().detach_surface(generation);
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativePresentFrame(
    JNIEnv* env,
    jobject,
    jlong frame_id,
    jobject hardware_buffer,
    jint width,
    jint height,
    jint format,
    jlong usage,
    jint acquire_fence_fd,
    jlong generation) {
    return carbonhv::graphics::presentation_session().present_frame(
        env,
        frame_id,
        hardware_buffer,
        width,
        height,
        format,
        static_cast<std::uint64_t>(usage),
        acquire_fence_fd,
        generation)
        ? JNI_TRUE
        : JNI_FALSE;
}

extern "C" JNIEXPORT void JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeReleaseFrame(JNIEnv*, jobject, jlong frame_id) {
    carbonhv::graphics::presentation_session().release_frame(frame_id);
}

extern "C" JNIEXPORT jlongArray JNICALL
Java_dev_carbonhv_session_GraphicsNative_nativeDrainReleasedFrames(JNIEnv* env, jobject) {
    auto released = carbonhv::graphics::presentation_session().drain_released_frames();
    const jsize element_count = static_cast<jsize>(released.size() * 2);
    jlongArray values = env->NewLongArray(element_count);
    if (values == nullptr) {
        for (const auto& frame : released) {
            if (frame.release_fence_fd >= 0) {
                close(frame.release_fence_fd);
            }
        }
        return nullptr;
    }

    std::vector<jlong> flattened;
    flattened.reserve(static_cast<std::size_t>(element_count));
    for (const auto& frame : released) {
        flattened.push_back(static_cast<jlong>(frame.frame_id));
        flattened.push_back(static_cast<jlong>(frame.release_fence_fd));
    }
    if (!flattened.empty()) {
        env->SetLongArrayRegion(values, 0, element_count, flattened.data());
    }
    if (env->ExceptionCheck()) {
        for (const auto& frame : released) {
            if (frame.release_fence_fd >= 0) {
                close(frame.release_fence_fd);
            }
        }
        return nullptr;
    }
    return values;
}
