# Emulated engine (QEMU TCG): qualification index

Everything here is about the **Emulated** engine described in
[ADR-017](../ARCHITECTURE.md). It is not a hypervisor. The same CVD1 guest ABI runs on both
engines; only the CPU/graphics transport differs, and the engine is always labelled *Emulated*.

## Status

`EXPERIMENTAL` — **no emulated guest has booted on a phone through CarbonHV.** What exists today:

| Layer | Evidence |
| --- | --- |
| Launch planning (`crates/carbon-emu`) | Unit and golden tests in CI: exact QEMU argv, TCG placement, memory admission, CPU model, boot-image + bootconfig assembly, QMP client, frame/input wire format |
| Broker control | `crates/carbon-broker` engine selection and the emulated adapter; `cargo test` covers "no stored engine never becomes emulated" |
| CLI review surface | `carbonctl emu plan --facts qualification/emu/fixtures/kunzite.json --profile low-memory` |
| QEMU build recipe | `emu/qemu/build-android.sh --build`: pinned, hash-checked sources and documented patches; the binary is git-ignored |
| Android binary on the phone | Gate 2: QEMU 11.1.3 + rutabaga/gfxstream runs on `kunzite` as the adb shell user, and gfxstream initializes Vulkan on the Adreno 710 |
| App-sandbox probe | Gate 3: the same binary runs as `untrusted_app` from a foreground service, with gfxstream on the Adreno and a cross-process AHB handoff |
| Guest boot / performance | **Not done.** Phases 1 and 4–13 remain |

## Gates

| Gate | State | Blocking evidence |
| --- | --- | --- |
| G0 upstream verification (V1–V14) | **Plan-changing claims passed** | V1, V3, V7 and V13 are ✅ (V7 in the app sandbox, Gate 3). V12 (the stand-in guest needs a manual download) blocks only G1; V14 needs a GKI boot. See the Phase 0 report |
| G1 desktop TCG boot | Not started | Needs the AOSP cuttlefish arm64 stand-in guest and a Linux/macOS gfxstream build |
| G2 Android QEMU binary | **Passed 2026-11-09** | Reproducible NDK build with rutabaga/gfxstream. On `kunzite` (shell user) TCG runs and gfxstream initializes Vulkan on the Adreno 710. See `reports/2026-11-09-gate2-android-qemu-build.md` |
| G3 on-device probes | **Passed except P5** (2026-11-09) | In `untrusted_app` on kunzite: exec and JIT ok, gfxstream on the Adreno ok, Vulkan→AHB→app handoff ok, O_DIRECT ok, uclamp EPERM, io_uring → SIGSYS. P5 (2 GiB allocate-and-touch) awaits the owner's yes. See `reports/2026-11-09-gate3-app-sandbox-probe.md` |
| G4–G14 | Not started | — |

## How to reproduce the planning evidence

```bash
# Unit tests + golden argv
cargo test -p carbon-emu

# The exact argv for the measured phone facts (refused: 2750 < 2048 + 768)
cargo run -q -p carbon-cli -- emu plan --facts qualification/emu/fixtures/kunzite.json
# ... and the profile that admits that reading
cargo run -q -p carbon-cli -- emu plan --facts qualification/emu/fixtures/kunzite.json --profile low-memory

# Pins, without network or NDK
emu/qemu/build-android.sh --check
```

`carbonctl emu plan` is a dry run: it uses placeholder descriptor numbers, starts nothing and reads
nothing from a phone. The argv it prints is the argv the `:vm` service passes to QEMU.

## Fixtures

`fixtures/kunzite.json` holds the app-side facts the planner needs. Only `mem_available_mib`
(2750) and the performance cores (`[4,5,6,7]`, from the 2026-11-04 probe) are measured. `w_xor_x`,
`o_direct`, the Vulkan `AHardwareBuffer` export capability and the affinity/uclamp permissions are
**planning assumptions** until Gate 3 probes them; the fixture's `source` string says so and must
be updated with the probe report when that changes.

## Report schema

A gate report is a Markdown file under `reports/` named `<date>-<gate>-<subject>.md` and states:

1. the exact command(s) run and on what host (device model, build fingerprint, kernel, commit);
2. the raw numbers (timings, p50/p95/p99, memory, binary size, sha256);
3. the pass/fail decision against the plan's gate, with any deviation from the plan called out;
4. what the result does **not** prove.

Numbers in reports are measured. Values that are still estimates are labelled as such and never
promoted into the support matrix or the README.
