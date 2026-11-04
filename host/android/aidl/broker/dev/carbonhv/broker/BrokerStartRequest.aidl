package dev.carbonhv.broker;

parcelable BrokerStartRequest {
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
