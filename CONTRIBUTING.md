# Contributing

Thanks for helping keep Bluetooth claims honest.

## Ground rules

- Do not add registry “unlocks”, driver installers, or stack replacement paths without
  an ADR, security review, and recovery design.
- Every codec state must be evidence-labelled. Never let selection imply activation.
- Add tests for parsers and state transitions.
- Keep the UI standard-user by default.
- No telemetry or network calls.
- Respect codec licensing and do not bundle proprietary binaries.

## Workflow

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target x86_64-pc-windows-msvc
```

For Windows hardware changes, include the hardware matrix row and a redacted capture
description in the PR.

## Commit style

Prefer focused commits with an imperative subject, for example:

```
fix(etw): reject unrecognised vendor tuples
```

## Code of conduct

Be precise, kind, and evidence-driven. Disagreement about a technical claim should be
resolved with documentation, a fixture, or a hardware test.
