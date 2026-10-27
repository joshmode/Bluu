# Bluu technical research and feasibility report

Status: research snapshot **2026-10-27**. Primary target Windows 11; Windows 10 is
supported only where Microsoft documents the same API and codec path. Every claim
below has an evidence label:

- **Documented** — stated in Microsoft, Bluetooth SIG, or vendor documentation.
- **Observed** — reproduced on hardware or in a checked fixture.
- **Inferred** — reasonable interpretation that has not been independently tested.
- **Unverified** — needs Windows hardware; Bluu does not claim it as tested.

This report is deliberately about boundaries. Windows does not expose an A2DP codec
preference API, a codec-registration API, or a complete remote codec capability list.
Bluu therefore implements an honest inventory/observation/diagnostics product first.

## 1. Windows Bluetooth audio architecture

### 1.1 Classic stack and audio profiles

Windows Classic audio has two distinct roles:

| Profile | Direction | Typical use | Codec family |
|---|---|---|---|
| A2DP | Host → remote sink | Music, video, games | SBC, AAC, aptX, vendor codecs |
| HFP | Bidirectional mono | Calls, microphone | CVSD/narrowband SBC and mSBC/wideband |
| HSP | Bidirectional mono | Legacy headset | CVSD-style speech |

Microsoft's *Bluetooth Classic audio* documentation states that A2DP requires SBC,
supports optional codecs, and chooses the first codec supported by both host and
device from its ordered list. Its Windows table is the strongest public evidence
available for the local in-box inventory:

| Codec | Windows 10 | Windows 11 |
|---|---|---|
| aptX Adaptive (lossless) | Not supported | Windows 11 24H2+ on select Qualcomm radios |
| AAC | Not listed | Windows 11 21H2+ |
| aptX Classic | Windows 10 1507+ | Windows 11 21H2+ |
| SBC | Windows 10 1507+ | Windows 11 21H2+ |

Evidence: **Documented**, high confidence. The table is a local-stack inventory,
not a claim about a particular headset or a particular current connection.

Windows 11 unifies A2DP/HFP endpoints. Opening the capture endpoint, or a stream with
the Communications category, can switch the device to HFP. Windows then resamples
playback. This is documented profile arbitration, not a codec preference the app can
override.

### 1.2 Endpoint and audio engine layers

Core Audio exposes `IMMDeviceEnumerator`, `IMMDevice`, `IAudioClient::GetMixFormat`,
`PKEY_AudioEngine_DeviceFormat`, and endpoint property stores. These describe:

- the Windows endpoint format used by the audio engine;
- the stored/shared-mode format chosen by the user;
- the mix format of a running endpoint;
- period/latency information available through `IAudioClient`;
- endpoint identities and containers.

They do **not** describe the A2DP codec's sample rate, channel mode, bitrate, or
bitpool. A 48 kHz engine mix can be resampled or encoded at another rate. Bluu shows
the two layers separately and never says that endpoint sample rate proves codec rate.

Evidence: **Documented**, high confidence. Endpoint-to-device association is not a
single documented Core Audio property; Bluu uses Windows container IDs when available
and labels name-based fallback as a low-confidence heuristic.

### 1.3 Device discovery and profiles

WinRT `BluetoothDevice` exposes connection status, class of device, Bluetooth
address, cached RFCOMM services, and SDP raw attributes. The cached SDP records can
show A2DP/HFP service existence and profile versions. They do not expose A2DP's
AVDTP media-codec capability list through a documented API.

Evidence: **Documented** for the interfaces; **observed** for the shape of parsed SDP
fixtures; **unverified** on a physical Windows radio in this build.

### 1.4 Is the negotiated codec exposed?

No stable documented property or API reports the active A2DP codec. Microsoft's
Bluetooth tracing profile enables the `Microsoft-Windows-Bluetooth-BthA2dp` ETW
provider (GUID `ddb6da39-08a7-4579-8d0c-68011146e205`). Its events include a duration
style `A2dpStreaming` event that third-party tools have decoded with fields:

- `A2dpStandardCodecId`
- `A2dpVendorId`
- `A2dpVendorCodecId`

Evidence: **Documented** provider/session information, **observed** by independent
implementations, but the field names and event layout are **not a stable public
contract**. Bluu treats this as an observation with a timestamp and confidence, not
configuration.

The trace normally requires administrator rights. Bluu therefore keeps the UI at
standard integrity and uses a tiny elevated helper only for a bounded trace.

