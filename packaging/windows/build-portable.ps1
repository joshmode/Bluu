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
    $providerNative = Join-Path $targetDir "bluu-provider-owb.exe"
    if (-not (Test-Path $exe)) { throw "bluu.exe was not produced" }
    if (-not (Test-Path $helper)) { throw "bluu-etw-helper.exe was not produced" }

    New-Item -ItemType Directory -Force -Path $Output | Out-Null
    Copy-Item $exe $Output -Force
    Copy-Item $helper $Output -Force
    if (Test-Path $providerDemo) {
        Copy-Item $providerDemo $Output -Force
    }
    if (Test-Path $providerNative) {
        $nativeProviderDir = Join-Path $Output "providers\bluu-native-a2dp"
        New-Item -ItemType Directory -Force -Path $nativeProviderDir | Out-Null
        Copy-Item $providerNative $nativeProviderDir -Force
        Copy-Item (Join-Path $root "providers\bluu-native-a2dp\provider.json") $nativeProviderDir -Force
    }
    else {
        Write-Warning "bluu-provider-owb.exe was not built; native provider package omitted"
    }
    Copy-Item (Join-Path $root "README.md") $Output -Force
    Copy-Item (Join-Path $root "LICENSE") $Output -Force
    Copy-Item (Join-Path $root "packaging\THIRD-PARTY-NOTICES.md") $Output -Force
    Copy-Item (Join-Path $root "docs\SECURITY.md") $Output -Force
    Copy-Item (Join-Path $root "docs\TESTING.md") $Output -Force
    Copy-Item (Join-Path $root "docs\NATIVE-A2DP.md") $Output -Force

    $sha = Get-FileHash (Join-Path $Output "bluu.exe") -Algorithm SHA256
    $helperSha = Get-FileHash (Join-Path $Output "bluu-etw-helper.exe") -Algorithm SHA256
    $providerSha = if (Test-Path (Join-Path $Output "bluu-provider-demo.exe")) {
        (Get-FileHash (Join-Path $Output "bluu-provider-demo.exe") -Algorithm SHA256).Hash
    } else { "not included" }
    $nativeProviderSha = if (Test-Path (Join-Path $Output "providers\bluu-native-a2dp\bluu-provider-owb.exe")) {
        (Get-FileHash (Join-Path $Output "providers\bluu-native-a2dp\bluu-provider-owb.exe") -Algorithm SHA256).Hash
    } else { "not included" }
    Copy-Item (Join-Path $root "packaging\THIRD-PARTY-NOTICES.md") $Output -Force
    Copy-Item (Join-Path $root "native\owb\third-party\LICENSES.md") (Join-Path $Output "NATIVE-THIRD-PARTY-NOTICES.md") -Force
    if (Test-Path (Join-Path $root "native\owb\third-party\ffmpeg-aptx\COPYING.LGPL-2.1")) {
        Copy-Item (Join-Path $root "native\owb\third-party\ffmpeg-aptx\COPYING.LGPL-2.1") $Output -Force
    }
    @"
Bluu portable Windows build
Version: see Cargo.toml
Built: $(Get-Date -Format o)
Architecture: x86_64-pc-windows-msvc

bluu.exe SHA-256: $($sha.Hash)
bluu-etw-helper.exe SHA-256: $($helperSha.Hash)
bluu-provider-demo.exe SHA-256: $providerSha
bluu-provider-owb.exe SHA-256: $nativeProviderSha

Run bluu.exe as a standard user. The helper is used only when the user chooses
"Verify live codec" and approves the one-time elevation prompt.

No driver, service, registry modification, or startup entry is installed by this
portable build. The AAC toggle is an explicit in-app action with its own consent.
The optional native provider is included as an uninstalled package; installing its
driver/service requires the separate explicit OpenWinBlue installer and administrator
approval.

Native optional-component notices: NATIVE-THIRD-PARTY-NOTICES.md
FFmpeg aptX LGPL text: COPYING.LGPL-2.1 (when the native package is included)
"@ | Set-Content (Join-Path $Output "BUILD-INFO.txt")

    Compress-Archive -Path (Join-Path $Output "*") -DestinationPath "$Output.zip" -Force
    Write-Host "Portable build written to $Output"
    Write-Host "ZIP: $Output.zip"
}
finally {
    Pop-Location
}
