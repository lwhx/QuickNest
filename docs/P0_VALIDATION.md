# P0 技术验证记录

## P0-01：基础工程（历史验收记录）

下文记录 P0-01 当时的被测产物与阶段边界；当前 P0-02 结果见本文后续章节，历史 SHA-256 不代表当前 Release 产物。

结论：**本任务验收通过（2026-11-03），停止在 P0-01**。P0 阶段尚未验收，P0-02～P0-15 均未开始，不进入 P1。

### 被测产物

- 路径：`target/release/quicknest.exe`。
- SHA-256：`99C19FC2BDAC98C6AD957E6B39DD7B84498BD278754BE5F533FA132EC4C81D95`。
- Rust：1.96.0 / x86_64-pc-windows-msvc；Slint/slint-build：1.18.1。
- 后端：原生 winit；渲染器：software；资源：EmbedFiles，演示 SVG 内嵌，动态文字使用系统字体。
- 测试系统：Microsoft Windows 11 专业版，25H2，Build 26200.6899，x64。
- 显示缩放：DPI 96（100%）；本次未修改系统缩放。

### 实现与验收对应

| 基线要求 | 结果 | 证据 |
| --- | --- | --- |
| Rust 工程、引入 Slint | 通过；版本固定，保留 Cargo.lock | Cargo.toml / rust-toolchain.toml / Cargo.lock |
| 显示主窗口、基础图标网格 | 通过；6 项图标和中文名称完整，实际截图复核 | Medium 运行截图与下方复核记录 |
| 显示/隐藏 | 通过；3 轮、同一 PID/HWND；隐藏期间无采样挂起；正常退出 | Medium results.json / window-samples.json |
| 普通模式实际按钮 | 通过；窗口鼠标消息触发已观察截图中的按钮，约 2.03 秒恢复，退出码 0 | buttons-modules/button-results.json |
| Release Windows x64 EXE | 通过；AMD64 0x8664，PE32+，GUI subsystem=2 | PE 检查与 dumpbin |
| 权限与 Manifest | 内嵌 asInvoker、uiAccess=false、PerMonitorV2 / true-pm；Medium 令牌实际运行通过 | 内嵌 Manifest、Medium 运行令牌记录 |
| 无浏览器内核、无 Node/Python 运行时依赖 | 通过；固定依赖图、PE 导入、实际加载模块与无子进程相互核验 | dependency-tree.log / pe-imports.json / buttons-modules |
| 不依赖源码目录资源 | 通过；只复制 EXE 到独立目录后，按钮隐藏/恢复、退出正常 | standalone/button-results.json |

### 格式、编译与测试

最终源码（修复动态字形后）运行结果：

| 命令 | 结果 | 日志（docs/test-results/P0-01/ 下） |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt-render-fix.log（无输出，退出码 0） |
| cargo check --locked --all-targets | 通过 | check-render-fix.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy-render-fix.log |
| cargo test --locked --all-targets | 通过；5 passed，0 failed | test-render-fix.log |
| cargo build --locked --release | 通过 | release-render-fix.log |

单元测试覆盖参数模式、未知/多余参数拒绝及演示数据约束，不用它们替代窗口实测。最终检查结果摘要另见 `acceptance.json`。

### Windows 实际测试

主验收运行目录：

`docs/test-results/P0-01/medium/run-20261103T063416625Z-1968166f/`

- `results.json`：验证脚本与两个应用进程均为 **Medium（RID=0x2000）**。
- 自动运行 PID 12200，HWND 0x69014E：观察 6 次可见性转换，约第 2/4/6/8/10/12 秒隐藏或恢复；保持同一进程和同一窗口。
- 约 13.973 秒后退出码 0，未强制终止；最大观察间隔约 0.236 秒，采样未报告挂起。此为短时稳定性证据，不是长期稳定性保证。
- 普通关闭运行 PID 35800：外部投递 WM_CLOSE，正常退出码 0，无强制终止。
- 5 张真实桌面像素截图均已实际阅读；主窗口、6 项名称、图标和按钮可见，隐藏恢复无布局丢失。截图工具仅临时置顶后恢复，不要求抢占前台焦点。
- PE 导入表 25 个 DLL，另由 dumpbin 交叉检查，mt.exe 提取并校验实际 EXE 内嵌 Manifest。

截图：

- [初次显示](test-results/P0-01/medium/run-20261103T063416625Z-1968166f/smoke-initial.png)
- [第一次恢复](test-results/P0-01/medium/run-20261103T063416625Z-1968166f/smoke-restored-1.png)
- [第二次恢复](test-results/P0-01/medium/run-20261103T063416625Z-1968166f/smoke-restored-2.png)
- [第三次恢复](test-results/P0-01/medium/run-20261103T063416625Z-1968166f/smoke-restored-3.png)
- [普通模式](test-results/P0-01/medium/run-20261103T063416625Z-1968166f/normal-before-wm-close.png)

按钮补充测试：`buttons-modules/button-results.json`，PID 39332、DPI 96；窗口鼠标消息派发到已观察截图中按钮中心，隐藏后 2.031 秒恢复同一窗口，退出码 0。该测试覆盖 Slint 按钮回调，但没有用物理鼠标或 UI Automation，不扩展宣称其他输入设备覆盖。另一个 `buttons/button-results.json` 也通过。它们是对主脚本 `manualItems` 中按钮项的独立补充，未改写原始结果。

运行时加载模块记录没有浏览器、Node、Python；没有子进程。独立目录测试只复制了 EXE，恢复耗时约 2.006 秒；未在缺少运行库的全新系统测试。

### 失败过程及修复

- `initial-check.log`：暂停前 rustc 工具链定位失败，未进入源码编译。
- `bootstrap-check.log`：暂停前依赖解析及部分下载，按用户要求中止，不算通过。
- Rust 1.96.0 安装恢复：`toolchain-recovery.log` / `build-environment.json`。
- `check.log` / `check-fixed.log`：Slint 固定尺寸与最小尺寸冲突、Rust 错误类型不一致；分别修复为 preferred 尺寸、退出循环错误单独处理。
- 首轮实际截图发现动态中文名称缺字：预栅格化资源嵌入只覆盖编译期字形，改为 EmbedFiles 并使用系统字体；最终截图确认修复。
- `run-20261103T062903554Z-5badce9f` 与 `run-20261103T063126661Z-ae9db398`：可见性、PE 和 Manifest 检查通过，部分截图因前台焦点失败，整体记 Failed。测试脚本改为临时置顶截图后再运行。
- `run-20261103T063314626Z-4012105d`：高完整性环境的自动检查通过，另有最终 Medium 运行完整通过。
- Explorer 启动仍继承 High；本机 Administrator 策略不提供普通桌面令牌。最终仅对测试子进程创建受限 Medium 令牌，禁用管理员组和权限；未修改 UAC/账户/系统策略，不属于产品降权实现。

主验证脚本始终将按钮/人工复核留作独立项，所以结果是 `AutomatedPassedManualPending`；本次已由实际截图阅读和额外按钮测试补齐，结论写入本文件及 `acceptance.json`，不把脚本退出码直接当完整验收。

### 剩余限制与阶段门禁

- 当前 EXE 导入 `VCRUNTIME140.dll` 与系统 DLL；本机运行库已存在。后续发布阶段需验证运行库部署或其他明确方案。
- asInvoker 不主动提升权限，也不会自动降低从高权限父进程继承的权限；安装后降权启动和其他权限场景尚未实现/验证。
- 未执行 Win10、多显示器/多 DPI、性能指标、200 项目或长期运行测试；声明 Per-Monitor V2 不等于这些测试已通过。
- 图标与名称为演示数据，不是 Shell 图标提取、项目管理或真实启动能力。
- 未实现产品内部 HWND 获取、COM/OLE、托盘、热键、单实例、配置保存和焦点状态机。
- P0-01：通过；P0-02～P0-15：在当时未开始；P0 GO / NO-GO：未决策；P1～P4：未开始。

## P0-02：HWND 生命周期（历史验收记录）

本节 SHA-256 和任务边界对应 P0-02 当时产物；当前 P0-03 结果见后续章节。

结论：**2026-11-03 本任务实际验收通过，停在 P0-02**。P0 阶段整体仍未验收；P0-03～P0-15、P1～P4 未开始。

### 被测产物与环境

- 当前 Release：`target/release/quicknest.exe`。
- SHA-256：`4A0A835D284FA1C0A89F2A82D38E19B238AB651E3B1B6104A22EC8AC070BF208`。
- Rust 1.96.0、Slint/slint-build 1.18.1、Windows x64 MSVC、原生 winit + software renderer，未调整技术架构。
- Slint 启用 `raw-window-handle-06`；直接依赖 raw-window-handle 0.6.2（std）和 windows-sys 0.61.2，均固定版本并更新 Cargo.lock。
- 本机 Windows 11 专业版 25H2 x64 / Build 26200.6899 / 100% 缩放，Medium（RID 8192）实际测试。

### 实现与生命周期约束

- `src/infrastructure/windows/window_handle.rs`：每次通过 Slint 官方 provider 及 `HasWindowHandle` 在 UI 线程获取 Win32 HWND，使用时检查 IsWindow 和所属 PID/TID。provider 与借用句柄限于函数作用域；不返回可随意跨线程操作窗口的裸 HWND。返回的数值是诊断快照，不保证长期有效。
- `src/hwnd_validation.rs`：新增 `--hwnd-test <report.tsv>`，原生创建后在 UI Timer 中重新获取；未就绪有五秒截止时间。不存在的句柄不进入窗口操作。
- 每次测试三代：created → hidden → restored → component_released → destroyed → destroyed_confirmed，最终 completed。每代显示/隐藏/恢复各约 500ms；销毁后确认至少 500ms，再创建下一代，完成后继续运行约 500ms 才退出。
- 默认 Windows 隐藏保留 HWND；销毁时先释放 Slint 可见组件引用与唯一 owner，所有 provider 已释放，weak 无法升级，再让同一消息循环自然派发 winit 销毁消息。不调用 DestroyWindow 代替后端清理。
- 弱引用失效和 HWND 失效分别检查。销毁后的旧 HWND 数值仅允许进行 IsWindow 诊断观察，不用于窗口操作。重建只要求上一代先失效，不要求新 HWND 数值不同。
- 检测到 `SLINT_DESTROY_WINDOW_ON_HIDE` 环境变量存在则返回错误，包括值为 `0` 的情形；测试不改变进程环境或产品架构。

### 编译与测试

