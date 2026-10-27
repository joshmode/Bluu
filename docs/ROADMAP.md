# Roadmap

## v0.1.0 — honest foundation (this release)

- Device discovery: paired/connected Bluetooth devices, A2DP/HFP SDP observations.
- Endpoint inventory and shared/mix formats.
- Codec inventory by Windows build and documented feasibility matrix.
- Separate capability/configuration/negotiated/observed/verified fields.
- Optional elevated ETW observation with explicit evidence labels.
- Diagnostics with actionable findings, redacted export, local settings.
- Narrow AAC/aptX Classic preference through the explicitly consented BthA2dp value.
- Codec preference controls through validated advanced providers, including an
  optional in-repo native backend.

## v0.2 — validation and polish

- Fill hardware matrix on Windows 10/11 with SBC/AAC/aptX and representative adapters.
- Test endpoint container mapping across Intel/Realtek/MediaTek/Qualcomm drivers.
- Improve accessibility: keyboard traversal, focus rings, screen-reader labels.
- Add optional `WPR`/ETL import for user-supplied independent verification.
- Add JSON schema version and export verification tests.
- Native Windows notifications (opt-in, local only).
- Hardware validation of the native A2DP provider on representative Windows radios.

## v0.3 — optional advanced backends

Only if legal and safety prerequisites are met:

- Vendor-provided codec/stack integration (signed, redistributable).
- External dongle detection and honest “handled outside Windows stack” display.
- LE Audio discovery/diagnostics through vendor ACX/VSAP interfaces.

## Out of scope unless separately approved

- Bundling proprietary codec binaries.
- Registry unlock utilities marketed as universal.
- Automatic kernel driver installation.
- Replacing the Windows Bluetooth stack by default.
- Claiming bitrate or perceptual quality without measurement.

## Future verification methods

- WPR/ETL trace comparison.
- HCI/AVDTP capture on a dedicated test radio.
- Loopback/measurement hardware for latency.
- Vendor LE Audio test tools for PACS/BAP configuration.
