# Third-party notices checklist

Bluu releases must include licence texts for all Rust dependencies. Generate a
complete inventory from `Cargo.lock` at release time with a reviewed tool such as
`cargo-about` or `cargo-deny`. Do not ship a hand-written list as a substitute.

Known direct dependencies:

- eframe / egui (MIT OR Apache-2.0), including default font assets under their
  respective licences.
- windows / windows-core / windows-future / windows-collections (MIT OR Apache-2.0).
- ferrisetw (MIT), used only by the elevated helper.
- serde / serde_json (MIT OR Apache-2.0).
- tracing / tracing-subscriber (MIT).
- chrono (MIT OR Apache-2.0).
- directories (MIT OR Apache-2.0).
- image (MIT OR Apache-2.0), used only for documentation/headless screenshots.

No proprietary codec binary is bundled. Refer to [../docs/LICENSING.md](../docs/LICENSING.md).
