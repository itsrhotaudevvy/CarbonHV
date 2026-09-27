# CarbonHV bundle source manifest v1

The repository uses a deliberately small TOML-like manifest subset so the privileged verifier can stay dependency-light. Each non-comment line is `key = value`. Strings are quoted. `artifact` may repeat; every other key is unique.

Required keys: `id`, `display_name`, `version`, `publisher`, `release_status`, `guest_arch`, `cvd_major`, `memory_mib`, `storage_mib`, `requires_hardware_virtualization`, `requires_gpu_acceleration`, `backend`, and `integration_profile`.

`guest_arch` is `arm64` for v1. `backend` is `avf` or `kvm`. The hardware-virtualization requirement must be `true`. `artifact` uses `role|relative/path|sha256`, and each role is unique. Release tooling must replace development zero hashes with real content hashes before signing/publishing.
