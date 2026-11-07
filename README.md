# QuickNest

QuickNest 是依据 [V1.2 开发基线](QuickNest_V1.2_正式开发基线.md) 开发的 Windows 原生快捷启动器，工程版本 **1.2.0**。支持分组/分类与项目管理、EXE/LNK/文件/文件夹/URI/UWP/MSI 入口、搜索与拼音、热键与托盘、拖拽、批量/组合/延时启动、主题、备份及导入导出；提供逐用户安装与便携打包方式。

**当前状态：2026-11-07 再次审核后的 12 项问题已修复，并通过统一安全回归与本地 Release 构建；P4 完整发布尚未通过，不宣称全矩阵验收。** 最新源码修复、验证命令、真实结果及跳过项目见 [再次审核修复记录](docs/AUDIT_REPAIRS_2026-11-07.md)。先前的测试数量和截图完成声明不代表本轮源码。

本轮只重建本地 `target/release/quicknest.exe`；已发布 v1.2.0 的安装包和便携包仍对应此前构建，其历史哈希见 [发布构建记录](docs/AUDIT_FIXES_2026-11-07.md)，不能用于认证本轮修复。完整发布仍待 Win10、安装完成页真实 Medium 启动、两个不同生产版本真实退出换版、独立生产 key + HTTPS 端点。更新仅支持 raw，setup/portablezip 在下载前拒绝；旧测试密钥已披露，不受生产信任。runasoriginaluser 和 asInvoker 不保证所有上下文都降到 Medium。

配置、图标、备份与崩溃日志使用统一数据根；安装版默认 %APPDATA%/QuickNest，便携版为程序目录下 data。详见 [发布说明](RELEASE_NOTES.md) 与 [测试隔离用法](docs/TEST_HARNESS.md)。

## 构建环境

- Windows x64，MSVC x64 构建工具及 Windows SDK。
- Rust Stable **1.96.0**（`rust-toolchain.toml` 固定）。
- Slint / slint-build **1.18.1**，原生 winit 后端与 software renderer。
- Cargo.lock 固定传递依赖；不依赖浏览器内核或 Node/Python 运行时。

首次构建需要取得 Rust 工具链和 Cargo 依赖，运行编译后的 EXE 不需要 Rust/Cargo。Windows 系统 DLL 和 MSVC 运行库的实际依赖以验证结果为准。

```powershell
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
```

## 历史操作与逐任务记录（已过时）

以下内容保留各任务当时的操作、预期、结果及后续计划，仅用于追溯。旧命令、固定坐标、路径、测试密钥与版本结论不能直接用于当前验收；执行测试前须遵循 [隔离契约](docs/TEST_HARNESS.md)。本轮没有运行桌面回归，当前状态仅见 [本轮修复记录](docs/AUDIT_FIXES_2026-11-07.md)。

## 历史操作（已过时）：运行 P0-01 验证程序

```powershell
.\target\release\quicknest.exe
.\target\release\quicknest.exe --smoke-test
```

P0-01 当时的原型普通模式仅有静态演示网格与隐藏/退出按钮。该行为已过时；当前产品支持真实项目、托盘与关闭隐藏，不应按原型操作说明判断当前构建。

`--smoke-test` 每隔 2 秒改变一次窗口可见性，执行 3 轮隐藏/恢复，约 14 秒后正常退出。隐藏时保留事件循环，恢复使用 Slint 窗口 API。

图标与中文名称均为内嵌演示数据，不执行启动命令，不读取或修改用户配置。

## 历史操作（已过时）：Windows 外部验证

在已解锁的交互式 Windows 桌面执行（直接启动会继承当前权限）：

```powershell
powershell.exe -NoProfile -File .\scripts\validate-p0-01.ps1 `
  -ExePath .\target\release\quicknest.exe `
  -ResultsDir .\docs\test-results\P0-01
```

脚本在新的 `run-*` 目录写入 PE、Manifest、窗口可见性、退出码和截图证据。它不会编译程序，也不会实现产品的 HWND 功能。截图期间临时置顶被测窗口，随后恢复原状态。它的 `manualItems` 必须结合截图和实际按钮操作检查，不能仅凭脚本退出码宣布 P0-01 完成。

辅助脚本：

- `scripts/run-p0-01-validation.ps1`：通过桌面 Explorer 启动验证器，实际权限仍以结果为准。
- `scripts/run-p0-01-medium-validation.ps1`：在当前高权限开发会话中，仅为测试子进程创建受限 Medium 令牌，不改系统策略；这不是产品降权实现。
- `scripts/test-p0-01-buttons.ps1 -ResultsDir <结果目录>`：按已观察的 DPI 96 窗口按钮坐标派发窗口鼠标消息，验证隐藏/恢复和退出回调，记录加载模块与子进程。不是物理鼠标或 UI Automation 测试。

P0-01 当时记录过动态 MSVC 依赖；这些属于旧构建。本轮最终 EXE 已检查普通及延迟导入表，无 VCRUNTIME/MSVCP/CONCRT 动态运行库依赖，仍依赖 Windows 系统 DLL；不代表 Win10 兼容性实测通过。

## 历史操作（已过时）：运行 P0-02 HWND 生命周期验证

```powershell
.\target\release\quicknest.exe --hwnd-test .\lifecycle.tsv
powershell.exe -NoProfile -File .\scripts\validate-p0-02.ps1
```

报告路径必须不存在，父目录须已存在。独立验证脚本会为每次运行创建新目录，保存内部 TSV、外部窗口采样、退出码、权限及 SHA-256。

每次验证创建三代窗口，每代显示、隐藏、恢复后释放组件与句柄 provider，保持事件循环运行，等待 `IsWindow` 变为 false 并确认至少 500ms，再创建下一代。数值 HWND 只作为诊断观察，不能作为长期有效的操作凭据；系统可复用句柄。若存在 `SLINT_DESTROY_WINDOW_ON_HIDE` 环境变量，验证拒绝运行，不擅自覆盖环境设置。

高权限开发会话可复用已有测试启动器：

```powershell
powershell.exe -NoProfile -File .\scripts\run-p0-01-medium-validation.ps1 -Task P0-02
```

`-Task` 默认仍为 P0-01。该启动器只处理测试子进程，不代表产品已实现降权启动。`scripts/test-p0-02-errors.ps1` 覆盖报告文件保护、环境覆盖拒绝和提前关闭失败路径。

## 历史操作（已过时）：运行 P0-03 OLE STA 初始化验证

```powershell
.\target\release\quicknest.exe --ole-test .\ole.tsv
powershell.exe -NoProfile -File .\scripts\validate-p0-03.ps1
powershell.exe -NoProfile -File .\scripts\run-p0-01-medium-validation.ps1 -Task P0-03
```

