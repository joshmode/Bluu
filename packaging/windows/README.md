# Windows portable packaging

This is intentionally a **portable ZIP**, not a driver or service installer.

## Build

From a Windows machine with Rust, the MSVC target, and Visual Studio Build Tools:

```powershell
cargo build --workspace --release --target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File packaging/windows/build-portable.ps1
```

The script:

1. builds both executables for the MSVC target;
2. copies the app, helper, licences, README, security notes, and testing notes;
3. includes the optional native provider package and its LGPL/Apache/GPL notices
   when `bluu-provider-owb.exe` is built;
4. writes SHA-256 hashes to `BUILD-INFO.txt`;
5. creates a ZIP under `dist/`.

`bluu-provider-demo.exe` is included only as a protocol fixture; it does not
provide a Bluetooth transport. Real advanced codecs require a separately installed
provider/driver that owns the encoder and A2DP transport.

## Signing

Production release artifacts should be Authenticode-signed. A self-signed local
build is not a distributable trust signal. The helper must be signed with the same
publisher identity as the UI so users can verify the elevation relationship.

## Uninstall

Delete the portable folder. Settings/logs live under the user's application-data
directory and can be removed from Bluu Settings or manually. No driver, service, or
registry entry is installed by this build.

## Installer status

The stale native MSI project is quarantined. A future MSI/MSIX must:

- be signed;
- install per-user or with a clear option;
- not install a driver/service;
- include a verified uninstall path;
- preserve the standard-user UI and separate helper.

The optional native component uses `native/owb/tools/install-native-a2dp.ps1`,
which fails closed on signature problems and records prior driver/service state.