日志均在 `docs/test-results/P0-02/`：

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt.log |
| cargo check --locked --all-targets | 通过 | check.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy.log |
| cargo test --locked --all-targets | 7 passed，0 failed | test.log |
| cargo build --locked --release | 通过 | release.log |

新增测试覆盖 HWND 模式报告参数约束和空句柄拒绝；真实窗口生命周期由实际 Release 测试验证，未以这些单元测试代替。首次源码检查 `initial-check.log` 直接通过，无本任务编译失败。

### Windows 实际运行证据

首次内部预检：`initial-runtime-9146daa3/`，三代完整通过，释放组件后约 20ms 观察到原生 HWND 失效，退出码 0。该预检来自高权限开发环境，正式结论采用后续普通权限独立观察。

普通权限独立观察共三次，每次三代，全部 pass、退出码 0、无强制终止：

| 运行目录（medium/ 下） | 目标 PID | 完整性 | 代数 |
| --- | --- | --- | --- |
| run-20261103T070416229Z-0ce8d9e52b974944a93d302b26c0bb40 | 38980 | Medium / 8192 | 3 |
| run-20261103T070500277Z-177c43b9a5fd43519943a111100e7dad | 33932 | Medium / 8192 | 3 |
| run-20261103T070500787Z-dc975c05b7af443cb5bd8e0cc96c0365 | 40700 | Medium / 8192 | 3 |

`scripts/validate-p0-02.ps1` 独立使用 PID 过滤 EnumWindows，并在已枚举的 HWND 上采样 IsWindow / IsWindowVisible。每个 run 保存 `lifecycle.tsv`、`external-samples.json`、`results.json` 和标准输出/错误。采样目标间隔为 20ms（含脚本执行开销，不是硬实时）。严格核对每代内部日志顺序、PID/TID、弱引用失效、可见性、进程存活期间的外部 false 证据及下一代创建前失效。报告始末与正在运行的 EXE 哈希一致。

第一轮内部日志中，三代组件分别在 1625/3733/5859ms 释放，HWND 在 1639/3745/5874ms 确认失效，失效至少 500ms 后才记录 destroyed_confirmed。外部也在进程仍存活时分别采样到 false；这些绝不是进程退出后窗口消失。

### 失败路径与回归

- `errors/a070acb0234347dab2d0db3ea6d2a10b/results.json`：已有报告内容未覆盖；存在隐藏销毁环境覆盖时拒绝运行；提前 WM_CLOSE 导致验证失败。三种场景都正常返回退出码 1，符合预期。未屏蔽错误或删除测试。
- `p0-01-regression/run-20261103T070304547Z-8e59aa68/`：当前 EXE 在 Medium 权限下 P0-01 三轮显示/隐藏、PE、Manifest、正常关闭检查通过；初次及第三次恢复截图已实际阅读，中文、六项网格与按钮正常。
- `button-regression/button-results.json`：实际窗口鼠标消息触发隐藏/退出按钮，约 2.038 秒恢复同一窗口，退出码 0；未使用物理鼠标或 UI Automation。模块/子进程记录正常。
- P0-01 的原始产物哈希与证据保留；当前产物哈希使用本节数据。

### 当前边界与阶段门禁

本任务无未解决阻塞。普通模式仍是技术验证程序；窗口标题沿用 P0-01，不代表本任务未测试，HWND 模式状态文本明确显示 P0-02。

- IsWindow 是时点观察；外部销毁与句柄复用仍可能发生，不能把原始 HWND 长期缓存后不再检查。
- 同进程销毁路径已验证，不宣称覆盖所有系统异常退出、第三方句柄保活场景或长时泄漏。
- 未执行 Win10、多显示器/不同 DPI、性能及长期稳定性测试。
- 保留 `VCRUNTIME140.dll` 运行库部署限制；高权限宿主直接启动 asInvoker 仍会继承高权限。
- 未实现 OleInitialize、COM Apartment、IDropTarget、拖放或 Shell STA；它们属于后续任务。
- P0-01、P0-02：通过；P0-03～P0-15：在当时未开始；P0 GO / NO-GO：未决策；P1～P4：未开始。

## P0-03：OLE STA 初始化（历史验收记录）

本节对应 P0-03 当时产物和默认后端行为；P0-04 修改建窗前拖放配置后的结果见后续章节。

结论：**2026-11-03 本任务验收通过，停在 P0-03**。此结论只对应第 29 章 P0-03 和第 5.2 节初始化约束，不是第 19 章完整 OLE/COM 大组验收。

### 被测产物与实现

- Release：`target/release/quicknest.exe`，SHA-256 `7C835AD326C604E1AD4E35E9E8EA8182D28213601FE3AFCCD242B434165AD5BD`。
- Rust 1.96.0、Slint 1.18.1、winit + software renderer，Windows 11 专业版 x64 / Build 26200.6899 / 100% 缩放；架构未变。
- windows-sys 0.61.2 显式增加 Win32_System_Ole / Win32_System_Com 功能。
- `src/infrastructure/windows/ole.rs`：OleInitialize(NULL)；仅 S_OK/S_FALSE 构造 RAII Guard；失败不构造、不 OleUninitialize。Guard 不可 Clone、!Send/!Sync，每个成功引用同线程恰好清理一次。CoGetApartmentType 只返回普通状态数据。
- 正常模式、HWND 模式在后端选择前建立本层 Guard；OLE 模式单独记录初始化基线。UI 验证持有本层 Guard 直到 HWND 生命周期结束，再清理本层引用。
- `src/ole_validation.rs`：全新线程、预先 COM STA、MTA 冲突三种情形；同一 UI 线程内实际运行 Timer、跨线程投递和已验证的三代 HWND 生命周期。不跨线程传递 COM 接口或 Guard。

### 编译与测试

最终源码检查日志位于 `docs/test-results/P0-03/`：

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt-final.log |
| cargo check --locked --all-targets | 通过 | check-final.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy-final.log |
| cargo test --locked --all-targets | 10 passed，0 failed | test-final.log |
| cargo test --locked --doc | 2 passed，0 failed；!Send/!Sync 编译失败测试 | doctest-final.log |
| cargo build --locked --release | 通过 | release-final.log |

单元测试包含真实 Windows 隔离线程的嵌套 OLE 清理与 MTA 冲突；Release 独立运行用于验证集成行为。首轮编译全部通过，但首轮 Release 测试在错误预期处失败，详见下方。

### 三轮普通权限 Release 实测

每轮通过外部 PID/HWND 采样和 OLE 报告核对，均 Medium / RID 8192、退出码 0、无强制终止；运行目录为 `docs/test-results/P0-03/medium/` 下：

| 目录 | 目标 PID | 结果 |
| --- | --- | --- |
| run-20261103T073003400Z-d96b79e737264dcd9731c3818a8d951c | 29644 | pass |
| run-20261103T073204515Z-8552654b74044c869acf23d7f19e629e | 28528 | pass |
| run-20261103T073205072Z-f185e2ab75dc4aaba6c8fcf87fba035d | 11472 | pass |

每轮有 `ole.tsv`、`ole.hwnd.tsv`、`external-samples.json`、`results.json`、stdout/stderr。OLE 报告 29 行，字段包含时间、case、phase、PID/TID、HRESULT、Apartment HRESULT/类型/qualifier。HWND 生命周期继续使用前一任务的 19 行和外部验证，不把进程退出当成窗口销毁。

实测要点：

1. fresh：基线 CO_E_NOTINITIALIZED；第一次 OleInitialize S_OK，Apartment=MAINSTA（3）；第二次 S_FALSE；释放内层后仍 STA；释放外层后恢复 CO_E_NOTINITIALIZED。
2. sta：先 CoInitializeEx(STA) S_OK；第一次 OLE **S_OK**，重复 OLE S_FALSE；释放 OLE 两层后 COM STA 基线保留；释放成功 COM 基线后恢复 CO_E_NOTINITIALIZED。COM STA 并不等于已初始化 OLE。
3. mta：先 CoInitializeEx(MTA) S_OK；OleInitialize 返回 **RPC_E_CHANGED_MODE / 0x80010106**；没有为失败调用执行 OleUninitialize；Apartment 仍 MTA（1）；清理唯一成功 COM 调用后恢复未初始化。
4. ui：应用在后端选择前第一次 OLE S_OK，始终 STA/MAINSTA；事件循环内嵌套调用 S_FALSE，释放内层后 STA 仍可用。至少 20 次 UI Timer 成功，来自普通后台线程的三次 invoke_from_event_loop 回调在相同 UI TID 观察到 STA（报告在循环结束后统一写入观察结果，不能将写入时间当作响应耗时）。
5. HWND：本层 OLE Guard 存活期间每轮三代实际创建、隐藏、恢复、组件释放及旧 HWND 失效；外部独立采样确认进程存活期间销毁；不注册 QuickNest 自有 IDropTarget。
6. 本层 Guard 释放后 UI 仍 STA，实测如实保留。隔离线程可证明本层引用计数处理正确；UI 后端自身引用不属于本层清理权限。

### 实际发现与修复

`initial-5f56463f/`：首轮 Release 实测退出码 1，断言错误地要求预先 CoInitializeEx(STA) 后第一次 OleInitialize 返回 S_FALSE。修正为第一次 OLE S_OK、第二次 OLE S_FALSE，并将首次返回值记录放在断言前。未删除测试、未屏蔽错误，失败日志保留。最终所有检查和 Release 运行重新执行通过。

本地 winit 0.30.13 默认窗口拖放功能启用，它在原生 HWND 的 WM_NCCREATE/create_window_data 期间调用 OleInitialize（window.rs:1168），接受 S_FALSE；源码搜索未发现配对 OleUninitialize。它另有线程局部 CoInitializeEx(STA)/CoUninitialize。窗口销毁时 RevokeDragDrop 不等价于 OleUninitialize。

因此当前 UI 清理只能证明 QuickNest 自己的成功引用按线程配对，不宣称所有后端 COM/OLE 引用已完全清空。此后端生命周期问题保留为后续 OLE/长期稳定性评估项；本次不擅改后端或替框架清理引用。

### 错误路径与回归

- `errors/095ab8170e584e06a6e8b4a301534099/results.json`：已有 OLE 报告、已有 HWND 报告均未被覆盖；缺失父目录、缺失报告参数均返回 1。四项符合预期。
- `p0-01-regression/run-20261103T073205891Z-6f92fa7e/`：Medium 下显示/隐藏、PE/Manifest、原生关闭自动检查通过；初次和第三次恢复截图已实际阅读，中文网格和按钮正常。
- `p0-02-regression/run-20261103T073206366Z-f0a0e3b734af4bbaaa0c1b0065db7598/`：正常启动持有新 OLE Guard 后，三代 HWND 独立生命周期观察通过。
- `button-regression/button-results.json`：按钮窗口消息测试通过，约 2.015 秒恢复，退出码 0。不是物理鼠标或 UI Automation 覆盖。
- 所有最终运行报告绑定相同 Release SHA-256；脚本语法检查通过，无遗留应用/编译进程。

