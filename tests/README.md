# Test layers

## Unit (runs everywhere)

`cargo test --workspace` exercises:

- codec ID/evidence rules;
- SDP bounds and profile decoding;
- Windows build feasibility thresholds;
- privacy redaction;
- settings validation/round-trip;
- screenshot fixture invariants.

## Integration (Windows)

Planned harness targets:

- fake backend with device add/remove transitions;
- helper JSON success/error/timeout;
- settings corruption recovery;
- export path/write failure;
- endpoint association container-vs-name ordering.

## Hardware

See `docs/TESTING.md`. A claim is only “verified” when the row includes an
independent observation (WPR/ETL or protocol trace), not just Bluu's UI label.
