#requires -Version 5.1
# P1-17 acceptance (baseline 5.4-5.6, 3.2):
# real Explorer -> QuickNest drops onto the grid, a category row, multiple
# files (order), a group tab (400 ms spring-load), plus ESC cancel and
# drag-out leave. Source files must stay untouched.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
. (Join-Path $root 'scripts\p1-17-focus.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-17-external-drop.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$dragsLog = Join-Path $results 'drags.jsonl'
if (Test-Path $dragsLog) { Remove-QnSandboxItem $dragsLog -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -eq 'QuickNest' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Find-NewExplorerWindow([string]$Needle, [long[]]$Before) {
    # /separate returns a launcher process that exits immediately; the real
    # window can belong to another Explorer process. Identify it as a new
    # visible window whose title contains the fixture folder name.
    $explorerPids = @(Get-Process explorer -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $matches2 = @([P005Desktop]::Windows() | Where-Object {
        $explorerPids -contains [int]$_.pid -and
        ($Before -notcontains [long]$_.hwnd) -and
        $_.title -ne '' -and
        $_.title -like "*$Needle*"
    })
    if ($matches2.Count -gt 0) { return [long]$matches2[0].hwnd }
    return 0
}

function Get-ShellWindowByHwnd([long]$Hwnd) {
    # Read only .HWND for every Shell window; touching Document on a user
    # window (Home/network) can block the whole regression.
    $shell = New-Object -ComObject Shell.Application
    foreach ($window in @($shell.Windows())) {
        try {
            if ([long]$window.HWND -eq $Hwnd) { return $window }
        } catch { }
    }
    return $null
}

function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            if (-not (Test-Path -LiteralPath $Path)) { throw 'config.json missing' }
            return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
        } catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
}

function Get-TotalItems($Config) {
    $count = 0
    foreach ($group in $Config.groups) {
        foreach ($category in $group.categories) { $count += @($category.items).Count }
    }
    return $count
}

function Get-ActivePlace($Config) {
    $group = @($Config.groups | Where-Object { $_.id -eq $Config.current_group_id })[0]
    if ($null -eq $group) { throw 'active group missing from config' }
    $category = $null
    if ($null -ne $group.last_category_id) {
        $category = @($group.categories | Where-Object { $_.id -eq $group.last_category_id })[0]
    }
    if ($null -eq $category) { $category = @($group.categories)[0] }
    if ($null -eq $category) { throw 'active category missing from config' }
    return [pscustomobject]@{ Group = $group; Category = $category }
}

function Find-Item($Config, [string]$Name) {
    foreach ($group in $Config.groups) {
        foreach ($category in $group.categories) {
            foreach ($item in @($category.items)) {
                if ($item.name -eq $Name) {
                    return [pscustomobject]@{ Group = $group; Category = $category; Item = $item }
                }
            }
        }
    }
    return $null
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-ClickAt([int]$X, [int]$Y) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 450
}

function Show-QuickNest([long]$Hwnd) {
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        if (@([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $Hwnd }).Count -gt 0) { return }
        [P005Desktop]::Key(0x11, $true)
        [P005Desktop]::Key(0x12, $true)
        [P005Desktop]::Key(0x20, $true)
        [P005Desktop]::Key(0x20, $false)
        [P005Desktop]::Key(0x12, $false)
        [P005Desktop]::Key(0x11, $false)
        Start-Sleep -Milliseconds 500
    }
    throw 'QuickNest did not become visible via Ctrl+Alt+Space'
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

# Isolated reference-only fixture set, same names/layout as P0-05 so Explorer
# item rows are at the already-verified Y positions.
$run = Join-Path ([IO.Path]::GetTempPath().TrimEnd('\')) ('QuickNestP117-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
$fixtures = Join-Path $run 'fixtures'
[void][IO.Directory]::CreateDirectory($fixtures)
$zh = ([string][char]0x4E2D) + [char]0x6587
[IO.File]::WriteAllText((Join-Path $fixtures ($zh + ' sample.txt')), 'QuickNest P1-17 reference-only drop fixture')
[IO.File]::WriteAllText((Join-Path $fixtures 'second file.txt'), 'QuickNest P1-17 second fixture')
Copy-Item (Join-Path $env:WINDIR 'System32\notepad.exe') (Join-Path $fixtures 'sample application.exe')
[void][IO.Directory]::CreateDirectory((Join-Path $fixtures ($zh + ' folder')))
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $fixtures ($zh + ' link.lnk')))
$shortcut.TargetPath = Join-Path $env:WINDIR 'System32\notepad.exe'
$shortcut.Save()

$sourceHashes = @{}
Get-ChildItem -LiteralPath $fixtures -File | ForEach-Object { $sourceHashes[$_.FullName] = (Get-FileHash -LiteralPath $_.FullName).Hash }
$sourceCount = @(Get-ChildItem -LiteralPath $fixtures).Count

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exePath
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
$psi.RedirectStandardOutput = $true
$app = New-Object System.Diagnostics.Process
$app.StartInfo = $psi
if (-not $app.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $app
$explorerWindow = $null
$explorerHwnd = 0
# Test hygiene: /separate can open an extra window (for example the default
# Documents folder) in addition to the fixture folder. Record the pre-test
# Explorer windows so every window opened by this script is closed again.
$explorerPidsBefore = @(Get-Process explorer -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$explorerHwndsBefore = @([P005Desktop]::Windows() | Where-Object { $explorerPidsBefore -contains [int]$_.pid } | ForEach-Object { [long]$_.hwnd })
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $window = $null
    while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $window = Get-QuickNestWindow $app.Id
    }
    if ($null -eq $window) { throw 'QuickNest window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$window.hwnd, [IntPtr](-1), 720, 40, 860, 620, 0x0050)
    Focus-P117Window $window.hwnd
    Start-Sleep -Milliseconds 600
    $window = Get-QuickNestWindow $app.Id
    $hwnd = [long]$window.hwnd
    $script:quicknestHwnd = $hwnd
    Record 'window_ready' ("pid=$($app.Id) hwnd=$hwnd rect=720,40,860,620")

    # P1-18 changed the default: the panel auto-hides after 120 ms without
    # focus. Pin it for this drag suite so the window stays on screen while
    # Explorer owns the foreground; P1-18 tests the unpinned behavior.
    Invoke-ClickAt (720 + 860 - 88) 62
    Start-Sleep -Milliseconds 400
    Record 'pinned_for_drag_suite' 'title-bar pin clicked'

    # The sample library is in memory until the first change; select another
    # category and return so config.json exists before the assertions.
    Invoke-ClickAt 790 176
    Invoke-ClickAt 790 148
    Start-Sleep -Milliseconds 400
    Record 'library_persisted' 'sample library written by category selections'

    Start-QnOwnedProcess -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList "/separate,`"$fixtures`"" | Out-Null
    $needle = Split-Path -Leaf $fixtures
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ($null -eq $explorerWindow -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
        $newHwnd = Find-NewExplorerWindow $needle $explorerHwndsBefore
        if ($newHwnd -ne 0) { $explorerWindow = Get-ShellWindowByHwnd $newHwnd }
    }
    if ($null -eq $explorerWindow) { throw 'Explorer window for the fixture folder not found' }
    $explorerHwnd = [long]$explorerWindow.HWND
    [void][P005Desktop]::SetWindowPos([IntPtr]$explorerHwnd, [IntPtr](-1), 10, 40, 700, 700, 0x0050)
    Focus-P117Window $explorerHwnd
    Focus-P117Window $hwnd
    Start-Sleep -Milliseconds 500
    $view = $explorerWindow.Document
    Record 'explorer_ready' ("hwnd=$explorerHwnd path=$fixtures")

    function Select-Fixture([string[]]$Names) {
        Show-QuickNest $script:quicknestHwnd
        $first = $true
        foreach ($name in $Names) {
            $item = $view.Folder.ParseName($name)
            if ($null -eq $item) { throw "fixture missing: $name" }
            $flags = if ($first) { 29 } else { 1 }
            $view.SelectItem($item, $flags)
            $first = $false
        }
        Start-Sleep -Milliseconds 200
    }

    $drag = Join-Path $PSScriptRoot 'drag-p1-17.ps1'
    $initial = Read-Config $configPath
    Assert-True ($initial.settings.pinned -eq $true) 'drag suite failed to pin the panel'
    $initialTotal = Get-TotalItems $initial
    $initialPlace = Get-ActivePlace $initial
    $initialActiveCategoryItems = @($initialPlace.Category.items).Count
    Record 'initial' ("total=$initialTotal active_category=$($initialPlace.Category.name) items=$initialActiveCategoryItems")

    Select-Fixture @('sample application.exe')
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 248 -TargetX 1300 -TargetY 350 -Action Drop
    Save-P005Screenshot -Path (Join-Path $results 'p1-17-grid-drop.png') -WindowHwnd $hwnd
    $config = Read-Config $configPath
    Assert-True ((Get-TotalItems $config) -eq ($initialTotal + 1)) 'grid drop did not add exactly one item'
    $gridItems = @((Get-ActivePlace $config).Category.items) | ForEach-Object { "$($_.name)|$($_.launch.path)" }
    Record 'grid_drop_items' ($gridItems -join ';')
    $found = Find-Item $config 'sample application'
    Assert-True ($null -ne $found) 'dropped EXE name not imported'
    Assert-True ($found.Item.item_type -eq 'exe') 'dropped EXE type not inferred'
    Assert-True ($found.Item.launch.path -eq (Join-Path $fixtures 'sample application.exe')) 'dropped EXE path not stored by reference'
    Assert-True ($found.Category.id -eq $initialPlace.Category.id) 'grid drop did not append to the active category'
    Record 'grid_drop' ("name=$($found.Item.name) type=$($found.Item.item_type) category=$($found.Category.name)")

    # b) Explorer -> second category row: adds to that category, keeps selection.
    $categoryPlace = Get-ActivePlace $config
    $secondCategory = @($categoryPlace.Group.categories)[1]
    Assert-True ($null -ne $secondCategory) 'sample group needs a second category'
    $beforeSecond = @($secondCategory.items).Count
    Select-Fixture @('second file.txt')
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 277 -TargetX 790 -TargetY 176 -Action Drop -Capture 'p1-17-category-hover.png'
    Save-P005Screenshot -Path (Join-Path $results 'p1-17-category-drop.png') -WindowHwnd $hwnd
    $config = Read-Config $configPath
    $group = @($config.groups | Where-Object { $_.id -eq $categoryPlace.Group.id })[0]
    $secondCategoryAfter = @($group.categories | Where-Object { $_.id -eq $secondCategory.id })[0]
    Assert-True ((Get-TotalItems $config) -eq ($initialTotal + 2)) 'category drop total count mismatch'
    Assert-True ((@($secondCategoryAfter.items).Count) -eq ($beforeSecond + 1)) 'category drop did not target the hovered category'
    $secondItem = @($secondCategoryAfter.items | Where-Object { $_.name -eq 'second file' })[0]
    Assert-True ($null -ne $secondItem) 'category drop did not import the file'
    Assert-True ((Get-ActivePlace $config).Category.id -eq $categoryPlace.Category.id) 'category drop changed the active selection'
    Record 'category_drop' ("category=$($secondCategoryAfter.name) items=$(@($secondCategoryAfter.items).Count)")

    # c) Multi-file drop: CF_HDROP order is preserved (baseline 5.6).
    Select-Fixture @('second file.txt', ($zh + ' sample.txt'))
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 277 -TargetX 1300 -TargetY 400 -Action Drop
    $config = Read-Config $configPath
    Assert-True ((Get-TotalItems $config) -eq ($initialTotal + 4)) 'multi-file drop count mismatch'
    $activeAfterMulti = Get-ActivePlace $config
    $lastTwo = @($activeAfterMulti.Category.items) | Select-Object -Last 2
    Assert-True ($lastTwo.Count -eq 2) 'multi-file drop did not append two items'
    Assert-True ($lastTwo[0].name -eq 'second file') 'multi-file order: first item mismatch'
    Assert-True ($lastTwo[1].name -eq ($zh + ' sample')) 'multi-file order: second item mismatch'
    Record 'multi_drop' ("order=$($lastTwo[0].name),$($lastTwo[1].name)")

    # d) Group tab spring-load: hovering 700 ms switches the target group.
    $groupsBefore = Read-Config $configPath
    $toolsCountBeforeSpring = @((Get-ActivePlace $groupsBefore).Category.items).Count
    $targetGroup = @($groupsBefore.groups)[1]
    Assert-True ($targetGroup.id -ne $groupsBefore.current_group_id) 'spring-load target must differ from the active group'
    Select-Fixture @($zh + ' folder')
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 220 -TargetX 857 -TargetY 104 -Action Spring -HoverMs 700 -Capture 'p1-17-spring-hover.png'
    $config = Read-Config $configPath
    Assert-True ($config.current_group_id -eq $targetGroup.id) 'spring-load did not switch the active group'
    Assert-True ((Get-TotalItems $config) -eq ($initialTotal + 5)) 'spring-load drop count mismatch'
    $folderItem = Find-Item $config ($zh + ' folder')
    Assert-True ($null -ne $folderItem) 'folder drop not imported'
    Assert-True ($folderItem.Item.item_type -eq 'folder') 'folder type not inferred'
    Assert-True ($folderItem.Group.id -eq $targetGroup.id) 'folder was not added to the spring-loaded group'
    $originalGroup = @($config.groups | Where-Object { $_.id -eq $groupsBefore.current_group_id })[0]
    $originalActive = @($originalGroup.categories | Where-Object { $_.id -eq $originalGroup.last_category_id })[0]
    if ($null -eq $originalActive) { $originalActive = @($originalGroup.categories)[0] }
    Assert-True ((@($originalActive.items).Count) -eq $toolsCountBeforeSpring) 'spring-loaded drop leaked into the original group'
    Record 'spring_load' ("group=$($targetGroup.name) item=$($folderItem.Item.name)")

    $countAfterSpring = Get-TotalItems $config

    # e) ESC during hover cancels the OLE drag: no data change (baseline 5.6).
    Select-Fixture @($zh + ' link.lnk')
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 304 -TargetX 1300 -TargetY 350 -Action Cancel
    $config = Read-Config $configPath
    Assert-True ((Get-TotalItems $config) -eq $countAfterSpring) 'ESC cancel wrote data'
    Assert-True ($null -eq (Find-Item $config ($zh + ' link'))) 'ESC cancel added the shortcut'
    Record 'esc_cancel' ("total=$countAfterSpring unchanged")

    # f) Leaving the window and cancelling must not write data either.
    Select-Fixture @($zh + ' sample.txt')
    & $drag -RunDir $results -SourceHwnd $explorerHwnd -TargetHwnd $hwnd -SourceY 332 -TargetX 1300 -TargetY 350 -Action Leave
    $config = Read-Config $configPath
    Assert-True ((Get-TotalItems $config) -eq $countAfterSpring) 'drag leave wrote data'
    Record 'leave' ("total=$countAfterSpring unchanged")

    # g) Source files are only referenced (baseline 3.2).
    $afterCount = @(Get-ChildItem -LiteralPath $fixtures).Count
    Assert-True ($afterCount -eq $sourceCount) 'drag changed the fixture directory'
    foreach ($entry in $sourceHashes.GetEnumerator()) {
        Assert-True ((Get-FileHash -LiteralPath $entry.Key).Hash -eq $entry.Value) "source file changed: $($entry.Key)"
    }
    Record 'source_unchanged' ("files=$afterCount hashes=$($sourceHashes.Count)")

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P1-17 external-drop phases passed'
    Write-Host 'All P1-17 external-drop phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    Record 'cleanup_start' 'closing the app and the explorer windows'
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    if ($explorerHwnd -ne 0) { try { Close-QnOwnedWindow -Hwnd ([IntPtr]$explorerHwnd) } catch { } }
    # Close every Explorer window that did not exist before this script. A
    # /separate launch can leave a topmost Documents window behind, which
    # would then cover QuickNest for the following acceptance tests.
    $explorerPidsAfter = @(Get-Process explorer -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $newExplorerWindows = @([P005Desktop]::Windows() | Where-Object {
        ($explorerPidsAfter -contains [int]$_.pid) -and ($explorerHwndsBefore -notcontains [long]$_.hwnd)
    })
    foreach ($leftover in $newExplorerWindows) {
        try { Close-QnOwnedWindow -Hwnd ([IntPtr]$leftover.hwnd) } catch { }
    }
    Start-Sleep -Milliseconds 500
    Record 'cleanup_windows' 'explorer windows closed'
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Record 'cleanup_config' 'config removed'
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
    Record 'cleanup_done' 'fixture folder removed'
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