### 当前门禁与未完成

P0-03 本层初始化验收无阻塞；后端自身 OLE 引用清理限制已明确记录。P0-04～P0-15、P1～P4 未开始，P0 GO / NO-GO 尚未决策。

Timer 和普通事件投递通过不能证明实际 OLE DragEnter/DragOver/Drop 消息泵通过；这些回调、自有 IDropTarget 注册/撤销、Explorer 拖入、Shell STA 服务在 P0-03 当时均未实现或验收。winit 自带的拖放支持不能替代 QuickNest 的后续验收。Win10、多 DPI、性能和长期泄漏测试未执行；现有 MSVC 运行库部署限制不变。

## P0-04：IDropTarget 注册和撤销（历史验收记录）

本节 SHA-256、范围与后续状态是 P0-04 当时记录；P0-05 文件接收和最终证据见后续章节。

结论：**2026-11-03 本任务实际验收通过，停在 P0-04**。P0 阶段整体尚未验收，P0-05～P0-15 及 P1～P4 未开始。

### 产物、依赖与后端所有权

- Release：`target/release/quicknest.exe`，12,245,504 字节。
- SHA-256：`A70908402952196B06A8B0718A0DB641777F4F8E954F17462A098DED1C301528`。
- Rust 1.96.0、Slint/slint-build 1.18.1、winit 0.30.13、software renderer，技术路线未变。
- 固定 windows/windows-core 0.62.2，使用 COM implement 宏；没有手写 vtable。IDropTarget 声明 Agile=false，外层注册所有者 !Send，UI 线程使用。
- Slint 增加固定版本的 unstable-winit-030 feature，仅在 `select_backend()` 的窗口 attributes hook 使用 `with_drag_and_drop(false)`，在原生 HWND 创建前关闭 winit 默认 RegisterDragDrop。没有先撤销后端目标再抢占注册，也没有更换宿主窗口架构。
- winit 默认 drag_and_drop=true 时会在 HWND 创建期间 OleInitialize/RegisterDragDrop；本次 false 跳过此分支。其 WM_DESTROY 仍无条件尝试 RevokeDragDrop，因此本层必须在窗口存活时先完成自己的撤销。后端销毁清理不算本层测试通过依据。

### 实现范围

`src/infrastructure/windows/drop_target.rs` 实现四个 COM 回调，当前仅输出 DROPEFFECT_NONE，拒绝数据效果，DragLeave 完成最小状态处理；不访问 IDataObject、不读用户路径、不扫描或提取图标。回调检查所属线程和输出指针，不做阻塞 IO。

`DropRegistration` 只在本次 RegisterDragDrop 成功后建立所有权，保留 Slint native provider、自有 COM 接口与 OleGuard，显式 revoke 可报告错误，Drop 执行同线程撤销后再释放字段。失败注册不会创建拥有者，不调用 RevokeDragDrop 清理其他对象。保留 provider 是有意保证原生窗口存活，撤销前不释放它。

`--drop-target-test <报告>` 复用 HWND 三代生命周期 runner，新增最小同步钩子在 created/hidden/restored/before_release/destroyed 阶段执行注册验证；普通 HWND/OLE 验证默认空钩子路径保留。注册功能只接入该独立技术验证模式，正常演示模式尚无文件拖入能力。

### 编译与测试

最终日志在 `docs/test-results/P0-04/`：

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt-final.log |
| cargo check --locked --all-targets | 通过 | check-final.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy-final.log |
| cargo test --locked --all-targets | 14 passed，0 failed | test-final.log |
| cargo test --locked --doc | 3 passed，0 failed | doctest-final.log |
| cargo build --locked --release | 通过 | release-final.log |

新增单元测试：四回调直接调用均拒绝效果、空 effect 输出返回 E_POINTER；IUnknown/IDropTarget QueryInterface 身份一致，不支持 IStream 返回 E_NOINTERFACE；空 HWND 观察拒绝；验证模式参数约束。新增注册 Guard !Send 编译失败测试。对象最后引用释放时析构计数为一。

这些直接接口测试只验证 COM 契约，不代表 Windows OLE 路由或 Explorer 拖入回调已实测。

### 三轮普通权限实测

外部采样/报告核对共三次，每次三代窗口，均 Medium（RID 8192）、退出码 0、无强制终止。目录为 `docs/test-results/P0-04/medium/` 下：

| 运行目录 | 目标 PID | 结果 |
| --- | --- | --- |
| run-20261103T080857748Z-d0873c893c964200afb5f86aaceb1e82 | 3756 | pass |
| run-20261103T081017521Z-890bc6d488804262b9868dcad06c03aa | 19620 | pass |
| run-20261103T081018003Z-d14bb383a62045179eb0a912c9a814d2 | 29592 | pass |

每代十一条注册记录，另有最终 completed，共 34 行；同目录保留 `drop.tsv`、`drop.hwnd.tsv`、`external-samples.json`、`results.json`、标准输出/错误。注册记录包含 PID/TID/HWND、真实 HRESULT、当前可见性、注册所有权和对象析构计数。

实测顺序与结果：

1. 窗口实际创建且所属 UI 线程正确后首次 RegisterDragDrop 返回 S_OK。
2. 注册 B 返回 DRAGDROP_E_ALREADYREGISTERED（0x80040101）；失败对象已析构。再次注册仍冲突，确认失败路径未撤销原目标 A。
3. 隐藏、恢复期间再次注册保持同一冲突，原注册仍在，窗口身份不变。
4. HWND 仍有效时显式 RevokeDragDrop 返回 S_OK；额外诊断 Revoke 返回 DRAGDROP_E_NOTREGISTERED（0x80040100），不把该诊断调用当作生产清理方式。
5. 释放显式撤销后的 Guard，COM 对象析构计数为 1。
6. 同一 HWND 重注册 C 成功，直接 drop Guard 触发 RAII 清理；随后的诊断确认 NOTREGISTERED 且 C 已最终析构。
7. 然后才释放 Slint 组件/provider；外部在进程仍存活时观察 HWND 失效，旧数值仅作诊断 IsWindow 检查，拒绝为有效窗口观察（0x80040102）。确认失效至少 500ms 后才创建下一代。

所有成功/冲突/撤销操作与实际 HWND 所属 UI TID 一致。三轮绑定同一 EXE SHA-256，所有内部 HWND 记录均由外部 PID 过滤窗口采样交叉验证。

### 失败记录、修复与回归

- `initial-check.log`：最小 COM 对象/后端配置首次编译通过。
- `check-integration.log`：日志格式字符串多一占位符导致编译失败；修正后重跑全部检查，失败日志保留。
- `initial-5df3a8ce/`：首次 Release 注册撤销通过，但销毁后诊断 PID/TID 为 0、完成行时间为 0；后续修复诊断数据，最终三轮均用修复后的产物。预检不代替最终验收。
- `errors/7daa80d6d64c4c73ab95bd2b6e2b638b/results.json`：已有注册报告及 HWND 报告未覆盖；缺父目录和缺参数均返回 1，四项符合预期。
- `p0-01-regression/run-20261103T080941091Z-3d529ff0/`：Medium 下显示隐藏、PE/Manifest、原生关闭自动检查通过；初次/第三次恢复截图实际阅读，中文/图标网格正常。
- `button-regression/button-results.json`：窗口鼠标消息按钮测试通过，约 2.011 秒恢复同一窗口，退出码 0；不是物理鼠标/UI Automation。
- `p0-02-regression/run-20261103T081018464Z-f69fa6a054d64f04bef6593577b12e2d/`：独立 HWND 三代生命周期通过。
- `p0-03-regression/run-20261103T080940555Z-f1863cd3371943c883ff00ad100b94ca/`：S_OK/S_FALSE/MTA 冲突、清理、UI STA 和投递事件全部通过。关闭后端 OLE 拖放分支后，UI 清理本层 OLE 仍 STA，不能将其笼统归因于后端旧分支或宣称全部 COM 引用清空。

### 限制与下一步

P0-04 注册撤销无未解决阻塞。Slint 窗口配置钩子属于版本相关 API，升级必须重验；winit 销毁时额外 Revoke 行为已记录。UI 后端仍有其他 COM 初始化/线程状态，不替其清理引用，长期泄漏未测试。

Explorer 实际拖入、CF_HDROP、中文/多文件路径、实际 DragEnter/DragOver/DragLeave/Drop OLE 路由、DPI 坐标转换均**未执行或未实现**；当前目标拒绝所有效果。不以 winit 自带能力或直接调用回调代替后续 P0-05/06 验收。Win10、多 DPI、长期稳定性、性能与全新机器运行库部署未测试。

P0-01～P0-04：通过；P0-05～P0-15：在当时未开始；P0 GO / NO-GO：未决策；P1～P4：未开始。

## P0-05：Explorer 文件拖入

结论：**2026-11-03 本任务实际验收通过，停在 P0-05**。P0 阶段整体未验收，DPI 转换与后续任务未开始。

### 最终产物与范围

- Release：`target/release/quicknest.exe`；SHA-256 `8BE01530722210A5661919592E4528E888B63BA6D2F8409DB40479BEC678A5E4`。
- Rust 1.96.0、Slint 1.18.1、winit/software、windows/windows-core 0.62.2，架构未变。
- windows features 增加 Gdi / Com_StructuredStorage / Shell；Memory 仅供合成 HGLOBAL 测试。无新运行时。
- `file_drop.rs`：接收目标使用 CF_HDROP + TYMED_HGLOBAL，QueryGetData 明确 S_OK，Drop 重新校验并 GetData。单个 STGMEDIUM RAII owner 对每个成功取得的介质恰好 ReleaseStgMedium 一次，不浅拷贝、不 GlobalFree/DragFinish、不把 COM 对象或介质带到队列/其他线程。
- DragQueryFileW 先查数量/长度，分配包含终止符的 UTF-16 缓冲区，复制为 PathBuf；保持源顺序、中文、空格、LNK 原路径。最多 4096 项、单路径 32767 UTF-16 单位、总 1048576 单位，超限失败不报告 COPY。
- 非敏捷接收目标只在 UI STA 线程操作。DragEnter/Over 仅协商源允许的 COPY，失败默认 NONE。DragLeave/Drop 清除悬停状态，Drop 成功复制到非阻塞有界队列才报告 COPY；过量 hover telemetry 可丢弃，提交数据事件不静默丢失。
- `--file-drop-test <报告>`：窗口实际创建后沿用 DropRegistration 注册；Slint 每 25ms 处理事件、显示状态及写验证日志，回调中不做文件属性/图标/Hash 工作。退出前撤销并释放目标，核对 acquired/released 与对象析构。此为技术验证入口，不是完整业务导入。
- 不复制/移动原文件、不执行 EXE、不解析 LNK、不递归文件夹、不创建分类数据、不写用户配置；只记录原始屏幕 POINTL，未做 DPI 转换。

