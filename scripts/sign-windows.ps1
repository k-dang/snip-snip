param([Parameter(Mandatory)][string]$Path)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrEmpty($env:WINDOWS_SIGNING_PFX_BASE64) -or [string]::IsNullOrEmpty($env:WINDOWS_SIGNING_PASSWORD)) {
    throw 'Signing requires WINDOWS_SIGNING_PFX_BASE64 and WINDOWS_SIGNING_PASSWORD.'
}

$binary = (Resolve-Path -LiteralPath $Path).Path
$sdk = Get-ItemPropertyValue 'HKLM:/SOFTWARE/WOW6432Node/Microsoft/Microsoft SDKs/Windows/v10.0' InstallationFolder
$signtool = Get-ChildItem (Join-Path $sdk 'bin/*/x64/signtool.exe') |
    Sort-Object { [version]$_.Directory.Parent.Name } -Descending |
    Select-Object -First 1
if (-not $signtool) { throw 'SignTool was not found in the Windows SDK.' }

$pfxPath = Join-Path ([IO.Path]::GetTempPath()) "snip-snip-signing-$([guid]::NewGuid().ToString('N')).pfx"
$importedKey = $false
$storePath = $null
try {
    [IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($env:WINDOWS_SIGNING_PFX_BASE64))
    $password = ConvertTo-SecureString $env:WINDOWS_SIGNING_PASSWORD -AsPlainText -Force
    $certificates = @((Get-PfxData -FilePath $pfxPath -Password $password).EndEntityCertificates)
    if ($certificates.Count -ne 1 -or $certificates[0].Subject -ne $certificates[0].Issuer) {
        throw 'The PFX must contain one self-signed code-signing certificate.'
    }
    $thumbprint = $certificates[0].Thumbprint
    $storePath = "Cert:/CurrentUser/My/$thumbprint"
    if (Test-Path -LiteralPath $storePath) {
        $certificate = Get-Item -LiteralPath $storePath
    } else {
        $certificate = Import-PfxCertificate -FilePath $pfxPath -Password $password -CertStoreLocation 'Cert:/CurrentUser/My'
        $importedKey = $true
    }
    if (-not $certificate.HasPrivateKey) { throw 'Signing certificate has no private key.' }

    & $signtool.FullName sign /s My /sha1 $thumbprint /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $binary
    if ($LASTEXITCODE -ne 0) { throw 'Signing or timestamping the executable failed.' }

    if (-not ('SnipSigning.Authenticode' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'Authenticode.cs')
    }
    # Accept only the expected untrusted root; all integrity/policy errors still fail.
    $verification = [SnipSigning.Authenticode]::Verify($binary)
    if ($verification -ne 0 -and $verification -ne [SnipSigning.Authenticode]::UntrustedRoot) {
        throw ('Executable signature verification failed: 0x{0:X8}.' -f $verification)
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $binary
    if ($signature.SignerCertificate.Thumbprint -ne $thumbprint -or -not $signature.TimeStamperCertificate) {
        throw 'Executable must have a timestamped signature from the configured certificate.'
    }
    Write-Host "Signed and verified $binary (certificate $thumbprint)."
} finally {
    try {
        if ($importedKey) { Remove-Item -LiteralPath $storePath -DeleteKey }
    } finally {
        if (Test-Path -LiteralPath $pfxPath) { Remove-Item -LiteralPath $pfxPath -Force }
    }
}
