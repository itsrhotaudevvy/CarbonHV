# Device frame probe

Runs one QEMU instance with `-display android,frame-fd=3` on the attached phone and saves the
frames it publishes, unrooted, without installing an APK.

It exists because the app's frame reader (`android/emu`, Phase 9) does not exist yet, and because
"the emulator started" is not evidence that pixels crossed the socket. The probe decodes the same
`ui/android-proto.h` the backend ships — extracted from patch 0003 at build time, never copied —
so it cannot drift from the producer.

```
build.sh        cross-builds build/carbon-frame-probe with the pinned NDK
run-on-phone.sh pushes emulator + probe + guest, runs one boot, pulls frames, writes PNGs
rgba-to-png.py  turns the raw frames into PNGs (standard library only, no PIL)
probe.c         the probe itself
```

## Use

```bash
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358
emu/qemu/build-android.sh --build            # produces the emulator this pushes

qualification/emu/device-frame-probe/run-on-phone.sh \
    --kernel ~/carbonhv-cache/alpine/vmlinuz-virt \
    --initrd ~/carbonhv-cache/alpine/initramfs-virt \
    [--serial 8b227126] [--memory 1024] [--frames 6] [--seconds 240]
```

Arguments are paths, not downloads: the script never uses the network. The guest command line is
inside the script (`-M virt -accel tcg -cpu max -device virtio-gpu-pci …`) so a boot is
reproducible from the report; edit it there if a different guest needs different devices.

Output lands in `qualification/emu/reports/data/<timestamp>-frame-probe/`: `probe.log`, the
guest's `serial.log`, and `frame-<seq>-<w>x<h>.png`. Raw `.rgba` dumps are git-ignored.

## What it reports

- `hello version=1` — the backend's version, so a protocol bump is visible in the log.
- `mode <w>x<h> format=1` — the guest's scanout geometry, which is what the app resizes to.
- `frame seq=…` — one line per saved frame, with the byte count it locked.
- Refusals: a datagram that is not a known message, a descriptor whose geometry disagrees with
  the frame message, or a lock failure. These are printed rather than swallowed, because the two
  real bugs found on the phone so far (a closed fd 3 and `A,B,G,R` channel order) both showed up
  as wrong *content* rather than as errors.

## Why it forks instead of using posix_spawn

`posix_spawn` file actions apply in list order, and `addclose(fd)` after `adddup2(fd → 3)`
closes the descriptor it was supposed to keep. QEMU then opens its `-serial file:` onto fd 3 and
the first write fails with `ENOTSOCK`. The probe forks and installs fd 3 itself, clears
close-on-exec, and closes both socketpair ends in the child, so fd 3 cannot be anything else.
