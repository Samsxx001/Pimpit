param([Parameter(Mandatory=$true)][string]$PreviousRustPath)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$newExecutable = Join-Path $projectRoot 'dist/Pimpit.exe'
$originalPath = 'C:\Program Files (x86)\RightLang\rightlang.exe'
$currentSession = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
$rustProcesses = @(Get-Process -Name Pimpit,RightLang-Rust -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $currentSession })
foreach ($rustProcess in $rustProcesses) {
    if ($rustProcess.Path -ne $PreviousRustPath -and $rustProcess.Path -ne $newExecutable) {
        throw "Unexpected Rust executable path: $($rustProcess.Path). No program was stopped."
    }
}
$originalProcesses = @(Get-Process -Name rightlang -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $currentSession })
foreach ($originalProcess in $originalProcesses) {
    if ($originalProcess.Path -ne $originalPath) { throw 'Cannot verify original RightLang path. No program was stopped.' }
}
if (-not (Test-Path -LiteralPath $newExecutable)) { throw 'Build the fixed executable first.' }
try {
    if ($rustProcesses.Count -gt 0) {
        try { & (Join-Path $PSScriptRoot 'Get-PimpitStatus.ps1') -Close } catch {
            if ($_.Exception.Message -ne 'Pimpit is not running.') { throw }
            # The old process can be alive without a usable hidden control window.
            # Its exact executable path was checked above before this fallback.
        }
        foreach ($rustProcess in $rustProcesses) {
            if (-not $rustProcess.WaitForExit(3000)) { Stop-Process -Id $rustProcess.Id -ErrorAction Stop; $rustProcess.WaitForExit() }
        }
    }
    foreach ($originalProcess in $originalProcesses) { Stop-Process -Id $originalProcess.Id -ErrorAction Stop; [void]$originalProcess.WaitForExit(5000) }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/pimpit.exe') -Destination $newExecutable
    $newProcess = Start-Process -FilePath $newExecutable -WorkingDirectory (Split-Path $newExecutable -Parent) -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 2
    $newProcess.Refresh()
    if ($newProcess.HasExited) { throw 'Fixed version exited during startup.' }
    & (Join-Path $PSScriptRoot 'Get-PimpitStatus.ps1')
} catch {
    if ($originalProcesses.Count -gt 0 -and -not (Get-Process -Name rightlang -ErrorAction SilentlyContinue)) {
        Start-Process -FilePath $originalPath -WorkingDirectory (Split-Path $originalPath -Parent) -WindowStyle Hidden
    }
    throw
}
