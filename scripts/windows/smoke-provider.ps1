# smoke-provider.ps1
# No-headset provider validation: manifest identity, hash, containment, and
# expected failure handling. It does not require the native driver or service.

[CmdletBinding()]
param(
    [string] $BluuRoot = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($BluuRoot)) {
    $BluuRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}
$providerDir = Join-Path $BluuRoot 'providers\bluu-native-a2dp'
$manifestPath = Join-Path $providerDir 'provider.json'
$providerExe = Join-Path $providerDir 'bluu-provider-owb.exe'

if (-not (Test-Path $manifestPath)) { throw "Missing provider manifest: $manifestPath" }
if (-not (Test-Path $providerExe)) {
    throw "Missing provider executable: $providerExe`nBuild with: cargo build --release --target x86_64-pc-windows-msvc -p bluu-provider-owb"
}

$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
if ($manifest.id -ne 'bluu.native-a2dp') { throw "Unexpected provider id: $($manifest.id)" }
if ($manifest.executable -ne 'bluu-provider-owb.exe') { throw "Unexpected executable: $($manifest.executable)" }
$manifestKeys = @($manifest.codecs | ForEach-Object { $_.parameters } | ForEach-Object { $_.key })
if (($manifestKeys | Select-Object -Unique).Count -ne $manifestKeys.Count) {
    throw 'Provider manifest contains duplicate parameter keys'
}

$hash = (Get-FileHash $providerExe -Algorithm SHA256).Hash
Write-Host "manifest: $manifestPath"
Write-Host "provider: $providerExe"
Write-Host "sha256: $hash"

$env:BLUU_PROVIDER_DIR = $providerDir
try {
    $exe = Join-Path $BluuRoot 'target\x86_64-pc-windows-msvc\release\bluu.exe'
    if (-not (Test-Path $exe)) {
        throw "Missing bluu.exe at $exe. Build the workspace first."
    }
    & $exe --provider-smoke
    if ($LASTEXITCODE -ne 0) { throw "provider smoke failed with exit $LASTEXITCODE" }
}
finally {
    Remove-Item Env:BLUU_PROVIDER_DIR -ErrorAction SilentlyContinue
}

Write-Host 'Provider package smoke checks passed (no native service required).'
