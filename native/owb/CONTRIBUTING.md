# Contributing to OpenWinBlue

OpenWinBlue is experimental: the audio path is wired and host-tested, but it has not yet been
validated end-to-end with real Bluetooth devices. Hardware test reports are as welcome as code.

## Before opening a pull request

1. Build and run the tests that cover your change (see [README.md](README.md)):
   ```powershell
   cmake --preset windows-debug
   cmake --build build/debug
   ctest --preset test-all
   ```
   Host-only AVDTP/packet tests also run on macOS/Linux:
   ```bash
   cmake -S . -B build/host
   cmake --build build/host --target owb_driver_tests
   build/host/tests/driver/owb_driver_tests
   ```
2. Driver changes (`driver/`) must still build with MSBuild + WDK 11 using
   `driver/owb_a2dp.vcxproj`. Say in the PR whether you tested them on hardware
   (headphones, Bluetooth adapter, Windows build) or only built them.
3. Keep `<type>: <description>` commit subjects and one purpose per commit.
4. Update the README and `docs/TESTING.md` when a feature or its validation status changes.

## Licensing

OpenWinBlue is GPLv3 ([LICENSE](LICENSE)). Contributions are accepted under the same license,
and new dependencies must be GPLv3-compatible — list them in
[third-party/LICENSES.md](third-party/LICENSES.md).

## Security

Report vulnerabilities privately through GitHub Security Advisories, not in public issues.
