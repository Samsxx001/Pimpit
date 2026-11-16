param(
    [string]$Repository,
    [string]$CurrentVersion,
    [string]$Executable,
    [int]$ParentProcessId,
    [switch]$LibraryOnly
)
$ErrorActionPreference = 'Stop'

function ConvertTo-StableVersion([string]$Value) {
    if ($Value -cnotmatch '^v?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        throw 'Invalid stable release version.'
    }
    return [version]($Value -replace '^v', '')
}

function Get-PackageHash([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $sha.Dispose() }
}

function Get-UpdateAsset($Release, [string]$Repo, [version]$InstalledVersion) {
    if ($Repo -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$' -or $Repo.Split('/') -contains '..') { throw 'Invalid repository.' }
    if ($Release.draft -or $Release.prerelease) { throw 'Only stable published releases are supported.' }
    $version = ConvertTo-StableVersion $Release.tag_name
    if ($version -le $InstalledVersion) { return $null }
    $name = "Pimpit-$version-windows-x64.zip"
    $assets = @($Release.assets | Where-Object { $_.name -ceq $name })
    if ($assets.Count -ne 1) { throw "Release is missing the Windows package: $name" }
    $asset = $assets[0]
    $expectedUrl = "https://github.com/$Repo/releases/download/$($Release.tag_name)/$name"
    if ($asset.browser_download_url -cne $expectedUrl) { throw 'Unexpected release asset URL.' }
    if ($asset.digest -cnotmatch '^sha256:[0-9a-fA-F]{64}$') { throw 'GitHub did not provide a SHA-256 digest. Re-upload the release asset.' }
    if ($asset.size -lt 64KB -or $asset.size -gt 64MB) { throw 'Unexpected package size.' }
    return [pscustomobject]@{ Version = $version; Url = $expectedUrl; Hash = $asset.digest.Substring(7); Size = [long]$asset.size }
}

function Test-WindowsBinary([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = New-Object IO.BinaryReader($stream)
    try {
        if ($stream.Length -lt 256 -or $stream.Length -gt 32MB -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'Invalid executable.' }
        $stream.Position = 0x3c
        $offset = $reader.ReadUInt32()
        if ($offset -lt 64 -or $offset -gt $stream.Length - 24) { throw 'Invalid PE header.' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0x8664) { throw 'Expected a Windows x64 executable.' }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

function Expand-UpdateBinary([string]$Archive, [string]$Destination) {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        # Extract exactly one named file, never paths supplied by the ZIP.
        $entries = @($zip.Entries | Where-Object { $_.FullName -ceq 'Pimpit.exe' })
        if ($entries.Count -ne 1 -or $entries[0].Length -gt 32MB -or $entries[0].Length -lt 256) { throw 'Invalid update package layout.' }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], $Destination, $false)
    } finally { $zip.Dispose() }
    Test-WindowsBinary $Destination
}

function Read-RuntimeStatus {
    $mapping = $null
    try {
        try { $mapping = [IO.MemoryMappedFiles.MemoryMappedFile]::OpenExisting('Local\RightLangRust.Status-0.1') }
        catch [IO.FileNotFoundException] { return $null }
        $view = $mapping.CreateViewAccessor(0, 4096)
        try {
            return [pscustomobject]@{ Code = $view.ReadUInt32(0); ProcessId = $view.ReadUInt32(4); Raw = $view.ReadUInt32(28); Stage = $view.ReadUInt32(40) }
        } finally { $view.Dispose() }
    } finally { if ($mapping) { $mapping.Dispose() } }
}

function Stop-InstalledRuntime([int]$ProcessId, [string]$Path) {
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
    if ($process.Path -ine $Path) { throw 'Running executable path changed. Update cancelled.' }
    $mapping = [IO.MemoryMappedFiles.MemoryMappedFile]::OpenExisting('Local\RightLangRust.Status-0.1')
    try {
        $view = $mapping.CreateViewAccessor(0, 4096)
        try {
            if ($view.ReadUInt32(4) -ne $ProcessId) { throw 'Runtime changed. Update cancelled.' }
            $view.Write(60, [int]1)
        } finally { $view.Dispose() }
    } finally { $mapping.Dispose() }
    if (-not $process.WaitForExit(12000)) { throw 'Program did not close safely. The installed version was not replaced.' }
}

function Start-HealthyRuntime([string]$Path, [version]$Version) {
    $process = Start-Process -FilePath $Path -WorkingDirectory ([IO.Path]::GetDirectoryName($Path)) -WindowStyle Hidden -PassThru
    $healthy = $false
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        $code = $Version.Major * 10000 + $Version.Minor * 100 + $Version.Build
        while ([DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
            if ($process.HasExited) { throw 'Updated version exited during startup.' }
            $status = Read-RuntimeStatus
            if ($status -and $status.ProcessId -eq $process.Id -and $status.Code -eq $code -and $status.Raw -eq 1 -and $status.Stage -ge 12) {
                $healthy = $true
                return
            }
        }
        throw 'Updated version did not become ready.'
    } finally {
        # Only the exact process we just created can be stopped for rollback.
        if (-not $healthy -and -not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(5000) }
        $process.Dispose()
    }
}

function Install-UpdateBinary([string]$Staged, [string]$Target, [scriptblock]$Verify) {
    $backup = "$Target.previous"
    [IO.File]::Replace($Staged, $Target, $backup)
    try { & $Verify $Target }
    catch {
        [IO.File]::Replace($backup, $Target, $Staged)
        throw
    }
}

function Show-UpdateMessage([string]$Text, [int]$Buttons = 64) {
    $shell = New-Object -ComObject WScript.Shell
    try { return $shell.Popup($Text, 0, 'Pimpit - Update', $Buttons) }
    finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
}

function Invoke-GitHubUpdate {
    $installed = ConvertTo-StableVersion $CurrentVersion
    if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$' -or $Repository.Split('/') -contains '..') { throw 'Invalid repository.' }
    $target = [IO.Path]::GetFullPath($Executable)
    if (-not (Test-Path -LiteralPath $target -PathType Leaf)) { throw 'Installed executable not found.' }
    $headers = @{ 'User-Agent' = "Pimpit/$CurrentVersion"; Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28' }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest" -Headers $headers -TimeoutSec 30
    $asset = Get-UpdateAsset $release $Repository $installed
    if (-not $asset) { [void](Show-UpdateMessage "คุณใช้รุ่นล่าสุดแล้ว ($CurrentVersion)"); return }
    $answer = Show-UpdateMessage "พบ Pimpit $($asset.Version) (รุ่นปัจจุบัน $CurrentVersion)`nแหล่งอัปเดต: github.com/$Repository`n`nดาวน์โหลดและติดตั้งหรือไม่? โปรแกรมจะเปิดใหม่และเก็บ pimpit.ini เดิมไว้" 36
    if ($answer -ne 6) { return }

    $download = Join-Path $PSScriptRoot 'release.zip'
    $staged = Join-Path ([IO.Path]::GetDirectoryName($target)) ('.Pimpit-update-' + [guid]::NewGuid().ToString('N') + '.exe')
    $closed = $false
    $installedOk = $false
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $asset.Url -Headers $headers -OutFile $download -TimeoutSec 120 | Out-Null
        if ((Get-Item -LiteralPath $download).Length -ne $asset.Size -or (Get-PackageHash $download) -ine $asset.Hash) { throw 'Downloaded package failed SHA-256/size verification.' }
        # Stage on the same volume and verify before asking the running app to exit.
        Expand-UpdateBinary $download $staged
        Stop-InstalledRuntime $ParentProcessId $target
        $closed = $true
        Install-UpdateBinary $staged $target { param($path) Start-HealthyRuntime $path $asset.Version }
        $installedOk = $true
    } finally {
        if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged }
        if ($closed -and -not $installedOk) { Start-Process -FilePath $target -WorkingDirectory ([IO.Path]::GetDirectoryName($target)) -WindowStyle Hidden }
    }
    [void](Show-UpdateMessage "อัปเดตเป็น $($asset.Version) แล้ว`nเก็บค่าตั้งค่าเดิมและสำรอง exe รุ่นก่อนเป็น .previous")
}

if ($LibraryOnly) { return }
$mutex = New-Object Threading.Mutex($false, 'Local\RightLangRust.Update-0.1')
$ownsLock = $false
try {
    try { $ownsLock = $mutex.WaitOne(0) } catch [Threading.AbandonedMutexException] { $ownsLock = $true }
    if (-not $ownsLock) { [void](Show-UpdateMessage 'มีหน้าต่างอัปเดตเปิดอยู่แล้ว'); return }
    Invoke-GitHubUpdate
} catch {
    [void](Show-UpdateMessage "อัปเดตไม่สำเร็จ`n$($_.Exception.Message)`n`nตรวจอินเทอร์เน็ตและ repository ที่เผยแพร่ Release แล้วลองใหม่" 16)
} finally {
    if ($ownsLock) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
    # Remove only known files in this unique helper directory; never recursively delete.
    if ($PSScriptRoot -like (Join-Path ([IO.Path]::GetTempPath()) 'Pimpit-update-*')) {
        foreach ($name in @('release.zip', 'Update.ps1')) {
            $file = Join-Path $PSScriptRoot $name
            if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -ErrorAction SilentlyContinue }
        }
        Remove-Item -LiteralPath $PSScriptRoot -ErrorAction SilentlyContinue
    }
}
