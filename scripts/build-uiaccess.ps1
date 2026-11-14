[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9A-Fa-f]{40}$')]
    [string]$CertificateThumbprint,

    [Parameter(Mandatory = $true)]
    [uri]$TimestampUrl,

    [ValidateSet('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')]
    [string]$Target = 'x86_64-pc-windows-msvc',

    [ValidateNotNullOrEmpty()]
    [ValidateSet('nsis', 'msi')]
    [string[]]$Bundles = @('nsis', 'msi')
)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'This build requires Windows.' }
if ($TimestampUrl.Scheme -notin @('http', 'https')) {
    throw 'TimestampUrl must be an HTTP(S) RFC 3161 timestamp server.'
}

# Use an existing certificate only. This script never creates certificates or changes trust stores.
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
if (-not $certificate.HasPrivateKey) { throw 'The certificate has no accessible private key.' }
if ($certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) {
    throw 'The signing certificate is not currently valid.'
}
$codeSigningOid = '1.3.6.1.5.5.7.3.3'
$enhancedKeyUsages = @($certificate.Extensions | Where-Object {
    $_ -is [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]
} | ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
if ($codeSigningOid -notin $enhancedKeyUsages) {
    throw 'The certificate must include the Code Signing enhanced key usage.'
}
$chain = New-Object System.Security.Cryptography.X509Certificates.X509Chain
try {
    if (-not $chain.Build($certificate)) {
        throw 'The signing certificate chain is not trusted/valid on this build machine.'
    }
} finally { $chain.Dispose() }

$uniqueBundles = @($Bundles | ForEach-Object { $_.ToLowerInvariant() } | Select-Object -Unique)

$projectRoot = Split-Path -Parent $PSScriptRoot
$targetDirectory = Join-Path $projectRoot 'src-tauri\target\uiaccess'
$configurationPath = Join-Path ([System.IO.Path]::GetTempPath()) ("wpz-uiaccess-{0}.json" -f [guid]::NewGuid())
$previousUiAccess = $env:WPZ_UIACCESS
$previousTargetDirectory = $env:CARGO_TARGET_DIR
$buildConfiguration = @{
    bundle = @{
        windows = @{
            certificateThumbprint = $CertificateThumbprint
            digestAlgorithm = 'sha256'
            timestampUrl = $TimestampUrl.AbsoluteUri
            tsp = $true
        }
    }
}

function Assert-SignedArtifact {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$ExpectedThumbprint
    )

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if (
        $signature.Status -ne 'Valid' -or
        $null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Thumbprint -ne $ExpectedThumbprint
    ) {
        throw "Signature verification failed for $Path (status: $($signature.Status))."
    }
    if ($null -eq $signature.TimeStamperCertificate) {
        throw "Missing trusted timestamp on $Path."
    }
}

Push-Location $projectRoot
try {
    # Windows PowerShell 5.1's Set-Content -Encoding UTF8 adds a BOM; JSON parsers
    # used by the CLI may reject it. Write explicit BOM-free UTF-8 on both PS versions.
    $configurationJson = $buildConfiguration | ConvertTo-Json -Depth 6
    [System.IO.File]::WriteAllText(
        $configurationPath,
        $configurationJson,
        (New-Object System.Text.UTF8Encoding($false))
    )
    $env:WPZ_UIACCESS = '1'
    # Isolate artifacts so a normal dev/release build cannot silently reuse this manifest.
    $env:CARGO_TARGET_DIR = $targetDirectory
    & npm.cmd run tauri -- build --target $Target --bundles ($uniqueBundles -join ',') --config $configurationPath
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed with exit code $LASTEXITCODE." }

    $releaseDirectory = Join-Path $targetDirectory "$Target\release"

    $bundleSpecs = @{
        nsis = @{ Subdir = 'bundle\nsis'; Filter = '*.exe' }
        msi  = @{ Subdir = 'bundle\msi';  Filter = '*.msi' }
    }
    $signedDirectories = @()
    foreach ($bundle in $uniqueBundles) {
        $spec = $bundleSpecs[$bundle]
        $bundleDirectory = Join-Path $releaseDirectory $spec.Subdir
        $installers = @(Get-ChildItem -LiteralPath $bundleDirectory -Filter $spec.Filter -File)
        if ($installers.Count -lt 1) {
            throw "No $($bundle.ToUpperInvariant()) installer was produced in $bundleDirectory."
        }
        foreach ($installer in $installers) {
            Assert-SignedArtifact -Path $installer.FullName -ExpectedThumbprint $CertificateThumbprint
            if ($bundle -eq 'msi') {
                $extractDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("wpz-msi-verify-{0}" -f [guid]::NewGuid())
                try {
                    $msiProcess = Start-Process -FilePath 'msiexec.exe' -ArgumentList @(
                        '/a', "`"$($installer.FullName)`"",
                        '/qn', "TARGETDIR=`"$extractDirectory`""
                    ) -Wait -PassThru -NoNewWindow
                    if ($msiProcess.ExitCode -ne 0) {
                        throw "Failed to extract MSI $($installer.FullName) for binary signature verification (exit code: $($msiProcess.ExitCode))."
                    }
                    $extractedExecutables = @(Get-ChildItem -LiteralPath $extractDirectory -Filter 'window-pointer-zoom.exe' -File -Recurse)
                    if ($extractedExecutables.Count -lt 1) {
                        throw "Packed executable window-pointer-zoom.exe was not found inside $($installer.FullName)."
                    }
                    foreach ($extractedExecutable in $extractedExecutables) {
                        Assert-SignedArtifact -Path $extractedExecutable.FullName -ExpectedThumbprint $CertificateThumbprint
                    }
                } finally {
                    Remove-Item -LiteralPath $extractDirectory -Recurse -Force -ErrorAction SilentlyContinue
                }
            }
        }
        $signedDirectories += $bundleDirectory
    }
    Write-Output "Signed installers: $($signedDirectories -join ', ')"
    Write-Output 'Install into Program Files, then verify the running input mapping status. Do not launch the build-folder executable.'
} finally {
    $env:WPZ_UIACCESS = $previousUiAccess
    $env:CARGO_TARGET_DIR = $previousTargetDirectory
    Remove-Item -LiteralPath $configurationPath -Force -ErrorAction SilentlyContinue
    Pop-Location
}
