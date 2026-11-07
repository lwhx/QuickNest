#requires -Version 5.1
# Every native geometry operation scopes PMv2 locally. UIA bounds are physical;
# do not assume a PowerShell/COM call preserves a thread's DPI awareness context.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if(-not ('P006Native' -as [type])){Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P006Native {
 public struct P {public int x,y;}
 public struct R {public int left,top,right,bottom;}
 public sealed class Geometry {public long hwnd;public uint dpi,pid;public bool pmv2,iconic;public R client,window;}
 [DllImport("user32.dll")]static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll")]static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")]static extern IntPtr GetWindowDpiAwarenessContext(IntPtr h);
 [DllImport("user32.dll")]static extern bool AreDpiAwarenessContextsEqual(IntPtr a,IntPtr b);
 [DllImport("user32.dll",SetLastError=true)]static extern bool GetClientRect(IntPtr h,out R r);
 [DllImport("user32.dll",SetLastError=true)]static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll",SetLastError=true)]static extern bool ClientToScreen(IntPtr h,ref P p);
 [DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll")]static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")]static extern bool IsWindow(IntPtr h);
 [DllImport("user32.dll")]static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll",SetLastError=true)]static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int width,int height,uint flags);
 [DllImport("user32.dll")]static extern IntPtr WindowFromPoint(P point);
 [DllImport("user32.dll")]static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll",SetLastError=true)]static extern bool SetPhysicalCursorPos(int x,int y);
 static void Check(bool ok){if(!ok)throw new Win32Exception(Marshal.GetLastWin32Error());}
 static IntPtr Begin(){IntPtr old=SetThreadDpiAwarenessContext(new IntPtr(-4));if(old==IntPtr.Zero)throw new InvalidOperationException("PMv2 test context unavailable");return old;}
 public static Geometry Read(long value){IntPtr old=Begin();try{IntPtr h=new IntPtr(value);Check(IsWindow(h));R r,w;P origin=new P();Check(GetClientRect(h,out r));Check(GetWindowRect(h,out w));Check(ClientToScreen(h,ref origin));uint pid;GetWindowThreadProcessId(h,out pid);return new Geometry{hwnd=value,dpi=GetDpiForWindow(h),pid=pid,pmv2=AreDpiAwarenessContextsEqual(GetWindowDpiAwarenessContext(h),new IntPtr(-4)),iconic=IsIconic(h),window=w,client=new R{left=origin.x,top=origin.y,right=origin.x+r.right,bottom=origin.y+r.bottom}};}finally{SetThreadDpiAwarenessContext(old);}}
 public static void Place(long h,int x,int y,int width,int height){IntPtr old=Begin();try{ShowWindow(new IntPtr(h),9);Check(SetWindowPos(new IntPtr(h),new IntPtr(-1),x,y,width,height,0x0050));}finally{SetThreadDpiAwarenessContext(old);}}
 public static long Hit(int x,int y){IntPtr old=Begin();try{return GetAncestor(WindowFromPoint(new P{x=x,y=y}),2).ToInt64();}finally{SetThreadDpiAwarenessContext(old);}}
 public static void Move(int x,int y){Check(SetPhysicalCursorPos(x,y));}
}
'@}
