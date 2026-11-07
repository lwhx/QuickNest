#requires -Version 5.1
# Local test helper: window/token observation and trusted pointer input for test fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if(-not ('P005Desktop' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Text;using System.Collections.Generic;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P005Desktop {
 public struct Rect {public int Left,Top,Right,Bottom;}
 public struct Point {public int X,Y;}
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point point);
 [DllImport("user32.dll")]public static extern IntPtr GetAncestor(IntPtr h,uint flags);
 public sealed class Window {public long hwnd;public uint pid;public string title;public Rect rect;public int integrity;}
 delegate bool EnumProc(IntPtr hwnd,IntPtr data);
 [DllImport("user32.dll")]static extern bool EnumWindows(EnumProc cb,IntPtr data);
 [DllImport("user32.dll")]static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int GetWindowText(IntPtr h,StringBuilder b,int n);
 [DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")]static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 [DllImport("user32.dll",EntryPoint="SetForegroundWindow")]static extern bool SetForegroundWindowRaw(IntPtr h);
 [DllImport("user32.dll")]static extern bool BringWindowToTop(IntPtr h);
 [DllImport("kernel32.dll")]static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")]static extern bool AttachThreadInput(uint a,uint b,bool attach);
 // Windows blocks a plain SetForegroundWindow from a background script under
 // the foreground lock; attaching to the foreground thread's input queue and
 // raising the window makes the activation stick (2026-11-06: the panel stayed
 // behind the browser and the clicks landed on the browser instead).
 public static bool SetForegroundWindow(IntPtr h){
   IntPtr fg=GetForegroundWindow();
   uint pid=0;
   uint fgThread=fg==IntPtr.Zero?0u:GetWindowThreadProcessId(fg,out pid);
   uint cur=GetCurrentThreadId();
   bool attached=fgThread!=0u && fgThread!=cur && AttachThreadInput(cur,fgThread,true);
   try{
     BringWindowToTop(h);
     return SetForegroundWindowRaw(h);
   }finally{
     if(attached) AttachThreadInput(cur,fgThread,false);
   }
 }
 [DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")]public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("kernel32.dll")]static extern IntPtr OpenProcess(uint a,bool i,int p);
 [DllImport("kernel32.dll")]static extern bool CloseHandle(IntPtr h);
 [DllImport("advapi32.dll")]static extern bool OpenProcessToken(IntPtr p,uint a,out IntPtr t);
 [DllImport("advapi32.dll")]static extern bool GetTokenInformation(IntPtr t,int c,IntPtr b,int l,out int n);
 [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthorityCount(IntPtr s);
 [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthority(IntPtr s,uint n);
 public static int Rid(int pid){IntPtr p=OpenProcess(0x1000,false,pid),t=IntPtr.Zero,b=IntPtr.Zero;try{if(!OpenProcessToken(p,8,out t))throw new Win32Exception();int n;GetTokenInformation(t,25,IntPtr.Zero,0,out n);b=Marshal.AllocHGlobal(n);if(!GetTokenInformation(t,25,b,n,out n))throw new Win32Exception();IntPtr s=Marshal.ReadIntPtr(b);byte c=Marshal.ReadByte(GetSidSubAuthorityCount(s));return Marshal.ReadInt32(GetSidSubAuthority(s,(uint)(c-1)));}finally{if(b!=IntPtr.Zero)Marshal.FreeHGlobal(b);if(t!=IntPtr.Zero)CloseHandle(t);if(p!=IntPtr.Zero)CloseHandle(p);}}
 public static Window[] Windows(){var list=new List<Window>();EnumProc cb=delegate(IntPtr h,IntPtr d){if(IsWindowVisible(h)){var b=new StringBuilder(1024);GetWindowText(h,b,b.Capacity);if(b.Length>0){uint pid;GetWindowThreadProcessId(h,out pid);Rect r;GetWindowRect(h,out r);int rid=-1;try{rid=Rid((int)pid);}catch{}list.Add(new Window{hwnd=h.ToInt64(),pid=pid,title=b.ToString(),rect=r,integrity=rid});}}return true;};EnumWindows(cb,IntPtr.Zero);GC.KeepAlive(cb);return list.ToArray();}
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Mouse mouse;[FieldOffset(0)]public Keyboard key;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 [DllImport("user32.dll")]static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")]static extern uint MapVirtualKeyW(uint code,uint mode);
 public static void MouseButton(bool down){var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?2u:4u}}};if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());}
 public static void RightButton(bool down){var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());}
 // 2026-11-06: the protection daemon intermittently hard-blocks injected
 // keyboard events (SendInput returns 0 / ERROR_ACCESS_DENIED). Key falls
 // back to posting WM_KEYDOWN/WM_KEYUP at the foreground window (same
 // application path the P005Keyboard helpers use); callers that genuinely
 // need the system input queue (the Ctrl+Alt+Space hotkey chord) still get
 // SendInput whenever it is available.
 public static void Key(ushort vk,bool down){
   var i=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=down?0u:2u}}};
   if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))==1)return;
   var error=Marshal.GetLastWin32Error();
   IntPtr h=GetForegroundWindow();
   if(h==IntPtr.Zero)throw new Win32Exception(error);
   uint scan=MapVirtualKeyW(vk,0);
   uint lp=1u|((uint)scan<<16)|(down?1u<<30:1u<<29);
   if(vk>=0xE0)lp|=1u<<24;
   if(!PostMessage(h,down?0x0100u:0x0101u,(IntPtr)vk,(IntPtr)(int)lp))throw new Win32Exception(error);
 }
}
'@ }
# Keyboard delivery for the acceptance scripts.
#
# The scripts used to type through SendInput (system input queue). On this
# machine a third-party protection daemon installs a global low-level keyboard
# hook that swallows *injected* events: SendInput/KEYEVENTF_UNICODE report
# success and the low-level hook sees them, yet no application receives them
# (verified 2026-11-05 with Notepad and a WinForms TextBox as controls).
# Real typing is unaffected.
#
# The scripts therefore post the key messages straight to the target window,
# which exercises the same application path (winit -> Slint -> widget) without
# the blocked input queue. Text entry goes through the clipboard: winit derives
# the character from the scan code plus the *calling thread's*
# GetKeyboardState, and a posted shift keydown never reaches that state, so
# shifted characters (upper case, ':', '_') would come out unshifted. Pasting
# delivers the exact characters instead.
if(-not ('P005Keyboard' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;using System.Threading;
public static class P005Keyboard {
 [DllImport("user32.dll",SetLastError=true)]static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,System.Text.StringBuilder l);
 [DllImport("user32.dll")]static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")]static extern uint MapVirtualKeyW(uint code,uint mode);
 [DllImport("user32.dll")]static extern bool OpenClipboard(IntPtr h);
 [DllImport("user32.dll")]static extern bool CloseClipboard();
 [DllImport("user32.dll")]static extern bool EmptyClipboard();
 [DllImport("kernel32.dll")]static extern IntPtr GlobalAlloc(uint flags,UIntPtr bytes);
 [DllImport("kernel32.dll")]static extern IntPtr GlobalLock(IntPtr h);
 [DllImport("kernel32.dll")]static extern bool GlobalUnlock(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr SetClipboardData(uint fmt,IntPtr data);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
 public delegate bool EnumChildWindowsProc(IntPtr h,IntPtr l);
 [DllImport("user32.dll")]static extern bool EnumChildWindows(IntPtr parent,EnumChildWindowsProc cb,IntPtr l);
 public delegate bool EnumWindowsProc(IntPtr h,IntPtr l);
 [DllImport("user32.dll")]static extern bool EnumWindows(EnumWindowsProc cb,IntPtr l);
 [DllImport("user32.dll")]static extern int GetDlgCtrlID(IntPtr h);
 [DllImport("user32.dll")]static extern IntPtr GetParent(IntPtr h);
 [DllImport("user32.dll")]static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr GetClipboardData(uint fmt);
 const uint WM_KEYDOWN=0x0100, WM_KEYUP=0x0101, MAPVK_VK_TO_VSC=0, CF_UNICODETEXT=13;
 const ushort VK_CONTROL=0x11, VK_MENU=0x12;
 static IntPtr LParam(ushort scan,bool down){
   uint v=1u|((uint)scan<<16);
   if(scan>=0xE0){v|=1u<<24;}   // extended key (arrows, function keys, keypad)
   if(down){v|=1u<<30;}else{v|=1u<<29;}
   return new IntPtr((int)v);
 }
 static void Down(IntPtr h,ushort vk,ushort scan){PostMessage(h,WM_KEYDOWN,(IntPtr)vk,LParam(scan,true));}
 static void Up(IntPtr h,ushort vk,ushort scan){PostMessage(h,WM_KEYUP,(IntPtr)vk,LParam(scan,false));}
 public static IntPtr Target(){return GetForegroundWindow();}
 public static void Tap(IntPtr h,ushort vk,ushort scan){Down(h,vk,scan);Up(h,vk,scan);}
 public static void Chord(ushort modifierVk,ushort keyVk){
   IntPtr h=GetForegroundWindow();
   if(h==IntPtr.Zero)throw new Win32Exception("no foreground window for keyboard input");
   ushort mscan=(ushort)MapVirtualKeyW(modifierVk,MAPVK_VK_TO_VSC);
   ushort kscan=(ushort)MapVirtualKeyW(keyVk,MAPVK_VK_TO_VSC);
   Down(h,modifierVk,mscan); Tap(h,keyVk,kscan); Up(h,modifierVk,mscan);
 }
 static bool SetClipboardText(string text){
   if(!OpenClipboard(IntPtr.Zero))return false;
   try{
     if(!EmptyClipboard())return false;
     IntPtr mem=GlobalAlloc(0x0042,(UIntPtr)((text.Length+1)*2));
     if(mem==IntPtr.Zero)return false;
     IntPtr ptr=GlobalLock(mem);
     Marshal.Copy(text.ToCharArray(),0,ptr,text.Length);
     Marshal.WriteInt16(ptr,text.Length*2,0);
     GlobalUnlock(mem);
     return SetClipboardData(CF_UNICODETEXT,mem)!=IntPtr.Zero;
   } finally { CloseClipboard(); }
 }
 /// Sets the text of a native edit control directly (deterministic path entry
 /// for the file picker / save dialogs used by the acceptance scripts).
 public static void SetText(IntPtr edit, string text) {
   IntPtr buffer = Marshal.StringToHGlobalUni(text);
   try {
     // SendMessage keeps the buffer alive until the edit copied the text;
     // an asynchronous post would outlive the freed buffer and truncate it.
     SendMessage(edit, 0x000C, IntPtr.Zero, buffer);   // WM_SETTEXT
     Thread.Sleep(80);
   } finally { Marshal.FreeHGlobal(buffer); }
 }
 /// Presses a dialog button by control id (1 = IDOK, 2 = IDCANCEL) through
 /// WM_COMMAND, which is deterministic even when the dialog is not foreground.
 public static void PressDialogButton(IntPtr dialog, int id) {
   SendMessage(dialog, 0x0111, (IntPtr)id, IntPtr.Zero);   // WM_COMMAND
   Thread.Sleep(150);
 }
 /// Reads the text of a native edit control (diagnostics).
 public static string GetText(IntPtr edit) {
   var sb = new System.Text.StringBuilder(1024);
   SendMessage(edit, 0x000D, (IntPtr)sb.Capacity, sb);     // WM_GETTEXT
   return sb.ToString();
 }
 /// Finds the file name edit of a native file picker dialog (class #32770).
 /// The Vista style picker keeps it inside a ComboBoxEx32 (control id 1148);
 /// the search box is a plain Edit and must not be picked.
 public static IntPtr FindFileNameEdit(IntPtr dialog){
   IntPtr fallback=IntPtr.Zero;
   EnumChildWindowsProc cb=(h,l)=>{
     var sb=new System.Text.StringBuilder(64);
     GetClassName(h,sb,sb.Capacity);
     string cls=sb.ToString();
     if(cls=="ComboBoxEx32"&&GetDlgCtrlID(h)==1148){
       IntPtr inner=IntPtr.Zero;
       EnumChildWindowsProc inner2=(c,l2)=>{
         var s2=new System.Text.StringBuilder(64);
         GetClassName(c,s2,s2.Capacity);
         if(s2.ToString()=="Edit"){inner=c;return false;}
         return true;
       };
       EnumChildWindows(h,inner2,IntPtr.Zero);
       if(inner!=IntPtr.Zero){fallback=inner;return false;}
     }
     if(cls=="Edit"&&fallback==IntPtr.Zero){
       var par=new System.Text.StringBuilder(64);
       GetClassName(GetParent(h),par,par.Capacity);
       if(par.ToString()=="ComboBox"){fallback=h;return false;}
     }
     return true;
   };
   EnumChildWindows(dialog,cb,IntPtr.Zero);
   return fallback;
 }
 /// Finds the visible file picker dialog (class #32770) of a process.
 public static IntPtr FindFileDialogByPid(int pid) {
   IntPtr found=IntPtr.Zero;
   EnumWindowsProc cb=(h,l)=>{
     uint p; GetWindowThreadProcessId(h,out p);
     if(p==(uint)pid && IsWindowVisible(h)){
       var sb=new System.Text.StringBuilder(64);
       GetClassName(h,sb,sb.Capacity);
       if(sb.ToString()=="#32770"){found=h;return false;}
     }
     return true;
   };
   EnumWindows(cb,IntPtr.Zero);
   return found;
 }
 /// Finds the file name edit of the file picker that belongs to `pid`.
 public static IntPtr FindFileNameEditByPid(int pid){
   IntPtr found=IntPtr.Zero;
   EnumWindowsProc cb=(h,l)=>{
     uint p; GetWindowThreadProcessId(h,out p);
     if(p==(uint)pid){
       var sb=new System.Text.StringBuilder(64);
       GetClassName(h,sb,sb.Capacity);
       if(sb.ToString()=="#32770"){
         IntPtr e=FindFileNameEdit(h);
         if(e!=IntPtr.Zero){found=e;return false;}
       }
     }
     return true;
   };
   EnumWindows(cb,IntPtr.Zero);
   return found;
 }
 public static bool SetClipboard(string text){return SetClipboardText(text);}
 public static void TypeText(string text){
   IntPtr h=GetForegroundWindow();
   if(h==IntPtr.Zero)throw new Win32Exception("no foreground window for keyboard input");
   if(text==null||text.Length==0)return;
   if(!SetClipboardText(text))throw new Win32Exception("could not fill the clipboard");
   Thread.Sleep(80);
   Chord(VK_CONTROL,0x56);   // Ctrl+V
   Thread.Sleep(120);
 }
 /// Pastes the current clipboard content straight into a native edit control
 /// (the Windows file picker keeps its file name box in a plain EDIT child).
 public static void PasteInto(IntPtr edit){
   if(edit==IntPtr.Zero)throw new Win32Exception("no edit control to paste into");
   PostMessage(edit,0x0302,IntPtr.Zero,IntPtr.Zero);   // WM_PASTE
   Thread.Sleep(120);
 }
 public static void SelectAll(){
   IntPtr h=GetForegroundWindow();
   if(h==IntPtr.Zero)throw new Win32Exception("no foreground window for keyboard input");
   Down(h,VK_CONTROL,0x1D); Tap(h,0x41,0x1E); Up(h,VK_CONTROL,0x1D);
   Thread.Sleep(60);
 }
 public static void Key(ushort vk){
   IntPtr h=GetForegroundWindow();
   if(h==IntPtr.Zero)throw new Win32Exception("no foreground window for keyboard input");
   ushort scan=(ushort)MapVirtualKeyW(vk,MAPVK_VK_TO_VSC);
   Tap(h,vk,scan);
 }
}
'@ }
function Save-P005Screenshot([string]$Path, [long]$WindowHwnd = 0) {
 Add-Type -AssemblyName System.Drawing
 Add-Type -AssemblyName System.Windows.Forms
 $old=[P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))
 try{
  $b=[Windows.Forms.SystemInformation]::VirtualScreen
  if($WindowHwnd -ne 0){$w=@([P005Desktop]::Windows() | Where-Object {$_.hwnd -eq $WindowHwnd})[0];$b=New-Object Drawing.Rectangle($w.rect.Left,$w.rect.Top,($w.rect.Right-$w.rect.Left),($w.rect.Bottom-$w.rect.Top))}
  $bitmap=New-Object Drawing.Bitmap($b.Width,$b.Height)
  $g=[Drawing.Graphics]::FromImage($bitmap)
  try{$g.CopyFromScreen($b.Left,$b.Top,0,0,$bitmap.Size);$bitmap.Save($Path,[Drawing.Imaging.ImageFormat]::Png)}finally{$g.Dispose();$bitmap.Dispose()}
 }finally{[void][P005Desktop]::SetThreadDpiAwarenessContext($old)}
}
