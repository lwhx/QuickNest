# 测试夹具与隔离用法

本文记录测试隔离契约、历史夹具局限与使用方式。最新产品修复及验证结果见 [后续审核修复记录](AUDIT_FOLLOWUP_2026-11-08.md)；2026-11-07 的修复与构建证据见 [上一轮记录](AUDIT_REPAIRS_2026-11-07.md) 和 [发布构建记录](AUDIT_FIXES_2026-11-07.md)。

## 1. 先运行安全 preflight

在仓库根目录执行；以下命令不会运行桌面验收脚本。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-regression.ps1 -PreflightOnly
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-sandbox-preflight.ps1 -Fixture
```

本机实际文件系统清点为 **91 个 PowerShell 脚本**：36 个 `BLOCKED`、41 个 verification-contract、3 个 support-library、9 个 utility、2 个 infrastructure。预检递归枚举所有实际 `.ps1`，包含未被 Git 跟踪或被忽略的本机补充入口；不同检出目录的总数可能不同。解析、安全分类及无桌面 fixtures 通过，不代表这些脚本的桌面行为验收通过。旧“51 个脚本”只覆盖了较窄的文件名集合，不能用于当前全入口安全结论。

旧主题补充脚本及未迁移的历史桌面入口以首条无条件 `BLOCKED` 拒绝；旧菜单的 7 个 verify 选择在分配 sandbox 前非零退出，不计为通过。不要删除门禁以复用旧坐标。核心 UI 回归改用正式界面的 Slint 测试后端：

```powershell
cargo test --locked --offline --bin quicknest ui_regression_tests -- --test-threads=1
cargo test --locked --offline --test ui_layout -- --test-threads=1
```

这些测试点击正式控件并检查实际回调、可见性、保存、选中值、焦点和布局。它们不操作现有桌面，也不证明 Windows 实机 DPI、多显示器和 Shell 行为。仅保存截图的旧 `target/ui-smoke.ps1` 已禁止执行，不能用 `SMOKE_OK` 代替行为断言。

此前发布构建的 full lib 为 **272 passed / 0 failed / 3 ignored**，不代表本轮源码；三项 ignored 分别为 panic helper（父测调用）、CMD native probe（父测调用两次）、signed_fixture（独立脚本显式执行已通过）；默认汇总中的 ignored 与实际子探针覆盖应分别记录。updater 11、签名 fixture 1、transport 4 与 full lib 存在重叠，不累计为新的通过总数。PS 5.1 临时 HTTPS manifest / 404 / 降级 redirect 验证已通过；它不替代生产更新链路验收。

## 2. 数据、namespace 与文件归属

- 统一 sandbox 入口为 `scripts/test-sandbox.ps1` 的 `Initialize-QnTestSandbox`；退出时用 `Restore-QnEnvironment` 恢复环境，包括 TEMP/TMP 与 QuickNest 测试变量。
- `QUICKNEST_TEST_SANDBOX_ROOT` 是容器，调用者容器不能被替换或整体删除；每次运行由 `New-QnSandboxRun` 分配新 run root，case root 与结果均位于其下。
- `QUICKNEST_DATA_DIR` 必须是 run root 内的非空绝对路径。调用者已设定时保留并校验；无效时拒绝执行，不能回落 `%APPDATA%\QuickNest` 或真实便携数据。产品配置、缓存、备份和崩溃日志共用该根。
- `QUICKNEST_INSTANCE_NAMESPACE` 隔离测试 Mutex/Pipe 与用户实例；运行器为子脚本设置独立 namespace，同一用例需通信的进程应共享该 namespace。
- 清理由 `Remove-QnSandboxItem` 和路径包含断言约束；拒绝 root 本身、同前缀兄弟路径、`..`、通配符越界、junction/reparse 跳转及真实用户受保护路径。`ResultsDir` 在桌面运行中也必须位于本次 run root 内。
- 仅通过 `Register-QnOwnedProcess` / `Start-QnOwnedProcess` 登记归属，并由 `Stop-QnOwnedProcess` / `Stop-QnTestProcesses` 清理测试拥有的进程；不得按进程名全局终止应用。

## 3. 桌面验收只在独立一次性账户或虚拟机中执行

`QUICKNEST_DATA_DIR` 只能隔离应用数据，无法隔离安装、文件关联、Shell、证书和共享桌面的副作用。运行器默认拒绝桌面执行；须先准备独立一次性 Windows 账户或 VM，再显式启用。下面只是用法示例，本轮不执行。

```powershell
# 仅在已准备好的独立一次性账户/VM中执行。
# 参数必须填该一次性环境当前账户的真实完整身份，不能仅设置标志模拟隔离。
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-regression.ps1 `
  -AllowIsolatedDesktop `
  -DisposableAccount 'VMNAME\qn-test' `
  -Only verify-p3-03-delay.ps1
```

