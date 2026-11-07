#requires -Version 5.1
[CmdletBinding()]
param(
    [string] $ResultsDir,
    [ValidateSet('P0-01', 'P0-02', 'P0-03', 'P0-04', 'P0-07', 'P0-08')]
    [string] $Task = 'P0-01',
    [switch] $LoadLauncherOnly
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root ("docs\test-results\{0}\medium" -f $Task) }
$validator = Join-Path $root ("scripts\validate-{0}.ps1" -f $Task.ToLowerInvariant())
$exe = Join-Path $root 'target\release\quicknest.exe'
$powerShell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
# Test harness only: create a reduced token for this child; do not change accounts,
# UAC, policy, Explorer or the product. The validator independently records integrity.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class P001MediumLauncher {
    [StructLayout(LayoutKind.Sequential)] struct SidAttributes { public IntPtr Sid; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct StartupInfo {
        public int cb; public string reserved, desktop, title;
        public uint x,y,width,height,xChars,yChars,fill,flags;
        public ushort show, reservedSize; public IntPtr reservedPtr, input, output, error;
    }
    [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process, thread; public uint pid, tid; }
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr memory);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool ConvertStringSidToSid(string text,out IntPtr sid);
    [DllImport("advapi32.dll")] static extern uint GetLengthSid(IntPtr sid);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disableCount,ref SidAttributes disable,uint deleteCount,IntPtr delete,uint restrictCount,IntPtr restrict,out IntPtr reduced);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool SetTokenInformation(IntPtr token,int kind,ref SidAttributes information,int length);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool CreateProcessWithTokenW(IntPtr token,uint logon,string app,StringBuilder command,uint flags,IntPtr environment,string directory,ref StartupInfo startup,out ProcessInfo process);
    static void Check(bool success) { if(!success) throw new Win32Exception(Marshal.GetLastWin32Error()); }
    public static uint Launch(string app,string arguments,string directory) {
        IntPtr original=IntPtr.Zero,reduced=IntPtr.Zero,admin=IntPtr.Zero,medium=IntPtr.Zero;
        try {
            Check(OpenProcessToken(GetCurrentProcess(),0x000F01FF,out original));
            Check(ConvertStringSidToSid("S-1-5-32-544",out admin));
            SidAttributes disabled=new SidAttributes { Sid=admin,Attributes=0 };
            Check(CreateRestrictedToken(original,1,1,ref disabled,0,IntPtr.Zero,0,IntPtr.Zero,out reduced));
            Check(ConvertStringSidToSid("S-1-16-8192",out medium));
            SidAttributes integrity=new SidAttributes { Sid=medium,Attributes=0x20 };
            Check(SetTokenInformation(reduced,25,ref integrity,Marshal.SizeOf(typeof(SidAttributes))+(int)GetLengthSid(medium)));
            StartupInfo startup=new StartupInfo { cb=Marshal.SizeOf(typeof(StartupInfo)),desktop="winsta0\\default" };
            ProcessInfo process;
            Check(CreateProcessWithTokenW(reduced,0,app,new StringBuilder("\""+app+"\" "+arguments),0,IntPtr.Zero,directory,ref startup,out process));
            CloseHandle(process.thread);CloseHandle(process.process);return process.pid;
        } finally {
            if(medium!=IntPtr.Zero) LocalFree(medium);if(admin!=IntPtr.Zero) LocalFree(admin);
            if(reduced!=IntPtr.Zero) CloseHandle(reduced);if(original!=IntPtr.Zero) CloseHandle(original);
        }
    }
}
'@
if ($LoadLauncherOnly) { return }
$arguments = '-NoProfile -File "{0}" -ExePath "{1}" -ResultsDir "{2}"' -f $validator, $exe, $ResultsDir
$childId = [P001MediumLauncher]::Launch($powerShell, $arguments, $root)
Write-Host "Started reduced-token validator PID=$childId. Actual token integrity is recorded in results.json."