### 格式、编译和测试

最终日志在 `docs/test-results/P0-05/`：

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt-final.log |
| cargo check --locked --all-targets | 通过 | check-final.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy-final.log |
| cargo test --locked --all-targets | 17 单元 + 6 合成数据测试通过，0 失败 | test-final.log |
| cargo test --locked --doc | 3 passed，0 failed | doctest-final.log |
| cargo build --locked --release | 通过 | release-final.log |

合成 IDataObject 测试覆盖 Unicode/空格/多路径/LNK/文件夹原值顺序、unsupported、GetData 失败、返回错误 tymed/空介质/空列表时的资源释放、严格 FORMATETC，以及成功取得介质后 UI 队列关闭时不报告 COPY且仍 released=acquired。这些不作为真实 Explorer 证据。

首次编译通过但有 File 未用警告，已清理；最终 Clippy -D warnings 通过。后续修正 UI 旧说明（去掉“OLE 未实现”）后完整检查和 Release 重建，最终真实矩阵与回归绑定上述最终哈希。

### 真实 Explorer 主验收

主验收目录：`docs/test-results/P0-05/interactive-39ab013c/`。

- 实际源 Explorer PID **9172**、HWND 5118084；实际目标 PID **29184**、HWND 6624734。
- `window-tokens.json` / `window-tokens-final.json`、每次 pointer-actions：双方均 **Medium / RID 8192**。桌面 Explorer 的 High 会话未作为本次源。测试通过受限令牌独立启动 Explorer，没有更改 UAC/账户或重启桌面。
- 样本目录为新建 fixtures，脚本在 scripts/ 下；样本包括复制来的 notepad EXE（从未执行）、新建普通文件、空文件夹及指向 notepad 的原始 LNK（从未执行）。
- 测试通过实际 Explorer COM 选择项目，再用 SendInput 鼠标拖动和 Esc；不是合成 IDataObject、直接调用接收回调或 WM_DROPFILES。
- 动作脚本按已观察的布局操作，先验证源/目标鼠标命中无遮挡；每次落下必须新增恰好一个 Drop 回调，移出/取消必须零新增。UI 图片实际阅读，中文、计数与取消提示正确。

| 动作 | 真实回调/结果 |
| --- | --- |
| EXE | Drop=1，完整 sample application.exe 路径 |
| 中文文件夹 | Drop=1，完整 中文 folder 路径，未递归 |
| 中文 LNK | Drop=1，保留 中文 link.lnk 原路径 |
| 中文空格文件 | Drop=1，完整 中文 sample.txt |
| 多选两文件 | Drop=1，paths=2，second file.txt → 中文 sample.txt，顺序与源选择相符 |
| 移出 | DragLeave，Drop 前后均为 5，接收总数仍 6 |
| Esc 取消 | DragLeave，Drop 前后均为 5，接收总数仍 6 |

最终 `events.tsv` 清理记录：

`acquired=5 released=5 destroyed=1 enters=7 overs=106 leaves=2 drops=5 errors=0 received=6 heartbeat=9372`

`exit.json`：正常 WM_CLOSE 后退出码 0，无强制终止；关闭前双方 Responding=true。UI 心跳持续，拖动期间也有回调和心跳交错记录；这是本次短时响应证据，不是全部性能指标验收。

`fixtures.json` / `fixtures-after.json`：逐项验证 4 个文件哈希、长度及空文件夹仍存在/为空，原始样本未改变、未复制到其他业务位置。`source-selection.jsonl` / `pointer-actions.jsonl` 保存源选择和动作计数。`assert-p0-05-interactive.ps1` 严格核对 5 Drop/6 Path/2 Leave、顺序、权限、配对清理、正常退出、最终哈希；通过并写入 acceptance.json。

截图（已实际阅读）：

- [多文件接收](test-results/P0-05/interactive-39ab013c/final-multiple.png)
- [Esc 取消](test-results/P0-05/interactive-39ab013c/final-cancel.png)

最终脚本只截目标窗口；早期全桌面截图属于原始调试，不作为主要交付展示。

### 预检/脚本问题记录

- `interactive-12ded51f/`：初轮真实拖放已验证 EXE/LNK/中文/多选，文件夹首次未正确选中而无回调，明确选择后通过。其 EXE 哈希是早期版本，不代替最终产物验收。
- `interactive-faf01b1e/`：最终 EXE 的前一轮记录包含矩阵外样本落下；一轮自动脚本因窗口遮挡在输入前终止。早期取消输入处理时序不足且日志出现额外 Drop，该轮不能作为严格取消通过。原日志不删除，主验收改用新的 39ab013c 会话。
- 取消脚本修正为按住拖动时持续 Esc 并等待处理，新增动作前后 Drop 数断言；新会话完整矩阵通过。
- 汇总脚本曾错误预期回归目录带 medium 子目录，未生成验收结果；后按实际路径核对。样本检查 PowerShell JSON 数组解包错误曾产生假“Fixture changed”；改为逐项反序列化后，主验收 5 个样本全部哈希/目录状态未变。
- 早期路径控制台出现编码乱码是 PowerShell 默认读取 UTF-8 的展示问题；按 Encoding UTF8 重读与原路径逐字核对通过，PathBuf 内部保留宽字符。

### 最终产物回归与错误路径

所有以下最终回归 EXE 哈希均与主验收相同：

- P0-01：`p0-01-regression/run-20261103T085546498Z-bd0a0c07/`，显示隐藏、PE/Manifest 和正常关闭自动检查通过，第三次恢复截图已阅读正常。
- P0-02：`p0-02-regression/run-20261103T085545908Z-95d1e9ad46f3432c9e7eb38c2bded907/`，三代 HWND 外部生命周期通过。
- P0-03：`p0-03-regression/run-20261103T085545220Z-1b93dd9780924c128b9924e6c4f12472/`，OLE STA/返回值/清理及 UI 投递回归通过。
- P0-04：`p0-04-regression/run-20261103T085544682Z-d5d7a1732ecb41c5b30dfd7d2d3075b9/`，自有目标注册撤销与对象释放回归通过。
- `button-regression-final/button-results.json`，普通演示按钮约 2.012 秒恢复、退出码 0；窗口消息测试，不是物理鼠标覆盖。
- `errors-final/82eb112e0dbf4de589008d7b5dea4abb/results.json`，已有报告保留、缺父目录、缺参数均返回 1，三项符合预期。

### 当前边界与门禁

P0-05 本任务无未解决阻塞。本次覆盖 Windows 11 x64 / 100% 缩放、普通 Explorer → 普通目标的本地文件引用拖放。不承诺提升权限 Explorer 任意互操作，不覆盖虚拟文件、网络路径或所有第三方 IDataObject；超限/不支持格式拒绝，不自动执行数据。

DPI/逻辑坐标换算、跨显示器命中、分组分类弹簧加载、项目持久化、图标读取仍未实现；UI 仅显示接收统计，普通默认模式仍是基础演示。长时间资源泄漏、Win10、多 DPI、完整性能指标、全新机器运行库部署均未测试。UI 后端自身 COM 引用的全清理限制保持原记录。

P0-01～P0-05：通过；P0-06～P0-15：在当时未开始；P0 GO / NO-GO：未决策；P1～P4：未开始。

## P0-06：DPI 坐标转换（已验收）

结论：**2026-11-03 本任务验收通过，停在 P0-06。** 用户明确确认 200% 不属于 V1.2 正常使用验收范围，正式基线 §5.5/§16.4 已调整为 100%、125%、150%、175% 四档；四档真实系统缩放、独立 Slint 指针参照和 Explorer Drop 均已通过。

### 实现与产物

- Windows 坐标模块 `src/infrastructure/windows/coordinates.rs`：UI 线程新取得并借用 Slint HWND，ScreenToClient 转客户区物理点，按实际 window.scale_factor() 转为浮点逻辑点；GetDpiForWindow 和运行时 PMv2/客户区尺寸交叉核对，不以窗口含边框位置相减、不 clamp 越界点、不取整逻辑值。
- `src/dpi_validation.rs` 和 `--dpi-test`：独立四象限参照层只在验证模式显示，Slint pointer-event 提供自身逻辑位置，转换红点与其独立对照；正常演示 UI 保留，不开发 P1 正式布局。
- OLE Enter/Over/Drop 原始屏幕点经普通数据队列在 UI 消费时转换。日志明确是消费时快照，窗口几何变化不是原子采样；sizes_match=false 等诊断不能算稳定命中。
- Win32 依赖只增加固定 windows-sys 0.61.2 的 Gdi/HiDpi 功能；Rust/Slint/winit/software 技术路线未改。
- 当前 Release SHA-256 `73096799DB451B4520DB8C12312AC5A44ED2FC9577F84BF19CE4F897F906FD5B`，12,380,160 字节。

### 格式、编译与测试

`docs/test-results/P0-06/` 最终检查：

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| cargo fmt --all -- --check | 通过 | fmt-final.log |
| cargo check --locked --all-targets | 通过 | check-final.log |
| cargo clippy --locked --all-targets -- -D warnings | 通过 | clippy-final.log |
| cargo test --locked --all-targets | 21 库单元 + 1 二进制单元、6 合成数据通过，0 失败 | test-final.log |
| cargo test --locked --doc | 3 passed，0 failed | doctest-final.log |
| cargo build --locked --release | 通过 | release-final.log |

数学测试覆盖五种因子（含保留的 2.0 数学边界）、负值、非整数、半开边界、无效因子及空区域；正式系统验收按用户确认的 100/125/150/175% 四档执行并通过。

### 真实系统缩放与 Explorer 实测

Windows 11 x64 / 单活动 DISPLAY1。通过 Windows Settings 的实际缩放选项切换，没有修改注册表/私有 DPI 协议，没有设置 SLINT_SCALE_FACTOR，也没有发送伪造 WM_DPICHANGED。