### 1.5 Is codec preference configurable?

No documented public API or supported registration mechanism was found for
preferred A2DP codec, priority, SBC bitpool, AAC bitrate, aptX mode, or LDAC
quality. Registry values circulating on the internet (for example
`BthA2dp\Parameters`) are **undocumented and version-sensitive**. A value such as
`BluetoothAacEnable` may work on a particular build, but it is not a supported codec
preference contract and is not used by Bluu.

Evidence: documented API absence plus **unverified** third-party registry anecdotes
and the official Windows codec table. Confidence: high that no supported
*registration* API exists; medium that no private build-specific knob is useful to a
safe generic app.

### 1.6 Windows 10 vs Windows 11

- Windows 11 21H2+ adds AAC and aptX to the documented in-box A2DP table.
- Windows 11 22H2+ adds LE Audio vendor-specific ACX/VSAP paths.
- Windows 11 24H2+ lists aptX Adaptive (lossless) for select Qualcomm radios.
- Windows 10 1507+ documents SBC and aptX only in the current table.
- HFP wideband exists from Windows 10 1703, with compatibility caveats.

Bluu's support matrix is build-gated and shows inventory, never a per-device promise.

## 2. Codec feasibility

The status vocabulary is the one requested in the brief. “Configurable” means a
documented local configuration operation exists; Bluu has not found one for A2DP
codec selection.

| Codec | Status | Windows path | Remote advertise | Configuration | Encoding/transport | Licensing / open source | Confidence |
|---|---|---|---|---|---|---|---|
| SBC | Natively supported | In-box A2DP | Required | No public preference | Windows supplies it | A2DP specification | High |
| AAC | Natively supported | Win11 21H2+ | Required | No public preference | Windows supplies it | In-box; not redistributed | High |
| aptX Classic | Natively supported | Win10 1507+/Win11 | Required | No public preference | Windows supplies vendor path | Qualcomm licensed | High |
| aptX HD | Requires substantial system integration | Not in-box | Required | None | Replacement/vendor A2DP stack | Qualcomm licence + driver signing | High |
| aptX Adaptive | Conditionally implementable | Win11 24H2 only on select Qualcomm radios | Required | OEM/radio dependent | Licensed Qualcomm integration | Qualcomm confidential/licensed | High |
| aptX Lossless | Not feasible safely for generic PCs | Only the conditional 24H2 OEM listing | Required | None | Vendor/hardware coupled | Qualcomm licensed | Medium |
| LDAC | Requires substantial system integration | Not in-box | Required | No supported registry key | Source implementation + A2DP stack | Sony states a licence is required to implement; trademark/patent review | High |
| LC3 / LE Audio | Requires substantial system integration | Win11 22H2+ vendor ACX/VSAP | PACS/BAP required | No app-level registration | ACX/VSAP + ISO transport; LC3 may be offloaded | Bluetooth SIG spec; implementation/patent review | High |
| MP3 | Not feasible on this backend | Standard A2DP ID, not in-box table | Required | None | Needs vendor stack | Patent/licensing review required | Medium |

### Codec implementation does not equal transport support

`libopenaptx` is a GPLv3+ encoder/decoder, not an A2DP stack. AOSP libldac is a
codec implementation; it is not a Windows profile driver. LC3 encoders similarly do
not create an ACX/VSAP audio endpoint. Bluu does not bundle any of these binaries.

### Decode vs encode

A decoder on the remote headset is not a Windows encoder. An open-source source
encoder is not a registered Windows A2DP codec. A vendor dongle can own both ends
and present a normal USB/Bluetooth audio device to Windows; that is a legitimate
external solution, but it is not “unlocking” the in-box stack.

## 3. API/interface classification

| Interface | Classification | Use in Bluu |
|---|---|---|
| WinRT `BluetoothDevice`, `DeviceInformation` | Public/documented | Paired device and connection inventory |
| WinRT cached RFCOMM/SDP attributes | Public but limited | Profile observation only |
| WinRT `BluetoothAdapter` | Public | Adapter capability flags |
| Core Audio `IMMDeviceEnumerator`, `IAudioClient` | Public/documented | Endpoint format/state/period |
| `PKEY_AudioEngine_DeviceFormat` | Public/documented read | Stored shared-mode format |
| `PKEY_AudioEngine_DeviceFormat` write | Not a supported configuration contract | Not used |
| `Microsoft-Windows-Bluetooth-BthA2dp` ETW | Documented provider, non-contract event payload | Elevated observation only |
| `BthA2dp\Parameters` registry values | Undocumented/version-specific | Not used |
| Replacement A2DP kernel driver | Driver signing, Secure Boot, kernel safety burden | Not shipped |
| LE Audio ACX/VSAP | Documented for IHVs, not app-level registration | Diagnostic only |

