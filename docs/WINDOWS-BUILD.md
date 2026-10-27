# Windows build and release procedure

## Prerequisites

- Windows 10 2004+ or Windows 11 development machine.
- Rust 1.88+ with the `x86_64-pc-windows-msvc` target.
- Visual Studio 2022 Build Tools with the Desktop development with C++ workload
  (MSVC linker and Windows SDK).
- PowerShell 5.1+.

## Build and test

```powershell
rustup target add x86_64-pc-windows-msvc
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release --target x86_64-pc-windows-msvc
```

## Run

```powershell
.\target\x86_64-pc-windows-msvc\release\bluu.exe
.\target\x86_64-pc-windows-msvc\release\bluu.exe --demo
```

`bluu-etw-helper.exe` must be beside `bluu.exe`. The app never launches it unless
the user clicks **Verify live codec** and approves the UAC prompt.

## Reproducible build notes

- `Cargo.lock` is committed.
- `rust-toolchain.toml` pins Rust 1.88.0.
- Release profile disables incremental builds, strips symbols, and uses thin LTO.
- Release ZIP script records SHA-256 hashes.
- A fully reproducible MSVC build also requires a pinned Windows SDK/MSVC toolset;
  record versions in release notes.

## Manual QA (required on Windows)

- app opens at standard integrity;
- paired devices appear, including a disconnected device;
- endpoint format is shown without claiming codec rate;
- refresh works with device removal;
- UAC cancellation leaves app usable;
- helper result is labelled unverified;
- export redacts identifiers;
- logs contain no Bluetooth addresses in default level;
- no network process/connection is created.
