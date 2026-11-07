#requires -Version 5.1
# P0-02 external validator. ASCII source; no build, PE parsing, screenshots or elevation.
# Use an interactive desktop. A caller's restricted-token launcher supplies Medium integrity.
# The target must flush the exact TSV header below and six ordered phases per generation,
# then one completed row for generation 3. HWND reuse is allowed after observed invalidation.
# Exit: 0=pass, 1=fail, 2=30-second timeout. Forced cleanup can never produce a pass.
[CmdletBinding()]
param(
    [string] $ExePath,
    [string] $ResultsDir,
    [ValidateSet('Hwnd', 'Ole', 'DropTarget')]
    [string] $Mode = 'Hwnd'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ExePath) { $ExePath = Join-Path $root 'target\release\quicknest.exe' }
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-02' }
$base = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ResultsDir)
[void][IO.Directory]::CreateDirectory($base)
$run = Join-Path $base ('run-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ') + '-' + [Guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory -Path $run)
$tsv = Join-Path $run 'lifecycle.tsv'
$oleReport = Join-Path $run 'ole.tsv'
if ($Mode -eq 'Ole') { $tsv = Join-Path $run 'ole.hwnd.tsv' }
$dropReport = Join-Path $run 'drop.tsv'
if ($Mode -eq 'DropTarget') { $tsv = Join-Path $run 'drop.hwnd.tsv' }
$checks = New-Object 'System.Collections.Generic.List[object]'
$samples = New-Object 'System.Collections.Generic.List[object]'
$events = New-Object 'System.Collections.Generic.List[object]'
$report = [ordered]@{ status='fail'; started_utc=[DateTime]::UtcNow.ToString('o'); run_dir=$run; exe=$ExePath; sha256=$null; child_sha256=$null; final_sha256=$null; child_exe=$null; pid=$null; integrity=$null; os=[Environment]::OSVersion.VersionString; os_64bit=[Environment]::Is64BitOperatingSystem; powershell=$PSVersionTable.PSVersion.ToString(); exit_code=$null; forced_termination=$false; error=$null; checks=$checks; generations=@(); poll_ms=20; timeout_ms=30000; phase_tolerance_ms=50; exit_observation_tolerance_ms=20 }
$p = $null; $outTask = $null; $errTask = $null; $exeLock = $null; $clock = $null
function Check([bool] $OK, [string] $Detail) {
    $checks.Add([pscustomobject]@{ passed=$OK; detail=$Detail })
    if (-not $OK) { throw $Detail }
}
try {
    # Safety: read-only Win32 observation, PID-filtered enumeration, TOKEN_QUERY only.
    # No window messages, input, hooks, token changes or remote memory access.
    # Native token buffers and handles are released in finally; the callback stays rooted.
    if (-not ('P002Native' -as [type])) { Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P002Native {
    public sealed class Window { public long hwnd; public bool is_window, visible, enumerated; public uint pid, tid; }
    delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll", SetLastError=true)] static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int kind, IntPtr buffer, int size, out int needed);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthority(IntPtr sid, uint index);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    public static Window[] Observe(int target, HashSet<long> known) {
        HashSet<long> current = new HashSet<long>();
        EnumProc callback = delegate(IntPtr h, IntPtr unused) { uint owner; GetWindowThreadProcessId(h, out owner); if (owner == (uint)target) { current.Add(h.ToInt64()); known.Add(h.ToInt64()); } return true; };
        if (!EnumWindows(callback, IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
        GC.KeepAlive(callback);
        List<Window> result = new List<Window>();
        foreach (long value in known) { IntPtr h = new IntPtr(value); uint owner; bool valid = IsWindow(h); bool visible = IsWindowVisible(h); uint thread = GetWindowThreadProcessId(h, out owner); result.Add(new Window { hwnd=value, is_window=valid, visible=visible, enumerated=current.Contains(value), pid=owner, tid=thread }); }
        return result.ToArray();
    }
    public static int IntegrityRid(IntPtr process) {
        IntPtr token=IntPtr.Zero, buffer=IntPtr.Zero;
        try {
            if (!OpenProcessToken(process, 8, out token)) throw new Win32Exception(Marshal.GetLastWin32Error());
            int needed; GetTokenInformation(token, 25, IntPtr.Zero, 0, out needed);
            if (needed < IntPtr.Size) throw new Win32Exception(Marshal.GetLastWin32Error());
            buffer=Marshal.AllocHGlobal(needed);
            if (!GetTokenInformation(token, 25, buffer, needed, out needed)) throw new Win32Exception(Marshal.GetLastWin32Error());
            IntPtr sid=Marshal.ReadIntPtr(buffer); byte count=Marshal.ReadByte(GetSidSubAuthorityCount(sid));
            if (count == 0) throw new InvalidOperationException("Empty integrity SID");
            return Marshal.ReadInt32(GetSidSubAuthority(sid, (uint)(count-1)));
        } finally { if (buffer != IntPtr.Zero) Marshal.FreeHGlobal(buffer); if (token != IntPtr.Zero) CloseHandle(token); }
    }
}
'@
    }
    $exe = Get-Item -LiteralPath $ExePath
    Check (-not $exe.PSIsContainer -and $exe.Extension -ieq '.exe') 'Executable is an existing EXE'
    $report.exe = $exe.FullName
    $exeLock = [IO.File]::Open($exe.FullName, 'Open', 'Read', 'Read') # Prevent replacement during validation.
    $report.sha256 = (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName=$exe.FullName; $info.WorkingDirectory=$exe.DirectoryName
    $info.Arguments=if ($Mode -eq 'Ole') { '--ole-test "' + $oleReport + '"' } elseif ($Mode -eq 'DropTarget') { '--drop-target-test "' + $dropReport + '"' } else { '--hwnd-test "' + $tsv + '"' }
    $info.UseShellExecute=$false
    $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    $p = New-Object Diagnostics.Process; $p.StartInfo=$info
    $clock = [Diagnostics.Stopwatch]::StartNew()
    Check ($p.Start()) 'Child started'
    $report.pid=$p.Id; $outTask=$p.StandardOutput.ReadToEndAsync(); $errTask=$p.StandardError.ReadToEndAsync()
    $rid=[P002Native]::IntegrityRid($p.Handle)
    $level=if ($rid -ge 8192 -and $rid -lt 12288) { 'Medium' } elseif ($rid -ge 12288 -and $rid -lt 16384) { 'High' } else { 'Other' }
    $report.integrity=[ordered]@{ level=$level; rid=$rid }
    Check ($level -in @('Medium','High')) 'Child token is Medium or High; no self-elevation or demotion'
    $report.child_exe=$p.MainModule.FileName
    $report.child_sha256=(Get-FileHash -LiteralPath $report.child_exe -Algorithm SHA256).Hash
    Check ($report.child_exe -ieq $exe.FullName -and $report.child_sha256 -ceq $report.sha256) 'Running image path and SHA256 match requested EXE'
    $known=New-Object 'System.Collections.Generic.HashSet[System.Int64]'
    $columns='elapsed_ms,generation,phase,hwnd,is_window,visible,component_alive,pid,tid'.Split(',')
    $seen=''; $generation=0; $phase='startup'; $previousElapsed=-1L
    do {
        $aliveBefore=-not $p.HasExited
        if (Test-Path -LiteralPath $tsv) {
            $stream=[IO.File]::Open($tsv, 'Open', 'Read', 'ReadWrite')
            try { if ($stream.Length -gt 65536) { throw 'TSV exceeds protocol size limit' }; $reader=New-Object IO.StreamReader($stream); $text=$reader.ReadToEnd() } finally { $stream.Dispose() }
            $complete=$text.Substring(0, $text.LastIndexOf("`n")+1)
            if (-not $complete.StartsWith($seen, [StringComparison]::Ordinal)) { throw 'TSV was truncated or rewritten' }
            foreach ($line in $complete.Substring($seen.Length).Split([char]10)) {
                if (-not $line) { continue }; $line=$line.TrimEnd([char]13)
                if ($seen.Length -eq 0) { Check ($line -ceq ($columns -join "`t")) 'Exact TSV header'; $seen=$line+"`n"; continue }
                if ($line.Split([char]9).Count -ne 9) { throw 'TSV row must contain nine tab-separated fields' }
                $row=$line | ConvertFrom-Csv -Delimiter "`t" -Header $columns
                foreach ($name in @('elapsed_ms','generation','hwnd','pid','tid')) { if ($row.$name -cnotmatch '^[0-9]+$') { throw "Invalid integer: $name" }; [void][long]::Parse($row.$name) }
                foreach ($name in @('is_window','visible','component_alive')) { if ($row.$name -cnotmatch '^(true|false)$') { throw "Invalid boolean: $name" } }
                Check ([long]$row.elapsed_ms -ge $previousElapsed -and [long]$row.elapsed_ms -lt 30000 -and $row.generation -cmatch '^[123]$' -and [long]$row.pid -eq $p.Id -and [long]$row.tid -gt 0) 'TSV time, generation, PID and TID valid'
                $previousElapsed=[long]$row.elapsed_ms; $generation=[int]$row.generation; $phase=$row.phase
                $events.Add([pscustomobject]@{ row=$row; observed_ms=$clock.Elapsed.TotalMilliseconds; process_alive=(-not $p.HasExited) })
            }
            $seen=$complete
        }
        $sampleStart=$clock.Elapsed.TotalMilliseconds; $windows=@([P002Native]::Observe($p.Id, $known))
        $live=$aliveBefore -and -not $p.HasExited
        $samples.Add([pscustomobject]@{ elapsed_ms=$sampleStart; end_ms=$clock.Elapsed.TotalMilliseconds; generation=$generation; phase=$phase; process_alive=$live; windows=$windows })
        if (-not $live) { break }
        if ($clock.ElapsedMilliseconds -ge 30000) { $report.status='timeout'; throw 'Child exceeded 30-second timeout' }
        Start-Sleep -Milliseconds 20
    } while ($true)
    $report.exit_code=$p.ExitCode; $report.exit_observed_ms=$clock.Elapsed.TotalMilliseconds
    Check ($report.exit_code -eq 0) 'Child exited normally with code 0'
    Check ($events.Count -eq 19 -and $text.EndsWith("`n")) 'Exactly three six-phase generations and one final completed row, all newline-flushed'
    $rows=@($events | ForEach-Object { $_.row }); $report.lifecycle=$events.ToArray()
    $phases=@('created','hidden','restored','component_released','destroyed','destroyed_confirmed')
    foreach ($g in 1..3) {
        $ev=@($events | Where-Object { [int]$_.row.generation -eq $g }); $r=@($ev | ForEach-Object { $_.row })
        $expected=@($phases); if ($g -eq 3) { $expected+= 'completed' }
        Check (($r.phase -join ',') -ceq ($expected -join ',')) "Generation $g exact phase order and one destroyed_confirmed"
        $hwnd=[long]$r[0].hwnd; $thread=[long]$r[0].tid
        Check ($hwnd -gt 0 -and @($r | Where-Object { [long]$_.hwnd -ne $hwnd -or [long]$_.tid -ne $thread }).Count -eq 0) "Generation $g retains its decimal HWND and UI thread"
        foreach ($i in 0..2) { Check ($r[$i].is_window -ceq 'true' -and $r[$i].visible -ceq @('true','false','true')[$i] -and $r[$i].component_alive -ceq 'true') "Generation $g $($phases[$i]) internal state" }
        foreach ($i in 3..($r.Count-1)) { Check ($r[$i].component_alive -ceq 'false') "Generation $g $($r[$i].phase) weak component reference is dead" }
        foreach ($i in 4..($r.Count-1)) { Check ($r[$i].is_window -ceq 'false' -and $r[$i].visible -ceq 'false') "Generation $g $($r[$i].phase) internal HWND invalid" }
        foreach ($i in 1..3) { $delta=[long]$r[$i].elapsed_ms-[long]$r[$i-1].elapsed_ms; Check ($delta -ge 450 -and $delta -le 1500) "Generation $g $($phases[$i-1]) residence approximately 500ms ($delta ms)" }
        Check (([long]$r[5].elapsed_ms-[long]$r[4].elapsed_ms) -ge 500) "Generation $g invalid for at least 500ms before confirmation"
        $s=@($samples | Where-Object { $_.generation -eq $g -and $_.process_alive })
        foreach ($state in @('created','hidden','restored')) {
            $proof=@($s | Where-Object { $_.phase -ceq $state } | ForEach-Object { $_.windows } | Where-Object { $_.hwnd -eq $hwnd -and $_.enumerated -and $_.is_window -and $_.pid -eq $p.Id -and $_.tid -eq $thread -and $_.visible -eq ($state -cne 'hidden') })
            Check ($proof.Count -gt 0) "Generation $g external $state evidence while child alive"
        }
        $dead=@($s | Where-Object { $_.phase -in @('component_released','destroyed','destroyed_confirmed','completed') -and @($_.windows | Where-Object { $_.hwnd -eq $hwnd -and -not $_.is_window }).Count -gt 0 })
        Check ($dead.Count -gt 0) "Generation $g previously enumerated HWND independently invalid while child alive"
        $revived=@($s | Where-Object { $_.elapsed_ms -gt $dead[0].elapsed_ms } | ForEach-Object { $_.windows } | Where-Object { $_.hwnd -eq $hwnd -and $_.is_window })
        Check ($revived.Count -eq 0) "Generation $g no HWND revival during invalidation confirmation"
        if ($g -lt 3) { $next=@($events | Where-Object { [int]$_.row.generation -eq ($g+1) })[0]; Check ([long]$next.row.elapsed_ms -ge [long]$r[5].elapsed_ms -and $dead[0].end_ms -lt $next.observed_ms) "Generation $g invalidated before next creation, including handle reuse" }
        $report.generations+= [ordered]@{ generation=$g; hwnd=$hwnd; tid=$thread; first_external_false_ms=$dead[0].elapsed_ms; destroyed_confirmed_ms=[long]$r[5].elapsed_ms }
    }
    Check (($rows.generation -join ',') -ceq ((@(1)*6+@(2)*6+@(3)*7) -join ',')) 'Global generation order is 1, 2, 3 with final generation-3 completion'
    $grace=$report.exit_observed_ms-$events[$events.Count-1].observed_ms
    Check ($events[$events.Count-1].process_alive -and $grace -ge 480) "Child stayed alive 500ms after completed (20ms observation tolerance; observed $grace ms)"
    Check ($outTask.Wait(2000) -and $errTask.Wait(2000)) 'Redirected stdout and stderr drained'
    $report.final_sha256=(Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash
    Check ($report.final_sha256 -ceq $report.sha256) 'EXE SHA256 unchanged throughout run'
    if ($Mode -eq 'Ole') {
        & (Join-Path $root 'scripts\assert-p0-03-report.ps1') -ReportPath $oleReport -HwndPath $tsv -TargetPid $p.Id
        $report['ole_report'] = $oleReport
        $report['mode'] = 'Ole'
    }
    if ($Mode -eq 'DropTarget') {
        & (Join-Path $root 'scripts\assert-p0-04-report.ps1') -ReportPath $dropReport -HwndPath $tsv -TargetPid $p.Id
        $report['drop_report'] = $dropReport
        $report['mode'] = 'DropTarget'
    }
    $report.status='pass'
} catch { $report.error=$_.Exception.Message }
finally {
    if ($p -and $report.pid) {
        try { if (-not $p.HasExited) { $report.forced_termination=$true; $p.Kill(); [void]$p.WaitForExit(3000) }; if ($p.HasExited) { $report.exit_code=$p.ExitCode } } catch { $report.error=([string]$report.error+'; cleanup: '+$_.Exception.Message) }
        foreach ($entry in @(@('stdout',$outTask),@('stderr',$errTask))) { $value='Capture incomplete'; if ($entry[1] -and $entry[1].Status -eq 'RanToCompletion') { $value=$entry[1].Result }; [IO.File]::WriteAllText((Join-Path $run ($entry[0]+'.log')), $value) }
        $p.Dispose()
    }
    if ($exeLock) { $exeLock.Dispose() }
    if ($report.forced_termination -and $report.status -eq 'pass') { $report.status='fail' }
    $report.finished_utc=[DateTime]::UtcNow.ToString('o'); $report.lifecycle=$events.ToArray()
    ConvertTo-Json -InputObject @($samples.ToArray()) -Depth 8 | Set-Content -LiteralPath (Join-Path $run 'external-samples.json') -Encoding UTF8
    $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $run 'results.json') -Encoding UTF8
    Write-Host ("P0-02 {0}: {1}" -f $report.status.ToUpperInvariant(), $run)
}
if ($report.status -eq 'pass') { exit 0 }; if ($report.status -eq 'timeout') { exit 2 }; exit 1
