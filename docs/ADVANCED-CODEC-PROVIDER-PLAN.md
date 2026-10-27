# Advanced codec provider plan

Status: implementation plan for the next phase (approved direction: 2026-10-27).

## Goal

Bluu must be able to manage codec preference and codec-provider registration for:

- SBC / AAC / aptX (where a provider exposes them)
- aptX HD
- aptX Adaptive / Lossless (where a provider exposes them)
- LDAC

This is not a claim that the Windows in-box A2DP stack supports those codecs. It
does not. The only honest mechanism found is a provider boundary: a separately
installed/signed (or explicitly user-installed) codec/transport provider owns the
encoder and A2DP/Bluetooth transport, while Bluu owns discovery, capability
validation, preference state, transaction handling, rollback, and diagnostics.

## Why this design

Windows exposes no supported API to register an A2DP encoder or choose a codec.
Registry values are not a codec registration contract. A user-mode codec library is
also insufficient: it does not create the A2DP transport. Therefore:

- The provider is the technically meaningful extension point.
- The provider manifest is the registration API.
- The provider executable is the implementation.
- Bluu remains the safe, auditable control plane.
- Bluu never labels a provider preference as active until the provider returns an
  active-codec observation with a device identity.

## Provider protocol

Providers are discovered from these directories, in order:

1. `%LOCALAPPDATA%\Bluu\providers\<id>\provider.json` (user-installed)
2. `<application>\providers\<id>\provider.json` (bundled/vendor)

`provider.json` is validated. Executables must be inside the provider directory;
absolute paths and `..` are rejected.

The executable is invoked with:

```text
provider.exe --request <json-file> --result <json-file>
```

Both files are app-generated in the user temp directory. The provider is never
started through a shell. Elevated providers are launched through `ShellExecuteExW`
with `runas`, only after a clear UI confirmation. This is the same minimum-isolation
pattern as the ETW helper.

### Request

```json
{
  "api_version": 1,
  "action": "describe",
  "device": {
    "id": "BTHENUM\\...",
    "name": "Headphones",
    "address": "AA:BB:CC:DD:EE:FF",
    "connected": true
  },
  "preference": null
}
```

Actions:

- `describe`: validate the provider’s active capabilities.
- `apply`: apply an ordered codec preference and optional parameters.
- `rollback`: restore the provider’s previous state/token.
- `verify`: report the active codec and evidence.

### Response

```json
{
  "api_version": 1,
  "ok": true,
  "message": "Preference applied; reconnect required",
  "active_codec": "ldac",
  "active_vendor_id": null,
  "active_codec_id": null,
  "rollback_token": "opaque-provider-token",
  "applied_preference": {
    "device_id": "BTHENUM\\...",
    "ordered_codecs": ["ldac", "aptx_hd", "sbc"],
    "parameters": {"quality": "990"}
  }
}
```

`ok` means the provider accepted the request. It does not mean the codec is active.
Only a response with `active_codec` and a device-bound verification result can
promote a codec to active/verified.

## Manifest

```json
{
  "api_version": 1,
  "id": "vendor.ldac",
  "name": "Vendor LDAC provider",
  "version": "1.2.0",
  "executable": "provider.exe",
  "publisher": "Vendor Name",
  "requires_elevation": true,
  "stack": "replacement-a2dp-driver",
  "capabilities": {
    "preferred_codec": true,
    "priority": true,
    "parameters": true,
    "active_codec": true,
    "rollback": true
  },
  "codecs": [
    {
      "codec": "ldac",
      "notes": "Requires device LDAC support and provider connection.",
      "parameters": [
        {
          "key": "quality",
          "label": "LDAC quality",
          "kind": "choice",
          "options": ["330", "660", "990"],
          "default": "990"
        }
      ]
    }
  ]
}
```

## Capability semantics

| State | Meaning |
|---|---|
| ProviderAvailable | Provider manifest advertises codec/hardware path |
| PreferenceApplied | Provider accepted a preference; not active proof |
| ActiveUnverified | Provider reports a codec without device-qualified evidence |
| ActiveProviderAttested | Provider reports active codec plus device-bound provider evidence |
| ActiveVerified | Independent trace/analyzer evidence; not a provider preference response |
| ProviderError | Startup, protocol, validation, or rollback failure |

An in-box inventory item and provider item are separate capabilities. A provider’s
manifest cannot cause `SupportedAvailable` to be shown for the Windows in-box stack.

## Hardware capability rules

A codec is selectable only when:

1. the provider manifest advertises that codec; and
2. the provider returns a compatible device/capability state; and
3. the provider confirms it can apply the preference; and
4. the provider reports actual negotiation/verification separately.

“Technically capable” is a provider/hardware claim, not a marketing-name inference.
Bluu records provider version, publisher, path hash after discovery, and evidence.

## Phases

### Phase 1 — Domain and manifest API

- Add manifest/preference/response types and validation.
- Add provider capability/status to snapshots and diagnostics.
- Add explicit `ProviderAvailable` and configuration facts.
- Tests for path traversal, unknown codecs, duplicate priorities, invalid parameters,
  and untrusted response shapes.

Review gate: `cargo fmt`, `cargo test --workspace`, Clippy; no UI changes yet.

### Phase 2 — Provider discovery and execution

- Discover user/bundled providers.
- Validate manifests and executable containment.
- Apply/rollback/verify through bounded JSON files.
- Add demo/test provider fixture.
- Add provider state persistence and last-error reporting.
- Add Windows elevated invocation path.

Review gate: protocol tests, failure injection (missing executable, malformed
manifest, timeout, oversized result, UAC cancellation), Windows-target Clippy.

### Phase 3 — UI and workflow

- Add provider card and per-device codec preference controls.
- Show exactly which codecs the provider supports.
- Allow LDAC, aptX HD, aptX Adaptive/Lossless and others when the provider lists
  them; do not hardcode them as unavailable.
- Require explicit “advanced provider” consent.
- Add Apply, Rollback, Verify; preserve state on failure.
- Never show a preference as active.

Review gate: UI/demo screenshot, keyboard/focus review, no fictional controls.

### Phase 4 — Release validation

- Windows build check and Clippy.
- Document provider installation, publisher verification, and recovery.
- Update research/matrix/docs.
- Add hardware validation rows for LDAC/aptX HD/Adaptive.
- Generate provider fixture report and diagnostic export.

## Safety and legal boundaries

The user explicitly accepts an advanced, less-safe path. Bluu still:

- never silently installs a driver or provider;
- never downloads a provider;
- never weakens Secure Boot or signature enforcement;
- shows provider identity and elevation requirements;
- offers dry-run/apply/rollback;
- preserves provider state and logs;
- does not bundle aptX, LDAC, or other proprietary codec binaries.

The provider itself is responsible for codec licensing, stack compatibility,
driver signing, and hardware support. A provider that cannot produce actual audio
must not claim enablement.
