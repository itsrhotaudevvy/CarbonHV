package dev.carbonhv.host;

import dev.carbonhv.host.GraphicalVmDisk;

parcelable GraphicalVmLaunchRequest {
    String instanceId;
    long generation;
    String kernelPath;
    String initrdPath;
    GraphicalVmDisk[] disks;
    long memoryMib;
    int vcpus;
    String[] kernelParams;
    int widthPixels;
    int heightPixels;
    int densityDpi;
    int refreshRate;
    /** AVF VirtualMachineRawConfig.hugePages: advise THP for guest RAM. */
    boolean hugePages;
    /** AVF VirtualMachineRawConfig.boostUclamp: keep vCPU threads on performance cores. */
    boolean boostUclamp;
    /** AVF VirtualMachineRawConfig.balloon: free-page reporting back to the host. */
    boolean balloon;
}
