#include "gpu_probe.h"

#include <jni.h>

#include <cstdint>
#include <optional>

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
