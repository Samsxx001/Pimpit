param([switch]$Apply)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Update.ps1') -LibraryOnly
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = Join-Path $root 'dist/Pimpit.exe'
$before = Read-RuntimeStatus
if (-not $before) { throw 'Start this project Pimpit before validating its updater.' }
$runtime = Get-Process -Id $before.ProcessId -ErrorAction Stop
if ($runtime.Path -ine $target) { throw "Unexpected running path. Expected $target" }
$installed = [version]('{0}.{1}.{2}' -f [math]::Floor($before.Code / 10000), [math]::Floor(($before.Code % 10000) / 100), ($before.Code % 100))
$repo = 'Samsxx001/Pimpit'
$headers = @{ 'User-Agent' = 'Pimpit-update-verification'; Accept = 'application/vnd.github+json' }
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers $headers -TimeoutSec 30
$asset = Get-UpdateAsset $release $repo $installed
if (-not $asset) { Write-Output "No update: installed $installed is current."; return }
$directory = Join-Path $root ('release/verify-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($directory)
$archive = Join-Path $directory 'package.zip'
$staged = Join-Path ([IO.Path]::GetDirectoryName($target)) ('.Pimpit-verify-' + [guid]::NewGuid().ToString('N') + '.exe')
$closed = $false
$installedOk = $false
$ini = Join-Path $root 'dist/pimpit.ini'
$iniHash = Get-PackageHash $ini
try {
    Invoke-WebRequest -UseBasicParsing -Uri $asset.Url -OutFile $archive -TimeoutSec 120 | Out-Null
    if ((Get-Item -LiteralPath $archive).Length -ne $asset.Size -or (Get-PackageHash $archive) -ine $asset.Hash) { throw 'GitHub package integrity failed.' }
    Expand-UpdateBinary $archive $staged
    $inspector = Join-Path $root '../savehaven-rust/.tools/w64devkit/bin/objdump.exe'
    if (Test-Path -LiteralPath $inspector) {
        $imports = @(& $inspector -p $staged | Select-String 'DLL Name:')
        if ($LASTEXITCODE -ne 0) { throw 'Import inspection failed.' }
        $imports | ForEach-Object { Write-Output $_.Line.Trim() }
        if ($imports -match 'VCRUNTIME|MSVCP[0-9]') { throw 'Package still requires a separate Visual C++ runtime.' }
    }
    if ($Apply) {
        Stop-InstalledRuntime $before.ProcessId $target
        $closed = $true
        Install-UpdateBinary $staged $target { param($path) Start-HealthyRuntime $path $asset.Version }
        $installedOk = $true
        if ((Get-PackageHash $ini) -cne $iniHash) { throw 'Settings changed during update.' }
        if ((Get-PackageHash $target) -cne (Get-PackageHash "$target.previous")) {
            Write-Output 'Executable changed and previous executable was backed up.'
        }
        $after = Read-RuntimeStatus
        [pscustomobject]@{ From = "$installed"; To = "$($asset.Version)"; Applied = $true; SettingsPreserved = $true; RawInputReady = $after.Raw -eq 1; ProcessId = $after.ProcessId; PackageSHA256 = $asset.Hash } | ConvertTo-Json
    } else {
        [pscustomobject]@{ From = "$installed"; To = "$($asset.Version)"; Applied = $false; PackageVerified = $true; PackageSHA256 = $asset.Hash } | ConvertTo-Json
    }
} finally {
    if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged }
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
    Remove-Item -LiteralPath $directory
    if ($closed -and -not $installedOk) { Start-Process -FilePath $target -WorkingDirectory ([IO.Path]::GetDirectoryName($target)) -WindowStyle Hidden }
}
