#include "gpu_probe.h"

#include <android/hardware_buffer.h>
#include <dlfcn.h>
#include <vulkan/vulkan.h>

#include <algorithm>
#include <array>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

namespace carbonhv::graphics {
namespace {

constexpr const char* kAhbExternalMemoryExtension =
    "VK_ANDROID_external_memory_android_hardware_buffer";
constexpr const char* kExternalSemaphoreFdExtension = "VK_KHR_external_semaphore_fd";
constexpr const char* kExternalFenceFdExtension = "VK_KHR_external_fence_fd";

struct DlCloser {
    void operator()(void* handle) const noexcept {
        if (handle != nullptr) {
            dlclose(handle);
        }
    }
};

using DlHandle = std::unique_ptr<void, DlCloser>;

std::string format_api_version(std::uint32_t version) {
    return std::to_string(VK_VERSION_MAJOR(version)) + "." +
           std::to_string(VK_VERSION_MINOR(version)) + "." +
           std::to_string(VK_VERSION_PATCH(version));
}

bool has_extension(const std::vector<VkExtensionProperties>& extensions, const char* name) {
    return std::any_of(
        extensions.begin(),
        extensions.end(),
        [name](const VkExtensionProperties& extension) {
            return std::strcmp(extension.extensionName, name) == 0;
        });
}

void probe_hardware_buffers(GpuProbeResult& result) {
    constexpr std::uint64_t usage =
        AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE | AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT;
    constexpr std::array<std::uint32_t, 4> formats = {
        AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
        AHARDWAREBUFFER_FORMAT_R8G8B8X8_UNORM,
        AHARDWAREBUFFER_FORMAT_R5G6B5_UNORM,
        AHARDWAREBUFFER_FORMAT_R16G16B16A16_FLOAT,
    };

    for (const auto format : formats) {
        AHardwareBuffer_Desc descriptor{};
        descriptor.width = 64;
        descriptor.height = 64;
        descriptor.layers = 1;
        descriptor.format = format;
        descriptor.usage = usage;
        if (AHardwareBuffer_isSupported(&descriptor) != 0) {
            result.native_buffer_formats.push_back(
                NativeBufferSupport{static_cast<std::int32_t>(format), usage});
        }
    }

    result.hardware_buffer = result.native_buffer_formats.empty()
        ? CapabilityState::Unsupported
        : CapabilityState::Supported;
}

void probe_vulkan(GpuProbeResult& result) {
    DlHandle vulkan(dlopen("libvulkan.so", RTLD_NOW | RTLD_LOCAL));
    if (!vulkan) {
        return;
    }

    const auto get_instance_proc_addr = reinterpret_cast<PFN_vkGetInstanceProcAddr>(
        dlsym(vulkan.get(), "vkGetInstanceProcAddr"));
    if (get_instance_proc_addr == nullptr) {
        return;
    }

    const auto create_instance = reinterpret_cast<PFN_vkCreateInstance>(
        get_instance_proc_addr(VK_NULL_HANDLE, "vkCreateInstance"));
    if (create_instance == nullptr) {
        return;
    }

    VkApplicationInfo app_info{};
    app_info.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
    app_info.pApplicationName = "CarbonHV GPU Probe";
    app_info.applicationVersion = 1;
    app_info.pEngineName = "CarbonHV";
    app_info.engineVersion = 1;
    app_info.apiVersion = VK_API_VERSION_1_0;

    VkInstanceCreateInfo create_info{};
    create_info.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    create_info.pApplicationInfo = &app_info;

    VkInstance instance = VK_NULL_HANDLE;
    if (create_instance(&create_info, nullptr, &instance) != VK_SUCCESS || instance == VK_NULL_HANDLE) {
        return;
    }

    const auto destroy_instance = reinterpret_cast<PFN_vkDestroyInstance>(
        get_instance_proc_addr(instance, "vkDestroyInstance"));
    const auto enumerate_physical_devices = reinterpret_cast<PFN_vkEnumeratePhysicalDevices>(
        get_instance_proc_addr(instance, "vkEnumeratePhysicalDevices"));
    const auto get_properties = reinterpret_cast<PFN_vkGetPhysicalDeviceProperties>(
        get_instance_proc_addr(instance, "vkGetPhysicalDeviceProperties"));
    const auto enumerate_extensions = reinterpret_cast<PFN_vkEnumerateDeviceExtensionProperties>(
        get_instance_proc_addr(instance, "vkEnumerateDeviceExtensionProperties"));

    if (enumerate_physical_devices != nullptr && get_properties != nullptr) {
        std::uint32_t device_count = 0;
        if (enumerate_physical_devices(instance, &device_count, nullptr) == VK_SUCCESS && device_count > 0) {
            std::vector<VkPhysicalDevice> devices(device_count);
            if (enumerate_physical_devices(instance, &device_count, devices.data()) == VK_SUCCESS &&
                device_count > 0) {
                const VkPhysicalDevice device = devices.front();
                VkPhysicalDeviceProperties properties{};
                get_properties(device, &properties);
                result.gpu_name = properties.deviceName;
                result.vulkan_api_version = format_api_version(properties.apiVersion);
                result.vulkan_driver_version = std::to_string(properties.driverVersion);

                if (enumerate_extensions != nullptr) {
                    std::uint32_t extension_count = 0;
                    const VkResult count_result =
                        enumerate_extensions(device, nullptr, &extension_count, nullptr);
                    if (count_result == VK_SUCCESS) {
                        std::vector<VkExtensionProperties> extensions(extension_count);
                        const VkResult extensions_result = extension_count == 0
                            ? VK_SUCCESS
                            : enumerate_extensions(
                                  device, nullptr, &extension_count, extensions.data());
                        if (extensions_result == VK_SUCCESS) {
                            result.external_memory = has_extension(
                                extensions, kAhbExternalMemoryExtension)
                                ? CapabilityState::Supported
                                : CapabilityState::Unsupported;
                            const bool semaphore_fd =
                                has_extension(extensions, kExternalSemaphoreFdExtension);
                            const bool fence_fd = has_extension(extensions, kExternalFenceFdExtension);
                            result.explicit_sync = semaphore_fd && fence_fd
                                ? CapabilityState::Supported
                                : CapabilityState::Unsupported;
                        }
                    }
                }

                const auto get_features2 = reinterpret_cast<PFN_vkGetPhysicalDeviceFeatures2>(
                    get_instance_proc_addr(instance, "vkGetPhysicalDeviceFeatures2"));
                if (get_features2 != nullptr) {
                    VkPhysicalDeviceProtectedMemoryFeatures protected_features{};
                    protected_features.sType =
                        VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROTECTED_MEMORY_FEATURES;
                    VkPhysicalDeviceFeatures2 features{};
                    features.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2;
                    features.pNext = &protected_features;
                    get_features2(device, &features);
                    result.protected_memory = protected_features.protectedMemory == VK_TRUE
                        ? CapabilityState::Supported
                        : CapabilityState::Unsupported;
                }
            }
        }
    }

    if (destroy_instance != nullptr) {
        destroy_instance(instance, nullptr);
    }
}

}  // namespace

GpuProbeResult probe_gpu() noexcept {
    GpuProbeResult result;
    probe_hardware_buffers(result);
    probe_vulkan(result);
    return result;
}

}  // namespace carbonhv::graphics
