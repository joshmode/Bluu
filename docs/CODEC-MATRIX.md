# Codec support and feasibility matrix

Snapshot: 2026-10-27. This is a **software feasibility** matrix, not a list of
headsets we have tested. The machine-readable source is
`bluu_core::feasibility::feasibility_matrix()`.

## Windows in-box A2DP inventory

| Codec | Windows 10 | Windows 11 | App-set preference | App-set parameters |
|---|---|---|---|---|
| SBC | Yes (1507+) | Yes | No documented API | No |
| AAC | Not in Microsoft's table | Yes (21H2+) | No documented API | No |
| aptX Classic | Yes (1507+) | Yes | No documented API | No |
| aptX HD | No | No | No | No |
| aptX Adaptive | No | Select Qualcomm radios, 24H2+ | OEM/radio only | No generic API |
| aptX Lossless | No | Select Qualcomm radios (listed together with Adaptive) | No generic API | No |
| LDAC | No | No | No | No |
| LC3 | No A2DP | LE Audio vendor path | Not an A2DP preference | ACX/VSAP only |

## Feasibility statuses

| Codec | Status | Evidence | Confidence | Why |
|---|---|---|---|---|
| SBC | Natively supported | Documented | High | Required by A2DP, in-box |
| AAC | Natively supported | Documented | High | Win11 21H2+ table |
| aptX Classic | Natively supported | Documented | High | Win10/11 table |
| aptX HD | Requires substantial system integration | Documented | High | Not in-box |
| aptX Adaptive | Conditionally implementable | Documented + inferred vendor IDs | High | Qualcomm/OEM radio path |
| aptX Lossless | Not feasible safe/supported | Inferred | Medium | Only vendor path |
| LDAC | Requires substantial system integration | Documented + vendor licence | High | Not in-box; Sony licence |
| LC3 | Requires substantial system integration | Documented | High | Separate LE Audio/VSAP |
| MP3 | Not feasible on this backend | Inferred | Medium | Standard ID, no in-box table |

## What the app can observe

| Layer | Observable? | How | Limits |
|---|---|---|---|
| Remote A2DP service | Sometimes | Cached SDP | Does not list AVDTP codecs |
| Local Windows in-box inventory | Yes (version table) | Documented | Not per-radio proof |
| Active A2DP codec ID | Often | ETW `A2dpStreaming` event | Undocumented payload; needs elevation |
| A2DP parameters (bitrate/bitpool) | No stable API | — | Replacement stack or protocol capture only |
| Endpoint sample rate/format | Yes | Core Audio | Not the codec rate |
| HFP active/capture endpoint | Yes | Core Audio | Does not prove codec negotiation |
| LE Audio codec config | Limited | endpoint/driver | No app-level documented API |

## “Enabling” reality

- Registry edits are **not** a supported registration mechanism and are not used.
- A codec decoder or encoder library does not create an A2DP encoder path.
- An external dongle/vendor stack is a legitimate way to get advanced codecs, but it
  is outside the in-box Bluetooth stack.
- Replacing the A2DP driver is a specialist, kernel-mode, signing-heavy product
  decision. It is not included in Bluu v0.1.0.
