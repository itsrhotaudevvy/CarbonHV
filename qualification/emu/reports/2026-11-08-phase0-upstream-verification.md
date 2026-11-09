# Phase 0 — upstream verification for the emulated engine

- **Date:** 2026-11-08
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`
- **Author:** implementation pass (agent), reviewed by the project owner
- **Gate:** **G0 NOT PASSED** — V1, V5, V7 and V13 are unresolved; the plan says any ❌ on V1, V3, V7 or V13 is plan-changing and must stop for the user.

This report records only what was actually executed. Claims that need an AOSP checkout, a
termux-packages checkout, a gfxstream build or a phone in hand are marked **unverified** rather
than guessed at. No item below is a capability claim for the product.

## Host used for the QEMU probes

| Property | Value |
| --- | --- |
| Host | macOS, Apple Silicon |
| QEMU | `qemu-system-aarch64` **11.1.3** (Homebrew build, no rutabaga) |
| Sources available locally | none (no QEMU, AOSP, termux-packages or gfxstream checkout) |
| Network during this pass | web search unavailable; no downloads attempted |

Because the pins in `emu/qemu/VERSIONS.toml` are against upstream **11.1.3** and not the plan's
"latest stable 10.x", V1–V4 below are evidence *at 11.1.3*. They are not evidence for 10.x.

## V1–V14

| ID | Claim | Evidence run | Status |
| --- | --- | --- | --- |
| V1 | `-device virtio-gpu-rutabaga` exists with gfxstream properties | `qemu-system-aarch64 -device help \| grep -i rutabaga` → no output. The Homebrew build is not configured with `--enable-rutabaga-gfx`, and no source checkout was available to read `hw/display/virtio-gpu-rutabaga.c` or `meson_options.txt` | **❌ unverified (blocking)** |
| V2 | `virtio-multitouch-*` (8.2+) and `virtio-sound-*` (8.2+) exist | `-device help` lists `virtio-multitouch-device`, `virtio-multitouch-pci`, `virtio-sound-device`, `virtio-sound-pci` (alias `virtio-sound`) | ✅ at 11.1.3 |
| V3 | `-accel tcg,thread=multi,tb-size=N,split-wx=on\|off` exist | `qemu-system-aarch64 -accel tcg,thread=multi,tb-size=256,split-wx=on -machine virt,gic-version=3 -cpu max,pauth-impdef=on,sve=off,sme=off -smp 2 -m 128 -display none -monitor none -serial none -S` ran until the 5 s timeout with no error (exit 124). The *semantic* claim (memfd dual mapping, no page ever W+X) was **not** read from source | ✅ flags / ⚠️ implementation unverified |
| V4 | `-cpu max` accepts `pauth-impdef=on`, `pauth=off`, `sve=off`, `sme=off`; `-machine virt,mte=on` exists | Both `-cpu max,pauth-impdef=on,sve=off,sme=off` and `-cpu max,pauth=off` started cleanly; `-machine virt,gic-version=3,highmem=on,mte=on` started cleanly | ✅ at 11.1.3 |
| V5 | Cuttlefish `--vm_manager=qemu_cli` builds an arm64 QEMU command line; a QEMU U-Boot exists | No AOSP checkout, no network | **❌ unverified (blocking for the preferred boot flow; the direct-kernel fallback is the interim path)** |
| V6 | Termux patches are a usable reference for Android | No termux-packages checkout | ⚠️ unverified (reference only, not a gate on V1/V3/V7/V13) |
| V7 | Apps with `targetSdk ≥ 29` cannot exec from app data but can exec from `nativeLibraryDir` with legacy packaging | Needs the probe APK on a device (Gate 3, P1) | **❌ unverified (blocking)** |
| V8 | memfd double mapping / anonymous RWX availability for `targetSdk 36` | Needs Gate 3 P2 | ⚠️ unverified (mitigated: the argv requires a probed `w^x` mode and refuses `Unknown`) |
| V9 | `io_uring` is blocked for apps | Needs Gate 3 P6 | ⚠️ unverified (mitigated: the argv never uses `io_uring`; `aio=threads`) |
| V10 | Apps may pin their own threads to CPUs 4–7 in the `top-app` cpuset and use `uclamp.min` | Needs Gate 3 P3 | ⚠️ unverified (mitigated: placement is reported, and `AffinityNotPermitted`/`UclampNotPermitted` warnings are emitted rather than assuming success) |
| V11 | `isolatedProcess` services cannot open the GPU device or sockets | Needs Gate 3 P7 | ⚠️ unverified (only affects Phase 14 hardening) |
| V12 | A public AOSP CI arm64 cuttlefish build is downloadable without login | No network | ⚠️ unverified (blocks G1) |
| V13 | gfxstream host builds for an Android host (bionic) against the Android Vulkan loader | No gfxstream checkout; `crates/carbon-renderer` has a `native-gfxstream` feature but no Android build evidence | **❌ unverified (blocking)** |
| V14 | Android 12+ GKI reads `androidboot.*` from an initrd bootconfig trailer | Implemented and unit-tested in `crates/carbon-emu/src/bootimg.rs` against the documented invariant (all 32-bit words plus `size` plus `checksum` sum to zero). Not yet exercised against a real GKI kernel | ⚠️ implementation tested, kernel unverified |

Reproduce the QEMU probes exactly:

```bash
qemu-system-aarch64 --version
qemu-system-aarch64 -device help | grep -iE 'rutabaga|multitouch|virtio-sound|vhost-user-vsock'
qemu-system-aarch64 -machine help | grep '^virt '
timeout 5 qemu-system-aarch64 -accel tcg,thread=multi,tb-size=256,split-wx=on \
  -machine virt,gic-version=3,highmem=on,mte=on -cpu max,pauth-impdef=on,sve=off,sme=off \
  -smp 2 -m 128 -display none -monitor none -serial none -S
