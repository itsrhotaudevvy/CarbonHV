#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace carbonhv::graphics {

enum class CapabilityState : std::uint8_t {
    Unknown = 0,
    Unsupported = 1,
    Supported = 2,
};

struct NativeBufferSupport {
    std::int32_t format;
    std::uint64_t usage;
};

struct GpuProbeResult {
    std::optional<std::string> gpu_name;
    std::optional<std::string> vulkan_api_version;
    std::optional<std::string> vulkan_driver_version;
    CapabilityState external_memory = CapabilityState::Unknown;
    CapabilityState explicit_sync = CapabilityState::Unknown;
    CapabilityState hardware_buffer = CapabilityState::Unknown;
    CapabilityState protected_memory = CapabilityState::Unknown;
    std::vector<NativeBufferSupport> native_buffer_formats;
    std::vector<float> display_refresh_rates_hz;
};

GpuProbeResult probe_gpu() noexcept;

}  // namespace carbonhv::graphics