`ole.tsv` 及同目录 `ole.hwnd.tsv` 必须不存在，父目录须存在。独立脚本保存 HRESULT/Apartment/线程日志，并复用 HWND 外部观察。隔离线程分别验证第一次 OLE S_OK、重复 OLE S_FALSE、MTA 冲突 RPC_E_CHANGED_MODE 和同线程清理；UI 持有本层 OLE 引用时验证 STA、定时回调、跨线程事件及窗口生命周期。

正常启动路径在后端创建前初始化 OLE，并使用不可跨线程的 RAII Guard 管理自己的成功引用。P0-03 当时后端默认拖放会持有自身 OLE 初始化引用；P0-04 起在建窗前关闭 winit 默认拖放注册。本层仍不替后端清理其他 COM 引用；释放自己的 OLE 引用后 UI 仍为 STA 是已记录的实测状态。

P0-03 验收范围不包含实际拖入功能、完整 OLE 回调或 Shell STA 服务。

`test-p0-03-errors.ps1` 检查已有报告保护、目录和参数错误路径。初始化测试通过不代表后续 OLE 拖放已通过。

## 历史操作（已过时）：运行 P0-04 IDropTarget 注册撤销验证

```powershell
.\target\release\quicknest.exe --drop-target-test .\drop.tsv
powershell.exe -NoProfile -File .\scripts\validate-p0-04.ps1
powershell.exe -NoProfile -File .\scripts\run-p0-01-medium-validation.ps1 -Task P0-04
```

`drop.tsv` 与 `drop.hwnd.tsv` 必须不存在，父目录须存在。验证模式在三代真实 Slint 窗口上注册自有 IDropTarget，检查重复注册冲突、显式撤销、重注册、RAII 清理、COM 对象析构和窗口销毁。外部脚本复用 HWND 独立观察，并核对 HRESULT 和线程归属。

此 P0-04 目标只拒绝数据效果，不读取 IDataObject。P0-05 新增了独立文件接收目标和验证模式，保留此拒绝目标用于注册回归。普通演示模式没有产品级文件导入；直接 COM 回调测试不能替代真实 Explorer 拖放测试。

Slint 的 `unstable-winit-030` 钩子仅用于建窗前关闭 winit 默认拖放注册，继续使用原 winit/software 后端。Slint 升级须重新验证此版本相关接口。winit 在窗口销毁时仍有额外撤销尝试，本层已在 HWND 存活时完成撤销，不依赖该后端清理。

错误路径可执行 `scripts/test-p0-03-errors.ps1 -Mode DropTarget -ResultsDir <目录>`；脚本名保留兼容，默认模式仍为 OLE。

## 历史操作（已过时）：运行 P0-05 Explorer 文件拖入验证

```powershell
.\target\release\quicknest.exe --file-drop-test .\files.tsv
powershell.exe -NoProfile -File .\scripts\start-p0-05-interactive.ps1
```

独立模式在窗口创建后注册接收 CF_HDROP 的 IDropTarget，复制 Unicode 路径及原始屏幕坐标到普通 Rust 事件队列，UI 显示接收统计并记录日志。报告文件必须不存在，父目录须存在。拖入只保存引用，不复制/移动/执行样本；LNK 不解析，文件夹不递归。正常演示模式仍只演示基础网格。

`start-p0-05-interactive.ps1` 建立新的受控样本和报告目录，以测试受限令牌启动 Medium 目标和独立 Explorer。必须查看实际窗口进程权限；启动请求本身不代表普通权限源已就绪。脚本不修改 UAC、账户或桌面 Explorer。

测试辅助脚本：

- `exercise-p0-05-explorer.ps1`：在已观察并定位的实际 Explorer 文件窗口中选择和拖动样本，覆盖 EXE/文件夹/LNK/Unicode/多选/移出/ESC；每个动作核对 Drop 回调计数，不接受被遮挡的鼠标命中。
- `finish-p0-05-interactive.ps1`：逐项核对样本哈希、双方响应和权限，再正常关闭目标并记录退出码/清理。
- `assert-p0-05-interactive.ps1`：核对实际路径顺序、动作计数、介质配对释放与 COM 析构，写入验收摘要。
- `test-p0-03-errors.ps1 -Mode FileDrop -ResultsDir <目录>`：已有报告保护、缺失目录和参数错误路径；默认仍为 OLE 模式。

真实 Explorer P0-05 主验收见 `docs/test-results/P0-05/interactive-39ab013c/`，Windows 11 x64、100% 缩放、双方 Medium，5 次落下共 6 路径，移出/取消不新增数据。该历史结果不包含后续 DPI 转换、分类入库、持久化、图标提取或启动。

## 历史操作（已过时）：运行 P0-09 Named Pipe IPC 验证

```powershell
.\target\release\quicknest.exe --ipc-test .\ipc.tsv
powershell.exe -NoProfile -File .\scripts\run-p0-09-medium.ps1
```

Pipe 名与 Mutex 同源（`\\.\pipe\QuickNest-{SID_HASH}-{SESSION_ID}`）。安全描述符为 SDDL `D:P(A;;GA;;;当前用户)(A;;GA;;;SY)`，管道标志含 `PIPE_REJECT_REMOTE_CLIENTS`，消息模式，单实例。协议为 4 字节小端长度前缀 + UTF-8 JSON（1..4096 字节），命令白名单 show/hide/toggle/show_and_focus/open_search，`deny_unknown_fields`，版本必须为 1。服务端通过 `ImpersonateNamedPipeClient` + 线程 TokenUser SID 比较验证客户端身份（PID 仅作审计）。客户端连接总预算 1500ms（基线 9.5 启动竞态），超时则重新竞争 Mutex，不启动第二主实例。

验证场景：Mutex 先于 Pipe 获取 → 延迟 400ms 启动 Pipe → 子进程在预算内重试连接 → 5 个白名单命令按序抵达 → 4 种畸形帧（未知命令/错误版本/坏 JSON/未知字段）全部被拒 → 服务停止/重启。Medium 受限令牌运行由 `scripts/run-p0-09-medium.ps1` 提供。

## 历史操作（已过时）：运行 P0-10 全局快捷键验证

```powershell
.\target\release\quicknest.exe --hotkey-test .\hotkey.tsv
powershell.exe -NoProfile -File .\scripts\run-p0-10-medium.ps1
```

默认组合 Ctrl+Alt+Space（`MOD_CONTROL|MOD_ALT|MOD_NOREPEAT`，基线 11.1）。`HotkeyGuard` 在专用消息循环线程上 `RegisterHotKey(NULL, …)` 并泵 `GetMessageW`；`WM_HOTKEY` 事件经 mpsc 送达，UI 侧通过 `invoke_from_event_loop` 线程安全转发；注销（Drop）在注册线程执行 `UnregisterHotKey`，配合 `PostThreadMessageW` 唤醒与 `PostQuitMessage` 优雅退出。`RegisterHotKey` 返回 `ERROR_HOTKEY_ALREADY_REGISTERED`（1409）被显式映射为冲突证据。

