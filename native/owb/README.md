# OpenWinBlue native A2DP component (Bluu fork)

This directory contains the optional experimental A2DP source driver and service
used by Bluu's `bluu.native-a2dp` provider. It is derived from
[santiquiroz/openwinblue](https://github.com/santiquiroz/openwinblue) at commit
`3de00befdabf05025842394567815bc64b5f677b` and is distributed under
GPL-3.0-or-later.

## Scope

- KMDF A2DP source driver with AVDTP signaling and L2CAP media transport.
- User-mode service for PCM capture, encoding, and driver IOCTLs.
- SBC, AAC, aptX, aptX HD, and LDAC encoder paths.
- Named-pipe codec/preference control consumed by `bluu-provider-owb`.

Not included: aptX Adaptive/Lossless, LC3/LE Audio transport, and AI/DirectML
processing. Unsupported codec IDs are rejected rather than silently substituted.

## Status

This is **experimental engineering code**, not a signed production driver. It has
not been hardware-validated in this repository. A test-signed build requires
test-signing mode; production distribution requires Microsoft attestation/WHQL
signing. Do not ask users to disable Secure Boot.

## Host tests

```bash
cmake -S . -B build/host
cmake --build build/host --target owb_driver_tests
build/host/tests/driver/owb_driver_tests
```

The host CMake build intentionally compiles only WDK-free code. The service, driver,
and Windows-only tests require Visual Studio/MSVC and the WDK.

## Licence notes

See [third-party/LICENSES.md](third-party/LICENSES.md). `libopenaptx` is not included
because its README adds non-OSI restrictions. aptX encoding uses a vendored
FFmpeg-derived LGPL codec core. Preserve all upstream notices.
