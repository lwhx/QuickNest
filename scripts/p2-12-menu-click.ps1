# P2-12 headless helper: posts a click to the QuickNest window and blocks in
# WM_LBUTTONUP until the native settings menu closes. Used by
# verify-p2-12-theme-headless.ps1 on a locked workstation.
param([long]$Hwnd, [int]$X, [int]$Y)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class P212MenuClick {
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
$lparam = [IntPtr]((($Y -band 0xFFFF) -shl 16) -bor ($X -band 0xFFFF))
[void][P212MenuClick]::SendMessage([IntPtr]$Hwnd, 0x0200, [IntPtr]::Zero, $lparam)
Start-Sleep -Milliseconds 80
[void][P212MenuClick]::SendMessage([IntPtr]$Hwnd, 0x0201, [IntPtr]1, $lparam)
Start-Sleep -Milliseconds 80
# Returns only after the native menu closes.
[void][P212MenuClick]::SendMessage([IntPtr]$Hwnd, 0x0202, [IntPtr]::Zero, $lparam)
