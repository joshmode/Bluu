# Dependency inventory and selection rationale

| Dependency | Version | Use | Licence | Why |
|---|---|---|---|---|
| eframe / egui | 0.31.1 | Desktop UI | MIT OR Apache-2.0 | Rust-native, compact, custom design, no extra Bluetooth API layer |
| windows | 0.62.2 | Win32/COM/WinRT bindings | MIT OR Apache-2.0 | Maintained official bindings; safe wrappers over raw APIs |
| windows-future | 0.3.2 | WinRT async waiting | MIT OR Apache-2.0 | Avoids a full async runtime for short device operations |
| windows-collections | 0.3.2 | WinRT collection parameters | MIT OR Apache-2.0 | Required by `DeviceInformation` API |
| ferrisetw | 1.2.0 | ETW consumer in helper | MIT | Small safe-ish ETW abstraction; helper-only |
| serde / serde_json | 1.0.229 / 1.0.154 | Settings, diagnostics | MIT OR Apache-2.0 | Standard, mature, audited ecosystem |
| chrono | 0.4.45 | Timestamps/export | MIT OR Apache-2.0 | Local/timezone formatting |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | Structured local logs | MIT | No network/telemetry |
| directories | 6.0.0 | App data paths | MIT OR Apache-2.0 | Cross-platform data dir handling |
| image | 0.25.x | Headless screenshot renderer | MIT OR Apache-2.0 | Docs/CI only |
| rfd | 0.15.4 | Native provider-folder picker (Windows) | MIT | Explicit local install only; no network |
| thiserror | 2.0.21 | Explicit error types | MIT OR Apache-2.0 | Clear backend errors |

No dynamic browser engine, analytics SDK, cloud SDK, or codec binary is a dependency.

Dependency versions are resolved in `Cargo.lock`; CI uses `cargo-deny` for licences,
advisories, bans, and source verification. Review this file whenever a dependency is
added or replaced.