## 4. Architecture alternatives

### A. Native Windows integration (chosen)

Use WinRT/Core Audio for read-only inventory and a short ETW helper for observation.
Pros: standard-user UI, no driver, no registry writes, no service, low maintenance,
clear failure modes. Cons: cannot set a codec preference; advanced codecs remain
unavailable on generic hardware.

### B. Optional codec integration layer

A modular codec backend is a sensible core abstraction, but there is no supported
Windows mechanism for an ordinary app to register an A2DP codec encoder/provider.
An open codec library alone cannot satisfy the transport requirement. This
architecture remains a documented extension point rather than a shipped claim.

### C. Advanced optional backend

Replace the in-box A2DP path with a signed kernel driver or alternate Bluetooth
stack. This is technically possible for specialist vendors, but requires kernel
development, attestation/signing, Secure Boot-compatible distribution, recovery
tooling, and extensive audio/radio validation. It is not a default, and Bluu does not
bundle or install such a component.

## 5. Recommended architecture

**A. Native Windows integration, read-only by default, with an isolateable ETW
verification helper and a validated advanced-provider boundary.** The app must
remain useful when configuration is not available. The value is trustworthy
inventory, endpoint/profile diagnostics, true negotiation observation, and clear
“why not” explanations. Advanced codec preference is delegated to a provider that
owns the encoder/transport and its legal/signing obligations.

## 6. Hardware requirements and limitations

- Windows 10 2004+ or Windows 11 for the documented desktop APIs; AAC/LE Audio
  require their documented Windows 11 thresholds.
- A Bluetooth Classic-capable radio for A2DP; dual-mode for LE Audio research.
- A paired A2DP sink/headset.
- Administrator approval only for the optional codec trace.
- No Windows hardware was available on the development host. All real-device claims
  in this repository are labelled **unverified** until the hardware matrix is filled.

## 7. References

1. Microsoft, *Bluetooth Classic audio* (2024-11-14),
   <https://learn.microsoft.com/en-us/windows-hardware/drivers/bluetooth/bluetooth-classic-audio>
2. Microsoft, *Bluetooth Low Energy (LE) Audio* (22H2/VSAP/ACX),
   <https://learn.microsoft.com/en-us/windows-hardware/drivers/bluetooth/bluetooth-low-energy-audio>
3. Microsoft, *Bluetooth tracing* / `BluetoothStack.wprp`, busiotools,
   <https://github.com/microsoft/busiotools>
4. Microsoft, *PKEY_AudioEngine_DeviceFormat* and Core Audio device formats.
5. Microsoft, *Configuring and Starting a Private Logger Session* (ETW privilege
   context).
6. Helge Klein, *How to Check Which Bluetooth A2DP Audio Codec Is Used on Windows*
   (ETW field mapping; independent observation).
7. imbushuo, *BluetoothAudioCodecInspector* (MIT) — independent ETW decoding.
8. Bluetooth SIG, A2DP, AVDTP, PACS/BAP specifications and assigned company IDs.
9. Sony, *LDAC developer site* — “A license from Sony is required to implement the
   LDAC software (SNK).”
10. Qualcomm aptX product documentation — adaptive/lossless positioning and
    hardware/licensing dependence.
11. ValdikSS, *Audio over Bluetooth* and `btcodecs` — vendor tuple catalog,
    independent technical analysis.

## 8. Open questions requiring hardware

- Does the BthA2dp event always expose the expected fields on Windows 10 22H2 and
  Windows 11 21H2/22H2/23H2/24H2?
- Does either event carry a device identifier on any build? Bluu assumes no.
- Which endpoint container IDs reliably match BTHENUM devices across driver vendors?
- Do the inferred Adaptive/Lossless vendor tuples occur in real Windows
  negotiations, and can they be distinguished from Low Latency without a protocol
  capture?
- Does the LE Audio VSAP surface any user-accessible codec diagnostics beyond the
  endpoint format?

These are explicitly not promoted to product claims.
