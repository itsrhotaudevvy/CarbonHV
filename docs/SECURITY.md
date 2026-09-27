# CarbonHV security model

CarbonHV is currently a development implementation. Its architecture is intentionally strict about privilege and provenance, but several security controls still depend on production Android platform integration and physical guest qualification. This document distinguishes implemented source-level controls from release requirements.

## 1. Trust model

The initial graphical VM profile is **non-protected**. The host Android platform, kernel/hypervisor, VM monitor/runtime, host renderer, GPU kernel driver, and CarbonHV privileged broker are trusted components.

CarbonHV does not claim that pKVM availability alone makes an interactive session protected. A future protected profile requires independent qualification of firmware/boot measurement, memory-sharing rules, display/input transport, storage, updates, and every device exposed to the guest.

Primary trust boundaries are:

1. bundle/catalog input into manifest/artifact verification;
2. native Android UI caller into the privileged broker;
3. guest memory/device requests into the VM runtime;
4. guest graphics commands into the renderer/GPU driver;
5. guest-agent messages into host integration handlers;
6. host storage metadata into runtime file descriptors;
7. release infrastructure into installed update trust.

## 2. Broker boundary

The broker is designed as a narrow capability service rather than a generic root shell.

Current Rust policy code exercises these invariants:

- caller identity is matched against configured broker policy rather than trusting a caller-supplied descriptive name;
- VM requests use typed specifications, generation numbers, request IDs, and resource caps;
- stale/replayed generations are rejected;
- a second non-idempotent start cannot replace an already-running instance;
- image references are broker-managed object IDs rather than arbitrary host paths;
- managed IDs reject path traversal and shell metacharacters;
- kernel/launch arguments are validated before backend planning;
- the AVF backend produces a fixed executable path plus argv/config data and never constructs a shell command;
- unsupported/missing hardware requirements fail closed rather than enabling CPU emulation.

Still required for a production Android host integration:

- authenticated Binder/platform IPC registration tied to platform identity/signing policy;
- enforcing SELinux policy compiled against the selected host branch;
- least-privilege runtime UID/GID, file labels, and private runtime directories;
- race-resistant file opening and descriptor ownership in the real service process;
- syscall/process sandboxing where the selected AVF/runtime stack permits it;
- explicit process lifetime/reconnect behavior across host service death and updates.

`host/carbonhvd.rc` and `host/carbonhvd.te` are review templates, not proof that these controls are installed on a device.

## 3. Bundle and guest trust

Bundle parsing is strict about critical fields, ARM64/CVD requirements, artifact roles, resource bounds, and unsafe paths. CVD1 guest contracts require the declared virtual hardware families expected by the first guest ports.

Production release trust is not complete yet. Before public supported bundles, CarbonHV still needs:

- cryptographic catalog/bundle signing and key-rotation policy;
- pinned, reviewed guest source revisions and complete provenance;
- artifact hashes bound to signed metadata;
- separate development and production signing roots;
- emergency revocation and compromised-key procedure;
- source/license notices and reproducibility/SBOM records where practical.

The GrapheneOS-derived guest is downstream CarbonHV work. It must never inherit official GrapheneOS supported-device, hardware-security, or maintenance claims simply because its source is derived from GrapheneOS.

Ubuntu Touch is treated as a genuine Ubuntu Touch/Lomiri guest. A generic Ubuntu image or desktop shell does not satisfy its identity or security/confinement requirements.

## 4. Guest isolation and identity

Each VM is modeled as its own instance with private writable state and a distinct machine identity. Storage transactions enforce one writer per disk lease. Restore preserves the backed-up machine identity, while clone planning regenerates identity metadata so a copied VM does not silently duplicate the original identity.

Guest-to-host integration uses a broker-created per-VM vsock channel. The runtime generation and instance identity bind a session before application messages are accepted. Guest-reported names are descriptive and cannot authorize host actions.

Protocol v1 requires bounded messages and explicit capability negotiation. Unknown mandatory operations, stale generations, oversized frames, rate-limit violations, and operations for revoked grants are intended to fail closed.

## 5. Display and renderer risk

The graphics path is a security boundary: guest graphics commands ultimately exercise a host renderer and GPU kernel driver. Sandboxing a renderer reduces privilege but cannot remove GPU driver attack surface.

No current repository test proves that the physical guest GPU path is secure or accelerated. Before release, the selected renderer/driver tuple needs:

- validated buffer sizes, formats, strides, ownership, and fence lifetime;
- bounded queues/backpressure and context-loss handling;
- process/resource isolation from broker secrets, signing keys, and unrelated VM disks;
- renderer-facing fuzzing and malformed guest workload testing;
- host GPU/kernel patch tracking and security-update policy.

Until that work is complete, the physical graphics path remains unqualified.

## 6. Updates, rollback, and recovery

The Rust transaction model stages a candidate, requires a health mark before commit, preserves the previous version as the rollback target, and blocks rollback when a newer candidate has migrated data to an incompatible epoch.

Backups are modeled for stopped guests. Release qualification still needs real filesystem/image tests, interruption/fault injection on physical guest data, capacity exhaustion tests, recovery-image policy, and cryptographic signing of update metadata.

CarbonHV must never claim a rollback is safe across an incompatible data migration. A compatible repair path or backup is required instead.

## 7. Data and diagnostics

Diagnostics are local by default in the architecture. Normal logs must avoid:

- clipboard contents;
- keystrokes;
- guest screenshots or memory dumps;
- credentials/secrets;
- network payloads;
- file contents and filenames where avoidable.

Guest serial logs can contain sensitive information and should only be collected under explicit diagnostic mode or a tightly bounded failure policy. Any export flow should show what will be included and support identifier redaction.

No telemetry or remote diagnostic uploader is implemented in the current source tree.

## 8. Networking and sharing

The first security posture assumes explicit per-VM grants for any host/guest sharing. Clipboard, staged file transfer, notifications, audio metadata, sensors, camera, microphone, and location must each have bounded protocol families and revocation behavior.

The current source tree does not qualify these bridges end to end. Production networking also needs tests for DNS, IPv4/IPv6, offline mode, Android VPN/lockdown interaction, host network changes, and prevention of privileged-path VPN bypass.

## 9. Physical qualification required before support

Source tests cannot establish:

- KVM/AVF authorization for the final service identity;
- real guest confinement behavior;
- GPU acceleration and renderer containment;
- touch/input correctness under surface/geometry changes;
- secure audio/network/sharing behavior;
- thermal/stability behavior over sustained workloads;
- host OTA compatibility;
- crash/recovery behavior with real guest filesystems;
- GrapheneOS-derived or Ubuntu Touch update/security maintenance.

Every supported release must identify an exact device + host build/firmware + kernel/AVF + CarbonHV backend + renderer + guest-build tuple. Qualification does not transfer automatically to another model or host OTA.

## 10. Security maturity and reporting

The project is pre-release and does not yet publish a production vulnerability-response SLA or dedicated security contact in this repository. Do not describe development builds as hardened or production-safe.

Before stable release, the project needs a documented private vulnerability-reporting channel, triage/patch timelines, release-signing custody, dependency/CVE tracking, guest security baseline publication, and host/renderer update policy.
