package dev.carbonhv.host;

parcelable HostGraphicalVmStartRequest {
    long requestId;
    String instanceId;
    String bundleId;
    int memoryMib;
    int vcpus;
    int widthPixels;
    int heightPixels;
    int densityDpi;
    int refreshRate;
}