| 档位 | 实测 HWND DPI / Slint scale | 稳定指针参照 | 真实 Explorer Drop |
| --- | --- | --- | --- |
| 100% | 96 / 1.00 | 通过 | 通过 |
| 125% | 120 / 1.25 | 通过；窗口调整瞬态单独排除 | 后续物理坐标复验通过 |
| 150% | 144 / 1.50 | 通过 | 通过 |
| 175% | 168 / 1.75 | 通过 | 通过 |

200% 根据用户 2026-11-03 的明确确认，不纳入 V1.2 必测范围；未以模拟比例计入验收。

- `final-100/interactive-4fd078cb/`：365 条独立 Slint 参照 delta 最大为 0，150/175% 各一次实际 Drop 的 PMv2、尺寸与 scale 检查均真；正常退出码 0、取得/释放 2/2、目标析构 1、errors=0。
- `physical-recheck/interactive-fff6ee10/`：源 Explorer PID39860、目标 PID14092，实际 Medium；384 条参照 delta 最大为 0，100/125% 各一次实际 Drop 通过。1 条窗口调整时快照 sizes_match=false（Win32 与 Slint 尺寸异步更新），明确不计稳定命中。
- 125% 正式 Drop：屏幕 (1378,527) → 客户区 (649,469) → 逻辑 (519.2,375.2)，dpi120/scale1.25/PMv2=true、尺寸/scale 一致。
- 100% 恢复后正式 Drop：屏幕 (1378,525) → 客户区 (650,474) → 逻辑 (650,474)，dpi96/scale1/PMv2=true。
- 最终复验正常退出码 0、取得/释放 2/2、目标析构 1、errors=0；样本哈希/目录状态未变，源与目标保持响应。
- 系统已恢复原始 `100% (推荐)`，Settings SelectionPattern 实际选中及真实 HWND DPI96/PMv2 交叉确认，见 restored-scale-final.json / restored-window-dpi.json。

### 测试现场问题

最初 TouchArea.moved 只在按住拖动时触发，普通悬停无参照；已改 pointer-event Move 并重新构建、重测。早期窗口遮挡、最小化占位坐标、PowerShell 数组运算和跨会话类型未加载等错误均未计通过。

125% 阶段的真实拖动起点曾被旧测试 Explorer、Settings 和 ZCode 覆盖；先前“125% 未通过”的说明是中间状态，最终已补齐。物理复验脚本将每个原生几何调用内部限定 PMv2，按 UIA 文件行与 Explorer 客户区交集选择可见起点，使用 SetPhysicalCursorPos；临时最小化测试 Settings/ZCode 后执行，随后恢复。没有绕过 WindowFromPoint 命中检查。被浮层覆盖的角落尝试保留为未测，不把截图上全部画面无覆盖作为结论。

完整明细见 [DPI 实测复核](test-results/P0-06/DPI_VALIDATION.md)，机器摘要见 `test-results/P0-06/validation-summary.json`，状态为 Passed。

### 回归与剩余门禁

P0-01～P0-04 自动回归均对应当前 EXE 哈希，目录分别为 p0-01-regression/run-20261103T114954817Z-e1bea1d8、p0-02-regression/run-20261103T114955285Z-58a78b476587443ba5dd9eb1ab612bf1、p0-03-regression/run-20261103T114954077Z-2619656cb8004360a286138e6d88f0be、p0-04-regression/run-20261103T114953562Z-9f373a90235246bc9c1b8843e7bebe70。P0-01 原始脚本保留人工检查项，不将其退出码当整体视觉验收。

当前 Settings 实际列出的缩放为 100/125/150/175%；这四档现为 V1.2 正式必测范围并全部通过。200% 根据用户明确确认移出 V1.2 必测范围，未以模拟因子计入验收。

真实负原点屏和混合 DPI 多屏未执行；Win10、长期泄漏、性能及全新机器运行库部署未测试。窗口快照不稳定时必须等待/重采样，不能用于正式命中。未实现分组分类坐标命中/跨屏恢复策略或项目持久化。

P0-01～P0-06：通过；P0-07～P0-15、P1～P4：在当时未开始；P0 GO / NO-GO：未决策。

## P0-07：Shell STA 服务（已验收）

结论：**2026-11-03 本任务验收通过，停在 P0-07**。第 29 章 P0-07（Shell STA 服务）与第 19 章 P0-07（JSON 保存）编号不同；本任务以第 29 章为准，实现基线 6.2 要求。

### 实现

- `src/infrastructure/windows/shell.rs`：独立 STA 线程 + `ShellService` 句柄。初始化严格使用 `CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)`；线程绑定 RAII `ComStaGuard`（!Send）配对 `CoUninitialize`。
- 消息队列在线程 ID 公布前用零超时 `MsgWaitForMultipleObjectsEx` 强制创建，消除 `PostThreadMessageW` 早投递竞态；主循环为 `GetMessageW` 三分支泵（>0 派发 / 0 退出 / -1 错误退出），请求通道在每次派发后 `try_recv` 排空，入队后投递自定义 WAKEUP 线程消息唤醒阻塞的泵。
- COM 接口（windows crate 官方生成的 `IShellLinkW`/`IPersistFile`）仅在 STA 线程内部使用；跨线程只传普通 Rust 数据与 per-request 回复通道。COM 对象按请求创建/释放，不驻留请求间。
- `--shell-test <report.tsv>` 验证入口 + `scripts/run-p0-07-medium.ps1` Medium 令牌运行器。IApplicationActivationManager 留作 UWP 阶段扩展点。

### 被测产物与检查

- Release SHA-256：`B30ABC15B25D981A4C17699B360A5A43E2FF80EA24D4121D6F29BEFC2DC96943`，12,525,056 字节。
- fmt 通过；`cargo check` / `cargo clippy --all-targets -- -D warnings` 通过；测试 22 lib + 1 bin + 6 合成 + 3 doc 全部通过、0 失败；Release 构建完成。日志在 `docs/test-results/P0-07/`。

### Windows 实际测试

- `run-30d2f796/`（开发会话）与 `medium-*/`（Medium RID 8192 受限令牌）完整运行均通过，退出码 0：
  - 服务启动并公布线程 ID；UI 线程 Apartment 前后均为 STA（kind=3）不变。
  - .lnk round-trip：Save 创建后 GetPath/GetIDList/GetWorkingDirectory/GetIconLocation 回读全部命中目标；路径按 `std::fs::canonicalize` 文件标识比较（shell 会展开 8.3 短路径）。
  - 真实系统 .lnk（115-Desktop.exe）解析出正确目标、工作目录。
  - `ShellExecuteExW` 打开测试目录成功并关闭返回的进程句柄。
  - 服务 Drop → WM_QUIT → 泵退出 → COM 反初始化 → join 无挂起；重启后新线程再次解析成功。
- `errors-*/results.json`：已有报告保护、缺父目录、缺参数均返回 1，内容未被覆盖。
- 回归：P0-03（OLE STA 初始化）与 P0-04（注册撤销）在含 Shell 服务的产物上均 pass。

### 开发问题与修复

首版手写 vtable 变参 shim 出现两类缺陷：请求超时（泵未被唤醒）与访问违例/乱码（ABI 错位）。修复泵唤醒后仍不安全，最终弃用手写方案，改用 windows crate 官方接口重写；废弃代码未保留，教训记录为本任务过程证据。测试侧修正了字符串路径比较与临时目录隔离问题。

### 边界与门禁

Resolve 当前使用 SLGP_RAWPATH 且不落盘；MSI 广告快捷方式、UWP/AUMID、`ShellExecuteExW` 启动真实项目（含 runas）在 P1/P2 对应任务处理。未执行 Win10、多显示器、性能与长期稳定性测试。

P0-01～P0-07：通过；P0-08～P0-15、P1～P4：在当时未开始；P0 GO / NO-GO：未决策。

## P0-08：Mutex 单实例（已验收）

结论：**2026-11-03 本任务验收通过，停在 P0-08**。基线 9.1/9.2（Mutex 部分）已实现并实测；9.3/9.4 Named Pipe 属 P0-09 未实现。基线 9.2 的 WAIT_* 返回清单与 Win32 实际语义不符（套用了 WaitForSingleObject），实现已按真实行为映射并记录。

### 实现

- `src/infrastructure/windows/single_instance.rs`：Mutex 名 `Local\QuickNest-{SID_HASH}-{SESSION_ID}`——`OpenProcessToken`+`GetTokenInformation(TokenUser)` 取 SID，对 Revision/IdentifierAuthority/SubAuthorities 字节做 FNV-1a 64 位哈希；`ProcessIdToSessionId` 取会话 ID。
- 获取流程：`CreateMutexW(null, TRUE, name)` → 紧读 `GetLastError`：`0`→首实例；`ERROR_ALREADY_EXISTS(183)`→零超时 `WaitForSingleObject` 探测，`WAIT_ABANDONED(128)`=原持有者已死、本实例接管，`WAIT_TIMEOUT(258)`=已有活实例（关闭探测句柄）；句柄为空=失败。
- `MutexGuard`：线程绑定（PhantomData<Rc>，仿 OleGuard），持有句柄至 Drop；owner 时 `ReleaseMutex`+`CloseHandle`。
- windows-sys 新增固定 feature：`Win32_Security`、`Win32_System_RemoteDesktop`。
- 验证入口：`--mutex-test/--mutex-child/--mutex-keeper`（`src/mutex_validation.rs`）+ `scripts/run-p0-08-medium.ps1`。

### 被测产物与检查

- Release SHA-256：`0735537A80FCF791DC94F915A59BEF6DFFE5BA4F0F9D0F125400BBC9BC1E515D`。
- fmt=0；check/clippy -D warnings 通过；测试 23 lib + 1 bin + 6 合成 + 3 doc 全部通过，0 失败。日志在 `docs/test-results/P0-08/`。

### Windows 实际测试

完整场景（开发会话 `run2/` 与 Medium 双轮 `medium-*/`）全部通过，parent exit 70 模拟崩溃：

| 阶段 | 证据 |
| --- | --- |
| 首实例 | parent acquired = FIRST_INSTANCE |
| 二次获取 | child acquired = ALREADY_RUNNING，probe = WAIT_TIMEOUT |
| keeper 保活 | keeper opened（OpenMutexW，SYNCHRONIZE） |
| 模拟崩溃 | parent exit = 70（跳过 Drop） |
| 接管 | keeper wait = WAIT_ABANDONED，abandoned_takeover |
| 释放 | keeper released → 对象可销毁；新进程可再以 FIRST_INSTANCE 获取 |

