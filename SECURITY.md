# Security policy

CarbonHV is a **pre-release development project**. No release is supported for production use,
and no physical host is qualified. See [`docs/SUPPORT_MATRIX.md`](docs/SUPPORT_MATRIX.md).

## Reporting a vulnerability

Please report suspected vulnerabilities **privately** through GitHub's
[private vulnerability reporting](https://github.com/itsrhotaudevvy/CarbonHV/security/advisories/new)
rather than in a public issue. Include:

- the affected component (`carbonhvd` broker, host integration, Android app, guest recipe,
  qualification tooling);
- the commit you tested and, for device findings, the host tuple from
  `qualification/carbon-qualify.sh --adb <serial>`, with serial numbers removed;
- reproduction steps and the impact you expect.

You should get an acknowledgement within 7 days. Fixes are developed on a private fork and
disclosed together with a fixed commit.

## Scope that matters most

The security boundaries CarbonHV relies on are described in [`docs/SECURITY.md`](docs/SECURITY.md).
The following are especially in scope:

- **The broker and host-runtime Binder boundaries:** caller identity, generation checks, and the
  managed-path-only launch model.
- **Anything that lets a guest, the UI APK or another app reach a host path, executable, kernel
  parameter, or AVF service that the broker did not choose.**
- **SELinux policy in `host/`** that grants more than its contract tests describe.
- **Any path that silently falls back to software CPU emulation**, or that labels unqualified
  hardware as qualified. The emulated engine is an explicit, stored, *Emulated*-labelled choice
  (ADR-017); anything that selects it without that choice, or hides its cost, is in scope.

Out of scope: weaknesses that need an already-compromised Android host kernel or hypervisor, and
the upstream projects CarbonHV drives (crosvm, AVF, GrapheneOS, Ubuntu Touch). Report those
upstream.