`-DisposableAccount` 必须与当前 Windows 身份完全一致；`-AllowIsolatedDesktop` 本身不会创建隔离环境。运行器分配新 run/case 目录并保存输出。即使显式启用，仍指向共享 Notepad/Explorer/mshta 等目标且尚无归属契约的脚本会被拒绝；先改为独立文件探针再使用，不能以取消防护或扩大清理范围来跑通。

准备失败、未知脚本、无 sandbox 初始化、解析失败和超时都必须返回非零，不计为通过。每个脚本独立 PowerShell 子进程，桌面脚本应串行运行；会占用热键、焦点或系统对象的测试也不可与其并行。

## 4. 启动探针与历史共享进程清理

历史脚本曾按进程名清理 QuickNest/Notepad、删除 Notepad TabState 或重启 Explorer。这些约定已作废：不得终止用户进程、删除用户会话或重置用户 Explorer。仅清理本次登记的测试进程、窗口与 sandbox 文件。

Win11 的 `System32\notepad.exe` 可能是打包应用启动器，多次启动的进程数/窗口数与实际启动次数不成比例，不能作为精确启动计数。优先使用测试拥有的独立探针文件，断言出现/缺席、内容与实际执行时序；同一探针可被多次写入时，还需记录唯一调用标记或追加次数。

## 5. 焦点、输入与坐标的证据边界

后台 `SetForegroundWindow` 请求可能被系统前台锁拒绝；即使测试助手尝试 `AttachThreadInput` / `BringWindowToTop`，也须实际确认目标 PID、窗口句柄、可见状态与命中位置，不能仅凭调用成功推定获得前台。未完成的 Explorer 拖放应在测试拥有的窗口中取消，不得以重启共享 Explorer 处理。

固定坐标依赖 DPI、窗口位置和当前布局，不能当作产品不变式。以下仅为历史几何同步记录，不是当前 UI 验收结果。

| 历史变化 | 对夹具的影响 |
| --- | --- |
| 分类行高由 28px 改为 36px / 38px 行距 | 分类、菜单及拖放落点需重新测量 |
| 侧栏由 148px 改为 160px | 可用网格宽度及列数随之变化，不应硬编码永远六列 |
| 卡片菜单新增收藏与批量项 | 行号需根据当前菜单重新确认 |

窗口消息、剪贴板或 `WM_PASTE` 探针仅覆盖对应应用消息路径；它们不证明物理输入或完整系统输入队列路径通过。失败后单独复跑通过仅是另一条运行证据，不能自动归类为环境抖动、排除产品缺陷，也不能改写原整轮结果。

## 6. 原因判断与历史结果

历史观察包括注入输入未送达、窗口遮挡、焦点变化、WinHTTP err=87。观察本身不能证明防护组件拦截、系统封锁或某个传输栈唯一可用；保留失败日志、当时 PID/令牌/窗口信息与可重复对照后再判断原因，不为跑通修改 UAC、账户或系统策略。

旧“48 脚本全绿”、批跑失败后的独立复跑以及更早的测试数、Release 哈希均属历史证据，不能认证本轮源码。最新命令、通过数量、跳过项目和产物范围统一见 [再次审核修复记录](AUDIT_REPAIRS_2026-11-07.md)。
