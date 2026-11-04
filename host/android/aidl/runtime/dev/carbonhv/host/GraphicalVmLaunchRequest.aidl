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
}
