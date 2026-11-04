package dev.carbonhv.broker;

import dev.carbonhv.broker.BrokerStartRequest;

interface ICarbonHvBroker {
    long startGraphicalVm(in BrokerStartRequest request);
    void stopVm(String instanceId, long generation, long requestId);
    long activeDisplayGeneration(String instanceId);
    void notifyRuntimeDied(String instanceId, long generation);
}
