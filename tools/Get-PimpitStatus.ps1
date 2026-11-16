param([switch]$Close)
$ErrorActionPreference = 'Stop'
$statusMapping = $null
try { $statusMapping = [System.IO.MemoryMappedFiles.MemoryMappedFile]::OpenExisting('Local\RightLangRust.Status-0.1') } catch [System.IO.FileNotFoundException] { }
if ($null -ne $statusMapping) {
    try {
        $view = $statusMapping.CreateViewAccessor(0, 4096)
        try {
            if ($Close) { $view.Write(60, [int]1); return }
            $values = @(0..14 | ForEach-Object { $view.ReadUInt32($_ * 4) })
            $runtimeProcess = Get-Process -Id $values[1] -ErrorAction Stop
            [pscustomobject]@{
                version = "{0}.{1}.{2}" -f [math]::Floor($values[0] / 10000), [math]::Floor(($values[0] % 10000) / 100), ($values[0] % 100)
                pid = $values[1]
                path = $runtimeProcess.Path
                raw_events = $values[2]
                manual_requests = $values[3]
                conversions = $values[4]
                last_result = $values[5]
                shift_backspace_registered = ($values[6] -band 32) -ne 0
                registered_shortcuts = $values[6]
                raw_input_registered = $values[7] -ne 0
                failed_batches = $values[8]
                heartbeat_tick = $values[9]
                startup_stage = $values[10]
                auto_enabled = $values[11] -ne 0
                auto_conversions = $values[12]
                game_active = $values[13] -ne 0
                auto_effective = $values[14] -ne 0
                working_set_mb = [math]::Round($runtimeProcess.WorkingSet64 / 1MB, 2)
                private_mb = [math]::Round($runtimeProcess.PrivateMemorySize64 / 1MB, 2)
            } | ConvertTo-Json
            return
        } finally { $view.Dispose() }
    } finally { $statusMapping.Dispose() }
}
if (-not ('RightLangRuntimeControl' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RightLangRuntimeControl {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string className, string title);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wp, IntPtr lp, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wp, IntPtr lp);
}
'@
}
$runtimeWindow = [RightLangRuntimeControl]::FindWindow('RightLangRustWindow', $null)
if ($runtimeWindow -eq [IntPtr]::Zero) { throw 'Pimpit is not running.' }
[uint32]$runtimeProcessId = 0
[void][RightLangRuntimeControl]::GetWindowThreadProcessId($runtimeWindow, [ref]$runtimeProcessId)
if ($Close) {
    [void][RightLangRuntimeControl]::PostMessage($runtimeWindow, 0x0010, [UIntPtr]::Zero, [IntPtr]::Zero)
    return
}
$values = @()
foreach ($index in 0..7) {
    $answer = [UIntPtr]::Zero
    $sent = [RightLangRuntimeControl]::SendMessageTimeout($runtimeWindow, 0x8032, [UIntPtr]$index, [IntPtr]::Zero, 2, 1500, [ref]$answer)
    if ($sent -eq [IntPtr]::Zero) { throw 'Pimpit is not responding.' }
    $values += $answer.ToUInt64()
}
$runtimeProcess = Get-Process -Id $runtimeProcessId
[pscustomobject]@{
    version = "{0}.{1}.{2}" -f [math]::Floor($values[0] / 10000), [math]::Floor(($values[0] % 10000) / 100), ($values[0] % 100)
    pid = $runtimeProcessId
    path = $runtimeProcess.Path
    raw_events = $values[1]
    manual_requests = $values[2]
    conversions = $values[3]
    last_result = $values[4]
    shift_backspace_registered = ($values[5] -band 32) -ne 0
    registered_shortcuts = $values[5]
    raw_input_registered = $values[6] -ne 0
    failed_batches = $values[7]
    working_set_mb = [math]::Round($runtimeProcess.WorkingSet64 / 1MB, 2)
    private_mb = [math]::Round($runtimeProcess.PrivateMemorySize64 / 1MB, 2)
} | ConvertTo-Json
