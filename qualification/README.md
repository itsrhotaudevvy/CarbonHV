# CarbonHV qualification

Qualification separates reproducible source checks from physical-device evidence.

`carbon-qualify.sh --profile ... --dry-run` evaluates a recorded capability tuple and is used by CI/self-tests. A READY fixture proves the policy evaluator works; it does not certify a phone.

`carbon-qualify.sh --adb SERIAL` probes an attached Android host for ARM64, the AVF `vm` tool, and `/dev/kvm`. A static ADB probe intentionally stops at `EXPERIMENTAL` because accelerated guest graphics, buffer sharing, touch latency, thermals, recovery, and sustained performance require real guest workloads and measured evidence.

A supported release stores a qualification dossier identifying the exact device, host build/firmware, kernel/AVF revision, renderer, guest build, and measurements described in `docs/ARCHITECTURE.md` section 24 and section 26.

