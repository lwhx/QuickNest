# Windows API 技术决策记录（P0-01 ～ P0-13）

> 基线 19 章 P0 交付物之一。每条决策均以实际代码与 Windows 11 x64 实测证据为依据；
> 证据目录 `docs/test-results/P0-*/`，逐项细节见 [P0_VALIDATION.md](P0_VALIDATION.md)。
> 最后更新：2026-11-04（P0-13 验收后）。

## 总体绑定

| 项 | 决策 | 依据 |
|---|---|---|
| 语言/工具链 | Rust 1.96.0（`rust-toolchain.toml` 固定） | P0-01 |
| UI 框架 | Slint 1.18.1 固定，winit backend + software renderer | P0-01：无浏览器内核/Node/Python 运行时 |
| Win32 绑定 | windows-sys 0.61.2（原始 API）；COM 生成接口用 windows 0.62.2 | P0-07：手写 vtable 变参 shim 导致超时+访问违例，弃用 |
| 清单 | asInvoker / uiAccess=false / Per-Monitor V2 | P0-01；高完整性宿主继承为已知边界（测试用受限令牌验证普通权限路径） |

## P0-02 HWND 生命周期

- **决策**：`raw-window-handle-06` 在原生窗口创建后获取 HWND，`IsWindow`/PID/TID 验证；隐藏不销毁默认 HWND（winit 行为实测）。
- **约束记录**：原生 HWND 在事件循环泵消息前不可用——所有依赖 HWND 的验证必须等待窗口就绪（P0-02/P0-12 两次确认）。

## P0-03/P0-04 OLE 初始化与 COM 对象

- **决策**：UI 线程 `OleInitialize`（OleGuard RAII，线程绑定 PhantomData<Rc<()>>）；S_OK/S_FALSE/RPC_E_CHANGED_MODE 语义按实测修正（预先 CoInitializeEx(STA) 后首次 OleInitialize 实为 S_OK）。
- **弃用方案**：手写 COM vtable + 变参 shim（P0-07 尝试）——请求超时 + 0xC0000005 + 乱码，全部弃用，改用 windows crate 官方生成接口（`#[implement(IDropTarget)]`）。
- **生命周期边界**：winit 0.30.13 建窗期间自行 OleInitialize 且 WM_DESTROY 时无条件 RevokeDragDrop；本层只管理自己的成功引用，必须在窗口存活时先撤销自有注册，不依赖后端清理。

## P0-04/P0-05 拖放

- **决策**：直接在 Slint 原生 HWND 上 `RegisterDragDrop`/`RevokeDragDrop`；另备独立拒绝目标（RejectingDropTarget）用于注册回归。
- **基线矩阵映射**：命中"Slint 主窗口直接 OLE 通过 → **GO**"路径（真实 Explorer 5 Drop/6 路径验证）。
- **winit 协同**：`unstable-winit-030` 钩子 `with_drag_and_drop(false)` 关闭 winit 默认拖放注册，避免抢占自有目标；Slint 升级须重新验证该版本相关 API。

## P0-06 DPI

- **决策**：Slint 逻辑坐标 + Per-Monitor V2 清单；真实系统缩放 100/125/150/175% 四档实测（用户确认 200% 不在范围）。`GetDpiForMonitor`/物理像素换算仅用于验证参照层。

## P0-07 Shell 操作

- **决策**：专用 STA 线程（`COINIT_APARTMENTTHREADED|COINIT_DISABLE_OLE1DDE`）+ `GetMessageW` 三分支泵 + WAKEUP 线程消息 + mpsc 控制——接口对象不跨线程，操作在创建线程执行。
- **证据**：真实 .lnk 解析（IShellLinkW）、文件夹执行（ShellExecuteExW）在 Medium 令牌下通过。

## P0-08 单实例

- **决策**：`CreateMutexW` + 紧读 GetLastError（`ERROR_ALREADY_EXISTS`）→ 零超时等待探测（`WAIT_ABANDONED` 接管 / `WAIT_TIMEOUT` 已有实例）。命名 `Local\QuickNest-{SID_HASH(FNV-1a64)}-{SESSION_ID}`。
- **语义修正**：基线 9.2 的 WAIT_* 清单实为 WaitForSingleObject 语义（CreateMutexW 只报 ERROR_ALREADY_EXISTS），实现按真实行为映射并记录。
- **附带修复**：MutexGuard 空句柄不再 CloseHandle(NULL)（Option<HANDLE>）。

## P0-09 Named Pipe IPC

- **决策**：`\\.\pipe\QuickNest-{SID_HASH}-{SESSION_ID}`，与 Mutex 同源；`PIPE_TYPE_MESSAGE|READMODE_MESSAGE|REJECT_REMOTE_CLIENTS|FILE_FLAG_FIRST_PIPE_INSTANCE`；SDDL `D:P(A;;GA;;;当前用户)(A;;GA;;;SY)`。
- **协议**：4 字节 LE 长度前缀 + UTF-8 JSON（1..4096B），`deny_unknown_fields` 白名单命令，版本化 ack。
- **身份验证**：`ImpersonateNamedPipeClient` + 线程 TokenUser SID 比较（`ImpersonationGuard` 无条件 RevertToSelf）；PID 仅审计。
- **message-mode 教训**：ReadFile 每次返回一条完整消息，短读是正常形状（ERROR_MORE_DATA 才是超容量）——固定缓冲误判 short read 已修。
- **启动竞态（9.5）**：Mutex 先于 Pipe；二实例 1500ms 预算内重试连接，超时重竞争 Mutex，绝不启动第二主实例。

