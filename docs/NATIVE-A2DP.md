# Optional native A2DP driver and provider

Status: **experimental, Windows-only, not hardware-validated in this repository.**

Bluu includes an optional native A2DP source component under `native/owb`, derived
from the GPLv3 OpenWinBlue project. It is not installed by the portable Bluu app and
must never be enabled silently.

## What it contains

- KMDF source for an experimental A2DP source driver.
- A LocalSystem service that captures the Windows render endpoint, encodes audio,
  and submits media frames through the driver.
- Encoders for SBC, AAC (Windows Media Foundation), aptX/aptX HD (vendored FFmpeg
  LGPL codec core), and LDAC (vendored Apache-2.0 libldac).
- A Bluu provider adapter (`bluu-provider-owb`) that lets the normal provider UI
  select a codec and parameters through the local named pipe.

It does **not** implement aptX Adaptive, aptX Lossless, or LC3/LE Audio. Those
entries are rejected rather than mapped to a different codec.

## Hard limits

1. The driver is not production-signed. Microsoft attestation/WHQL signing is
   required for a distributable release.
2. A test-signed driver requires test-signing mode. Do not instruct users to disable
   Secure Boot; use a properly signed build for real releases.
3. Hardware capability is still required at both ends: the Bluetooth radio/driver,
   the remote sink's advertised AVDTP capabilities, and codec licensing.
4. The app's selected codec is not proof of active transmission. The provider reports
   driver state, which Bluu labels as provider-attested. Independent confirmation
   requires an A2DP protocol trace or equivalent instrumentation.
5. No real Windows Bluetooth hardware validation has been performed in this
   repository. The rows in `docs/TESTING.md` remain pending.
6. The service captures the current default render endpoint with WASAPI loopback.
   It has no endpoint picker yet. If the Bluetooth endpoint is the default render
   endpoint, this architecture can capture the wrong signal or create a routing
   loop. End-to-end audio must not be claimed until this is validated on hardware.

## Installation and recovery

Build and sign the driver on Windows with the WDK, review the generated package, then
install it explicitly with `tools/install-native-a2dp.ps1`. The script refuses a
missing/invalid signature unless `-AllowTestSigned` is explicitly supplied on a
controlled test machine; it records the prior `oemNN.inf` identity and service state
before changes. The legacy MSI project in `installer/` is quarantined and must not be
built. The emergency rollback script deletes only the matching driver package and
service; reboot to return to the inbox path.

The AAC/aptX Classic BthA2dp preference is separate from this driver. It is a
build-sensitive whole-PC value, saved and restorable through Bluu.

## Development build

Host-side AVDTP packet logic can be built and tested without Windows:

```bash
cmake -S native/owb -B build/native-host
cmake --build build/native-host --target owb_driver_tests
build/native-host/tests/driver/owb_driver_tests
```

The service and driver targets require Windows, MSVC, and the WDK. Never publish a
locally built driver as production-ready.

## Validation

On a Windows test machine, run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows/smoke-provider.ps1
powershell -ExecutionPolicy Bypass -File scripts/windows/hardware-validation.ps1
```

The first command validates provider discovery, manifest identity, hashes, and
expected failure handling without a headset. The second records machine/radio/
endpoint state, WPR trace metadata, provider behavior, diagnostic redaction, and
basic performance measurements. Neither command marks a codec verified; record the
independent result in `docs/TESTING.md`.
