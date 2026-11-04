package dev.carbonhv.host;

import android.view.Surface;
import dev.carbonhv.host.HostGraphicalVmStartRequest;

interface ICarbonHvHostService {
    long startGraphicalVm(in HostGraphicalVmStartRequest request);
    void stopVm(String instanceId, long generation, long requestId);
    long activeDisplayGeneration(String instanceId);
    void attachMainSurface(String instanceId, long generation, in Surface surface);
    void detachMainSurface(String instanceId, long generation);
    boolean sendGuestKey(String instanceId, long generation, int linuxKeyCode, boolean down);
    boolean sendGuestTouch(
        String instanceId,
        long generation,
        int action,
        int actionPointerId,
        int viewportWidth,
        int viewportHeight,
        in int[] pointerIds,
        in float[] xs,
        in float[] ys
    );
}
