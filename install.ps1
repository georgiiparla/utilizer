# Install or update Utilizer for the current Windows user. No administrator needed.
$ErrorActionPreference = 'Stop'
$repository = 'georgiiparla/utilizer'
$assetName = 'utilizer-windows-x86_64.exe'
$installDirectory = Join-Path $env:LOCALAPPDATA 'Programs\Utilizer'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or -not [Environment]::Is64BitOperatingSystem) {
    throw 'Utilizer requires 64-bit Windows.'
}
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
    throw 'This release targets Windows x64. Native ARM64 is not supported yet.'
}
if (Get-Process -Name utilizer -ErrorAction SilentlyContinue) {
    throw 'Close the Utilizer console before installing or updating.'
}
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('utilizer-install-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
$utilizerPreviousProgress = $ProgressPreference
$ProgressPreference = 'Continue'
function Write-UtilizerStep {
    param([int]$Step, [string]$Message)
    Write-Host "  [$Step/5] " -ForegroundColor DarkYellow -NoNewline
    Write-Host $Message
    Write-Progress -Id 47825 -Activity 'Installing Utilizer' -Status $Message -PercentComplete (($Step - 1) * 20)
}
try {
    Write-Host ''
    Write-Host '  /\ /\' -ForegroundColor DarkYellow
    Write-Host '  < X >  Utilizer' -ForegroundColor Yellow
    Write-Host '  \/ \/' -ForegroundColor DarkYellow
    Write-Host ''
    Write-UtilizerStep 1 'Checking latest release'
    $headers = @{ 'User-Agent' = 'Utilizer-Installer'; 'Accept' = 'application/vnd.github+json' }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/releases/latest" -Headers $headers
    $executableAsset = @($release.assets | Where-Object name -eq $assetName)
    $checksumAsset = @($release.assets | Where-Object name -eq 'SHA256SUMS')
    if ($executableAsset.Count -ne 1 -or $checksumAsset.Count -ne 1) {
        throw 'The latest release is missing its executable or checksum. Nothing was installed.'
    }
    $download = Join-Path $temporaryDirectory $assetName
    $checksums = Join-Path $temporaryDirectory 'SHA256SUMS'
    Write-UtilizerStep 2 "Downloading $($release.tag_name)"
    Invoke-WebRequest -UseBasicParsing -Uri $executableAsset[0].browser_download_url -OutFile $download
    Invoke-WebRequest -UseBasicParsing -Uri $checksumAsset[0].browser_download_url -OutFile $checksums
    Write-UtilizerStep 3 'Verifying SHA-256'
    $lines = @(Get-Content -LiteralPath $checksums | Where-Object { $_ -match '^[a-fA-F0-9]{64}  utilizer-windows-x86_64\.exe$' })
    if ($lines.Count -ne 1) { throw 'The release checksum file is invalid.' }
    $expected = $lines[0].Substring(0, 64)
    if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $expected) {
        throw 'Download checksum did not match. Nothing was installed.'
    }
    Write-UtilizerStep 4 'Installing executable'
    New-Item -ItemType Directory -Path $installDirectory -Force | Out-Null
    $destination = Join-Path $installDirectory 'utilizer.exe'
    Copy-Item -LiteralPath $download -Destination $destination -Force
    Write-UtilizerStep 5 'Updating user PATH'
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    if ($entries -notcontains $installDirectory) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $installDirectory) -join ';'), 'User')
    }
    if (($env:Path -split ';') -notcontains $installDirectory) { $env:Path += ";$installDirectory" }
    Write-Host "`n  Installed $($release.tag_name)" -ForegroundColor Green
    Write-Host "  $destination"
    Write-Host '  Run: utilizer' -ForegroundColor Yellow
} finally {
    Write-Progress -Id 47825 -Activity 'Installing Utilizer' -Completed
    $ProgressPreference = $utilizerPreviousProgress
    # Remove only the unique temporary directory created by this script.
    $resolvedTemporary = [IO.Path]::GetFullPath($temporaryDirectory)
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($resolvedTemporary.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolvedTemporary) -match '^utilizer-install-[0-9a-f]{32}$') {
        Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force
    }
}