验证场景：注册成功 → 子进程 `--hotkey-conflict` 探测同组合被 1409 拒绝 → `SendInput` 注入真实按键、事件到达 UI（`forwarded_to_ui=1`）→ Drop 后子进程注册成功（注销清理证据）→ 重注册并再次注入事件到达 → Drop 退出。搜索快捷键（基线 11.2）与系统托盘（11.3）属后续任务。Medium 受限令牌运行由 `scripts/run-p0-10-medium.ps1` 提供。

## 历史操作（已过时）：运行 P0-11 系统托盘验证

```powershell
.\target\release\quicknest.exe --tray-test .\tray.tsv
powershell.exe -NoProfile -File .\scripts\run-p0-11-medium.ps1
```

`TrayService` 在专用消息循环线程上持有通知区域图标（`Shell_NotifyIconW` NIM_ADD/NIM_MODIFY/NIM_DELETE）与隐藏回调窗口；右键弹出基线 11.3 六项菜单（显示/隐藏 QuickNest、固定-取消固定、设置、检查更新、退出程序），左键/双击/右键经回调消息路由为事件。Explorer 重启广播 `TaskbarCreated` 时托盘线程自动重加图标。

验证场景：NIM_ADD → NIM_MODIFY → 菜单六项经菜单 API 核查 → 左键回调路由（与 Explorer 生产投递形态一致）→ 真实弹出菜单 + SendInput ESC 关闭 → Down+Enter 选中第一项（`MenuSelected(Show)`）→ 删图标后广播 TaskbarCreated 验证重建 → Drop 后托盘线程 NIM_DELETE 成功。注意：在本机（Explorer 为高完整性会话）Medium 受限令牌被 UIPI 拒绝（`ERROR_ACCESS_DENIED`），`run-p0-11-medium.ps1` 会把该拒绝识别为已记录的环境限制而非通过；完整验收以开发会话实测为准。托盘菜单命令的真实窗口动作属 P0-12。

## 历史操作（已过时）：运行 P0-12 窗口焦点状态机验证

```powershell
.\target\release\quicknest.exe --focus-test .\focus.tsv
powershell.exe -NoProfile -File .\scripts\run-p0-12-medium.ps1
```

`window_state.rs` 是纯逻辑状态机（Hidden/Showing/Visible/Hiding/ShuttingDown + 六类 `SuppressionToken` RAII 抑制锁 + 固定镜像为 Pinned 锁 + 120ms 一次性失焦复查），14 项单测覆盖全部转换与抑制语义。`--focus-test` 把 ESC（Slint FocusScope）、Ctrl+Alt+Space 热键、托盘左键/菜单三类真实事件源接到状态机上，并用第二个实例窗口做真实失焦源。

验证场景：状态转换路径 → ESC 隐藏 → 热键 toggle ×2 → 托盘 toggle ×2 → 托盘菜单 Show/Hide（真实鼠标点击菜单项）→ 外部窗口夺前台后 120ms 复查再隐藏 → 固定后失焦不隐藏 → 取消固定恢复 → RAII 抑制锁保持/释放 → 回归前台后复查不闪隐 → ShuttingDown 终态拒绝。Medium 下 ESC 走窗口消息路径、热键/托盘注入受 UIPI 环境限制时如实记录（本机 Explorer 为高完整性，物理按键不受影响），blur 全链在 Medium 真实跑通。

## 历史操作（已过时）：运行 P0-13 ReplaceFileW 保存验证

```powershell
.\target\release\quicknest.exe --save-test .\save.tsv
powershell.exe -NoProfile -File .\scripts\run-p0-13-medium.ps1
```

`config_save.rs` 实现基线第 15 章的原子保存链：同目录唯一临时文件 `config.<PID>.<UUID>.tmp`（独占创建 + `sync_all` + 关句柄）→ 首次创建 `MoveFileExW(MOVEFILE_WRITE_THROUGH)` 同卷移动、目标已存在走 `ReplaceFileW(flags=0)` 原子替换并生成唯一备份 `config.backup.<UTC戳>.json`；失败保留原配置、保留临时文件供诊断、记录 GetLastError。并发创建（首存竞态）回退替换路径，被竞态创建的文件进备份不丢失。`recover_latest_backup` 按文件名倒序找最近**有效** JSON 备份恢复，损坏文件改名 `config.corrupted.<戳>.json`，无有效备份进安全模式不动原文件；保存与恢复共用单写者锁，持锁期间并发保存被 `Busy` 拒绝。

验证场景：首存无残留 → 替换生成备份持旧内容 → 真实锁定目标（`ERROR_SHARING_VIOLATION`）下保存失败且原配置完好、tmp 保留诊断 → 备份恢复选最新有效并改名损坏文件 → 安全模式不动原文件。防抖、备份保留上限、Schema 迁移属 P1。

## 历史操作（已过时）：运行 P3-05/06/07 排序验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-0567-sorting.ps1
```

网格支持四种排序模式：手动（默认，即拖拽排序的配置顺序）、最近使用（last_used 降序）、高频（launch_count 降序）、收藏置顶（favorite 优先，其余保持手动序）；从标题栏“设置”菜单的四个“排序：…”项即时切换并持久化，状态栏标题显示当前模式。单击/批量/组合的成功启动都会记录 last_used 与 launch_count；卡片右键菜单可“收藏/取消收藏”（即时生效、双向落盘）。

验证脚本：种子手动序 A/B/C（B 高频最旧、C 次新且收藏）→ recent 网格 B、C、A → 设置菜单切高频（config.sort_mode=frequent，网格 B、C、A）→ 切收藏置顶（网格 C、A、B）→ 卡片菜单取消 C 收藏（config 中 favorite=false）→ 退出 exit=0 且排序模式保持。


## 历史操作（已过时）：运行 P3-08 自定义命令验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-08-command.ps1
```

命令类型项目经结构化命令引擎启动：direct（CreateProcessW，不经过 cmd.exe）、cmd（`cmd.exe /d /s /c`）、powershell（`powershell.exe -NoProfile -File`）；编辑器为命令类型提供“执行模式”下拉。首次执行未确认命令弹出确认对话框（程序/参数/工作目录/执行方式，可取消）；确认一次后标记清除、后续直接启动；批量与组合启动跳过未确认命令并在状态栏报告。

验证脚本：种子 4 个未确认命令项（direct/cmd/powershell/批量跳过，各写探针文件）→ direct 确认（探针+标记清除）→ 二次单击无对话框 → cmd 取消（探针不存在+标记保留）再确认 → powershell 确认 → 批量启动 c1+c4（c1 启动、c4 跳过且标记保留）→ exit=0。


