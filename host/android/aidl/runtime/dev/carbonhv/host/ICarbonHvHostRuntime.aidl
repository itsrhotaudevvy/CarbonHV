package dev.carbonhv.host;

import dev.carbonhv.host.GraphicalVmLaunchRequest;

interface ICarbonHvHostRuntime {
    void startGraphicalVm(in GraphicalVmLaunchRequest request);
    void stopGraphicalVm(String instanceId, long generation);
}
