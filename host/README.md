# CarbonHV host integration

`carbonhvd` is the narrow privileged broker used by an integrated Android host build. The standalone Rust crate in this repository validates caller identity, VM generation, resource limits, managed image IDs, and launch parameters before any backend-specific launch occurs.

The first development backend targets Android's AVF `vm` tool at `/apex/com.android.virt/bin/vm` and `/dev/kvm`. The exact Android platform service registration, Binder transport, SELinux labels, and AVF API surface must be pinned to the target host branch during device integration; they are deliberately kept outside the ordinary APK.

`carbonhvd.rc` and `carbonhvd.te` are integration templates for an AOSP/system_ext build. They must be compiled and reviewed against the selected host source tree. They are not intended to be installed onto an arbitrary stock device.

The broker never accepts a shell command. Runtime requests reference broker-managed object IDs. The backend produces a program path, argv vector, and generated AVF config file. The integrated service owns writing that config into its private runtime directory and spawning the fixed program directly.