## 历史操作（已过时）：运行 P3-09 Everything 集成验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-09-everything.ps1
```

Everything 集成为可选（opt-in）：设置菜单“Everything 搜索：开/关”切换并持久化；开启后搜索中心在有关键词时追加“在 Everything 中搜索…”移交行，激活即以 `Everything.exe -search <查询>` 启动；路径按“配置路径→标准安装位置回退”解析，全部缺失时状态栏提示“未找到 Everything”，主程序不受影响。

验证脚本：种子开启集成（配置路径指向探针 .cmd）→ 搜索 qnprobe → 回车激活移交行（探针含 `-search qnprobe`）→ 设置菜单切换关闭（空结果回车无操作、探针不变）→ 重启配置缺失路径（仅提示、主程序存活）→ exit=0。


## 运行卡片 UI 规范 V1.0 验证

卡片规范不改变任何业务逻辑，验证以截图与单元测试为主：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-10-icons.ps1
```

- 卡片统一为一套组件（88×100px、圆角 11px）：图标区 40×40 内放 32×32 `contain` 图像，标题 12px 单行省略，图标与标题处于同一垂直中心线。
- 图标按类型取用：EXE/文件/文件夹用系统原生图标，URL 用地球图标（`assets/browser.svg`），命令用终端图标（`assets/terminal.svg`）。
- 红色角标（15px，右上 5px）悬浮显示具体原因（"目标不存在或路径已失效：…"）。
- hover（浅蓝底 + 描边 + 轻微阴影）、按下、焦点（蓝色描边）三态齐备，深浅色主题自适应。


## 历史操作（已过时）：运行 P4-01 安装包验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-01-installer.ps1
```

标准 Inno Setup 向导安装包（installer/QuickNest.iss → installer/output/QuickNest-1.2.0-setup.exe）：逐用户安装（%LOCALAPPDATA%ProgramsQuickNest，无 UAC），开始菜单快捷方式（应用+卸载）、可选桌面快捷方式、可选开机启动（默认关闭）、"应用和功能"卸载入口、安装后普通权限启动（exe 清单 asInvoker）。

验证脚本（全链路自动装/卸，幂等）：静默安装（含桌面+开机启动任务）→ 验证文件/快捷方式/HKCU Run/ARP → 启动应用（asInvoker + 逐用户）→ 静默卸载 → 验证全清理 → 默认任务重装（桌面图标创建、开机启动关闭）→ 二次卸载 → exit=0。


## 历史操作（已过时）：运行 P4-02 便携版验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\installer\build-portable.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-02-portable.ps1
```

便携包（installer/output/QuickNest-1.2.0-portable.zip）内含 quicknest.exe + QuickNest.portable 标记：解压到任意目录双击即用；数据目录落在 <exe 目录>data（随文件夹移动）；相对路径项目（如 toolsapp.exe）在启动、缺失校验、图标提取、"打开文件所在位置"四个边界统一按便携根解析——整包移动到另一盘符后数据与相对路径项目照常工作；安装版（无标记）行为不变。

验证脚本（自动构建→解压→种相对探针→启动→跨盘移动→新根再启动→清理）：首启 config 落在文件夹内、%APPDATA% 无写入（反向断言）、探针随新根解析、跨盘 E:→C: 实测，exit=0。


## 历史操作（已过时）：运行 P4-03 降权启动验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-03-deelevate.ps1
```

历史检查确认完成页条目含 `runasoriginaluser`，但该标志只能使用安装器可取得的原始令牌；原始上下文已是 High 时不保证降权。`asInvoker` 继承调用者权限；完成页真实启动到 Medium 仍待补验。

旧脚本曾以提权上下文模拟启动报告 exit=0，未覆盖真实完成页；当前已删除模拟与全通过声明，以完成页未实测 blocked / 非零收口。


## 历史操作（已过时）：运行 P4-04/05/06 自动更新验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-040506-update.ps1
```

自动更新（基线 23 章九步链路）：检查版本 → HTTPS 获取签名清单 → ECDSA P-256 验签 → 下载包 → SHA-256 校验 → 备份旧程序（.bak 回滚点）→ 放置 → 放置后哈希复验 → 成功/回滚（旧程序逐字节恢复）。data 目录从不被触碰；更新默认不静默应用（需用户确认）；检查可关闭。

验证脚本自动编排：本地 HTTPS 服务器（自签名证书入 Root，用后即清）+ ECDSA 签名清单 → 应用 --update-test 走 12 阶段链（含篡改清单拒绝、损坏包拒绝两条反向断言）→ exit=0。


## 历史操作（已过时）：运行 P4-07 崩溃日志验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-07-crash-log.ps1
```

panic hook 在启动最早期安装；panic 时向数据目录的 crash-log.txt 追加记录（时间戳/线程/源位置/消息）。验证脚本：--crash-test 触发真实 panic（exit=101）→ 日志内容断言 → 二次崩溃追加语义 → 清理，exit=0。


## 历史操作（已过时）：运行 P4-10 数据迁移验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p4-10-migration.ps1
```

数据迁移（版本升级场景）：种 v1.1 形状 config（无 P3/P4 新增字段）→ 当前版本启动 → 用户数据（分组/分类/项目/主题）完整呈现 → 触发一次保存 → config 断言 schema_version=2、数据未丢、新字段（延时/执行模式/使用统计/排序等）以默认值补齐 → 迁移后的项目可正常启动 → exit=0。


## 历史产物路径（不认证当前构建）

```text
installer/output/QuickNest-1.2.0-setup.exe      安装版（逐用户向导安装）
installer/output/QuickNest-1.2.0-portable.zip   便携版（解压即用）
target/release/quicknest.exe                    裸程序
```

以上三个路径的本轮产物已重建并核验。发布说明见 [RELEASE_NOTES.md](RELEASE_NOTES.md)；本轮门禁结果、构建证据和完整哈希见 [本轮修复记录](docs/AUDIT_FIXES_2026-11-07.md)。P4 仍有多项发布前置条件，不能按仅 Win10 遗留处理。


P0 阶段当时的顺序门禁与操作仅作历史追溯，见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P0 验证记录](docs/P0_VALIDATION.md)；不据此认证当前构建。

## 历史操作（已过时）：运行 P1-01 主界面验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-01-ui.ps1
```

主界面按基线 2.1 实现五区域：无边框窗口 + 自定义标题栏（Logo/名称/拖动区/搜索/固定/设置/×）+ 顶部一级分组条 + 左侧二级分类栏 + 右侧项目网格 + 状态栏。标题栏拖动由 Slint 内置 `WindowMoveArea` 驱动（winit `drag_window`），关闭按钮按基线 2.5 隐藏面板而非退出。

验证脚本依次核对：窗口发现 → 无边框判定（客户区覆盖窗口顶部，client_top_offset=0）→ 布局截图 → 标题栏拖动位移（实测 dx=180 dy=90 与光标位移匹配）→ WM_CLOSE 干净退出。分组/分类/网格的交互分别属 P1-02～P1-05。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-02 一级分组验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-02-groups.ps1
```

