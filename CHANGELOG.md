# Changelog

## 0.1.0 — 2026-10-27

First engineering foundation:

- Rust workspace with core, platform, app, and ETW helper.
- Read-only WinRT Bluetooth/device discovery and cached SDP profile observations.
- Core Audio endpoint enumeration, stored/shared format, mix format, and engine period.
- Codec inventory by documented Windows build and evidence-labelled feasibility matrix.
- Optional one-shot elevated A2DP ETW observation.
- Honest active/verified vs supported/unverified states.
- Local settings, redacted diagnostic export, and privacy controls.
- Demo mode for non-Windows UI development.
- CI, unit tests, cross-target type-check, build documentation, and a portable release
  script.

Known limitations: no hardware validation on Windows yet; no codec preference API; no
LDAC/aptX HD enabling; ARM64 packaging not yet verified.
