# Third-party notices

Bluu is distributed under GPL-3.0-or-later. The complete licence text is in
`LICENSE`. Third-party components retain their own licences and attribution.

## Rust dependencies

Release artifacts must include licence texts generated from `Cargo.lock` with a
reviewed tool such as `cargo-about` or `cargo-deny`. Known direct dependencies:

- eframe / egui (MIT OR Apache-2.0), including default font assets.
- windows / windows-core / windows-future / windows-collections (MIT OR Apache-2.0).
- ferrisetw (MIT), used only by the elevated helper.
- serde / serde_json (MIT OR Apache-2.0).
- tracing / tracing-subscriber (MIT).
- chrono (MIT OR Apache-2.0).
- directories (MIT OR Apache-2.0).
- image (MIT OR Apache-2.0), used only for documentation/headless screenshots.
- tempfile (MIT OR Apache-2.0), tests and bounded provider result handling.

## Native optional A2DP component

- OpenWinBlue, https://github.com/santiquiroz/openwinblue — GPLv3 foundation.
- libldac, AOSP-derived Sony LDAC encoder — Apache-2.0.
- libsbc, BlueZ SBC library — LGPL-2.1-or-later.
- FFmpeg aptX codec core (`aptx.c`, `aptxenc.c` codec tables/DSP) — LGPL-2.1-or-later.
  Only the self-contained codec core is vendored; the FFmpeg framework is not.
- GoogleTest v1.14.0 via CMake FetchContent — BSD-3-Clause, host tests only.

`libopenaptx` is not included. Its README carries non-OSI restrictions beyond its GPL
text, so it is not suitable for redistribution here.

AAC encoding uses the Windows Media Foundation encoder supplied by the operating
system. No proprietary Qualcomm or Sony codec binary is bundled. Licences and patents
are not granted by trademark names; review any redistributed binary with counsel.