分组条与分类栏由三层数据模型驱动（`library_model.rs`：分组→分类→项目，全部实体 UUID v4，持久化形状对齐基线 14.2——`current_group_id`/`color`/`last_category_id`/`item_type`+`launch`/顶层 `settings`）。点击分组切换其二级分类列表并记忆 `last_category_id`；新建分组为真实用户分组（UUID 标识）；重命名/强调色/排序/删除确认的操作逻辑与原子持久化（P0-13 `save_atomic`）已就绪并有单测，对话框 UI 与拖拽手势属后续迭代。

验证脚本依次点击 应用→自动化→常用工具 并截图核对：高亮切换、分类栏联动（应用的 2 项 / 自动化的 1 项 / 常用工具的 7 项）、`last_category_id` 记忆、WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-03 二级分类验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-03-categories.ps1
```

分类栏支持创建（对话框）、右键菜单（重命名/上移/下移/删除，Slint `ContextMenuArea` 原生菜单）、重命名（预填原名、空名/重名拒绝）、删除（空分类直接删除；非空分类确认对话框可选迁移项目或删除项目）与垂直滚动；全部操作经 P0-13 `save_atomic` 写入 `%APPDATA%\QuickNest\config.json`，重启后顺序与名称保持。

验证脚本对新建的 TestCat 行执行完整链路：右键第 8 行 → 重命名（全选替换为 RenamedCat）→ 上移（断言配置顺序在 AI 之前）→ 下移（恢复在 AI 之后）→ 删除（断言 RenamedCat 消失且示例分类 `tool-category-1` 保留），并在每阶段截图、核对配置文件、WM_CLOSE 干净退出；stderr 收集证明无 panic。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-04 项目网格验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-04-grid.ps1
```

项目网格由当前分类的 `Category.items` 驱动：切换分类/分组时网格联动；列数随窗口宽度自适应；图标三档（small/medium/large，默认 medium，来自 `settings.icon_size`）；名称最多两行省略；路径失效的 Exe/Lnk/File/Folder 显示红色 "!" 角标；悬停显示完整名称/路径/备注；卡片右键（打开/编辑/删除）与空白右键（添加项目）菜单已就绪（操作分别属 P1-05/P1-06）。

验证脚本依次采集：分类切换（空分类 0 项目）→ 中图标初始截图（6 示例项目、2 个角标）→ 悬停 tooltip → 卡片菜单 → 空白菜单 → 680px（5 列两行）与 1100px 窗口截图 → `icon_size=small/large` 重启截图（76x88/128x140）→ 三次 WM_CLOSE exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-05 项目 CRUD 验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-05-crud.ps1
```

项目 CRUD 已按基线 3.1～3.5 实现：空白右键"添加项目"打开编辑对话框（名称/类型/目标/参数/工作目录/图标/窗口状态/管理员/备注），"浏览…"调用原生文件选择窗口并自动推断名称/类型/默认工作目录；右键菜单提供 打开/管理员身份运行/编辑/打开文件所在位置/复制路径/移动到其他分类/复制项目/删除（打开类入口待 P1-06+ 接线）；删除带确认对话框。

验证脚本对配置文件逐阶段断言：添加（含引号参数）→ 编辑改名 → 复制项目 → 复制路径（读回剪贴板）→ 移动到第二分类 → 删除确认 → 原项目保留 → 浏览选择 notepad.exe 后自动填充并保存，最后 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-06 EXE 启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-06-launch.ps1
```

EXE 启动按基线 4.6 实现：`CreateProcessW`（参数、工作目录、窗口状态 normal/minimized/maximized）优先，失败回退 `ShellExecuteExW`；管理员运行与项目 `run_as_admin` 走 `runas`；所有失败返回结构化错误并显示在状态栏。单击卡片与右键"打开/管理员身份运行"均已接线。

验证脚本用四个预置项目实测：CmdProbe（`cmd /c echo > probe.txt`，验证参数与工作目录）、MshtaTest（菜单"打开"与"管理员身份运行"各启动一个新进程）、StartupProbeTest（自编探针断言 STARTUPINFO `flags=1;show=3`）、MissingTest（状态栏"目标不存在"，无进程），最后 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-07 普通 LNK 启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-07-lnk.ps1
```

LNK 项目按基线 4.1/4.6 通过 `ShellExecuteExW` 启动**原始快捷方式**，保留 Shell 自己的目标/参数/工作目录语义（UWP 与 MSI 广告快捷方式同样适用）；管理员运行走 `runas` 原始 LNK。导入快捷方式时用 `IShellLinkW`/`IPersistFile` 读取参数/工作目录/图标作为元数据（浏览选择与保存时都会解析）。

验证脚本创建真实 LNK（cmd + 参数 + 工作目录）后实测：单击启动生成探针文件 → 缺失 LNK 错误提示 → 编辑对话框保存 LNK（类型=快捷方式、目标=LNK 路径）→ 配置断言参数/工作目录来自 LNK → WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-08 文件和文件夹启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-08-shell-targets.ps1
```

普通文件按基线 4.6 通过 `ShellExecuteExW` 交给系统文件关联打开；文件夹在 Explorer 中打开；模型的工作目录仅作元数据不强制传入（基线 3.3）。项目右键"打开文件所在位置"通过 `SHOpenFolderAndSelectItems` 打开父目录并选中目标。

验证脚本实测：文件夹启动（Explorer 窗口标题）、文件启动（状态栏"已启动"）、右键"打开文件所在位置"（父目录窗口标题）、缺失文件错误提示，最后 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-09 URI 启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-09-uri.ps1
```

URI 项目（HTTP/HTTPS/steam/自定义协议）按基线 4.6 经 `ShellExecuteExW` 交给系统协议处理；非法 URI 与空目标返回结构化错误并显示在状态栏。

验证脚本在 HKCU 临时注册 `quicknest-p109` 协议（指向 cmd 写探针文件），单击项目后探针文件生成证明系统协议处理生效；随后验证非法 URI 错误提示；结束时清理注册表与临时目录，WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-10 图标提取验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-10-icons.ps1
```

项目图标经后台线程用 `IShellItemImageFactory::GetImage`（64x64，`SIIGBF_ICONONLY`）提取，`SHGetFileInfoW` 作为兼容回退；结果转直通 alpha RGBA 后由 UI 定时器合并，不阻塞界面。自定义图标路径优先；URI/命令/自动化项目无文件图标，提取失败保留默认素材。

验证脚本预置 cmd.exe 项目（真实图标）与 URI 项目（默认素材）：截图核对并断言图标区域默认蓝色像素比例（cmd 0.0 / URI 0.35），最后 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-11 图标缓存验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-11-icon-cache.ps1
```

图标快照以 BLAKE3 键（规范化来源 + 修改时间 + 文件大小 + 尺寸）缓存在 `%APPDATA%\QuickNest\icons\<key>.qni`（`QNI1` + RGBA）。后台线程命中读盘、未命中提取后写盘；程序更新改变键自动失效；启动时按当前库引用清理无引用快照（删除项目后下次启动回收，共享图标在仍有引用时保留）。

验证脚本实测：首启生成一个 16396 字节快照 → 二次启动命中（mtime 不变）→ 删除项目后启动清理为 0 → 三次 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-12 JSON 原子保存验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-12-atomic-save.ps1
```