```

## Pins recorded

`emu/qemu/VERSIONS.toml` pins QEMU **11.1.3** (upstream latest stable at pin time), `rutabaga_gfx`
**0.1.80** (equal to `Cargo.lock`; `build-android.sh --check` enforces the equality), gfxstream
`ebe5b42…` (equal to `Cargo.toml`'s renderer upstream revision), glib 2.82.4, pixman 0.44.2,
libslirp 4.9.1, dtc/libfdt 1.7.2, zlib 1.3.1, libffi 3.4.6, pcre2 10.44, NDK
28.0.13004108 / `aarch64-linux-android30`.

Source hashes are intentionally empty: they can only be recorded from downloaded archives, and
`--check --require-pins` refuses a build until they are. This keeps the repository honest instead
of inventing hashes.

## Findings that change plan assumptions

1. **The memory budget in §5.5 does not admit its own default on the measured phone.** The plan
   requires `MemAvailable ≥ guest + 768 MiB` and a 2048 MiB default guest; the 2026-11-04 probe
   measured **2750 MiB**, i.e. 66 MiB short. The implementation keeps the plan's rule and makes
   the refusal explicit (`carbonctl emu plan` exits 65 with the numbers), and adds a `low-memory`
   profile whose 1536 MiB guest does fit (2304 MiB required). Phase 11 must decide, from measured
   G1/G11 data, between a smaller default guest and a lower reserve. **Do not** silently clamp.
2. **The plan's "latest stable 10.x" pin is stale** relative to this host's QEMU 11.1.3. The
   rutabaga device predates both, so 11.x is not a higher risk, but V1 must be confirmed on the
   built binary before Gate 2 can pass.
3. **`virtio-gpu-rutabaga` is not in a stock Homebrew QEMU.** Any readback-only bring-up must not
   be mistaken for the hardware path; the argv refuses a host without Vulkan and refuses a pin
   whose `w^x` mode is unprobed.
4. `-cpu max,help` and `-accel tcg,help` are rejected as malformed at 11.1.3, so feature
   acceptance has to be tested by starting a VM (as above) rather than by asking QEMU to print
   properties.

## What this pass did build (and what it does not claim)

Because Phase 8 is pure planning with no runtime claim, it was implemented in parallel and is
reviewable today:

- `crates/carbon-emu` with golden-tested argv, placement, memory admission, CPU model, boot-image
  and bootconfig assembly, QMP client and frame protocol;
- `ExecutionEngine`/`EmulationReadiness` in `crates/carbon-core`, and engine selection in
  `crates/carbon-broker` that can never turn a hardware-VM instance into an emulated one;
- `carbonctl emu plan` as the review surface, plus `scripts/verify.sh` coverage;
- `emu/qemu/` pins and the offline `--check` gate.

Nothing above claims that an emulated guest boots, that graphics work, or that any performance
target is met. The engine stays `EXPERIMENTAL` and `cpu.native=false`.

## G0 decision

**Not passed.** The plan says a ❌ on V1, V3, V7 or V13 is plan-changing and requires stopping to
ask the user. V1 and V13 need a pinned QEMU/gfxstream build environment (Linux host or cloud
builder); V7 needs the probe APK on a device with consent.

Questions for the project owner before continuing:

1. **Builder:** is there a Linux x86_64 (or arm64) machine with ~150 GB free and NDK r28+ for the
   QEMU cross-build (Gate 2) and, later, the 400 GB guest build (Phase 10)? A cloud VM is
   acceptable but costs money.
2. **Licensing:** confirm the GPL-2.0 QEMU approach — separate executable, source offer, `NOTICE`,
   published tarballs + patches with every release — before any binary ships.
3. **Product framing:** confirm that "Run emulated (slow)" as an explicit per-instance opt-in,
   always labelled *Emulated*, is the wording and behaviour you want.
4. **Device testing:** confirm when the probe APK may be installed on the `kunzite` (Gate 3) and
   whether all iteration until then stays on an arm64 AVD.
