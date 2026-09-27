# CarbonHV qualification

Qualification separates reproducible source checks from physical-device evidence.

`carbon-qualify.sh --profile ... --dry-run` evaluates a recorded capability tuple and is used by CI/self-tests. A READY fixture proves the policy evaluator works; it does not certify a phone.

`carbon-qualify.sh --adb SERIAL` probes an attached Android host for ARM64, the AVF `vm` tool, `/dev/kvm`, SoC/GPU identity, EGL/Vulkan providers, build/kernel identity, the observable AVF revision, Vulkan driver version, and display-mode data. Missing observations are emitted as `unknown`; their absence never becomes a positive capability.

A static ADB probe intentionally stops at `EXPERIMENTAL` even when Vulkan and a known GPU are present. Accelerated guest graphics, native-buffer sharing, explicit synchronization, touch latency, thermals, recovery, and sustained performance require real guest workloads and measured evidence before CarbonHV grants the corresponding `gpu.*` capability.

A supported release stores a qualification dossier identifying the exact device, host build/firmware, kernel/AVF revision, renderer, guest build, and measurements described in `docs/ARCHITECTURE.md` section 24 and section 26.
