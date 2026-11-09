# Gate 3 probe app (`dev.carbonhv.emuprobe`)

A throwaway qualification app for the emulated engine (plan Phase 3). It runs in the real app sandbox
(`untrusted_app`, `targetSdk 36`) and checks what that sandbox allows:
- exec from `nativeLibraryDir`;
- JIT memory modes and QEMU TCG with `split-wx` on and off;
- thread affinity and uclamp;
- gfxstream on the GPU inside QEMU;
- a Vulkan-made `AHardwareBuffer` handed from a child process to the app;
- memory facts, `O_DIRECT` and `io_uring`;
- an `isolatedProcess` comparison, QEMU `-sandbox`, and a thermal baseline.

It is **never shipped**. Install it on a person's phone only with their consent, and uninstall it afterwards.
It starts as a foreground service, so it never covers the screen:

```bash
emu/qemu/build-android.sh --build
ANDROID_NDK_HOME=… qualification/emu/probe-app/build-native.sh
./gradlew :qualification:emu:probe-app:assembleDebug
adb install qualification/emu/probe-app/build/outputs/apk/debug/probe-app-debug.apk
adb shell am start-foreground-service -n dev.carbonhv.emuprobe/.ProbeService
adb logcat -s CarbonEmuProbe:I             # REPORT-COMPLETE after ~30 s
adb shell run-as dev.carbonhv.emuprobe cat files/report.json
adb uninstall dev.carbonhv.emuprobe
```

`ProbeActivity` runs the same checks from the foreground app (top-app) when launched explicitly. Do that
only when the device owner agrees to having the screen taken over.

Deliberately not done on a personal phone: the 2.5 GiB allocate-and-touch (P5), which would evict the owner's apps, and
the 5-minute thermal spin (P9).