配置经 `save_atomic`（临时文件 + sync + `ReplaceFileW` + 备份）写入。启动时按 `schema_version` 校验：高于/低于当前版本进入安全模式且不覆盖原文件；解析失败调用 `recover_latest_backup` 恢复最近有效备份，损坏文件改名 `config.corrupted.<timestamp>.json`；无有效备份时安全模式，所有修改只在内存生效。保存失败不再静默，状态栏显示原因。

验证脚本实测：正常保存产生备份 → 破坏配置后启动恢复（config 合法、corrupted 文件保留）→ 删除备份后再破坏（安全模式，修改操作后原文件仍为 garbage）→ `schema_version=3`（安全模式且文件未变），最后清理测试配置。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-13 基础备份验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-13-backups.ps1
```

每次保存成功后按命名时间倒序保留最近 10 份有效备份（无效 JSON 与超量旧备份删除）。状态栏"备份"入口可手动创建时间戳命名的备份（同样受 10 份保留策略约束），成功显示"已备份：<路径>"；安全模式拒绝备份。

验证脚本实测：连续 14 次保存后备份恰为 10 份 → 点击"备份"生成新备份且数量仍为 10 → 清理测试配置。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-14 快捷键验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-14-hotkeys.ps1
```

全局快捷键读取 `settings.hotkey`（默认 `Ctrl+Alt+Space`，支持自定义），经 `RegisterHotKey` + `MOD_NOREPEAT` 注册，专用线程消息循环把 WM_HOTKEY 经线程安全通道转发；触发后窗口显示并置于前台。注册/解析失败在状态栏明确提示。交互模式点击 × 保持隐藏（验证模式仍自动恢复）。Ctrl+F 路由搜索意图（搜索中心属 P2-01）。

验证脚本实测：× 隐藏 → Ctrl+Alt+Space 恢复且前台 → Ctrl+F → 自定义 `Ctrl+Alt+K` 恢复 → 无效配置错误提示，四次 WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-15 托盘验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-15-tray.ps1
```

托盘图标经 `Shell_NotifyIconW` 注册（专用线程消息循环、Explorer 重启重建、Drop 删除）。左键/双击切换面板；右键菜单六项（显示/隐藏/固定·取消固定/设置/检查更新/退出程序）。退出程序先保存配置再退出；注销快捷键、删除托盘图标与后台线程清理由 Drop 顺序完成。

验证脚本通过托盘回调消息驱动：托盘宿主窗口存在 → 左键隐藏/显示 → 菜单"隐藏 QuickNest"（菜单矩形探测，TPM_BOTTOMALIGN 在光标上方）→ 左键恢复 → 菜单"退出程序"（exit=0，配置已保存）。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-16 单实例唤醒验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-16-single-instance.ps1
```

交互模式启动时先获取 `Local\QuickNest-{SID_HASH}-{SESSION_ID}` Mutex；次实例检测到已有主实例后，经命名管道发送 `show_and_focus` 并立即退出，不创建窗口。主实例以 100ms Timer 排空管道命令，收到后显示并置前窗口；唤醒失败时按基线 9.5 重新检查所有权，只有确认原实例失效才接管。

验证脚本实测：首实例窗口 → 第二进程 exit=0 且可见窗口数仍为 1 → × 隐藏首实例 → 第三次启动唤醒（exit=0，窗口恢复可见）→ WM_CLOSE 干净退出。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-17 外部拖拽验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-17-external-drop.ps1
```

交互模式主窗口注册 OLE `IDropTarget`（`CF_HDROP`）。把文件/文件夹从资源管理器拖到右侧网格会追加到当前分类；拖到左侧分类行会加入该分类；拖到顶部分组标签并悬停 400ms 会切换目标分组。多文件按拖入顺序添加；只保存路径引用，不复制或移动源文件。拖拽期间目标行/网格高亮并在状态栏提示，ESC 或拖出窗口取消不写入数据。

验证脚本使用真实 Explorer 与鼠标输入：网格 EXE、分类行文件、双文件顺序、分组弹簧加载、ESC 取消、拖出取消、源文件哈希不变、WM_CLOSE exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P1-18 自动隐藏验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p1-18-auto-hide.ps1
```

交互模式按基线第 10 章管理焦点：失焦后等待 120ms 复查，确认前台仍是外部窗口才隐藏；标题栏与托盘"固定"抑制失焦/启动隐藏；对话框、托盘菜单、外部拖拽与管理员启动期间持有抑制锁；成功启动项目且 `auto_hide_on_launch=true` 时隐藏，失败保留面板。启动后首次获得前台之前不会自动隐藏，避免未见面板即消失。

