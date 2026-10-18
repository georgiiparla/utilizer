param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$fixture = (Resolve-Path -LiteralPath $Executable).Path
$installer = Join-Path $PSScriptRoot '..\install.ps1'
# Exercise the private embedded helper for filesystem/PATH edge cases; the
# actual installed CLI and self-removal are covered by tests/uninstall.rs.
$uninstallSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\src\uninstall.rs') -Raw
$embedded = [regex]::Match($uninstallSource, '(?s)const SCRIPT: &str = r###"(.*?)"###;')
if (-not $embedded.Success) { throw 'Cannot locate the embedded uninstall helper' }
$targetRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\target'))
$sandbox = Join-Path $targetRoot ('installer-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $sandbox 'tmp') -Force | Out-Null
$env:LOCALAPPDATA = $sandbox
$env:TEMP = Join-Path $sandbox 'tmp'
$env:TMP = $env:TEMP
$originalPath = [System.Environment]::GetEnvironmentVariable('Path', 'User')

# Test the real script, replacing network/process discovery and the persistent
# environment writer. This process cannot alter the real user's PATH.
Add-Type @'
public static class InstallerTestEnvironment {
    public static System.OperatingSystem OSVersion { get { return System.Environment.OSVersion; } }
    public static bool Is64BitOperatingSystem { get { return true; } }
    public static string UserPath = "";
    public static string GetEnvironmentVariable(string key, string target) { return UserPath; }
    public static void SetEnvironmentVariable(string key, string value, string target) { UserPath = value; }
}
'@
$accelerators = [psobject].Assembly.GetType('System.Management.Automation.TypeAccelerators')
$accelerators::Add('Environment', [InstallerTestEnvironment])
if ((& ([scriptblock]::Create('[Environment]'))) -ne [InstallerTestEnvironment]) { throw 'Test environment isolation failed' }
$uninstaller = [scriptblock]::Create($embedded.Groups[1].Value)
$global:UtilizerTestBadChecksum = $false
$global:UtilizerTestRunning = $false
$global:UtilizerTestProgress = [Collections.Generic.List[string]]::new()
function Write-Progress {
    param($Id, $Activity, $Status, $PercentComplete, [switch]$Completed)
    if ($Id -ne 47825) { throw 'Unexpected installer progress ID' }
    if ($Completed) { $global:UtilizerTestProgress.Add('completed') }
    else { $global:UtilizerTestProgress.Add($Status) }
}
function Get-Process { param($Name, $ErrorAction) if ($global:UtilizerTestRunning) { [pscustomobject]@{ Name = 'utilizer' } } }
function Invoke-RestMethod {
    param($Uri, $Headers)
    [pscustomobject]@{ tag_name = 'v0.2.0'; assets = @(
        [pscustomobject]@{ name = 'utilizer-windows-x86_64.exe'; browser_download_url = 'https://fixture/exe' },
        [pscustomobject]@{ name = 'SHA256SUMS'; browser_download_url = 'https://fixture/hash' }
    ) }
}
function Invoke-WebRequest {
    param([switch]$UseBasicParsing, $Uri, $OutFile)
    if ($Uri -eq 'https://fixture/exe') { Copy-Item -LiteralPath $fixture -Destination $OutFile }
    elseif ($Uri -eq 'https://fixture/hash') {
        $hash = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash
        if ($global:UtilizerTestBadChecksum) { $hash = '0' * 64 }
        "$hash  utilizer-windows-x86_64.exe" | Set-Content -LiteralPath $OutFile -Encoding ascii
    } else { throw 'Unexpected network request' }
}
try {
    $ProgressPreference = 'SilentlyContinue'
    # Exercise the same scope as irm | iex, including preference restoration.
    Invoke-Expression ([IO.File]::ReadAllText($installer))
    if ($ProgressPreference -ne 'SilentlyContinue') { throw 'Installer changed the caller progress preference' }
    $expectedProgress = 'Checking latest release|Downloading v0.2.0|Verifying SHA-256|Installing executable|Updating user PATH|completed'
    if (($global:UtilizerTestProgress -join '|') -ne $expectedProgress) { throw 'Installer stages missing or out of order' }
    $installed = Join-Path $sandbox 'Programs\Utilizer\utilizer.exe'
    $expected = (Get-FileHash -LiteralPath $fixture).Hash
    if ((Get-FileHash -LiteralPath $installed).Hash -ne $expected) { throw 'Installed bytes differ' }
    & $installer
    $entries = @([InstallerTestEnvironment]::UserPath -split ';' | Where-Object { $_ })
    if ($entries.Count -ne 1) { throw 'PATH entry duplicated during update' }
    $global:UtilizerTestBadChecksum = $true
    $global:UtilizerTestProgress.Clear()
    $rejected = $false
    try { & $installer } catch { $rejected = $_.ToString() -match 'checksum did not match' }
    if (-not $rejected) { throw 'Corrupt download was not rejected' }
    if (($global:UtilizerTestProgress -join '|') -ne 'Checking latest release|Downloading v0.2.0|Verifying SHA-256|completed') {
        throw 'Failed verification reported installation progress or left progress open'
    }
    if ((Get-FileHash -LiteralPath $installed).Hash -ne $expected) { throw 'Failed update damaged existing installation' }
    $global:UtilizerTestRunning = $true
    $rejected = $false
    try { & $installer } catch { $rejected = $_.ToString() -match 'Close the Utilizer console' }
    if (-not $rejected) { throw 'Running executable was not protected' }
    if (@(Get-ChildItem -LiteralPath $env:TEMP -Filter 'utilizer-install-*').Count -ne 0) {
        throw 'Temporary downloads were not cleaned up'
    }

    $rejected = $false
    try { & $uninstaller } catch { $rejected = $_.ToString() -match 'Close the Utilizer console' }
    if (-not $rejected -or -not (Test-Path -LiteralPath $installed)) { throw 'Uninstall did not protect a running executable' }
    $global:UtilizerTestRunning = $false
    $data = Join-Path $sandbox 'Utilizer'
    New-Item -ItemType Directory -Path $data | Out-Null
    'playlist data' | Set-Content -LiteralPath (Join-Path $data 'utilizer.sqlite3')
    'unrelated' | Set-Content -LiteralPath (Join-Path $data 'keep.txt')
    'unrelated' | Set-Content -LiteralPath (Join-Path (Split-Path $installed) 'keep.txt')
    $installDirectory = Split-Path $installed
    [InstallerTestEnvironment]::UserPath = 'C:\Keep;' + $installDirectory + ';"' + $installDirectory + '\";C:\KeepToo'
    & $uninstaller
    if (Test-Path -LiteralPath $installed) { throw 'Uninstaller left the executable' }
    if (-not (Test-Path -LiteralPath (Join-Path $data 'utilizer.sqlite3'))) { throw 'Default uninstall removed playlists' }
    if ([InstallerTestEnvironment]::UserPath -cne 'C:\Keep;C:\KeepToo') { throw 'Uninstaller damaged PATH or left installation entries' }
    & $uninstaller
    if (-not (Test-Path -LiteralPath (Join-Path $data 'keep.txt')) -or
        -not (Test-Path -LiteralPath (Join-Path $installDirectory 'keep.txt'))) { throw 'Uninstaller removed unrelated files' }

    # A junction in the installation path must be refused before deleting files.
    $realDirectory = Join-Path $sandbox 'linked-install-target'
    foreach ($candidate in @($installDirectory, $realDirectory)) {
        if (-not [IO.Path]::GetFullPath($candidate).StartsWith($sandbox + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Junction test target escaped its sandbox'
        }
    }
    Move-Item -LiteralPath $installDirectory -Destination $realDirectory
    'sentinel' | Set-Content -LiteralPath (Join-Path $realDirectory 'utilizer.exe')
    New-Item -ItemType Junction -Path $installDirectory -Target $realDirectory | Out-Null
    try {
        $rejected = $false
        try { & $uninstaller } catch { $rejected = $_.ToString() -match 'Refusing to uninstall through a link' }
        if (-not $rejected -or -not (Test-Path -LiteralPath (Join-Path $realDirectory 'utilizer.exe'))) {
            throw 'Uninstaller followed a junction'
        }
    } finally {
        [IO.Directory]::Delete($installDirectory)
    }
    New-Item -ItemType Directory -Path $installDirectory | Out-Null
    'app file' | Set-Content -LiteralPath (Join-Path $installDirectory 'utilizer.exe')
    New-Item -ItemType Directory -Path (Join-Path $data 'cache\nested') -Force | Out-Null
    'cache' | Set-Content -LiteralPath (Join-Path $data 'cache\nested\entry')
    # A nested junction must also be refused before any purge deletion begins.
    $junction = Join-Path $data 'linked-cache'
    New-Item -ItemType Junction -Path $junction -Target $realDirectory | Out-Null
    try {
        $rejected = $false
        try { & $uninstaller -Purge } catch { $rejected = $_.ToString() -match 'Refusing to uninstall through a link' }
        if (-not $rejected -or -not (Test-Path -LiteralPath (Join-Path $installDirectory 'utilizer.exe'))) {
            throw 'Purge followed a nested junction or deleted before preflight completed'
        }
    } finally {
        [IO.Directory]::Delete($junction)
    }
    & $uninstaller -Purge
    if ((Test-Path -LiteralPath $data) -or (Test-Path -LiteralPath $installDirectory)) { throw 'Purge left an owned application folder' }
    if (-not (Test-Path -LiteralPath (Join-Path $realDirectory 'utilizer.exe'))) { throw 'Purge removed files outside its owned folders' }
    & $uninstaller -Purge
    if ([System.Environment]::GetEnvironmentVariable('Path', 'User') -ne $originalPath) { throw 'Real user PATH changed' }
    Write-Host 'PASS: install/update, checksum rejection, running-process guards, uninstall/repeat, PATH cleanup, full purge, unrelated-file protection, and junction refusal'
} finally {
    [void]$accelerators::Remove('Environment')
    $resolved = [IO.Path]::GetFullPath($sandbox)
    if ($resolved.StartsWith($targetRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolved) -match '^installer-test-[0-9a-f]{32}$') {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
