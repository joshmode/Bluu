param(
    [string]$Profile = "release",
    [string]$Output = ""
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrWhiteSpace($Output)) {
    $Output = Join-Path $root "dist\bluu-windows-x64-$Profile"
}

Write-Host "Building Bluu for Windows ($Profile)..."
Push-Location $root
try {
    cargo build --workspace --profile $Profile --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $targetDir = Join-Path $root "target\x86_64-pc-windows-msvc\$Profile"
    $exe = Join-Path $targetDir "bluu.exe"
    $helper = Join-Path $targetDir "bluu-etw-helper.exe"
    $providerDemo = Join-Path $targetDir "bluu-provider-demo.exe"
    if (-not (Test-Path $exe)) { throw "bluu.exe was not produced" }
    if (-not (Test-Path $helper)) { throw "bluu-etw-helper.exe was not produced" }

    New-Item -ItemType Directory -Force -Path $Output | Out-Null
    Copy-Item $exe $Output -Force
    Copy-Item $helper $Output -Force
    if (Test-Path $providerDemo) {
        Copy-Item $providerDemo $Output -Force
    }
    Copy-Item (Join-Path $root "README.md") $Output -Force
    Copy-Item (Join-Path $root "LICENSE-MIT") $Output -Force
    Copy-Item (Join-Path $root "LICENSE-APACHE") $Output -Force
    Copy-Item (Join-Path $root "docs\SECURITY.md") $Output -Force
    Copy-Item (Join-Path $root "docs\TESTING.md") $Output -Force

    $sha = Get-FileHash (Join-Path $Output "bluu.exe") -Algorithm SHA256
    $helperSha = Get-FileHash (Join-Path $Output "bluu-etw-helper.exe") -Algorithm SHA256
    $providerSha = if (Test-Path (Join-Path $Output "bluu-provider-demo.exe")) {
        (Get-FileHash (Join-Path $Output "bluu-provider-demo.exe") -Algorithm SHA256).Hash
    } else { "not included" }
    @"
Bluu portable Windows build
Version: see Cargo.toml
Built: $(Get-Date -Format o)
Architecture: x86_64-pc-windows-msvc

bluu.exe SHA-256: $($sha.Hash)
bluu-etw-helper.exe SHA-256: $($helperSha.Hash)
bluu-provider-demo.exe SHA-256: $providerSha

Run bluu.exe as a standard user. The helper is used only when the user chooses
"Verify live codec" and approves the one-time elevation prompt.

No driver, service, registry write, or startup entry is installed.
"@ | Set-Content (Join-Path $Output "BUILD-INFO.txt")

    Compress-Archive -Path (Join-Path $Output "*") -DestinationPath "$Output.zip" -Force
    Write-Host "Portable build written to $Output"
    Write-Host "ZIP: $Output.zip"
}
finally {
    Pop-Location
}