验证脚本实测：外部前台隐藏 → 热键恢复 → 固定不隐藏 → 固定下启动不隐藏 → 取消固定恢复隐藏 → 对话框抑制 → 关闭后隐藏 → 启动失败保留 → 成功启动隐藏 → WM_CLOSE exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P1 验证记录](docs/P1_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-01 搜索中心验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-01-search.ps1
```

`Ctrl+F` 或点击标题栏"搜索"打开搜索中心：输入即时过滤已登记项目（名称完全/前缀/包含/模糊、路径包含，大小写不敏感），默认范围当前分组，点击"范围"切换全部分组；点击结果启动并关闭，Esc 先清空关键词再关闭。拼音与纯键盘流分别属 P2-02/P2-03。

验证脚本使用真实输入与点击：Ctrl+F 打开 → `alpha` 过滤并启动 → 标题入口重开 → `beta` 当前分组无结果 → 全部分组启动 BetaTwo → Esc 清空 → Esc 关闭 → WM_CLOSE exit=0。

## 历史操作（已过时）：运行 P2-02 中文拼音验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-02-pinyin.ps1
```

项目名称会生成拼音索引：全拼（`weixin`）、首字母（`wx`）与常见多音字候选（`音乐` 的 `yinyue`）。索引按项目名称缓存，重命名后自动重建；拼音匹配排在直接名称之后、路径匹配之前。

验证脚本用真实输入验证：`weixin`/`wx` → 微信测试，`wyy` → 网易云音乐，`yinyue` → 音乐盒，每次点击结果并校验启动探针。

## 历史操作（已过时）：运行 P2-03 纯键盘搜索验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-03-keyboard.ps1
```

搜索中心默认选中第一条结果；↑/← 与 ↓/→ 移动选中项并循环，Enter 启动，Ctrl+Enter 管理员启动，Ctrl+D 打开所在位置，Tab 切换当前分组/全部分组，Esc 先清空再关闭。选中行有高亮，鼠标点击结果仍可用。

验证脚本用真实按键：默认首条 → Down 选中并启动第二项 → Up 从首条循环到最后一项 → Tab 切换全部分组并启动跨组结果 → Ctrl+D 打开所在文件夹 → Esc 清空/关闭 → exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-04 内部排序验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-04-sorting.ps1
```

当前分类内按住卡片拖动超过 8px 进入排序：源卡片变暗，网格显示蓝色插入位置竖线；释放后项目移动到目标槽位并立即保存。拖拽中按 Esc 取消，不改变顺序；普通单击仍启动项目，右键菜单不受影响。

验证脚本用真实鼠标：SortA 拖到第三槽（顺序变 B,C,A）→ 再次拖拽中途 Esc（顺序不变）→ 单击 SortB 启动 → exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-05 跨分类拖拽验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-05-cross-category.ps1
```

把网格卡片拖到左侧另一个分类行：目标分类行高亮并在状态栏提示"松开以移动到「分类」"；释放后项目移动到该分类并立即保存，源分类网格与目标分类计数同步更新。拖到当前分类或网格仍按内部排序；拖拽中 Esc 取消。

验证脚本用真实鼠标：MoveMe 从 CatA 拖到 CatB（配置变为 CatA=StayHere、CatB=MoveMe）→ 再次拖拽中途 Esc（分类不变）→ exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-06 跨分组拖拽验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-06-cross-group.ps1
```

把网格卡片拖到顶部一级分组标签：悬停 400ms 弹簧加载切换到该分组（侧栏随动，状态栏提示"已切换到「分组」"），释放后项目移动到该分组当前分类；快速落点（<400ms）直接按目标分组移动。拖拽中 Esc 取消移动，仅保留已发生的分组切换。

验证脚本用真实鼠标：CrossItem 弹簧悬停 700ms 移入 GroupB → FastItem 快速落点移入 GroupB → KeepItem 悬停后 Esc（项目不动、分组已切换）→ exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-07 UWP 启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-07-uwp.ps1
```

导入指向 `shell:AppsFolder` 的 UWP 快捷方式时，程序从目标 Shell 项读取 `System.AppUserModel.ID` 并保存到项目的 `launch.aumid`（不凭显示名称猜测）；项目保持原始 LNK 引用，启动仍通过 `ShellExecuteExW` 原始快捷方式。AUMID 直接激活属 P2-08。

验证脚本：创建计算器的 UWP 快捷方式 → 添加对话框导入（断言配置中 aumid 持久化）→ 右键"打开"启动原始 LNK → 计算器窗口出现 → 关闭 → exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-08 AUMID 激活验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-08-aumid.ps1
```

带 AUMID 的 UWP 项目（非管理员）启动时先经 `IApplicationActivationManager::ActivateApplication` 直接激活（不要求原 LNK 文件存在）；激活失败回退原始 LNK 的 ShellExecuteExW；管理员运行保持 runas 原始 LNK 的 Shell 语义。已运行的应用重新激活聚焦现有窗口。

验证脚本：种子项目 AUMID=计算器 且原 LNK 路径不存在 → 单击卡片 → 计算器窗口出现（证明 AUMID 激活）→ 再次单击重新激活 → 关闭 → exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-09 MSI 快捷方式验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-09-msi.ps1
```

MSI 广告快捷方式（LNK 带 `SLDF_HAS_DARWINID`/Darwin 块）导入后保留原始 LNK，不解析 EXE、不复制或重写；启动始终交给 `ShellExecuteExW`，由 Windows Installer 负责安装、修复和组件激活。快捷方式失效时状态栏提示"请修复安装或重新添加入口"。

验证脚本会在临时目录生成一个最小 MSI（Windows Installer COM 建表 + Windows SDK `MsiInfo.exe` 写 SummaryInformation），安装出真实的 Start Menu 广告快捷方式，然后通过添加对话框导入、启动 payload、卸载并验证失效提示；结束时自动卸载/清理。需要本机安装 Windows SDK 的 `MsiInfo.exe` 与 `rustc`。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-10 特殊 Shell 项目验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-10-shell.ps1
```

`shell:`/`ms-settings:` 协议、`::{CLSID}` Shell Namespace、特殊文件夹和 UNC 网络位置保存原始目标字符串，通过 `ShellExecuteExW` 交给 Windows Shell 启动；Shell/UNC 不做本地 `exists()` 探测，网格不会误报路径失效。空名称导入 `shell:` 项目时从 Shell 命名空间读取显示名（例如 `shell:MyComputerFolder` → 此电脑），"打开文件所在位置"对这类项目提示没有本地文件位置。

验证脚本：空名称导入 `shell:MyComputerFolder`（断言 type=uri、名称已补全、路径未改写）→ 启动；再追加 `::{CLSID}`、控制面板 LNK、`ms-settings:display`、`\\localhost\C$` 四项并逐个启动、截图、关闭；最后断言 5 个项目的 target/type 未被启动改写、exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-11 右键菜单验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-11-menus.ps1
```

卡片菜单保留基线 3.5 的 8 项；分组标签右键可新建/重命名分组、设置强调色、左移/右移、删除（非空分组确认迁移或删除全部）；分类菜单新增"新建项目"并作用于被右键的分类；空白区菜单提供添加项目/新建分类/新建分组。菜单在屏幕右下角会自动上翻/左移，不被截断。

验证脚本：分组重命名/设色 → 分类内新建项目（断言只进被右键分类）→ 空白区新建分组 → 删除非空分组（迁移删除与删除全部各一次）→ 分组右移 → 把窗口移到工作区右下角并打开卡片菜单，断言菜单矩形完全在工作区内；exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-12 主题切换验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-12-theme.ps1
```

标题栏"设置"菜单可即时切换浅色/深色/跟随系统与强调色（蓝/绿/橙/紫），无需重启并持久化到 `settings.theme`/`settings.accent_color`；跟随系统读取 `AppsUseLightTheme`，运行中每 2 秒轮询。对话框等 std-widgets 也随 `Palette.color-scheme` 切换。

验证脚本：浅色启动像素 → 菜单切深色（像素 + 配置）→ 切回浅色 → 跟随系统（与注册表比对）→ 绿色强调色（分类选中底色）→ 重启后深色保持；两次 exit=0。

工作站锁定时可运行补充脚本 `scripts/p2-12-theme-headless.ps1`（窗口消息驱动真实菜单 + PrintWindow 采样）。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-13 透明度和圆角验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-13-opacity-corners.ps1
```

窗口透明度 50%–100%（默认 95%）经 `WS_EX_LAYERED`+`SetLayeredWindowAttributes` 应用；圆角经 `DWMWA_WINDOW_CORNER_PREFERENCE`（Windows 11 生效，旧系统降级直角）。两者都可从标题栏"设置"菜单即时切换并持久化。

验证脚本：在面板后方放置纯红参考窗体，按合成公式（app×α + red×(1−α)，±4 容差）断言 95%/100%/70%/50% 的像素；以角落像素断言圆角开（红）与关（白标题栏）；重启后 0.7+方角保持；钳制配置 1.5→100%、0.2→50%；exit=0。

## 历史操作（已过时）：运行 P2-14 Mica/Acrylic 背景验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-14-backdrop.ps1
```

背景材质经 `DWMWA_SYSTEMBACKDROP_TYPE` 应用（Mica=`DWMSBT_MAINWINDOW`、Acrylic=`DWMSBT_TRANSIENTWINDOW`、纯色=`DWMSBT_NONE`），调用成功同时充当 Windows 11 22H2 能力探测，旧系统失败回退纯色。标题栏"设置"菜单 17 项新增 背景纯色/Mica/Acrylic，切换即时生效并持久化。当前渲染器绘制不透明客户区，材质在圆角等 DWM 管理区域合成；让材质透过客户区主体可见需要支持 alpha 的呈现路径（属渲染器升级）。

验证脚本：经设置菜单切换 纯色→Mica→Acrylic→纯色，每步以 `DwmGetWindowAttribute` 读回 1/2/3/1 并断言 config.json 同步；重启后 Mica 读回=2 保持；exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-15 DPI 和多显示器验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-15-dpi.ps1
```

Manifest 声明 Per-Monitor V2（`dpiAware=true/pm` + `dpiAwareness=PerMonitorV2`），运行窗口以 `AreDpiAwarenessContextsEqual` 验证。缩放验证用 `WM_DPICHANGED` 在应用上实测：逻辑尺寸恒为 760x520，物理尺寸在 100%/125%/150%/175% 下为 760x520 / 950x650 / 1140x780 / 1330x910。窗口位置记忆到 `settings.window_bounds`（400ms 轮询即防抖），启动时恢复；显示器缺失或位置完全离屏时按最近显示器工作区修正，不会恢复到屏幕外。

验证脚本：确认窗口 PMv2 上下文 → 四档 DPI 下读回应用报告的缩放与尺寸 → 种子离屏位置 (6000,4000) 被修正到工作区内 → 移动窗口后检查持久化的 x/y/monitor → 重启后位置恢复；exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。

## 历史操作（已过时）：运行 P2-16 备份恢复验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-16-backups.ps1
```

标题栏“设置”菜单第 18 项“历史备份恢复…”打开备份对话框：只列出内容可解析的备份（按时间倒序，默认选中最新），“立即备份”写出一份当前配置的备份，“恢复所选”经同一原子替换链写入并把被替换的配置留为备份（恢复可回退），保留窗口维持最近 10 份有效备份。

验证脚本：种子实时配置 `Current` + 有效备份 `2026-12-31`（`Restored`）+ 更晚的不可解析备份 → 打开对话框 → 恢复后实时配置变为 `Restored`（坏备份未被列出）→ 断言被替换配置留为备份且数量 ≤10 → 立即备份新增文件 → 取消关闭 → 退出后恢复结果保持；exit=0。

## 历史操作（已过时）：运行 P2-17 配置导入导出验证

```powershell
.\target\release\quicknest.exe
.\target\release\quicknest.exe --transfer-test .\docs\test-results\P2-17\p2-17-transfer-validation.tsv
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p2-17-transfer.ps1
```

设置菜单第 19 项“配置导入导出…”打开对话框：导出全部配置 / 导出当前分组写出 `quicknest-export` 文档（普通 `config.json` 也可导入）；合并导入保留现有数据、重新生成 id 并重命名同名分组；覆盖导入替换全部分组与设置。导入前自动备份当前配置，导入的命令/自动化项目标记为未验证（`confirm_on_first_run`），损坏或版本不兼容的文档被拒绝且不改动现有数据。

验证脚本：`--transfer-test` 以真实临时目录与控制器验证导出全部/单分组、合并/覆盖导入、坏文档拒绝；live 脚本验证设置菜单打开对话框、导出按钮打开原生保存对话框（取消）、合并/覆盖导入经原生打开对话框端到端生效（命令项未验证、导入前备份），退出后结果持久化。

## 历史操作（已过时）：运行 P3-01 批量启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-01-batch.ps1
```

卡片右键菜单新增“加入批量选择/移出批量选择”与“批量启动所选（N）”（N=选中数，0 时禁用），卡片高亮选中态；Ctrl+单击与菜单等效。批量启动按网格顺序逐项启动并在状态栏报告结果，完成后自动清空选择（内存态，不入库）。

验证脚本：种子 3 个探针项目（各启动 cmd 写自己的探针文件）→ 经卡片菜单将前两个加入批量选择 → 批量启动（探针 1+2 出现、探针 3 缺席）→ 再次批量启动无新增（选择已清空，探针不被改写）→ exit=0。

## 历史操作（已过时）：运行 P3-02 启动组合验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-02-combinations.ps1
```

卡片右键菜单新增“保存批量选择为组合…”（命名对话框，预填“组合 N”）与“启动组合…”；组合对话框列出全部组合（名称+项目数），选中后可“启动组合”（按保存顺序逐项启动、跳过已删除项目）或“删除”。组合随配置持久化。

验证脚本：种子 3 个探针项目（各启动 cmd 写自己的探针文件）→ 经卡片菜单选两个 → 保存组合（config.combinations=1）→ 从组合对话框启动（探针 1+2 出现）→ 删除组合（config=0）→ exit=0。`--transfer-test` 与单元测试覆盖模型行为。

## 历史操作（已过时）：运行 P3-03 延时启动验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-03-delay.ps1
```

项目可带逐项启动延时（秒，`launch.delay_seconds`）；批量启动与启动组合尊重该延时——延时项由独立线程定时启动，不阻塞其余项目与 UI；单项目单击忽略延时。

验证脚本：种子延时 0/3/6 秒的三个项目（各启动 cmd 写探针文件）→ 三卡加入批量选择 → 批量启动 → 采样探针文件：t=0.5s 仅立即项、t=4s 3 秒项、t=7.5s 6 秒项，内容=各自延时值 → exit。

## 历史操作（已过时）：运行 P3-04 鼠标热区验证

```powershell
.\target\release\quicknest.exe
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-p3-04-hotzone.ps1
```

`settings.hot_zone_enabled`（默认关闭）开启后，鼠标在配置的屏幕边缘（`hot_zone_edge`：left/right/top/bottom）停留约 600ms 切换面板显示/隐藏；光标离开边缘后才能再次触发，停留不会反复切换。热区为显式用户动作，不受 pinned/auto-hide 影响。

验证脚本：启用左边缘热区 → 面板可见 → 左边缘悬停隐藏 → 再次悬停显示 → dwell 时序断言（触边 200ms 未触发/1.4s 已触发）→ 禁用重启后悬停无效果 → exit=0。

实际完成情况见 [开发进度](docs/DEVELOPMENT_PROGRESS.md) 和 [P2 验证记录](docs/P2_VALIDATION.md)。
