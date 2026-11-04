# Contributing to CarbonHV

Thank you for helping. CarbonHV is a pre-release Android-native VM manager. It holds itself to
one rule above all others: **never claim more than the evidence shows.** Code, docs and
qualification reports all follow that rule.

## Ground rules

- **ARM64 hardware virtualization only.** No software CPU emulation fallback, ever.
- **The broker (`carbonhvd`) owns every VM generation, host path and launch parameter.** The UI
  APK and the host integration never invent paths, executables or kernel parameters.
- **No qualification promotion without physical evidence.** A capability becomes `true` only from
  a recorded run on an exact host tuple (`qualification/`). Fixtures test policy; they do not
  certify phones.
- **Vendor differences stay host-side** (ADR-015): a new SoC family adds probe facts and a renderer
  profile, never a guest-visible fork of CVD1.
- **Performance never trades away isolation** (ADR-014): no disabled mitigations, no protected VM
  shortcuts, no weakened SELinux or AppArmor.

## Development setup

You need Rust 1.90+ (edition 2024), JDK 17, Android SDK 36 (build-tools, CMake 3.22.1, NDK), and
Python 3.11+.

```bash
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"   # macOS
export GRADLE_USER_HOME="$PWD/.gradle-local"
scripts/verify.sh
```

`scripts/verify.sh` is the same gate CI runs. A change is ready when it passes, `git diff --check`
is clean, and `cargo fmt --all -- --check` passes.

## Making a change

1. **Write the test first.** Rust behaviour belongs in `crates/*/tests`. Shell and policy contracts
   belong in the `test-*.sh` scripts next to what they pin.
2. **Keep commits focused.** Use conventional prefixes (`feat:`, `fix:`, `test:`, `docs:`).
3. **Update `docs/SUPPORT_MATRIX.md`** whenever a status changes. Say *what* is proven and *what is
   still gated*.
4. **Architectural decisions get an ADR row** in `docs/ARCHITECTURE.md` §31.

## Contributing device evidence

A real host probe is one of the most useful contributions. Connect an ARM64 Android phone with
ADB enabled and run:

```bash
qualification/carbon-qualify.sh --adb <serial>
```

The probe is read-only. Open a **Host probe report** issue with the output. Before posting,
remove the `source=adb:<serial>` line and anything else you consider private. Please never
modify `/system_ext` on a stock user build to try CarbonHV. The host integration needs a
controlled product build (`host/product/README.md`).

## Licensing

By contributing, you agree that your contribution is licensed under the Apache License 2.0
(`LICENSE`). Do not add code or vendor blobs you cannot license that way.
