$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Update.ps1') -LibraryOnly
$checks = 0
function Assert-True([bool]$Condition, [string]$Name) {
    if (-not $Condition) { throw "FAILED: $Name" }
    $script:checks++
}
function Assert-Throws([scriptblock]$Action, [string]$Name) {
    $thrown = $false
    try { & $Action | Out-Null } catch { $thrown = $true }
    Assert-True $thrown $Name
}
$hash = 'a' * 64
$asset = [pscustomobject]@{
    name = 'Pimpit-0.2.0-windows-x64.zip'; size = 100000
    browser_download_url = 'https://github.com/test/pimpit/releases/download/v0.2.0/Pimpit-0.2.0-windows-x64.zip'
    digest = "sha256:$hash"
}
$release = [pscustomobject]@{ tag_name = 'v0.2.0'; draft = $false; prerelease = $false; assets = @($asset) }
Assert-True ((Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.9')).Version -eq [version]'0.2.0') 'New stable release'
Assert-True ($null -eq (Get-UpdateAsset $release 'test/pimpit' ([version]'0.2.0'))) 'Same version does not update'
Assert-True ($null -eq (Get-UpdateAsset $release 'test/pimpit' ([version]'0.3.0'))) 'No downgrade'
Assert-Throws { ConvertTo-StableVersion 'v1.0.0-beta' } 'Reject prerelease version'
Assert-Throws { ConvertTo-StableVersion '1.0' } 'Reject incomplete version'
Assert-Throws { ConvertTo-StableVersion '01.2.3' } 'Reject noncanonical version'
$release.prerelease = $true
Assert-Throws { Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.0') } 'Reject prerelease'
$release.prerelease = $false
$asset.browser_download_url = 'https://evil.example/download.exe'
Assert-Throws { Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.0') } 'Reject foreign download URL'
$asset.browser_download_url = 'https://github.com/test/pimpit/releases/download/v0.2.0/Pimpit-0.2.0-windows-x64.zip'
$asset.digest = $null
Assert-Throws { Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.0') } 'Require SHA-256'
$asset.digest = "sha256:$hash"
$asset.size = 100MB
Assert-Throws { Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.0') } 'Reject oversized metadata'
$asset.size = 100000
$release.assets = @($asset, $asset)
Assert-Throws { Get-UpdateAsset $release 'test/pimpit' ([version]'0.1.0') } 'Reject duplicate package assets'

$testDir = Join-Path ([IO.Path]::GetTempPath()) ('Pimpit-updater-test-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($testDir)
try {
    $target = Join-Path $testDir 'Pimpit.exe'
    $staged = Join-Path $testDir 'staged.exe'
    $ini = Join-Path $testDir 'pimpit.ini'
    [IO.File]::WriteAllText($target, 'old')
    Assert-True ((Get-PackageHash $target) -eq 'cba06b5736faf67e54b07b561eae94395e774c517a7d910a54369e1263ccfbd4') 'SHA-256 of file contents'
    [IO.File]::WriteAllText($staged, 'new')
    [IO.File]::WriteAllText($ini, 'personal settings')
    Install-UpdateBinary $staged $target { param($path) if ([IO.File]::ReadAllText($path) -ne 'new') { throw 'Not replaced' } }
    Assert-True ([IO.File]::ReadAllText($target) -eq 'new') 'Install on disk'
    Assert-True ([IO.File]::ReadAllText("$target.previous") -eq 'old') 'Backup previous executable'
    [IO.File]::WriteAllText($staged, 'broken')
    Assert-Throws { Install-UpdateBinary $staged $target { throw 'Simulated startup failure' } } 'Startup failure propagated'
    Assert-True ([IO.File]::ReadAllText($target) -eq 'new') 'Rollback on failed startup'
    Assert-True ([IO.File]::ReadAllText($ini) -eq 'personal settings') 'Preserve settings after install and rollback'
    Assert-Throws { Test-WindowsBinary $target } 'Reject non-PE executable'
    $zipPath = Join-Path $testDir 'bad.zip'
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::Open($zipPath, [IO.Compression.ZipArchiveMode]::Create)
    try { [void]$zip.CreateEntry('../Pimpit.exe') } finally { $zip.Dispose() }
    Assert-Throws { Expand-UpdateBinary $zipPath $staged } 'Reject path traversal package'
} finally {
    # The unique temporary directory above contains only fixtures from this test.
    Get-ChildItem -LiteralPath $testDir -File | Remove-Item
    Remove-Item -LiteralPath $testDir
}
Write-Output "Updater: $checks checks passed (no downloads, no running apps changed)."
