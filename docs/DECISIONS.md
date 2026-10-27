# Decision log

This is the short index of decisions that shape the product. Detailed rationale is
in `docs/ARCHITECTURE.md` and `docs/RESEARCH.md`.

| ID | Date | Decision | Why |
|---|---|---|---|
| D-001 | 2026-10-27 | No registry codec toggles | Undocumented, build-dependent, and not codec registration |
| D-002 | 2026-10-27 | No bundled driver/stack replacement; provider-gated | Signing, Secure Boot, recovery, and support burden remain the provider's responsibility |
| D-003 | 2026-10-27 | Codec inventory ≠ active codec | Prevents false “unlocked” claims |
| D-004 | 2026-10-27 | ETW observation is elevated and one-shot | Least privilege; helper has no configuration path |
| D-005 | 2026-10-27 | Unknown vendor tuples stay unknown | Avoids mislabeling Low Latency/Adaptive/Lossless |
| D-006 | 2026-10-27 | Endpoint format ≠ codec rate | Core Audio and A2DP are separate layers |
| D-007 | 2026-10-27 | egui shell v0.1 | Rust-native, cross-platform core/demo, compact app; no additional BT API from WinUI |
| D-008 | 2026-10-27 | Privacy redaction on by default | Local-first product with safe diagnostics |
| D-009 | 2026-10-27 | Hardware claims require a Windows test row | No fabricated validation |
| D-010 | 2026-10-27 | MIT OR Apache-2.0 | Permissive OSI-approved licensing, dual-licence compatibility |
