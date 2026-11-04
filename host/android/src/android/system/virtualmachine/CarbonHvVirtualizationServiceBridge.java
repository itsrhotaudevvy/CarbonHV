package android.system.virtualmachine;

import android.content.Context;
import android.os.ParcelFileDescriptor;
import android.system.virtualizationservice.CpuOptions;
import android.system.virtualizationservice.InputDevice;
import android.system.virtualizationservice.IVirtualizationService;
import android.system.virtualizationservice.VirtualMachineRawConfig;
import java.io.IOException;

/**
 * Narrow platform bridge that keeps AVF's own VirtualizationService instance strongly reachable.
 * CarbonHV links against framework-virtualization.impl only in the privileged host package.
 *
 * Raw custom-image configuration is deliberately produced by the device framework first. This
 * keeps CarbonHV aligned with the exact framework-virtualization parcelable revision shipped on
 * the host instead of reconstructing every VirtualMachineRawConfig field from a source snapshot.
 */
public final class CarbonHvVirtualizationServiceBridge {
    private CarbonHvVirtualizationServiceBridge() {}

    public static Connection connect() throws VirtualMachineException {
        synchronized (VirtualMachineManager.sCreateLock) {
            return new Connection(VirtualizationService.getInstance());
        }
    }

    public static VirtualMachineRawConfig buildRawConfig(
            Context context,
            String name,
            String kernelPath,
            String initrdPath,
            String[] diskPaths,
            boolean[] diskWritables,
            String[] params,
            long memoryMib,
            int vcpuCount,
            int widthPixels,
            int heightPixels,
            int densityDpi,
            int refreshRate,
            String gpuBackend,
            String[] gpuContextTypes,
            String gpuRendererFeatures,
            ParcelFileDescriptor touchGuestSocket,
            ParcelFileDescriptor keyGuestSocket,
            ParcelFileDescriptor mouseGuestSocket)
            throws IOException {
        if (diskPaths.length != diskWritables.length) {
            throw new IllegalArgumentException("disk path/writable arrays must have equal length");
        }

        VirtualMachineCustomImageConfig.DisplayConfig display =
                new VirtualMachineCustomImageConfig.DisplayConfig.Builder()
                        .setWidth(widthPixels)
                        .setHeight(heightPixels)
                        .setHorizontalDpi(densityDpi)
                        .setVerticalDpi(densityDpi)
                        .setRefreshRate(refreshRate)
                        .build();

        VirtualMachineCustomImageConfig.GpuConfig gpu =
                new VirtualMachineCustomImageConfig.GpuConfig.Builder()
                        .setBackend(gpuBackend)
                        .setContextTypes(gpuContextTypes)
                        .setRendererFeatures(gpuRendererFeatures)
                        .setRendererUseEgl(false)
                        .setRendererUseGles(false)
                        .setRendererUseGlx(false)
                        .setRendererUseSurfaceless(true)
                        .setRendererUseVulkan(true)
                        .build();

        VirtualMachineCustomImageConfig.Builder customImage =
                new VirtualMachineCustomImageConfig.Builder()
                        .setName(name)
                        .setOsName("")
                        .setKernelPath(kernelPath)
                        .setInitrdPath(initrdPath)
                        .setDisplayConfig(display)
                        .setGpuConfig(gpu)
                        .useTouch(true)
                        .useKeyboard(true)
                        .useMouse(true)
                        .useNetwork(false);

        for (String param : params) {
            customImage.addParam(param);
        }
        for (int i = 0; i < diskPaths.length; i++) {
            VirtualMachineCustomImageConfig.Disk disk =
                    diskWritables[i]
                            ? VirtualMachineCustomImageConfig.Disk.RWDisk(diskPaths[i])
                            : VirtualMachineCustomImageConfig.Disk.RODisk(diskPaths[i]);
            customImage.addDisk(disk);
        }

        VirtualMachineConfig frameworkConfig =
                new VirtualMachineConfig.Builder(context)
                        .setProtectedVm(false)
                        .setMemoryBytes(Math.multiplyExact(memoryMib, 1024L * 1024L))
                        .setCpuTopology(0)
                        .setCustomImageConfig(customImage.build())
                        .build();

        VirtualMachineRawConfig rawConfig = frameworkConfig.toVsRawConfig();

        CpuOptions cpu = new CpuOptions();
        cpu.cpuTopology = CpuOptions.CpuTopology.cpuCount(vcpuCount);
        rawConfig.cpuOptions = cpu;

        InputDevice.MultiTouch touch = new InputDevice.MultiTouch();
        touch.width = widthPixels;
        touch.height = heightPixels;
        touch.pfd = touchGuestSocket;

        InputDevice.Keyboard keyboard = new InputDevice.Keyboard();
        keyboard.pfd = keyGuestSocket;

        InputDevice.Mouse mouse = new InputDevice.Mouse();
        mouse.pfd = mouseGuestSocket;

        rawConfig.inputDevices =
                new InputDevice[] {
                    InputDevice.multiTouch(touch),
                    InputDevice.keyboard(keyboard),
                    InputDevice.mouse(mouse),
                };
        rawConfig.networkSupported = false;
        return rawConfig;
    }

    public static final class Connection {
        private final VirtualizationService service;

        private Connection(VirtualizationService service) {
            this.service = service;
        }

        public IVirtualizationService getBinder() {
            return service.getBinder();
        }
    }
}
