$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:OS -ne 'Windows_NT') {
    throw 'Build the Windows ZIP on Windows with the MSVC toolchain and Windows SDK installed.'
}

$repo = Split-Path -Parent $PSScriptRoot
$releaseTarget = 'x86_64-pc-windows-msvc'
$previousRustflags = $env:RUSTFLAGS
Push-Location $repo
try {
    # An explicit target keeps static CRT flags off host tools such as proc macros.
    $env:RUSTFLAGS = "$previousRustflags -C target-feature=+crt-static".Trim()
    & cargo build --release --locked --target $releaseTarget -p snip-gpui
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }

    $metadata = (& cargo metadata --no-deps --format-version 1) | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Reading Cargo metadata failed.' }
    $version = ($metadata.packages | Where-Object name -eq 'snip-gpui').version
    $dist = Join-Path $metadata.target_directory 'dist'
    New-Item -ItemType Directory -Path $dist -Force | Out-Null

    $binary = Join-Path $metadata.target_directory "$releaseTarget/release/snip-snip.exe"
    $zip = Join-Path $dist "snip-snip-$version-windows-x64.zip"
    Compress-Archive -LiteralPath @(
        $binary,
        'crates/snip-core/fonts/Inter-LICENSE.txt'
    ) -DestinationPath $zip -Force
    Get-Item -LiteralPath $zip
    Get-FileHash -LiteralPath $zip -Algorithm SHA256
} finally {
    $env:RUSTFLAGS = $previousRustflags
    Pop-Location
}