## P0-10 全局快捷键

- **决策**：专用消息循环线程 `RegisterHotKey(NULL, MOD_CONTROL|MOD_ALT|MOD_NOREPEAT, VK_SPACE)`；注册前 `MsgWaitForMultipleObjectsEx(0)` 强制建队列消除 PostThreadMessage 竞态；`WM_HOTKEY` 经 mpsc 送达；Drop 在注册线程 `UnregisterHotKey` 收口。
- **三层挂死教训**（P0-10 三轮实测定位）：
  1. `invoke_from_event_loop` 在事件循环未运行时永久阻塞 → 场景线程 + 主线程事件循环分离；
  2. 泵阻塞 GetMessageW 观察不到控制 → Drop 前投递 `PostThreadMessageW(WAKEUP)`；
  3. 控制通道 Disconnected 分支不退出导致空转 → Shutdown|Disconnected 统一 PostQuitMessage，注销单次收口。
- **GetLastError 纪律**：诊断输出插在 Win32 调用与 GetLastError 之间会覆盖 last error（实测 0x00000006 假象）——GetLastError 必须紧跟失败调用。

## P0-11 系统托盘

- **决策**：`Shell_NotifyIconW`（NIM_ADD/MODIFY/DELETE，NIF_MESSAGE|NIF_TIP|NIF_SHOWTIP）+ 隐藏顶层回调窗口（**非** HWND_MESSAGE——消息专用窗口无法承载前台/弹出菜单）+ 右键 `SetForegroundWindow` + `TrackPopupMenu(TPM_RETURNCMD)` + `PostMessageW(WM_NULL)` + `SW_HIDE` 还原。
- **弃用方案**：跨进程 `AttachThreadInput` 前台转移——共享输入队列使 TrackPopupMenu 打开即自关（diag5）；普通 SetForegroundWindow 方案成立（diag6）。
- **TaskbarCreated**：`RegisterWindowMessageW` 注册广播，WndProc 收到即在托盘线程重加图标；模拟须先删图标（图标存在时重复 NIM_ADD 被 Shell 以 E_FAIL 拒绝，diag7）。
- **环境边界**：本机 Explorer 高完整性 → Medium 令牌被 UIPI 拒绝（ERROR_ACCESS_DENIED=5），如实记录非 Medium 通过；物理会话（标准用户）不受影响。

## P0-12 窗口焦点状态机

- **决策**：纯逻辑状态机（零 Win32/Slint 依赖，14 项单测）——Hidden/Showing/Visible/Hiding/ShuttingDown 五态；六类 `SuppressionToken` RAII 引用计数抑制锁（含 Pinned 镜像）；120ms 一次性失焦复查（pending 槽仅在空时设置——每 tick 重设会导致永不触发，实测教训）；前台归属 = GA_ROOTOWNER 链相同或同进程（10.3 第 7 步 + 10.4 事件源）。
- **事件源接线**：Slint FocusScope ESC、RegisterHotKey 热键、托盘左键/菜单三路真实事件驱动状态机。
- **UIPI 合成输入边界**：Medium 进程向 High 前台（本机 Explorer）SendInput 被静默过滤（SendInput 返回成功但按键不到达）——注入类验证要求"前台归我"；物理按键不受影响。
- **SendInput 坐标**：`SetCursorPos` 在 Medium 被拒（错误 183）→ 统一 SendInput 绝对移动（虚拟桌面归一化，P0-05 通道）。

## P0-13 原子保存

- **决策**：基线 15.2 链——同目录唯一临时文件 `config.<PID>.<UUID>.tmp`（`UuidCreate`）+ `create_new` 独占 + `sync_all` + 关句柄；首建 `MoveFileExW(MOVEFILE_WRITE_THROUGH)`；已存在走 `ReplaceFileW(flags=0)` + 唯一同卷备份；失败保留原配置与 tmp 并记录 GetLastError。
- **并发创建回退**：首存竞态（ERROR_ALREADY_EXISTS）回退替换路径——被抢建文件进备份不丢失。
- **恢复（15.8）**：按文件名倒序（UTC 戳字典序=时间序）扫 JSON 有效备份、恢复最近有效、损坏文件改名 `config.corrupted.<戳>.json`、恢复失败回滚原名、无有效备份安全模式不动原文件、活配置仍有效拒绝恢复。
- **单写者锁（15.7）**：进程内互斥贯穿 save/recovery，持锁并发保存以 Busy 拒绝；毒化容忍（panic 后接管）。std `File::open` 默认共享含 FILE_SHARE_DELETE、不构成替换锁——失败注入须 CreateFileW 收窄共享。

## 通用纪律（贯穿各任务）

1. GetLastError 紧跟失败调用，中间不得插入任何 Rust 标准库调用（P0-10 run4 教训）。
2. RAII Guard 模式统一：线程绑定（PhantomData<Rc<()>>）、无条件回滚（ImpersonationGuard/RevertToSelf）、Drop 单口收尾（热键 UnregisterHotKey 收口、托盘 NIM_DELETE 收口）。
3. 消息循环线程模板：GetMessageW 三分支 + 控制通道 + WAKEUP 线程消息 + 注册前强制建队列。
4. 验证报告 TSV + 失败记录一律保留不计入通过；未经实测不声称完成。
