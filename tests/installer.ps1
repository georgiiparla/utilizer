param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$fixture = (Resolve-Path -LiteralPath $Executable).Path
$installer = Join-Path $PSScriptRoot '..\install.ps1'
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
$global:UtilizerTestBadChecksum = $false
$global:UtilizerTestRunning = $false
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
    & $installer
    $installed = Join-Path $sandbox 'Programs\Utilizer\utilizer.exe'
    $expected = (Get-FileHash -LiteralPath $fixture).Hash
    if ((Get-FileHash -LiteralPath $installed).Hash -ne $expected) { throw 'Installed bytes differ' }
    & $installer
    $entries = @([InstallerTestEnvironment]::UserPath -split ';' | Where-Object { $_ })
    if ($entries.Count -ne 1) { throw 'PATH entry duplicated during update' }
    $global:UtilizerTestBadChecksum = $true
    $rejected = $false
    try { & $installer } catch { $rejected = $_.ToString() -match 'checksum did not match' }
    if (-not $rejected) { throw 'Corrupt download was not rejected' }
    if ((Get-FileHash -LiteralPath $installed).Hash -ne $expected) { throw 'Failed update damaged existing installation' }
    $global:UtilizerTestRunning = $true
    $rejected = $false
    try { & $installer } catch { $rejected = $_.ToString() -match 'Close the Utilizer console' }
    if (-not $rejected) { throw 'Running executable was not protected' }
    if (@(Get-ChildItem -LiteralPath $env:TEMP).Count -ne 0) { throw 'Temporary downloads were not cleaned up' }
    if ([System.Environment]::GetEnvironmentVariable('Path', 'User') -ne $originalPath) { throw 'Real user PATH changed' }
    Write-Host 'PASS: installer, repeat update, PATH deduplication, checksum rejection, running-process guard, and cleanup'
} finally {
    [void]$accelerators::Remove('Environment')
    $resolved = [IO.Path]::GetFullPath($sandbox)
    if ($resolved.StartsWith($targetRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolved) -match '^installer-test-[0-9a-f]{32}$') {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
