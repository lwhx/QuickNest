#requires -Version 5.1
# P1-17 test helper: raise a window owned by another process. Windows blocks a
# plain SetForegroundWindow from a background script under the foreground lock;
# attaching to the foreground thread's input queue makes the activation work.
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class P117Focus {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
'@
function Focus-P117Window([long]$Hwnd) {
    $foreground = [P117Focus]::GetForegroundWindow()
    $processId = 0
    $foregroundThread = [P117Focus]::GetWindowThreadProcessId($foreground, [ref]$processId)
    $currentThread = [P117Focus]::GetCurrentThreadId()
    [void][P117Focus]::AttachThreadInput($currentThread, $foregroundThread, $true)
    try {
        [void][P117Focus]::BringWindowToTop([IntPtr]$Hwnd)
        [void][P117Focus]::SetForegroundWindow([IntPtr]$Hwnd)
    } finally {
        [void][P117Focus]::AttachThreadInput($currentThread, $foregroundThread, $false)
    }
    Start-Sleep -Milliseconds 250
}