Mutex 名实测：`Local\QuickNest-7523641ad8f38453-1`（同用户同会话一致）。

- 错误路径 `errors-*/`：已有报告保护、`--mutex-test` 缺参数、`--mutex-keeper` 缺参数均返回 1，已有报告未覆盖。
- Medium 双轮以受限令牌运行，完整场景及断言（parent FIRST_INSTANCE / child ALREADY_RUNNING+WAIT_TIMEOUT / keeper WAIT_ABANDONED+abandoned_takeover）全部通过。
- 回归：P0-03 Medium 运行 pass 且哈希一致；P0-07 Shell STA Medium 探针重跑全部阶段通过、exit 0。

### 开发问题与修复

1. `spawn_exe` 以空白切分参数，带空格的报告路径被拆散（InvalidFilename）→ 改为参数数组传递。
2. 子进程报告打不开（Os code 2）：`OpenOptions::append(true)` 单独使用为 OPEN_EXISTING 语义，补 `.create(true)`。
3. 其余为一次性类型适配（HRESULT/WPARAM/LPARAM 新类型、windows crate feature 门控），修复后全部检查通过。

### 边界与门禁

当前仅验证 Local 命名空间内同用户同会话的单实例；跨会话隔离由 `Local\` + SESSION_ID 保证，SID_HASH 防跨用户冲突，但未实测第二用户会话。未实现 Named Pipe（P0-09）、二次实例唤醒主实例、WAIT_ABANDONED 时的配置数据检查（依赖 P1-12 原子保存）。Win10、多会话、性能与长期稳定性未测试。

P0-01～P0-08：通过；P0-09～P0-15、P1～P4：在当时未开始；P0 GO / NO-GO：未决策。

## P0-09：Named Pipe IPC（已验收）

结论：**2026-11-03 本任务验收通过，停在 P0-09**。基线 9.3（管道安全）、9.4（消息协议）、9.5（1500ms 启动竞态）已实现并实测；热键/托盘/窗口控制属 P0-10～12 未实现。

### 实现

- `src/infrastructure/windows/ipc.rs`：Pipe 名与 Mutex 同源（复用 `InstanceIdentity`），`\\.\pipe\QuickNest-{SID_HASH}-{SESSION_ID}`。
- 安全描述符：SDDL `D:P(A;;GA;;;当前用户)(A;;GA;;;SY)`，`ConvertStringSecurityDescriptorToSecurityDescriptorW` 生成，RAII `LocalFree`。严格仅当前用户 + SYSTEM；基线"必要时允许 SYSTEM"按此实现，其余主体（含远程）被保护 DACL 拒绝。
- 管道标志：`PIPE_ACCESS_DUPLEX | PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS`，单实例，缓冲各 4100 字节，`FILE_FLAG_FIRST_PIPE_INSTANCE`。
- 协议：4 字节小端长度前缀 + UTF-8 JSON payload（1..4096 字节，frame 上限 4100）；`IpcMessage { version:1, command, source }`，`deny_unknown_fields`，命令白名单 show/hide/toggle/show_and_focus/open_search；整个 frame 单次 WriteFile 写入。版本化 ack（accepted/rejected）使客户端能区分"写入成功"与"命令被接受"。
- 客户端身份验证：`GetNamedPipeClientProcessId`（PID 仅审计）+ `ImpersonateNamedPipeClient` → `OpenThreadToken` → `TokenUser` SID 字节比较，`ImpersonationGuard` 保证 `RevertToSelf` 无条件执行。SID 不匹配或 impersonation 失败即拒绝。
- 启动竞态（基线 9.5）：Mutex 先于 Pipe 获取；二实例发现 Mutex 已占用后在 1500ms 总预算内重试（`ERROR_FILE_NOT_FOUND` 短退避、`ERROR_PIPE_BUSY` 用 `WaitNamedPipeW`），超时则重新竞争 Mutex，未确认原实例失效绝不启动第二主实例。
- 依赖：直接固定 serde 1.0.229 / serde_json 1.0.154（复用锁文件已有版本）；windows-sys 新增固定 feature `Win32_System_Pipes`、`Win32_System_IO`、`Win32_Security_Authorization`。
- 附带修复 P0-08 遗留隐患：`MutexGuard` 的 `AlreadyRunning` 分支不再返回空句柄（避免 `CloseHandle(NULL)`），改为 `Option<HANDLE>`；SID 哈希/session 逻辑提取为共享 `InstanceIdentity`。

### 被测产物与检查

- Release SHA-256：`C3657B7243B3C500F6C339E12D0BF2F4CE3B8A55899C493B0A947D1315237C53`。
- fmt=0；check/clippy -D warnings 通过；测试 24 lib + 1 bin + 6 合成 + 3 doc 全部通过、0 失败。日志在 `docs/test-results/P0-09/`。

### Windows 实际测试（双进程）

- 开发会话 `run4/` 与 Medium 双轮（`medium-*/`，RID 8192）完整场景通过，Exit=0：
  1. Mutex 先于 Pipe 获取；Pipe 延迟 400ms 启动。
  2. race probe 子进程在 1500ms 预算内重试并成功连接（ipc.race.tsv 记录 elapsed_ms < 1500）。
  3. 5 个白名单命令（show/hide/toggle/show_and_focus/open_search）各由独立子进程发送，服务端按序接收（dispatched 行：expected 与 received 完全一致）。
  4. 4 种畸形帧（unknown-command/wrong-version/bad-json/unknown-field）各由独立子进程发送，全部被拒绝（rejected_frame），无副作用。
  5. 服务停止/重启，句柄清理无泄漏。
- `acceptance.json` 汇总协议细节、安全模型、竞态预算与回归结果。

### 开发问题与修复

1. 首轮实测客户端 `short read` 失败：message 模式下 ReadFile 每次返回一条完整消息，固定 128 字节 ack 缓冲导致短读被误判错误；改为 `read_ack`（按消息实际长度读取）。服务器帧读取同理改为 `read_frame`。
2. PowerShell 字符串替换脚本曾把 ipc.rs 破坏成单行/半截文件；最终用 Write 工具完整重写，内容与格式均恢复。教训：不用内联字符串替换改源码。
3. 其余为一次性类型适配（IpcError 未实现 Error trait 的错误映射、Arc<AtomicBool> 替代 Rc、HLOCAL/LocalFree 导入位置），均已修复。

### 边界与门禁

- 当前仅同用户同会话验证；远程客户端拒绝的动态测试需要第二台 Windows 主机（`\\HOST\pipe\...`），本机 `\\localhost\pipe\...` 受 SMB/防火墙影响不能替代；静态证据为 CreateNamedPipeW 参数中的 PIPE_REJECT_REMOTE_CLIENTS（代码固定，不可配置关闭）。
- 服务器同步阻塞在 ConnectNamedPipe；通过可信本地唤醒连接实现优雅停止，未引入 overlapped I/O（留作后续优化）。
- IPC 命令仅被记录；真实窗口 show/hide/toggle 派发属 P0-10～12 窗口行为任务。
- 未执行 Win10、第二用户会话负向测试、性能与长期稳定性测试。

P0-01～P0-09：通过；P0-10～P0-15、P1～P4：未开始；P0 GO / NO-GO：未决策。

## P0-10：全局快捷键（已验收）

结论：**2026-11-04 本任务验收通过，停在 P0-10**。基线 11.1（默认 Ctrl+Alt+Space、RegisterHotKey、冲突检测、优先 MOD_NOREPEAT、失败提示、可靠消息循环、线程安全通知 UI）已实现并实测；搜索快捷键（11.2）与系统托盘（11.3）属后续任务未实现。

### 实现

- `src/infrastructure/windows/hotkey.rs`：`HotkeyGuard` 线程绑定 RAII（PhantomData<Rc<()>>），专用消息循环线程执行 `RegisterHotKey(NULL, MOD_CONTROL|MOD_ALT|MOD_NOREPEAT, VK_SPACE)`；注册前 `MsgWaitForMultipleObjectsEx(0)` 零超时强制建立消息队列，消除 PostThreadMessage 竞态后再发布线程 ID。
- 消息泵：`GetMessageW` 三分支循环（0/−1/其余），`WM_HOTKEY` 事件带自增 sequence 经 mpsc 送达所有者；控制通道（Shutdown）+ `PostThreadMessageW(WAKEUP)` 唤醒阻塞的泵。
- 注销路径唯一收口：所有退出路径（stop 标志、WM_QUIT、GetMessageW 错误、事件通道关闭）统一落到循环后的单次 `UnregisterHotKey`，保证在注册线程释放。
- 冲突映射：`GetLastError` 紧跟失败调用读取，`ERROR_HOTKEY_ALREADY_REGISTERED(1409)` 显式映射为 `AlreadyRegistered`，其余为 `Failed(code)`。
- `inject_default_hotkey()`（SendInput 合成 Ctrl+Alt+Space，修饰键顺序：按下顺序 Ctrl→Alt→Space，释放逆序）仅用于验证注入。
- 依赖：windows-sys 新增固定 feature `Win32_UI_Input_KeyboardAndMouse`；版本不变。

### 被测产物与检查

- Release SHA-256：`DA3EC6B1F7EC75DF87EB6ACAFB49DC412E9E2366A3708CBB989BD62530C2C33`。
- fmt=0；check/clippy -D warnings 通过；测试 26 lib（含热键生命周期回归单测）+ 1 bin + 6 合成 + 3 doc 全部通过、0 失败。日志在 `docs/test-results/P0-10/`。

### Windows 实际测试（双进程 + 真实注入）

- 开发会话 `hotkey-run5.tsv` 与 Medium 受限令牌（`medium-f6866f94/`，RID 8192）完整场景通过，Exit=0：
  1. `registered`：Ctrl+Alt+Space MOD_NOREPEAT 注册成功。
  2. `conflict_detected`：子进程 `--hotkey-conflict` 探测同组合被拒，报告 `ERROR_HOTKEY_ALREADY_REGISTERED`（冲突检测证据）。
  3. `hotkey_event`：SendInput 注入真实按键 → `WM_HOTKEY` 经消息泵到达 → `sequence=1 forwarded_to_ui=1`（经 `invoke_from_event_loop` 线程安全转发 UI）。
  4. `unregistered_cleanup`：Drop 后独立子进程注册同一组合成功（注销清理证据）。
  5. `re_registered` + `second_event`：重注册后再次注入，事件到达。
  6. `completed`：全部阶段通过，正常退出。
- 失败/诊断记录保留（不算通过）：`run1/`（invoke_from_event_loop 无事件循环阻塞）、`run2/`（Drop 无唤醒挂死）、`hotkey-run3.tsv`（控制通道 Disconnected 空转挂死，杀进程收场）、`hotkey-run4.tsv`（挂死修复后 debug 构建，冲突码被诊断日志污染为 0x00000006）。

### 开发问题与修复

1. run1 挂死 60s：`invoke_from_event_loop` 在事件循环未运行时永久阻塞；重构为顺序场景在 worker 线程执行、主线程跑 Slint 事件循环（`run_event_loop_until_quit`）。
2. run2 挂死：Drop 发送 Shutdown 控制后，泵阻塞在 `GetMessageW` 永远观察不到；`HotkeyGuard` 增加 `thread_id`，Drop 在 join 前投递 `PostThreadMessageW(WAKEUP)`。
3. run3 挂死（决定性证据见 `run4/diagnostic-isolation-hang.log`）：控制通道排空循环只在 `Empty` 时退出，而 Drop 后 Sender 已丢弃、`try_recv` 永远返回 `Disconnected`，导致泵无限空转 `UnregisterHotKey`+`PostQuitMessage`；修复为 `Shutdown|Disconnected` 统一 `PostQuitMessage` 并交由收口 `UnregisterHotKey` 释放。新增无 Slint/无注入的最小生命周期单测 `drop_unregisters_and_allows_re_registration` 固化该行为。
4. run4 假象：诊断 `eprintln` 插在 `RegisterHotKey` 与 `GetLastError` 之间覆盖 last error（0x00000006），被误读为冲突语义变化；移除全部诊断日志后 run5 恢复 1409。教训：GetLastError 必须紧跟失败调用，中间不得插入任何 Rust 标准库调用。

### 边界与门禁

- 事件经 mpsc 直达验证持有者；真实 UI 状态文本更新仅验证了转发路径（`forwarded_to_ui=1`），完整交互行为（弹出/隐藏窗口）属 P0-12 窗口焦点状态机。
- `MOD_NOREPEAT` 行为依赖系统合成，未做长按压力测试；未验证与其他应用注册同组合时的抢占顺序语义（Windows 行为为后注册者失败，已由冲突探测覆盖）。
- SendInput 注入需交互式桌面；Medium 令牌同会话注入已通过，跨桌面/服务会话未测试。
- 未执行 Win10、注销/锁屏下的热键行为、性能与长期稳定性测试。

P0-01～P0-10：通过；P0-11～P0-15、P1～P4：未开始；P0 GO / NO-GO：未决策。

## P0-11：系统托盘（已验收）

结论：**2026-11-04 本任务验收通过，停在 P0-11**。基线 11.3 的图标生命周期、回调路由、六项菜单与 TaskbarCreated 重建已实现并实测；托盘呼出后的真实窗口动作与菜单命令执行属 P0-12 未实现。

### 实现

- `src/infrastructure/windows/tray.rs`：`TrayService` 线程绑定 RAII（沿用 HotkeyGuard 模式：专用消息循环线程 + 控制通道 + PostThreadMessageW WAKEUP 唤醒 + join）。回调窗口为隐藏顶层窗口（style 0，非 HWND_MESSAGE——消息专用窗口无法承载前台/弹出菜单），菜单期间 `SW_SHOWNOACTIVATE` 显示零尺寸宿主窗口。
- `Shell_NotifyIconW` NIM_ADD/NIM_MODIFY/NIM_DELETE，NIF_MESSAGE|NIF_TIP|NIF_SHOWTIP，回调消息 `WM_APP+0x5452`，图标 ID 1，图标 `IDI_APPLICATION`（正式图标属 P1）。
- 菜单：`CreatePopupMenu`+`AppendMenuW` 六项按基线 11.3 顺序（显示 QuickNest/隐藏 QuickNest/固定-取消固定/设置/检查更新/退出程序），id 1..6 映射 `TrayCommand`；右键路径 `SetForegroundWindow`+`TrackPopupMenu(TPM_RETURNCMD|TPM_RIGHTBUTTON|TPM_BOTTOMALIGN)`+`PostMessageW(WM_NULL)`（文档化点击吞噬怪癖）+`SW_HIDE` 还原。
- 事件：`TrayEvent`（Added/ReAdded/MenuVerified/LeftClick/DoubleClick/RightClick/MenuOpened/MenuDismissed/MenuSelected/Deleted）经 mpsc 送达；`take_events` 允许在 Drop 后仍观察最终 Deleted 证据。
- TaskbarCreated：`RegisterWindowMessageW` 注册广播消息，WndProc 收到即在托盘线程重加图标。
- 注销：所有退出路径收口到托盘线程上的 NIM_DELETE+DestroyMenu+DestroyWindow（基线 11.3 退出序列第 3 步）。
- 新模块 `keyboard_inject.rs`：SendInput 单键注入供验证使用；依赖无新增 feature（Win32_UI_Shell/WindowsAndMessaging 均已有）。

### 被测产物与检查

- Release SHA-256：`1F717F051133634657EA37ED66A59F24FCFBD1CB8FB9FC08A0ECCFDB01B9CC19`。
- fmt=0；check/clippy -D warnings 通过；测试 27 lib + 1 bin + 6 集成 + 3 doc 全部通过、0 失败。日志在 `docs/test-results/P0-11/`。

### Windows 实际测试（真实通知区域交互）

- 开发会话 `tray-run8.tsv`（当前哈希二进制）九阶段全部通过，Exit=0：
  1. `added`：NIM_ADD 成功，图标真实出现在通知区域。
  2. `tooltip_modified`：NIM_MODIFY 修改提示成功。
  3. `menu_verified`：托盘线程自建菜单，GetMenuItemCount=6，首项"显示 QuickNest"、末项"退出程序"与基线一致。
  4. `callback_routed`：按 Explorer 生产方式投递回调消息（wParam=图标 id，lParam=鼠标消息），WM_LBUTTONUP→LeftClick 事件→`invoke_from_event_loop` 转发 UI。
  5. `menu_shown_dismissed`：WM_RBUTTONUP→菜单真实弹出（#32768 窗口可见性独立核查）→SendInput ESC→MenuDismissed。
  6. `menu_selected`：再次弹出菜单→SendInput Down+Enter→`MenuSelected(Show)`（键盘导航+选中返回 id 的真实路径）。
  7. `taskbar_readded`：先 NIM_DELETE 再广播 TaskbarCreated（忠实模拟 Explorer 重启），托盘线程 NIM_ADD 成功。
  8. `deleted`：Drop 后托盘线程 NIM_DELETE 成功（`Deleted{success=true}`）。
  9. `completed`：全部阶段通过，正常退出。
- 失败/诊断记录保留（不算通过）：`logs/tray-run1~6.tsv`（事件顺序缺陷、前台方案迭代、VK 常量混用）；`diag5-attach-menu-instant-dismissal.log`（跨进程 AttachThreadInput 使菜单打开即关——弃用方案）、`diag6-plain-setforeground-menu-works.log`（普通 SetForegroundWindow 方案成立）、`diag7-taskbar-readd-efail.log`（图标未删时重复 NIM_ADD 返回 E_FAIL）。

### 开发问题与修复

1. 验证框架缺陷：场景线程出错时事件循环永不退出、错误被吞（run1~run5 表现为超时挂死）。修复为失败时 `quit_event_loop` 并把 `scenario_error` 写入报告；该改进直接暴露了 Medium 下的真实错误。
2. 菜单打开即关：跨进程 `AttachThreadInput` 共享输入队列导致 TrackPopupMenu 立即返回 0（gle=0，菜单窗口从未出现，diag5）。弃用 attach，改用普通 `SetForegroundWindow`——菜单保持打开且 SendInput 键盘可达（diag6）。隐藏窗口不能成为前台窗口是菜单需要 SW_SHOWNOACTIVATE 的原因。
3. 测试按键错误：VK_UP(0x26) 被当作 VK_DOWN(0x28)，菜单按"上"从未选中状态环绕到末项，返回 id 6"退出程序"——这反向验证了 id→命令映射与键盘导航的正确性；修正 VK 后返回第一项 Show。
4. TaskbarCreated 模拟失真：图标存在时重复 NIM_ADD 被 Shell 拒绝（E_FAIL，diag7）；增加 `remove_icon()` 先删再广播。
5. 托盘线程早期错误不上报：pump 的 RegisterClassW/CreateWindowExW/NIM_ADD/菜单错误路径未走 ready 通道，注册失败只报"exited before registration"；全部错误路径补 `ready.send(Err(...))` 后 Medium 证据可见。

### 边界与门禁

- **Medium 令牌在本机无法验证托盘**（环境限制，非产品缺陷）：本机交互会话的 Explorer 为高完整性（P0-05 已记录），任何 Medium 进程对 Explorer 托盘窗口的 WM_COPYDATA 被 UIPI 拒绝，`Shell_NotifyIconW` 返回 ERROR_ACCESS_DENIED(5)（`medium-*/tray.tsv` 留证）。生产环境标准用户会话中 Explorer 为 Medium，托盘为普通操作；`run-p0-11-medium.ps1` 明确识别该拒绝形态并标记"环境限制、非 Medium 通过"。
- 回调路由用 PostMessageW 直接投递回调消息（与 Explorer 生产投递形态一致）；真实鼠标点击任务栏通知区图标未自动化（坐标依赖 Explorer 内部布局）。
- 菜单命令只验证路由与映射（MenuSelected(Show)），不执行真实窗口动作；托盘呼出显示/隐藏面板属 P0-12。
- 图标使用系统 IDI_APPLICATION；正式图标提取与缓存属第 12 章/P1。
- 未执行 Win10、多显示器下菜单定位、DPI 缩放下的菜单尺寸、性能与长期稳定性测试。

P0-01～P0-11：通过；P0-12～P0-15、P1～P4：未开始；P0 GO / NO-GO：未决策。

## P0-12：窗口焦点状态机（已验收）

结论：**2026-11-04 本任务验收通过，停在 P0-12**。基线第 10 章（状态机、抑制锁、失焦流程、事件源、系统弹窗抑制、固定模式）与 19 章 P0-06 窗口行为（显示/隐藏、固定、失焦、ESC、快捷键、托盘呼出）已实现并实测；UAC 启动对话框（LaunchingElevated 的真实来源）与设置持久化属 P1+。

### 实现

- `src/window_state.rs`：纯逻辑状态机（零 Win32/Slint 依赖，14 项单测全覆盖）。`PanelState` 五态含 ShuttingDown 终态拒绝；转换返回完整路径供证据记录。`SuppressionSet` 引用计数 + `SuppressionToken` RAII（Drop 自动释放、与机器解耦存活、超释放饱和为零）。固定状态独立管理并镜像为 Pinned 抑制锁（10.1/10.2/10.7 统一机制）。失焦两段式：`observe_blur`（状态/抑制/前台检查，绝不直接隐藏）+ `evaluate_blur_recheck`（120ms 后复查，前台归我或出现抑制则 Stay）。
- `src/infrastructure/windows/focus.rs`：前台归属 = GA_ROOTOWNER 链相同或同进程（10.3 第 7 步"QuickNest 或其拥有的弹窗"+ 10.4 GetWindowThreadProcessId）；进程可见窗口枚举、窗口矩形/中心、标准激活路径、#32768 菜单窗口探测。
- `ui/main_window.slint`：FocusScope ESC → `escape-pressed` 回调（`\u{1b}` accept/else reject）。
- `src/focus_validation.rs`：验证壳。UI 线程 setup Timer 等 HWND 就绪（P0-02 同款时序）；blur 管线（Slint Timer 60ms 轮询 + 一次性 120ms pending 槽）；hotkey/tray 事件 forwarder 线程把真实事件源镜像为状态机动作；场景线程逐阶段断言并写 TSV。
- `keyboard_inject.rs` 新增 `click_at`：SendInput 绝对移动（虚拟桌面归一化）+ 左键，替代被 Medium 拒绝的 SetCursorPos（错误 183）。

### 被测产物与检查

- Release SHA-256：`C11E2BC61CA1143F148F1123185C8F1448AFEFC07EFFBDB99AAD70C134E0`。
- fmt=0；check/clippy -D warnings 通过；测试 41 lib（14 项状态机单测）+ 1 bin + 6 集成 + 3 doc 全部通过、0 失败。日志在 `docs/test-results/P0-12/`。

### Windows 实际测试（dev 会话全链 + Medium 核心链）

- dev 会话 `focus-run7.tsv` 17 行全部通过，Exit=0：状态转换路径记录 → ESC（SendInput 真实按键）→ 热键 toggle ×2（注册→注入→事件 drain→UI 线程驱动）→ 托盘左键 toggle ×2 → 托盘菜单 Show/Hide（MenuOpened 握手 + #32768 坐标真实鼠标点击）→ blur 隐藏（外部子进程窗口夺前台 → 120ms recheck → 隐藏）→ pinned 失焦不隐藏 → 取消固定恢复 → ContextMenuOpen RAII 抑制 → token Drop 后隐藏 → refocus 后 recheck 不闪隐 → 再次失焦 → ShuttingDown 拒绝转换 → completed。
- Medium `medium-dfe8b11e/` 13 行 Exit=0：状态机核心、ESC（消息路径）、**blur 全链真实跑通**（child 借创建期前台权成功夺前台）；hotkey 注入与托盘按环境限制如实降级（见下）。
- 失败/诊断记录保留（不算通过）：`logs/focus-run1~6.tsv`（HWND 时序、事件顺序、前台竞态）、`logs/diag-recheck-postpone-loop.log`（recheck 无限顺延的决定性证据）。

### 开发问题与修复

1. HWND 就绪时序：show() 返回不等于原生句柄可用；setup Timer 在事件循环内检查 inspect_window 成功后再同步机器并停放 Hidden。
2. recheck 无限顺延：轮询闭包每 tick 重设 deadline，pending 永不到期；改为 pending 空时才设置（基线 10.3"一次性延迟检查"的准确语义）。
3. Medium 下键盘选菜单不可靠：改 MenuOpened 握手 + 真实鼠标点击菜单项（鼠标点击经菜单捕获，不依赖前台键盘）。
4. SetCursorPos 在 Medium 返回 183：换 SendInput 绝对移动通道。
5. Medium 热键注入无事件（forwarder consumed=0、drain 亦空）：鉴别实验（当日重跑 P0-10 Medium 通过）锁定根因为 **UIPI 丢弃 Medium 进程向 High 前台注入的合成输入**（本机 Explorer 为 High，P0-05/P0-11 同源限制；SendInput 返回成功但 win32k 过滤按键）。phase c 改为"前台归我才注入"，否则记录 hotkey_env_limited；P0-10 Medium 的通过正是因为其注入时前台是自己的窗口。物理按键不受 UIPI 影响，产品行为无缺陷。
6. 单测与并行实测实例争抢全局热键偶发失败：串行执行（教训：全局资源测试不可并行）。

### 边界与门禁

- Medium 下两类降级为环境限制而非失败，均有 P0-05/P0-11 同源证据：hotkey 注入（UIPI，前台为 High 时合成输入被过滤）与托盘（Shell_NotifyIconW 被拒）。两者在 dev 会话（High 令牌）全链验证，P0-10 另有 Medium 独立证据。
- ESC 的 Medium 路径走 PostMessageW(WM_KEYDOWN/KEYUP)（winit 按真实按键处理）；SendInput ESC 在 dev 会话验证。两条路径最终都进入同一 slint escape-pressed 回调。
- blur 管线的"重新获取前台窗口"由 60ms 轮询实现；基线所述"一次性延迟检查"默认 120ms 已实现，轮询间隔带来的最大 ±60ms 检测延迟已记录。
- Showing/Hiding 为瞬态（转换路径记录在案）；真实动画/淡入淡出属 P1 视觉层。
- 未执行 Win10、多显示器失焦行为、锁屏/注销、性能与长期稳定性测试。

P0-01～P0-12：通过；P0-13～P0-15、P1～P4：未开始；P0 GO / NO-GO：未决策。

## P0-13：ReplaceFileW 保存验证（已验收）

结论：**2026-11-04 本任务验收通过，停在 P0-13**。基线第 15 章的保存链（15.2-15.6）与损坏恢复（15.8）已实现并实测；防抖、备份保留策略、Schema 迁移（15.7/15.9）属 P1。

### 实现

- `src/infrastructure/windows/config_save.rs`：`save_atomic` 保存链——同目录唯一临时文件 `config.<PID>.<UUID>.tmp`（`UuidCreate` 生成 GUID，冲突自动换名重试）→ `create_new` 独占创建 → 完整写入 + `sync_all` → 关闭句柄。目标不存在走 `MoveFileExW(MOVEFILE_WRITE_THROUGH)` 同卷移动（15.5）；`ERROR_ALREADY_EXISTS(183)` 并发创建回退 `ReplaceFileW`——被竞态创建的文件进唯一备份 `config.backup.<UTC戳>.json` 而非丢失。替换 flags=0（15.4：无 WRITE_THROUGH——不受支持；无忽略 ACL 标志）；失败保留原配置与 tmp（诊断用）并携带 GetLastError。
- `recover_latest_backup`（15.8）：按文件名倒序扫描 `<stem>.backup.*.json`（时间戳格式保证字典序=时间序）、逐份 JSON 解析验证、恢复最近**有效**份；损坏活文件改名 `config.corrupted.<戳>.json`；恢复写回失败时把损坏文件改回原名（不留缺失态）；无有效备份进安全模式（不动任何文件）；活配置仍有效时拒绝恢复（15.8 的契约是"解析失败"）。
- 单写者锁（15.7）：save 与 recovery 共用进程内互斥，持锁期间并发保存以 `SaveError::Busy` 拒绝；锁毒化（panic 后）自动恢复 guard 不楔死。跨实例写锁留待 P1 与 P0-08 单实例锁整合。
- 依赖：windows-sys 新增固定 feature `Win32_System_Rpc`（UuidCreate）。

### 被测产物与检查

- Release SHA-256：`86A3D930799F41FEA74668E90512F0D0E7EFB51AF09F7821E479E658B0D32D62`。
- fmt=0；check/clippy -D warnings 通过；测试 53 lib（含 11 项保存链单测）+ 1 bin + 6 集成 + 3 doc 全部通过、0 失败。日志在 `docs/test-results/P0-13/`。

### Windows 实际测试（真实文件系统，dev + Medium）

- dev 会话 `save-run2.tsv` 与 Medium `medium-10928112/` 七阶段各通过，Exit=0：
  1. `first_create`：空目录首存 → Created，内容正确，无 tmp/backup 残留。
  2. `atomic_replace`：ReplaceFileW flags=0 → 新内容生效、备份持旧内容、无 tmp。
  3. `failure_protection`：CreateFileW(FILE_SHARE_READ) 锁定目标（真实注入）→ 保存失败 ERROR_SHARING_VIOLATION(32)、旧配置完好、tmp 保留且含失败写入内容、错误码入报告。
  4. `temp_lifecycle`：成功路径零残留；失败路径 tmp 保留至诊断后手动清理。
  5. `backup_recovery`：备份集（旧有效/中有效/最新损坏）+ 损坏活文件 → 恢复最新**有效**（{"gen":11}）、损坏文件改名 `config.corrupted.<戳>.json` 且内容完整。
  6. `safe_mode`：全部备份损坏 → 原文件逐字节未动。
  7. `completed`。
- 失败记录保留（不算通过）：`logs/run1.tsv`——场景缺陷（阶段 a/b 真实备份未被轮换，恢复正确选中它而断言失败）；实现行为正确，场景已修。

### 开发问题与修复

1. 并行单测与进程级写锁互斥：cargo test 多线程争锁大面积 Busy；测试模块自串行化 + 锁毒化容忍（clear_poison 后接管，不永久楔死）。
2. Rust std `File::open` 默认共享含 DELETE，不构成替换锁：失败注入改 CreateFileW(FILE_SHARE_READ)；std 宽松共享行为已记录。
3. 锁验证时序竞态：对真实恢复的 30ms racer 在快机器赶不上锁窗口；改确定性验证（持锁→断言 Busy→释放→断言可保存），recovery 走同一锁故语义等价。

### 边界与门禁

- 单写者锁为进程内实现；两个 QuickNest 进程间的写入互斥由 P0-08 单实例 Mutex 承担（同用户同会话只有一个主实例），P1 接入真实配置时再评估是否需要跨进程保存锁。
- 备份唯一性依赖 UTC 毫秒戳；同毫秒多次保存理论上可能重名（ReplaceFileW 会覆盖旧备份），P1 备份轮换策略（保留最近 10 份）落地时一并处理。
- 恢复验证只做 JSON 解析（serde_json）；Schema 级校验属 15.9/P1。
- 未测网络共享/可移动介质（基线 15.6 明示不承诺同等保证）、NTFS 之外的文件系统、断电级故障注入（sync_all 为软件层最强持久化点）。

P0-01～P0-13：通过；P0-14～P0-15、P1～P4：未开始；P0 GO / NO-GO：未决策。





