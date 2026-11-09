// Gate 3 probe P4: a separately exec'd child (like QEMU will be) renders into a Vulkan image
// that is exported as an AHardwareBuffer, and sends it to the app over an abstract unix socket
// with the public NDK API. The app wraps it as a HardwareBuffer and samples it.
//
// usage: libemuprobe_child.so <abstract-socket-name>
//        libemuprobe_child.so --io-uring   (P6: calls io_uring_setup; the app seccomp filter may kill it)
// Prints `step: …` lines on stdout; exit status 0 only when the buffer was sent and acknowledged.
#define VK_USE_PLATFORM_ANDROID_KHR 1
#include <android/hardware_buffer.h>
#include <errno.h>
#include <poll.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <unistd.h>
#include <vulkan/vulkan.h>

#define CHECK(call)                                                         \
    do {                                                                    \
        VkResult _r = (call);                                               \
        if (_r != VK_SUCCESS) {                                             \
            printf("fail: %s -> VkResult %d\n", #call, (int)_r);            \
            return 2;                                                       \
        }                                                                   \
    } while (0)

static int has_extension(VkPhysicalDevice gpu, const char *name) {
    uint32_t count = 0;
    vkEnumerateDeviceExtensionProperties(gpu, NULL, &count, NULL);
    VkExtensionProperties props[512];
    if (count > 512) count = 512;
    vkEnumerateDeviceExtensionProperties(gpu, NULL, &count, props);
    for (uint32_t i = 0; i < count; i++)
        if (strcmp(props[i].extensionName, name) == 0) return 1;
    return 0;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc != 2) {
        printf("fail: usage\n");
        return 64;
    }
    if (strcmp(argv[1], "--io-uring") == 0) {
        unsigned char params[120] = {0};
        long fd = syscall(425 /* __NR_io_uring_setup */, 1, params);
        printf("io_uring_setup returned %ld errno=%d (%s)\n", fd, fd < 0 ? errno : 0, fd < 0 ? strerror(errno) : "ok");
        return fd >= 0 ? 0 : 1;
    }

    VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
                             .pApplicationName = "carbonhv-emuprobe-child",
                             .apiVersion = VK_API_VERSION_1_1};
    VkInstanceCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app};
    VkInstance instance;
    CHECK(vkCreateInstance(&ici, NULL, &instance));

    uint32_t gpu_count = 1;
    VkPhysicalDevice gpu;
    VkResult enum_rc = vkEnumeratePhysicalDevices(instance, &gpu_count, &gpu);
    if ((enum_rc != VK_SUCCESS && enum_rc != VK_INCOMPLETE) || gpu_count == 0) {
        printf("fail: no physical device\n");
        return 2;
    }
    VkPhysicalDeviceProperties props;
    vkGetPhysicalDeviceProperties(gpu, &props);
    printf("step: device=%s api=%u.%u.%u\n", props.deviceName, VK_API_VERSION_MAJOR(props.apiVersion),
           VK_API_VERSION_MINOR(props.apiVersion), VK_API_VERSION_PATCH(props.apiVersion));

    const char *exts[] = {VK_ANDROID_EXTERNAL_MEMORY_ANDROID_HARDWARE_BUFFER_EXTENSION_NAME,
                          VK_EXT_QUEUE_FAMILY_FOREIGN_EXTENSION_NAME};
    for (int i = 0; i < 2; i++) {
        if (!has_extension(gpu, exts[i])) {
            printf("fail: missing %s\n", exts[i]);
            return 2;
        }
    }
    printf("step: AHB external memory extension present\n");

    uint32_t qf_count = 0;
    vkGetPhysicalDeviceQueueFamilyProperties(gpu, &qf_count, NULL);
    VkQueueFamilyProperties qfs[16];
    if (qf_count > 16) qf_count = 16;
    vkGetPhysicalDeviceQueueFamilyProperties(gpu, &qf_count, qfs);
    uint32_t qf = UINT32_MAX;
    for (uint32_t i = 0; i < qf_count; i++)
        if (qfs[i].queueFlags & VK_QUEUE_GRAPHICS_BIT) {
            qf = i;
            break;
        }
    if (qf == UINT32_MAX) {
        printf("fail: no graphics queue\n");
        return 2;
    }

    float priority = 1.0f;
    VkDeviceQueueCreateInfo qci = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                                   .queueFamilyIndex = qf,
                                   .queueCount = 1,
                                   .pQueuePriorities = &priority};
    VkDeviceCreateInfo dci = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                              .queueCreateInfoCount = 1,
                              .pQueueCreateInfos = &qci,
                              .enabledExtensionCount = 2,
                              .ppEnabledExtensionNames = exts};
    VkDevice device;
    CHECK(vkCreateDevice(gpu, &dci, NULL, &device));
    VkQueue queue;
    vkGetDeviceQueue(device, qf, 0, &queue);

    VkExternalMemoryImageCreateInfo ext_image = {
        .sType = VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO,
        .handleTypes = VK_EXTERNAL_MEMORY_HANDLE_TYPE_ANDROID_HARDWARE_BUFFER_BIT_ANDROID};
    VkImageCreateInfo ici2 = {.sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                              .pNext = &ext_image,
                              .imageType = VK_IMAGE_TYPE_2D,
                              .format = VK_FORMAT_R8G8B8A8_UNORM,
                              .extent = {256, 256, 1},
                              .mipLevels = 1,
                              .arrayLayers = 1,
                              .samples = VK_SAMPLE_COUNT_1_BIT,
                              .tiling = VK_IMAGE_TILING_OPTIMAL,
                              .usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT |
                                       VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
                              .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
                              .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED};
    VkImage image;
    CHECK(vkCreateImage(device, &ici2, NULL, &image));

    VkMemoryRequirements req;
    vkGetImageMemoryRequirements(device, image, &req);
    VkPhysicalDeviceMemoryProperties mem;
    vkGetPhysicalDeviceMemoryProperties(gpu, &mem);
    uint32_t type = UINT32_MAX;
    for (uint32_t i = 0; i < mem.memoryTypeCount; i++)
        if ((req.memoryTypeBits & (1u << i)) &&
            (mem.memoryTypes[i].propertyFlags & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT)) {
            type = i;
            break;
        }
    if (type == UINT32_MAX) {
        printf("fail: no device-local memory type\n");
        return 2;
    }
    VkMemoryDedicatedAllocateInfo dedicated = {.sType = VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
                                               .image = image};
    VkExportMemoryAllocateInfo export_info = {
        .sType = VK_STRUCTURE_TYPE_EXPORT_MEMORY_ALLOCATE_INFO,
        .pNext = &dedicated,
        .handleTypes = VK_EXTERNAL_MEMORY_HANDLE_TYPE_ANDROID_HARDWARE_BUFFER_BIT_ANDROID};
    VkMemoryAllocateInfo mai = {.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                                .pNext = &export_info,
                                .allocationSize = req.size,
                                .memoryTypeIndex = type};
    VkDeviceMemory memory;
    CHECK(vkAllocateMemory(device, &mai, NULL, &memory));
    CHECK(vkBindImageMemory(device, image, memory, 0));

    VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = qf};
    VkCommandPool pool;
    CHECK(vkCreateCommandPool(device, &cpci, NULL, &pool));
    VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                                        .commandPool = pool,
                                        .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY,
                                        .commandBufferCount = 1};
    VkCommandBuffer cmd;
    CHECK(vkAllocateCommandBuffers(device, &cbai, &cmd));
    VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
                                      .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
    CHECK(vkBeginCommandBuffer(cmd, &begin));
    VkImageSubresourceRange range = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    VkImageMemoryBarrier to_dst = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
                                   .dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
                                   .oldLayout = VK_IMAGE_LAYOUT_UNDEFINED,
                                   .newLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                                   .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
                                   .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
                                   .image = image,
                                   .subresourceRange = range};
    vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT, 0, 0, NULL, 0,
                         NULL, 1, &to_dst);
    // Distinctive colour the app checks for: R=0x33 G=0x66 B=0xCC A=0xFF.
    VkClearColorValue colour = {.float32 = {0x33 / 255.0f, 0x66 / 255.0f, 0xCC / 255.0f, 1.0f}};
    vkCmdClearColorImage(cmd, image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &colour, 1, &range);
    // Release ownership to the foreign (consumer) queue, in a layout samplers accept.
    VkImageMemoryBarrier release = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
                                    .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
                                    .oldLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                                    .newLayout = VK_IMAGE_LAYOUT_GENERAL,
                                    .srcQueueFamilyIndex = qf,
                                    .dstQueueFamilyIndex = VK_QUEUE_FAMILY_FOREIGN_EXT,
                                    .image = image,
                                    .subresourceRange = range};
    vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, 0, 0, NULL,
                         0, NULL, 1, &release);
    CHECK(vkEndCommandBuffer(cmd));
    VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VkFence fence;
    CHECK(vkCreateFence(device, &fci, NULL, &fence));
    VkSubmitInfo submit = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &cmd};
    CHECK(vkQueueSubmit(queue, 1, &submit, fence));
    CHECK(vkWaitForFences(device, 1, &fence, VK_TRUE, 5000000000ull));
    printf("step: GPU cleared the exportable image\n");

    PFN_vkGetMemoryAndroidHardwareBufferANDROID get_ahb =
        (PFN_vkGetMemoryAndroidHardwareBufferANDROID)vkGetDeviceProcAddr(device,
                                                                         "vkGetMemoryAndroidHardwareBufferANDROID");
    VkMemoryGetAndroidHardwareBufferInfoANDROID gi = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_GET_ANDROID_HARDWARE_BUFFER_INFO_ANDROID, .memory = memory};
    AHardwareBuffer *ahb = NULL;
    CHECK(get_ahb(device, &gi, &ahb));
    AHardwareBuffer_Desc desc;
    AHardwareBuffer_describe(ahb, &desc);
    printf("step: exported AHB %ux%u format=%u usage=0x%llx\n", desc.width, desc.height, desc.format,
           (unsigned long long)desc.usage);

    int sock = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    size_t len = strlen(argv[1]);
    if (len > sizeof(addr.sun_path) - 2) len = sizeof(addr.sun_path) - 2;
    memcpy(addr.sun_path + 1, argv[1], len);
    if (sock < 0 ||
        connect(sock, (struct sockaddr *)&addr, (socklen_t)(offsetof(struct sockaddr_un, sun_path) + 1 + len)) != 0) {
        printf("fail: connect errno=%d (%s)\n", errno, strerror(errno));
        return 3;
    }
    int send_rc = AHardwareBuffer_sendHandleToUnixSocket(ahb, sock);
    if (send_rc != 0) {
        printf("fail: AHardwareBuffer_sendHandleToUnixSocket=%d\n", send_rc);
        return 3;
    }
    struct pollfd pfd = {.fd = sock, .events = POLLIN};
    char ack = 0;
    if (poll(&pfd, 1, 5000) == 1) read(sock, &ack, 1);
    printf("step: sent AHB, app ack=%c\n", ack ? ack : '-');
    close(sock);

    AHardwareBuffer_release(ahb);
    vkDestroyFence(device, fence, NULL);
    vkDestroyCommandPool(device, pool, NULL);
    vkDestroyImage(device, image, NULL);
    vkFreeMemory(device, memory, NULL);
    vkDestroyDevice(device, NULL);
    vkDestroyInstance(instance, NULL);
    return ack == 'k' ? 0 : 4;
}
