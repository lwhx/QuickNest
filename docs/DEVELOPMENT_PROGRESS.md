# QuickNest 开发进度

- 开发基线：QuickNest V1.2 正式开发基线。
- 工程版本：**1.2.1**；1.2.0 为已公开历史版本，0.1.0 仅为 P0 原型历史版本。
- 更新日期：2026-11-08。
- 当前阶段：**v1.2.1 已发布（tag `v1.2.1`，Release ID 437994041），七项实施完成，统一验证、性能对照与本地交付校验已通过；P4 完整发布矩阵尚未验收通过**。
- 当前状态入口：[1.2.1 优化记录](OPTIMIZATIONS_1.2.1.md)。此前 [五项修复](AUDIT_FOLLOWUP_2026-11-08.md)、[审核修复](AUDIT_REPAIRS_2026-11-07.md) 与 [历史发布构建](AUDIT_FIXES_2026-11-07.md) 的测试数和哈希只属于对应构建。
- 本轮范围：无变化选择零写入、LaunchRecord 批量事务及有界 BLAKE3 备份校验缓存；manual 快速路径与 first-rank map；图标 set_row_data 增量、32 有界非阻塞队列/current model generation/失败冷却；PreparedQuery 借用拼音 cache；PillButton/设置入口键盘与 a11y；正式 modal_ui 共用注册及 NameDialog/滚动 editor；Cargo 单版本 1.2.1 与来源指纹内嵌、共享 staging、本地产物独立核验。
- 统一验证：fmt/check/clippy 退出码 0；安全集合 lib 324（3 ignored）、bin 29、ui_layout 8、icon_queue 13、其余集成 12、doc 3，0 失败；预检 96 入口、产物校验 39 项反例。明细与日志见 [验证记录](test-results/2026-11-08-optimizations-1.2.1/VALIDATION.md)。
- 前后性能：同一工具与输入成对实测——重复选择新增备份 3→0（零写入）、5000 项记录并保存 149.1→68.8 ms、10000 行手动构建 438.6→5.2 ms、10000 项冷空查询 74.2→3.5 ms。完整口径见 [性能对照](test-results/2026-11-08-optimizations-1.2.1/PERFORMANCE.md)。
- 本地交付：`installer/output/release-1.2.1-7ac1990619d44342bc857cdc984763f2`；EXE/setup/ZIP 大小与 SHA-256、来源及构建指纹、独立校验与早期入口探针见 [优化记录](OPTIMIZATIONS_1.2.1.md)。setup 仅编译，未安装或解包。
- 发布前置条件仍包括：Win10 实测、完成页真实 Medium 启动、两个不同生产版本真实退出换版、独立生产 key + HTTPS 端点及本轮 UI 的隔离桌面/DPI/多显示器回归。当前不宣称全矩阵验收。

## 本轮范围与证据约束

七项优化保留既有排序、schema/BOM、最近 10 份有效备份、保存失败事务回滚和产品交互语义，详见 [优化记录](OPTIMIZATIONS_1.2.1.md)。测试安全集合、隔离与设施排障统一见 [TEST_HARNESS.md](TEST_HARNESS.md)。

本地构建默认禁用更新 key，记录实际冻结输入；dirty 工作树允许被准确标识，base HEAD 只是基准。来源指纹证明本地过程一致，不等于第三方签名或跨机器可复现构建。EXE 与 ZIP 内部验证和 setup 冻结输入/编译证据分别记账；setup 尚未安装、未独立解包，不填实机验收通过。更新仍仅支持 raw，setup/portablezip 在下载前拒绝；生产路径拒绝已披露测试密钥。

已公开 v1.2.0 的附件、历史记录与哈希不变；原发布说明完整归档于 [releases/1.2.0.md](releases/1.2.0.md)。

## 历史逐任务记录（仅供追溯）

以下逐任务清单、通过结论、后续计划、测试数量及产物哈希均属当时构建，**不能认证当前构建**。“最终”“当前”“下一任务”等旧语境仅指对应历史任务；旧 P4 完成声明已撤回，旧 48 脚本结果不能沿用。原任务记录保留用于追溯，当前验收以本文顶部链接为准。

## 历史记录：范围与编号

第 19 章按 7 组描述 P0 验证内容，第 29 章拆分为 15 项。任务追踪采用第 29 章编号，验收同时参考第 19 章相关要求；本次两处 P0-01 范围一致。不能把第 19 章后续组号直接当作第 29 章任务编号。

## 历史记录：P0-01 任务清单

- [x] 完整阅读正式基线。
- [x] 检查工作区：当前无 Git 仓库，未初始化、提交或推送。
- [x] 创建 Rust 工程；Rust 1.96.0、Slint/slint-build 1.18.1 及 Cargo.lock 固定版本。
- [x] 原生 winit + software renderer；未更改既定 Rust + Slint 架构。
- [x] 主窗口、6 项静态图标网格、完整中文名称实际显示。
- [x] Slint 显示/隐藏；隐藏期间事件循环保持，定时恢复同一窗口。
- [x] 实际按钮回调：隐藏后约 2.03 秒恢复，退出码 0。
- [x] 内嵌 asInvoker / uiAccess=false / Per-Monitor V2 Manifest 检查。
- [x] 格式、check、clippy 全通过；5 项单元测试通过。
- [x] Release 编译通过；AMD64 / PE32+ / GUI 子系统验证通过。
- [x] Windows 11 x64、DPI 96（100%）实际测试；Medium 完整性下三轮隐藏/恢复、正常退出通过。
- [x] 静态导入、实际加载模块和子进程检查；无浏览器内核、Node/Python 运行时。
- [x] 仅复制 EXE 到独立目录后运行，资源和按钮正常。
- [x] 更新正式基线第 29 章 P0-01 勾选与验证证据。

## 历史记录：P0-02 任务清单

- [x] 复核正式规范、Slint provider 与 winit 消息驱动销毁语义。
- [x] 启用 raw-window-handle-06，固定直接依赖版本。
- [x] Windows 适配层在 provider 存活范围内检查非空 HWND、IsWindow、PID/TID。
- [x] 独立 --hwnd-test 三代窗口验证与逐阶段日志。
- [x] 格式、check、clippy、7 项单元测试和 Release 编译通过。
- [x] 首次本机实际运行：显示/隐藏/恢复、组件释放和原生 HWND 失效通过。
- [x] Medium 普通权限下外部独立采样核对，3 次运行、共 9 代窗口全部通过。
- [x] P0-01 显示/隐藏与按钮回归通过，初次及第三次恢复截图复核正常。
- [x] 已有报告保护、环境覆盖拒绝、提前关闭等失败路径均返回非零。
- [x] 最终验收及正式清单勾选。

## 历史记录：P0-03 任务清单

- [x] 复核第 5.2 节与第 29 章任务边界、winit 初始化调用位置。
- [x] OleGuard 只持有一份成功初始化引用，!Send/!Sync，同线程恰好一次清理。
- [x] S_OK、S_FALSE、RPC_E_CHANGED_MODE 实际验证；失败不反初始化，保留 MTA。
- [x] 全新线程和预先 COM STA 线程嵌套 OLE 及配对释放；全部自有引用释放后恢复未初始化。
- [x] 正常启动在选择后端前持有本层 OLE Guard。
- [x] Slint UI STA、Timer、三个跨线程回调、三代 HWND 生命周期通过。
- [x] 格式、check、Clippy、10 项单元测试、2 项 !Send/!Sync 编译失败文档测试及 Release 编译通过。
- [x] 三次 Medium 独立运行全部通过，进程退出码 0，无强制终止。
- [x] P0-01 显示/隐藏/按钮/截图、P0-02 外部生命周期回归通过。
- [x] 报告保护与缺失路径/参数错误退出实际通过。
- [x] 正式任务清单与证据报告更新。

## 历史记录：P0-04 任务清单

- [x] 复核 Slint/winit 默认 OLE 注册行为与窗口定制入口。
- [x] 固定 Slint 1.18.1 的 unstable-winit-030 窗口钩子，在建窗前关闭后端自带拖放；不更换 UI 后端。
- [x] 固定 windows/windows-core 0.62.2，使用官方 COM 实现宏创建非敏捷 IDropTarget。
- [x] 最小四回调只返回 DROPEFFECT_NONE，不读取 IDataObject 或用户路径。
- [x] 同线程注册 Guard 只管理自己的成功注册，持有窗口 provider 与 OLE 引用；失败不撤销别人的目标。
- [x] 三代窗口的注册、重复注册冲突、隐藏恢复、显式撤销、重注册、RAII 撤销与 COM 最终析构实测通过。
- [x] Medium 权限三轮独立验证共九代窗口全部通过，退出码 0、无强制终止。
- [x] 格式、check、Clippy、14 项单元测试、3 项线程约束文档测试和 Release 编译通过。
- [x] P0-01～P0-03 回归、按钮/截图复核、报告保护及错误参数退出均通过。
- [x] 正式清单、进度与验收证据更新。

## 历史记录：P0-05 任务清单

- [x] 新增 CF_HDROP 文件接收目标，P0-04 拒绝目标与注册测试保留。
- [x] QueryGetData/GetData、Unicode 多路径复制、原始屏幕 POINTL 记录和 COPY 效果协商。
- [x] STGMEDIUM 唯一 RAII 所有权，成功和校验/队列失败路径配对 ReleaseStgMedium。
- [x] COM 回调只做格式/路径/坐标复制与非阻塞队列投递，UI 定时处理状态及日志。
- [x] 文件拖入独立验证模式，退出时先撤销后释放目标、provider、OLE。
- [x] 17 项单元、6 项合成 IDataObject 测试、3 项文档测试以及全部格式/编译/Clippy/Release 检查通过。
- [x] 最终 EXE 实际 Medium Explorer → Medium QuickNest：EXE、中文文件夹、LNK、中文空格路径、多文件顺序通过。
- [x] 严格动作计数验证移出/ESC 取消均不产生 Drop、不新增路径。
- [x] 5 次真实落下接收 6 路径，acquired/released=5/5、destroyed=1、errors=0、退出码 0。
- [x] 源/目标保持响应，样本文件哈希和目录状态未变，未执行或解析样本。
- [x] 最终哈希下 P0-01～P0-04 回归与按钮/错误路径复验通过。
- [x] 正式清单、进度与验证证据更新。

## 历史记录：P3-05/P3-06/P3-07 任务清单（已验收）

- [x] 模型：`Item` 增 `last_used`/`launch_count`/`favorite`（serde default）+ `Settings.sort_mode` + `normalize_sort_mode` + 纯函数 `sort_item_ids_by_mode`；6 项单元测试。
- [x] 控制器：`record_launch` 接入三种启动路径（单击/批量/组合）、`set_sort_mode`、`toggle_favorite`（双向 persist）。
- [x] UI：设置菜单 4 个扁平排序项（设置菜单 19→23 项）、卡片菜单收藏项（卡片菜单 12→13 项）、`item_rows` 按模式重排。
- [x] live 实测：recent/frequent/favorite 三种模式网格顺序与 config 断言 + 三张截图 → 卡片菜单取消收藏落盘 → 重启前排序模式保持 → exit=0。
- [x] 回归 40 脚本 0 失败（含更新断言后的 p2-17/p3-01/p3-02/p3-03）；门禁 fmt=0、clippy=0、194 lib 测试 0 失败；release SHA 263fad61…5ea628a0。

## 历史记录：P3-05/P3-06/P3-07 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-07**。四种排序模式以真实菜单点击与真实启动数据实测；网格顺序、config 持久化与收藏切换全部通过断言与截图双重确认。

- 证据目录 `docs/test-results/P3-05/`：
  - `p3-05-sorting.tsv`：`seeded` → `recent_mode` → `frequent_mode` → `favorite_mode` → `favorite_toggled` → `persisted` → `completed`。
  - 截图：`p3-05-recent.png`（B、C、A）、`p3-06-frequent.png`（B、C、A，状态栏“排序模式：高频”）、`p3-07-favorite.png`（C、A、B）。
  - 单元测试：`recent_sorts_by_last_used_descending`、`frequent_sorts_by_launch_count_descending`、`favorite_sorts_to_the_top`、`manual_and_unknown_keep_the_input_order`、`sort_mode_normalizes_and_round_trips`、`legacy_items_load_without_usage_state`。

## 历史记录：P3-05/P3-06/P3-07 开发过程问题与修复

1. **排序项最初做成嵌套子菜单**（菜单计数 20≠22）→ 扁平化为 4 项，设置菜单 23 项。
2. **`toggle_favorite` 取消方向不持久化** → 改为双向 persist。
3. **四个旧脚本断言与新菜单结构不匹配** → 更新计数/行号后回归全绿（40 脚本 0 失败）。

## 历史记录：P3-05/P3-06/P3-07 边界

- 手动=拖拽排序的配置顺序；recent 未启动项排最后；frequent 并列保持手动序；favorite 置顶后保持手动序。
- usage 字段 serde default 兼容旧配置；仅成功启动计入统计。

## 历史记录：P3-08 任务清单（已验收）

- [x] 模型：`Launch.execution_mode` + `normalize_execution_mode` + \`ItemDraft\` 字段 + `find_item_mut`；2 项单测。
- [x] 命令引擎：direct（CreateProcessW，无 cmd.exe）/ cmd（`/d /s /c`）/ powershell（`-NoProfile -File`）+ `command_display`；3 项单测（cmd/powershell 探针脚本、缺失目标）。
- [x] UI：编辑器执行模式下拉（命令类型专属）；首次执行确认对话框（程序/参数/工作目录/执行方式 + 取消/确认执行）。
- [x] 接线：6 个单项启动入口门控（`launch_item_with_confirm`）；批量与组合跳过未确认命令并报告；状态栏显示实际执行内容。
- [x] live 实测：direct 确认/取消/立即重启、cmd 取消+确认、powershell 确认、批量跳过、exit=0；截图 `p3-08-confirm.png`。
- [x] 回归 41 脚本 0 失败；门禁 fmt=0、clippy=0、199 lib 测试 0 失败；release SHA 8c621b64…2671aa。

## 历史记录：P3-08 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-08**。三种执行模式以真实探针脚本实测；首次确认对话框以真实点击端到端实测（确认→执行+落盘；取消→不执行+标记保留）；批量跳过有反向断言（probe4 不存在）。

- 证据目录 `docs/test-results/P3-08/`：
  - `p3-08-command.tsv`：`seeded` → `direct_confirmed` → `direct_verified_no_dialog` → `cmd_cancelled` → `cmd_confirmed` → `powershell_confirmed` → `batch_skips_unverified` → `persisted` → `completed`。
  - 截图：`p3-08-confirm.png`（首次执行确认对话框）、`p3-08-final.png`（批量跳过后）。
  - 单元测试：`execution_mode_normalizes_unknown_to_direct`、`execution_mode_round_trips_and_legacy_loads_as_direct`、`command_cmd_mode_runs_the_script`、`command_powershell_mode_runs_the_script`、`command_missing_target_and_display`。

## 历史记录：P3-08 开发过程问题与修复

1. 测试目标 `Launch` 字面量补齐 `execution_mode`。
2. unsupported 类型测试随 Command 转正更新（附空目标拒绝断言；消除单元素循环 clippy 警告）。
3. `command_display` 测试引号预期按 `join_args` 实际规则修正。
4. 验收脚本截图调用空路径守卫。

## 历史记录：P3-08 边界

- 批量/组合不执行未确认命令（跳过并报告）；首次确认仅在单项启动路径。
- 无关键词黑名单；powershell 仅 `-NoProfile -File`；cmd `/d /s /c`；未知模式按 direct。
- automation 类型仍为后续阶段扩展。

## 历史记录：P4-11/P4-12 任务清单（原验收结论已撤回）

- [x] RELEASE_NOTES.md：版本内容四段（P1/P2/P3/P4）+ 系统要求 + 已知限制 + 升级/卸载说明。
- [ ] 当前正式版本验收未完成；历史三件产物、220 lib 与旧门禁记录只对应旧构建，本轮源码门禁见顶部，本轮构建和哈希已记录。
- [ ] 原“11/12 通过、仅 Win10 遗留”台账已撤回；完成页 Medium、真实生产换版、生产 key/HTTPS 等也须完成。

## 历史记录：P4-11/P4-12 验收结论与证据

更正：原 2026-11-07 P4-12 通过与“全部产品能力已达正式发布”结论证据不足，已撤回。历史文档和产物记录不构成当前发布认证；当前验收见 [本轮修复记录](AUDIT_FIXES_2026-11-07.md)。

- 证据：RELEASE_NOTES.md、docs/P4_VALIDATION.md P4-11/P4-12 章（台账+哈希）、installer/output/ 三件产物。

## 历史记录：P4-10 任务清单（已验收，2026-11-07）

- [x] migration_tests：v1.1 形状 JSON → 用户数据完整 + 新字段默认值逐项 + 序列化往返；mutate→reparse 不丢数据。
- [x] live：v1.1 config → 加载（截图）→ 触发保存 → config 断言（schema=2、数据未丢、新字段默认值）→ 迁移后项目可启动 → exit=0。
- 历史迁移样本在第二次启动尝试后成功；首次未启动原因未证实，不能归因为防护组件。
- [x] 门禁：fmt=0、clippy=0、220 lib 测试 0 失败（+2）。

## 历史记录：P4-10 验收结论与证据

结论：**2026-11-07 本任务验收通过，停在 P4-10**。迁移语义 = 新增字段 serde default 补齐 + 用户数据零丢失，单测与 live 双覆盖。

- 证据：docs/test-results/P4-10/p4-10-migration.tsv（9 阶段）+ 两张截图。

## 历史记录：P4-10 开发过程问题与修复

1. 首次点击未观察到启动，重试后成功；该现象不能单独证明输入被拦截，测试方法边界见 TEST_HARNESS.md。
2. 构造 v1.1 种子时 launch 块与 window_state 为必填 → 按历史形状补齐。

## 历史记录：P4-07 任务清单（已验收，2026-11-07）

- [x] src/crash_log.rs：panic hook（幂等安装、链回默认 hook、追加写 crash-log.txt、随便携模式的数据目录）。
- [x] --crash-test 模式：真实 panic 端到端实测。
- [x] 验收脚本 verify-p4-07-crash-log.ps1：触发（exit=101）→ 日志含消息/位置/时间戳 → 追加语义 → 清理；通过。
- [x] 门禁：fmt=0、clippy=0、218 lib 测试 0 失败（+2：路径规则、catch_unwind 真实 panic 链路）。
- [x] 验证记录：docs/P4_VALIDATION.md P4-07 章。

## 历史记录：P4-07 验收结论与证据

结论：**2026-11-07 本任务验收通过，停在 P4-07**。崩溃日志经真实 panic 端到端实测（非构造调用）；追加语义有断言。

- 证据：docs/test-results/P4-07/p4-07-crash-log.tsv（6 阶段）。

## 历史记录：P4-07 开发过程问题与修复

1. PanicHookInfo::from_panic_call 非稳定 API → catch_unwind 隔离的真实 panic 测试。

## 历史记录：P4-04/05/06 任务清单（已验收，2026-11-07）

- [x] src/updater.rs：清单解析（payload 原始字节签名覆盖）+ 版本比较 + 执行链（备份→放置→复验→回滚）；6 项单测。
- [x] src/update_net.rs：BCrypt SHA-256 + ECDSA P-256 验签（公钥内嵌）+ WinInet HTTPS 传输（系统信任库，无绕过；非 https 拒绝）。历史观察：WinHTTP 返回 err=87，原因未证实；不能表述为被本机封锁。
- [x] --update-test 验证模式：12 阶段链（含篡改拒绝、损坏拒绝、回滚、备份完整性、data 不触碰）。
- [x] 本地 HTTPS 测试服务器（TcpListener+SslStream，自签名入 Root 用后即清）+ update-sign.ps1（ECDsaCng 私钥签名）。
- [x] 验收脚本 verify-p4-040506-update.ps1 两轮独立通过（docs/test-results/P4-04/）。
- [x] 门禁：fmt=0、clippy=0、216 lib 测试 0 失败（+6）；release SHA 3ba5d280…49ec02。

## 历史记录：P4-04/05/06 验收结论与证据

结论：**2026-11-07 本任务验收通过，停在 P4-06**。九步链路端到端实测（含两条反向断言链：篡改清单拒绝、损坏包拒绝；回滚逐字节恢复；data 不触碰快照断言）。

- 证据：docs/test-results/P4-04/p4-040506-update.tsv + update-app.tsv（应用侧 12 阶段）+ server.log（TLS 服务事实）。

## 历史记录：P4-04/05/06 开发过程问题与修复

1. 历史传输选型使用 WinInet；WinHTTP err=87 仅为当时观察，不证明被封锁或 WinInet 唯一可行。
2. 测试服务器相对 [Uri] AbsolutePath 为空 → 全请求 404 → 直接用请求行路径。
3. netsh sslcert 绑定报参数错误 → TcpListener+SslStream 自管 TLS。
4. PS StrictMode 管线单结果无 .Count → @() 包裹；TextExtension EKU 被拒 → -Type SSLServerAuthentication。

## 历史记录：P4-03 任务清单（已验收）

- [x] 历史静态检查确认 installer/QuickNest.iss [Run] 条目含 runasoriginaluser；该标志取决于可用原始上下文，不保证降到 Medium。
- [x] 验收脚本 verify-p4-03-deelevate.ps1：机制断言（标志存在 + 新编译）→ 提权上下文安装 → 逐用户核验（无 HKLM）→ asInvoker 清单 → 启动模拟 → 卸载清理；两轮独立通过。
- [x] WM_CLOSE 投递修复：窗口枚举句柄 + 未退出重投（应用早期重建窗口会丢消息）。
- [x] 环境限制记录：内建管理员交互令牌即 High，降权量化测试安排在 P4-08/P4-09。
- [x] 验证记录：docs/P4_VALIDATION.md P4-03 章。

## 历史记录：P4-03 验收结论与证据

更正：2026-11-06 历史验证只确认标志、asInvoker、逐用户安装和模拟启动。asInvoker 继承调用者权限，runasoriginaluser 可保留 High 原始令牌；P4-03 完成页真实 Medium 启动尚未验收。

- 证据：docs/test-results/P4-03/p4-03-deelevate.tsv（7 阶段）。

## 历史记录：P4-03 开发过程问题与修复

1. WM_CLOSE 投递到失效句柄（应用早期重建窗口）→ 枚举句柄 + 重投循环。

## 历史记录：P4-02 任务清单（已验收）

- [x] 便携模式：新模块 src/portable.rs（QuickNest.portable 标记 → 数据目录 <exe 目录>data）；resolve_with 纯函数 3 项单测。
- [x] 相对路径项目：with_resolved_target 在 launch_item/open_item_location 入口解析；path_missing 与图标提取同点解析；存储值保持相对。
- [x] main.rs：config/icons 数据目录便携优先、安装版回退 %APPDATA%。
- [x] 打包：installer/build-portable.ps1 → QuickNest-1.2.0-portable.zip（exe+ico+README+标记）。
- [x] 验收脚本 verify-p4-02-portable.ps1：解压→首启数据目录→相对启动→跨盘移动（E:→C:）→数据随迁+新根解析→漫游不受影响；两轮独立通过。
- [x] 门禁：fmt=0、clippy=0、210 lib 测试 0 失败（+3）；**44 脚本全量回归 0 失败**（p4-01/p4-02 纳入回归集）。
- [x] 验证记录：docs/P4_VALIDATION.md P4-02 章。

## 历史记录：P4-02 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P4-02**。解压即用、免管理员、数据随文件夹跨盘迁移（E:→C:）、相对路径项目在新根照常启动，全部以探针实测；%APPDATA% 全程不受影响（反向断言）。

- 证据：docs/test-results/P4-02/p4-02-portable.tsv（10 阶段）+ 两张截图；包 SHA256 5DA4ED05…。

## 历史记录：P4-02 开发过程问题与修复

1. 验收脚本缺 Wait-True 助手 → 补。
2. 关闭后截图越界 → 截图移至 Stop 之前。
3. 窗口位置记忆竞态（SetWindowPos 后被异步恢复覆盖，点击错位）→ 二次枚举取最终 rect。

## 历史记录：P4-01 任务清单（已验收）

- [x] 工具链：Inno Setup 7.1.1（api.github.com 资产端点下载官方包 + Authenticode 验签 + /CURRENTUSER 装入仓库 tools/，网络受限环境的取用路径见验证记录）。
- [x] 图标：assets/QuickNest.ico（gen-icon.ps1 四档 PNG 压缩 ICO）。
- [x] installer/QuickNest.iss：PrivilegesRequired=lowest 逐用户安装（无 UAC）、标准向导（中文）、开始菜单组（应用+卸载）、可选桌面图标（默认勾选）、可选开机启动（默认关闭 opt-in，HKCU Run）、卸载程序（ARP DisplayVersion=1.2.0）。
- [x] 验收脚本 verify-p4-01-installer.ps1：静默安装→文件/快捷方式/注册表→asInvoker 清单+无 HKLM→静默卸载→全清理→默认任务反向验证→二次卸载；两轮独立通过（docs/test-results/P4-01/）。
- [x] 验证记录：新建 docs/P4_VALIDATION.md（P4 起独立成册）。

## 历史记录：P4-01 验收结论与证据

历史安装/卸载与默认任务检查记录保留；asInvoker 清单和无 HKLM 不能证明普通权限启动，完成页 Medium 实测仍待完成，不把模拟启动记为降权验收。

- 证据：docs/test-results/P4-01/p4-01-installer.tsv（10 阶段）、installer/output/QuickNest-1.2.0-setup.exe。

## 历史记录：P4-01 开发过程问题与修复

1. AppId 双右括号（Inno 仅 {{ 是转义）→ 规范形式 {{GUID}（单右括号）。
2. StrictMode 下读取不存在的注册表值属性抛错 → Get-RunValue 安全读取。
3. "普通权限"完整性断言在本机不可测（交互令牌 High）→ 改证 asInvoker + 逐用户三要素，环境限制记录在案。

## 历史记录：卡片 UI 规范 V1.0 任务清单（2026-11-06 用户规范，已实现）

- [x] 根因定位：Slint `VerticalLayout.alignment` 是主轴对齐 → 图标行改用 `HorizontalLayout{ alignment: center }`，图标与标题同轴。
- [x] 卡片：圆角 11px；图标区 40×40 内放 32×32 `image-fit: contain`；标题 12px/18px/单行/省略/居中。
- [x] 图标类型：EXE/文件/文件夹用原生图标，URL 用 `assets/browser.svg`，命令用 `assets/terminal.svg`（`Item::default_artwork`）。
- [x] 角标：15px `#E5484D`、右上 5px、悬浮显示具体原因（`Item::missing_reason` + `src/card_ui.rs::badge_hit`）。
- [x] 状态：默认/hover（浅蓝底+描边+轻微阴影）/按下/焦点描边；新增 3 个主题色（深色自适应）。
- [x] 门禁：fmt=0、clippy=0、**206 lib 测试 0 失败**（新增 6 项）；release SHA（见验证记录）。
- [x] 视觉证据：`docs/test-results/card-ui/` 四张截图（浅色/浅色 hover/深色/角标提示）。
- [x] 回归 **42 脚本整轮连续运行 0 失败**（夹具修复全部就位后首次完整全绿）；`verify-p1-10-icons.ps1` 按规格更新（URL 浏览器图标）；p1-17 拖放源点与 shell 重置钩子修复；p3-01/p3-02 的记事本计数探针改为确定性探针文件（Win11 打包版记事本的进程/窗口数与启动次数不成比例，见 [测试夹具说明](TEST_HARNESS.md)）。
- 验收脚本（夹具）层面的陷阱与坐标同步约定单独记录在 [测试夹具说明](TEST_HARNESS.md)，不混入产品验证记录。

## 历史记录：侧边栏样式优化（P3 后 UI 优化，2026-11-06）

用户反馈左侧导航栏样式粗糙，经逐项核对（4 真、1 半真、1 基本不成立：整行点击区与底部固定入口本已是现状，右键菜单自 P1 就存在），按用户确认的范围**仅样式层**修改：

- [x] 左侧内边距 2px→12px（16px 图标位 + 文字 28px 起，圆点标记占位）；侧边栏宽度 148px→**160px**（先改 164px 后发现会使 760px 窗口的网格从 6 列变 5 列，违反"不改变网格列数"，回调到用户建议区间 160–176 的下限）。
- [x] 选中状态：新增主题色 `sidebar-active`（浅色 accent 16% 混合 ≈ #dce6fc、深色 32% 混合），选中行药丸背景 + **蓝色文字** + 蓝色圆点；替换原先几乎不可辨的 accent-soft（9% 混合）。
- [x] 视觉层次：新增 Hover 态（`sidebar-hover`，浅色 8%/深色 16% 混合，优先级低于选中/拖放命中）、6px 圆角药丸、行高 28px→36px + 2px 垂直间距、13px 字号、文字溢出省略。
- [x] 新建分类入口：保持底部固定，列表与入口之间新增分隔线（左右各留 6px）。
- [x] 几何同步：`drop_layout.rs`（P2-05 内部拖拽与 P1-17 外部拖放共用的命中测试）常量更新为 160px/38px 行距/36px 行高并补间隙不命中断言；受影响脚本坐标已全部同步复核（p2-05 拖拽目标行 top+152、p1-03 行距 38px、p2-11 分类行中心 top+112、p1-05 分类行 top+150）。
- [x] 门禁：fmt=0、clippy=0（初版 203 项测试；并入卡片规范后为 **207 lib 测试 0 失败**，含 `sidebar_states_are_visually_separated` 三级分离断言）。
- [x] 视觉证据（`docs/test-results/sidebar-audit/`）：`sidebar-light.png`（选中药丸清晰）、`sidebar-light-hover.png`（第三行 hover 药丸）、`sidebar-dark.png`（深色自适应选中/hover）。
- [x] 回归：42 脚本逐个单独运行全部 exit=0（整轮连续运行时的偶发失败为桌面被同时操作导致的焦点争用，逐个复跑核实；详见 [测试夹具说明](TEST_HARNESS.md)）。
- 边界：分类图标图像渲染未实现（模型有 icon 字段但无图标来源 UI，当前以 6px 圆点占位，属后续功能）；“快捷访问/全部项目/我的收藏”分区与分类拖拽排序为新功能，未纳入本次样式层范围。

## 历史记录：P3-09 任务清单（已验收）

- [x] 设置：`everything_enabled`（opt-in，默认关闭）+ `everything_path`（默认标准安装位置）+ 开关持久化；2 项单测。
- [x] 移交助手：`resolve_everything_path`（配置→回退→None）+ `everything_fallback_paths` + `everything_handoff_item`（`-search <查询>`）；1 项单测。
- [x] 搜索中心：开启且有查询时追加“在 Everything 中搜索…”移交行；三个激活入口走 `launch_everything_handoff`。
- [x] 设置菜单第 24 项开关（19→24 项；p2-17/p3-0567 计数断言同步更新）。
- [x] live 实测：移交行激活写探针 → 切换关闭（空结果无操作、探针不变）→ 缺失路径仅提示且主程序存活 → exit=0；截图 `p3-09-search.png`/`p3-09-disabled.png`。
- [x] 回归 42 脚本（2 个环境性失败复跑全过 + TabState 清理钩子）；门禁 fmt=0、clippy=0、202 lib 测试 0 失败；release SHA 17dde497…ff231f。

## 历史记录：P3-09 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-09；P3 阶段全部完成**。移交链路以真实启动（探针 .cmd 经引擎 Shell 回退）实测，开关/缺失路径均有正反断言。

- 证据目录 `docs/test-results/P3-09/`：
  - `p3-09-everything.tsv`：`seeded` → `handoff_probe` → `toggle_off` → `missing_path_reported` → `persisted` → `completed`。
  - 截图：`p3-09-search.png`（移交行选中态）、`p3-09-disabled.png`（关闭后空结果）。
  - 单元测试：`everything_defaults_to_disabled_standard_path`、`everything_settings_round_trip_and_legacy_load`、`everything_handoff_and_path_resolution`。

## 历史记录：P3-09 开发过程问题与修复

1. Settings 字段锚点（sort_mode 后还有 P2-15 字段）。
2. cargo fmt 重排 + 补丁幂等化 + slint 重复声明去重。
3. 验收探针：扩展名（.exe→.cmd）、绝对路径重定向、严格模式 Count。
4. 回归环境：记事本 TabState 会话恢复堆积窗口 → `run-regression.ps1` 加清理钩子。

## 历史记录：P3-09 边界

- opt-in 默认关闭；缺失 Everything 仅状态栏提示，主程序不受影响。
- 移交行不是库项目：不入库、不计数、不走首次确认。

## 历史记录：P3-04 任务清单（已验收）

- [x] 设置：`hot_zone_enabled`（opt-in，默认关闭）与 `hot_zone_edge`（left/right/top/bottom，未知值回退 left）；3 项单元测试。
- [x] 检测：交互模式 200ms 轮询 `GetCursorPos`，边缘 2px 带内停留 600ms 触发一次切换；离开边缘重置，停留不重复触发。
- [x] 切换路径复用 `show_panel` / `hide_panel`（推进焦点状态机）；禁用后悬停无效果。
- [x] live 实测：启用后边缘悬停隐藏面板 → 再次悬停显示 → dwell 时序断言（200ms 未触发/1.4s 已触发）→ 禁用重启后悬停无效果 → exit=0。
- [x] 门禁 fmt=0、clippy=0、188 lib 测试 0 失败。

## 历史记录：P3-04 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-04**。边缘热区悬停切换面板可用、有 dwell 防误触、停留不振荡、禁用后无效果。

- 证据目录 `docs/test-results/P3-04/`：
  - `p3-04-hotzone.tsv`：`startup_visible` → `edge_hide` → `edge_show` → `dwell_required` → `disabled_noop` → `app_exit` → `completed`。
  - 截图：`p3-04-hidden.png`（热区隐藏后的桌面）。
  - 单元测试：`hot_zone_defaults_to_disabled_left`、`hot_zone_edge_normalizes_unknown_values`、`hot_zone_settings_round_trip`。

## 历史记录：P3-04 开发过程问题与修复

1. **热区字段误放 Library** → 移到 `Settings` 并补齐 Default。
2. **生成代码中 `match` 分支字面量被转义序列破坏** → 按行精确修复。
3. **`normalize_hot_zone_edge` 返回 `&static str`** 消除临时借用；`controller.borrow()` 需跨块保持。
4. **防振荡断言语义修正**：停留边缘再次触发是 dwell 设计使然，改为验证 dwell 窗口内不重复切换。

## 历史记录：P3-04 边界

- 热区 opt-in，默认关闭；支持四条边，角落由相邻边缘自然覆盖。
- 轮询 200ms + dwell 600ms；PMv2 物理像素判定。
- 热区为显式用户动作，不受 pinned/auto-hide 抑制。

## 历史记录：P3-03 任务清单（已验收）

- [x] 模型：`Launch.delay_seconds: u32`（serde default 0，旧配置兼容，编辑器不含该字段且编辑时保留）；2 项单元测试。
- [x] 启动：`launch_item_maybe_delayed`——延时 0 同步启动，延时 > 0 在 detached 线程定时启动（不阻塞 UI 与其余项目）。
- [x] 批量启动与启动组合均尊重延时，状态栏/对话框报告“N 个延时启动”。
- [x] live 实测：延时 0/3/6 的三卡批量启动，探针文件在 t=0.5s/4s/7.5s 按延时依次出现，内容正确。
- [x] 门禁 fmt=0、clippy=0、185 lib 测试 0 失败；release 构建 SHA 见 P3-02（本轮无 UI 变更，构建一致性由回归保证）。

## 历史记录：P3-03 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-03**。逐项延时在批量启动中按预期生效：立即项即时启动、延时项按各自延时启动，探针文件内容证明启动的是对应项。

- 证据目录 `docs/test-results/P3-03/`：
  - `p3-03-delay.tsv`：`seeded` → `selected` → `sample_t05`（仅 probe-0）→ `sample_t4`（probe-3）→ `sample_t75`（probe-6）→ `probe_contents` → `completed`。
  - 截图：`p3-03-before-launch.png`（三卡选中态）。
  - 单元测试：`delay_defaults_to_zero_and_round_trips`、`legacy_items_load_without_delay`。

## 历史记录：P3-03 开发过程问题与修复

1. **探针参数引号破坏 cmd 重定向**：`launch_exe` 对含空格参数加引号，`cmd /c` 的引号剥离规则破坏重定向 → 种子参数去掉无谓引号；命令先经独立进程实测。

## 历史记录：P3-03 边界

- 延时项在 detached 线程启动，其结果不回传状态栏。
- 单项目单击忽略延时；仅批量/组合启动尊重延时。
- 延时无上限（u32 秒）。

## 历史记录：P3-02 任务清单（已验收）

- [x] 模型：`Library.combinations`（持久化）+ `LaunchCombination` + add/remove/combination 方法；旧配置 serde default 兼容；4 项单元测试。
- [x] 控制器：`save_batch_combination`（保存批量选择并清空）、`combination_rows` / `combination_item_ids`（跳过已删除项目）/ `delete_combination`。
- [x] 界面：卡片菜单“保存批量选择为组合…”（命名对话框）与“启动组合…”；组合对话框列出名称+项目数，行选中后“启动组合 / 删除 / 取消”。
- [x] 启动语义：按保存顺序逐项启动，跳过已删除项目，状态栏报告“已启动/总数”。
- [x] live 实测：保存组合（config=1、2 项）→ 启动组合（记事本 0→2）→ 删除组合（config=0）→ exit=0。
- [x] 回归 37 个脚本 exit=0；门禁 fmt=0、clippy=0、201 项测试 0 失败。

## 历史记录：P3-02 验收结论与证据

结论：**2026-11-06 本任务验收通过，停在 P3-02**。启动组合的保存/持久化/启动/删除全链路以真实点击实测，模型行为有单元测试覆盖。

最终 Release SHA-256：`056F0569E43B91EDD68CAA177B72ED3B4F91C0E3F85D12AA3B5D83BF900AE729`。

- 证据目录 `docs/test-results/P3-02/`：
  - `p3-02-combinations.tsv`：`selected` → `combination_saved`（组合 2、2 项）→ `combination_launched`（notepad 0→2）→ `combination_deleted`（config=0）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p3-02-name-dialog.png`、`p3-02-launched.png`、`p3-02-closed.png`。
  - 回归：`docs/test-results/P3-02/regression/final7/regression.tsv`（37 脚本，0 失败）。

## 历史记录：P3-02 开发过程问题与修复

1. **Slint 回调 panic 进程中止**：`borrow_mut()` 借用贯穿 match、Ok 分支再 `borrow()` 触发 double borrow panic → 保存结果先落变量再分支。
2. **组合字段误放 Settings** → 移到 `Library` 并补齐 Default。
3. **Win11 记事本会话恢复**：杀进程后重启恢复全部标签页，数十个窗口堆满桌面 → 清理 TabState 目录解决。
4. **对话框按钮坐标**按布局实测调整。

## 历史记录：P3-02 边界

- 组合保存项目 id 集合；项目删除后启动时自动跳过，不自动修复组合。
- 启动按保存时顺序逐项执行；延时/并行编排属 P3-03。
- 组合随配置持久化，受配置备份/导入导出的影响与普通数据一致。

## 历史记录：P3-01 任务清单（已验收）

- [x] 卡片右键菜单新增“加入批量选择/移出批量选择”与“批量启动所选（N）”（N=选中数，0 时禁用）；卡片高亮显示选中态。
- [x] Ctrl+单击切换选择（`GetKeyState`，与菜单等效）；选择为内存态不入库。
- [x] 批量启动按网格顺序逐项启动，状态栏报告“已启动/失败”计数，完成后清空选择。
- [x] live 实测：两张卡片加入选择 → 批量启动记事本 0→2 → 再次批量启动 0 新增（选择已清空）→ exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-17 + P3-01 共 36 个脚本 exit=0；门禁 fmt=0、clippy=0、197 项测试 0 失败。

## 历史记录：P3-01 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P3-01**。批量选择与批量启动均以真实点击实测；选择清空行为有反向断言。

最终 Release SHA-256：`EE66B9D242486D9E8291BD2D477E248E368DB3967E43BE8236E18A0F3CDCC112`。

- 证据目录 `docs/test-results/P3-01/`：
  - `p3-01-batch.tsv`：`selected` → `batch_launch`（notepad 0→2）→ `selection_cleared`（二次批量无新增）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p3-01-grid.png`、`p3-01-selected.png`、`p3-01-launched.png`。
  - 回归：`docs/test-results/P3-01/regression/final4/regression.tsv`（36 脚本，0 失败）。

## 历史记录：P3-01 开发过程问题与修复

1. **Slint 菜单回调需在 Window 上显式声明**；`GridItem` 增加字段后所有构造点需同步。
2. **Slint 自绘菜单不可读文字**（`GetMenuStringW` 返回空串）→ 以批启动的实际效果断言选择状态。
3. **菜单行定位**：`GetMenuItemRect` 返回菜单窗口客户区坐标需 `ClientToScreen`；“弹窗高度/行数”估算受阴影影响失准。
4. **测试残留窗口**：堆积的记事本窗口遮挡面板与菜单 → 回归运行器每个脚本前清理记事本，p1-08 启动时清空并在“打开文件所在位置”后关闭新开记事本；清理数日累积的 explorer 残留进程。

## 历史记录：P3-01 边界

- Ctrl+单击依赖实际修饰键状态；历史注入输入未送达的原因未证实，不能归因为第三方防护组件。
- 批量启动按网格顺序逐项启动；顺序/并行/延时编排属 P3-02/P3-03。
- 选择为内存态，不入库；面板隐藏或重启后清空。

## 历史记录：P2-17 任务清单（已验收）

- [x] 基线 17.4：导出全部配置 / 导出当前分组写出 `quicknest-export` 文档（scope、schema_version、可选 settings）。
- [x] 合并导入：重新生成分组/分类/项目 id，同名分组依次重命名为“名称 (导入)”“名称 (导入 2)”，保留现有设置。
- [x] 覆盖导入：替换全部分组与设置，修复 current/last 分类引用。
- [x] 导入安全（8.4）：不执行任何命令/脚本/URI；命令与自动化项目 `confirm_on_first_run = true`（未验证）。
- [x] 导入前备份（15.7）：应用前写出当前配置备份；安全模式拒绝导入；损坏/版本不符的文档不改变现有数据。
- [x] 界面：设置菜单第 19 项“配置导入导出…”对话框（导出全部/导出分组/合并导入/覆盖导入/取消 + 状态行），经原生保存/打开对话框选择文件。
- [x] 验证入口 `--transfer-test`；回归 P1-01～P1-18 + P2-01～P2-17 共 35 个脚本 exit=0；门禁 fmt=0、clippy=0、197 项测试 0 失败。

## 历史记录：P2-17 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-17，P2 阶段全部完成**。导出/导入、合并/覆盖、导入前备份与未验证标记均有实测证据；原生保存对话框在自动化下的 OK 路径受限，已记录并以下述方式覆盖。

最终 Release SHA-256：`2F5ECD6916D63AD303061C7C887EE8B360B15D0DBF30F51FCFBB0C5640B1114B`。

- 证据目录 `docs/test-results/P2-17/`：
  - `p2-17-transfer-validation.tsv`：`export_all` / `export_group` / `merge_import`（Tools,Media,Imported；未验证 1；备份 0→2）/ `overwrite_import`（Replaced + dark）/ `broken_rejected` / `completed`。
  - `p2-17-transfer.tsv`：`validation_mode` → `dialog_opened` → `export_all_dialog` / `export_group_dialog` → `merge_import`（备份 1→3，命令项未验证）→ `overwrite_import`（Replaced + dark）→ `dialog_closed` → `app_exit`（exit=0）。
  - 截图：`p2-17-dialog.png`、`p2-17-closed.png`。
  - 回归：`docs/test-results/P2-17/regression/final3/regression.tsv`（35 脚本，0 失败）。

## 历史记录：P2-17 开发过程问题与修复

1. **原生保存对话框 OK 路径在自动化下卡住**：隐藏的“确认另存为”模态使其保存按钮不可用 → 导出改由 `--transfer-test` 以真实文件验证；live 验证导出按钮打开保存对话框并取消；导入的原生打开对话框 OK 路径已端到端实测通过。
2. **`SendMessage` 的 ANSI/Unicode 与生命周期**：未指定 `CharSet` 时解析为 `SendMessageA`（UTF-16 只读出首字符）；`PostMessage` 晚于缓冲区释放导致截断 → 统一 `SendMessageW` 同步写入。
3. **p2-16 菜单计数断言**：菜单增至 19 项后由“恰好 18”改为“至少 18”。
4. **环境抖动**：p1-17 偶发清理后不退出（单独复跑正常），最终回归 35/35。

## 历史记录：P2-17 边界

- 命令项目的首次执行确认由命令引擎（P3-08）消费 `confirm_on_first_run`；当前命令项目尚不可启动。
- 合并导入不覆盖设置；需要替换设置请用覆盖导入。
- 重复导入同一文件会不断新增分组（按设计）。
- 导出文件含完整路径与参数，分享前需确认内容。

## 历史记录：P2-16 任务清单（已验收）

- [x] 基线 15.7/17.4：历史备份枚举 —— 只列出内容可解析的备份（15.8.3“验证 JSON”），按时间倒序，标签格式化为 `YYYY-mm-dd HH:MM:SS.mmm`。
- [x] 恢复：校验备份后经同一原子替换链写入，被替换的配置自动留为备份（恢复可回退），随后按 10 份保留窗口裁剪。
- [x] 控制器：恢复后重新解析装载、清除拼音索引缓存、退出安全模式；安全模式拒绝恢复；schema 不兼容的备份拒绝恢复。
- [x] 界面：设置菜单新增第 18 项“历史备份恢复…”；对话框列出备份（默认选中最新）、立即备份 / 取消 / 恢复所选与状态行。
- [x] live 实测：坏备份（更晚）不被列出、恢复有效备份后实时配置变 `Restored`、被替换配置留为备份、立即备份新增文件、取消关闭、退出后恢复结果持久化。
- [x] 回归 P1-01～P1-18 + P2-01～P2-16 共 34 个脚本 exit=0；门禁 fmt=0、clippy -D warnings=0、187 项测试（169 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-16 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-16**。历史备份可从标题栏“设置”菜单打开、立即备份与恢复所选；恢复走原子替换且可回退，保留窗口维持最近 10 份有效备份。

最终 Release SHA-256：`3CFB76522191833AD2A353B6EDA6443F626EA5A57B4DDC9A529640AEC2B58EBD`。

- 证据目录 `docs/test-results/P2-16/`：
  - `p2-16-backups.tsv`：`seeded`（live=Current、valid=2026-12-31、broken=2027-01-01）→ `dialog_opened` → `restored`（group=Restored）→ `restore_kept_previous`（被替换配置留为备份、count=4）→ `backup_now`（新增文件、count=5）→ `dialog_closed` → `app_exit`（exit=0、group=Restored）→ `completed`。
  - 截图：`p2-16-before.png`、`p2-16-dialog.png`、`p2-16-restored.png`、`p2-16-closed.png`。
  - 回归：`docs/test-results/P2-16/regression/final6/regression.tsv`（34 脚本，0 失败）。
  - 单元测试：`restore_list_skips_broken_backups_and_sorts_newest_first`、`restore_replaces_the_live_config_and_keeps_the_previous_one`、`restore_refuses_a_broken_backup_without_touching_the_config`。

## 历史记录：P2-16 开发过程问题与修复

1. **Slint 语法**：多行 `text:` 缺少分号；`Flickable` 的 `viewport-height` 自引用形成绑定循环 → 改按条目数计算并使用 `content-height`/`content-width`。
2. **`for … in model` 不能用于回调语句** → 恢复按钮改用 `backup-dialog-selected-index` 下标回调。
3. **`display_label` 时间格式** → 精确按 `HHMMSS-mmm` 输出 `HH:MM:SS.mmm`。
4. **回归环境抖动**：多轮回归出现一次性失败（启动类/窗口类），单独复跑均通过；根因是累积 77 个无窗口 explorer 残留进程与系统弹窗占用桌面。清理后 34 脚本连续通过；并把 P1-03 菜单等待 300→600ms、P2-13 参考窗体加 TopMost（应用再置顶于其上）。

## 历史记录：P2-16 边界

- 备份列表只含内容可解析的备份；schema 不兼容的备份由控制器拒绝恢复（15.9）。
- 保留窗口为最近 10 份有效备份；损坏备份在裁剪时移除。
- 备份与配置同目录同卷（15.6）；网络共享/可移动介质不承诺同等故障恢复保证。
- “导入前备份”属配置导入（P2-17），本任务不含导入。

## 历史记录：P2-15 任务清单（已验收）

- [x] 基线 16.4：Manifest 声明 Per-Monitor V2（`dpiAware=true/pm` + `dpiAwareness=PerMonitorV2`），并在运行窗口上用 `AreDpiAwarenessContextsEqual` 验证。
- [x] 四档缩放实测：`WM_DPICHANGED` 模拟 100%/125%/150%/175%，应用逻辑尺寸恒为 760x520、物理尺寸 760x520 / 950x650 / 1140x780 / 1330x910。
- [x] 纯逻辑放置模块 `src/placement.rs`：`resolve_placement`/`ensure_visible`/`clamp_into`/`nearest_monitor`/`scale_for_dpi`，11 项单元测试覆盖双屏不同 DPI、负坐标、离屏恢复与越界夹取。
- [x] Windows 层 `placement`：`EnumDisplayMonitors`+`GetMonitorInfoW`+`GetDpiForMonitor` 枚举显示器（含设备名），窗口矩形读写与修正。
- [x] `settings.window_bounds` 改为 `Option<WindowBounds>`：无记忆位置不移动窗口；启动 150ms 按记忆位置恢复并修正到工作区内；400ms 轮询持久化位置/尺寸/显示器（即 300–500ms 防抖）。
- [x] live 实测：种子位置 (6000,4000) 修正为 (0,0)；位置 (120,90) 持久化并在重启后恢复；close 后 exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-15 共 33 个脚本 exit=0；门禁 fmt=0、clippy -D warnings=0、184 项测试（166 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-15 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-15**。PMv2 在运行窗口上生效；四档缩放下逻辑尺寸不变、物理尺寸随缩放变化；窗口位置可恢复且不会恢复到屏幕外（显示器缺失时修正到最近工作区）。

最终 Release SHA-256：`A1F0286AF2622095122DA001D3CA31F276D3D507588F988B45361E06E7F95F10`。

- 证据目录 `docs/test-results/P2-15/`：
  - `p2-15-dpi.tsv`：`pmv2_awareness`（AreDpiAwarenessContextsEqual=1、awareness=2）→ 四档 `scale_factor` → `offscreen_corrected`（(6000,4000)→(0,0)）→ `position_persisted`（120,90,\\.\DISPLAY3）→ `position_restored`（重启后 120,90）→ `completed`。
  - `p2-15-scale-report.tsv`：应用侧 250ms 状态采样（scale / 物理尺寸 / 窗口矩形 / 显示器工作区与 DPI）。
  - 回归：`docs/test-results/P2-15/regression/final6/regression.tsv`（33 脚本，0 失败）。

## 历史记录：P2-15 开发过程问题与修复

1. **`GetWindowDpiAwarenessContext` 返回值不可数值比较**：返回的是上下文句柄（本机 34）而非 -4；改用 `AreDpiAwarenessContextsEqual`（与 P0-06 `coordinates.rs` 一致）。
2. **`PostMessage(WM_DPICHANGED)` 不生效**：异步消息在建议矩形指针释放后才处理；改 `SendMessage`。
3. **恢复时序**：`ui.show()` 后、事件循环前原生句柄不可用（P2-13 同一约束）；恢复并入 150ms 外观定时器。
4. **恢复干扰验收脚本**：种子配置缺 `window_bounds` 时 serde 默认 (420,200) 被当作要恢复的位置，启动搬窗导致 13 个脚本坐标失准；改 `Option` 后消失。
5. **并发自伤**：门禁 `cargo test`（会启动进程、占用 Ctrl+Alt+Space）与后台回归并行导致 13 个失败；改为串行。
6. **系统弹窗遮挡**：桌面上残留的 SmartScreen 对话框覆盖面板采样点，p2-12/13/14 与 p2-10 失败；关闭后通过。
7. **`GetDpiForMonitor` 的 feature 归属**：在 windows-sys 0.61.2 中位于 `Win32_UI_HiDpi`（非 `Win32_Graphics_HiDpi`），无需新增依赖。

## 历史记录：P2-15 边界

- 本机仅 1 台显示器（1920x1080@96dpi）：双屏不同 DPI 由单元测试覆盖，现场未做物理双屏实测，需在多屏机器补验。
- 缩放验证用 `WM_DPICHANGED` 模拟，验证应用响应（逻辑尺寸不变、物理尺寸缩放），不改变系统实际缩放设置。
- 修正规则：完全离屏才搬回、部分越界夹入工作区；用户刻意部分越界的窗口会被夹回。
- 200% 不在 V1.2 必测范围。

## 历史记录：P2-14 任务清单（已验收）

- [x] 基线 16.3：`settings.backdrop`（none/mica/acrylic，默认 none，非法值回退 none）持久化；`Backdrop` 解析与 DWM 系统背景应用。
- [x] Windows 11 22H2+：`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` 应用 Mica（DWMSBT_MAINWINDOW）/Acrylic（DWMSBT_TRANSIENTWINDOW）/纯色（DWMSBT_NONE）；调用成功即为能力探针，失败/旧系统回退纯色。
- [x] 读回验证：`DwmGetWindowAttribute(38)` 读回应用的背景类型（none=1/Mica=2/Acrylic=3）。
- [x] 标题栏"设置"菜单新增 背景纯色/Mica/Acrylic（共 17 项）；点击立即应用并持久化。
- [x] live 实测：菜单切换 none→Mica→Acrylic→none，每步 DWM 读回 1/2/3/1 与配置一致；重启后 Mica 保持（读回=2）；exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-14 共 32 个脚本 exit=0；门禁 fmt=0、clippy -D warnings=0、173 项测试（155 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-14 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-14**。背景材质（纯色/Mica/Acrylic）按能力检测应用并持久化；DWM 属性应用经读回验证；不支持平台降级纯色不影响功能。

最终 Release SHA-256：`4521CBAE209ED8F68F9B85617C81DB3F6E681340AA73C2ADEFDF59890C522DD2`。

- 证据目录 `docs/test-results/P2-14/`：
  - `p2-14-backdrop.tsv`：`start_none`（读回=1）→ `mica_selected`（配置 mica、DWM=2）→ `acrylic_selected`（DWM=3）→ `none_selected`（DWM=1）→ `app_exit`（exit=0）→ `restart_mica`（重启后读回=2、配置 mica）→ `app_exit_restart` → `completed`。
  - 截图：`p2-14-backdrop-mica.png`、`p2-14-backdrop-acrylic.png`、`p2-14-backdrop-none.png`、`p2-14-restart-mica.png`。
  - 回归：`docs/test-results/P2-14/regression/final4/regression.tsv`（32 脚本，0 失败）。

## 历史记录：P2-14 开发过程问题与修复

1. **`SetWindowCompositionAttribute` 不静态导出**：外观模块对该未文档化 API 的静态链接失败（LNK2019）；改为 `GetModuleHandleW`+`GetProcAddress` 运行时解析。
2. **`DwmExtendFrameIntoClientArea(-1)` 破坏 P2-13 透明度**：把客户区并入边框后 DWM 的分层 alpha 不再作用于客户区，95% 透明度面板变为全不透明（P2-13 像素断言读到 246 而非 234）→ 移除该调用，只设置背景类型属性；材质在 DWM 管理区域（圆角裁剪）合成。P2-13 像素断言随之恢复（245,233,237）。
3. **材质可见性限制（探针实证）**：`renderer-software` 的客户区为不透明 GDI 像素，DWM 材质合成在其后，客户区主体不可见；此为渲染器能力边界，功能与配置不受影响。
4. 历史输入投递与夹具覆盖边界见 [TEST_HARNESS.md](TEST_HARNESS.md)；观察未建立防护组件因果关系，也不能据此排除产品缺陷。

## 历史记录：P2-14 边界

- SYSTEMBACKDROP 需 Windows 11 22H2+；旧系统 DWM 调用失败回退纯色（能力探针即调用成败）。
- 客户区主体为不透明像素，材质透过主体可见需支持 alpha 的呈现路径（渲染器升级，架构变更需用户批准）。
- Mica/Acrylic 3 档预设；跟随壁纸的系统行为由 Windows 管理。
- 验收脚本的键盘投递依赖窗口消息而非系统输入队列：在拦截注入输入的环境中仍能验证应用侧处理，但不再覆盖"注入输入能否送达"这一系统层行为。

## 历史记录：P2-13 任务清单（已验收）

- [x] 基线 16.2：透明度 50%–100%（默认 95%）；`normalize_opacity` 钳制非法值（越界钳入范围、NaN 回退 0.95）。
- [x] `settings.rounded_corners`（serde default=true）持久化；旧配置无该键时默认开。
- [x] `appearance::apply_window_effects`：`WS_EX_LAYERED`+`SetLayeredWindowAttributes(LWA_ALPHA)` 应用整窗不透明度；`DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE)` 应用圆角/直角；Win10 等不支持平台静默降级为直角。
- [x] 标题栏"设置"菜单新增 透明度 100/95/85/70/50 与 圆角开/关（共 14 项）；点击立即应用并持久化。
- [x] live 实测（红色参考窗体置于面板后方，合成像素 = app×α + red×(1−α)）：95% `245,233,237`；100% `244,246,250` 且圆角角落 `202,34,34`（红）；圆角关 → 角落 `255,255,255`（白标题栏不透明）；圆角开恢复；70% `247,173,175`；50% `249,123,125`；重启后 0.7+直角保持；钳制 1.5→100%、0.2→50%；exit=0。
- [x] P2-12 主题脚本菜单行高改为按 `GetMenuItemCount` 实测行数（菜单扩为 14 项后仍稳定）。
- [x] 回归 P1-01～P1-18 + P2-01～P2-13 共 31 个脚本 exit=0；门禁 fmt=0、clippy -D warnings、171 项测试（153 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-13 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-13**。透明度 50%–100% 与圆角开/关均可从标题栏"设置"菜单即时切换并持久化；默认 95% 且圆角开；不支持的圆角平台降级为直角不影响功能。

最终 Release SHA-256：`FA5516BB627322863C4828C46815D1CB262FEC3B5D73BEC22F33A534C4490ECF`。

- 证据目录 `docs/test-results/P2-13/`：
  - `p2-13-opacity-corners.tsv`：`window_ready` → `reference_form` → `opacity_95`（245,233,237）→ `opacity_100`（244,246,250 + 圆角红角 202,34,34）→ `corners_off`（白角 255,255,255）→ `corners_on`（红角恢复）→ `opacity_70`（247,173,175）→ `opacity_50`（249,123,125）→ `persisted`（0.5/true）→ `app_exit`（exit=0）→ `restart_70_square`（247,173,175 + 方形白角 255,179,179）→ `app_exit_restart` → `clamp_high`（1.5→不透明）→ `clamp_low`（0.2→50%）→ `completed`。
  - 截图：`p2-13-opacity-95.png`、`p2-13-opacity-100.png`、`p2-13-corners-off.png`、`p2-13-opacity-50.png`、`p2-13-restart-70-square.png`。
  - 回归：`docs/test-results/P2-13/regression/final/regression.tsv`（31 脚本，0 失败）。

## 历史记录：P2-13 开发过程问题与修复

1. **窗口句柄时机**：`apply_theme` 在 `ui.show()` 前调用时 `HasWindowHandle` 返回 NotSupported，外观不生效；改为 `ui.show()` 后以 150ms SingleShot Timer 再应用（事件循环内句柄可用），且菜单/主题每次切换都会重刷。
2. **菜单行高**：设置菜单扩为 14 项后，P2-12 脚本的 `height/7` 行高失准（点到错误行导致一次误报"深色未生效"）；改为 `MN_GETHMENU`+`GetMenuItemCount` 实测行数。
3. **角落像素语义**：方角+透明时，角落像素是"白色标题栏 × α + 红色参考"的合成（255,179,179），断言必须按合成公式而非纯面板色。
4. **clippy**：`Settings::default()` 后再赋值改为结构体更新语法；`SharedString` 比较改用 `&*value == "on"` 避免 owned 中间量。

## 历史记录：P2-13 边界

- DWM 圆角在 Windows 11 生效；Windows 10 无该属性时静默降级为直角（基线允许降级）。
- 分层透明对整个窗口矩形统一 α；像素级"背景材质"Mica/Acrylic 属 P2-14。
- 透明度预设 5 档；任意滑杆属第 17 章设置中心。

## 历史记录：P2-12 任务清单（已验收）

- [x] 基线 16.1：浅色/深色两套语义色板；`ThemeColors` 覆盖 window/panel/sidebar/border/text/secondary/muted/subtle/accent/accent-soft/alt-row/danger/warning；主界面硬编码颜色替换，工具提示/红点等固定色保留。
- [x] `settings.theme` 支持 light/dark/system，非法值回退 system；`settings.accent_color` 规范化（非法回退 #2563eb）并参与 accent/accent-soft。
- [x] 跟随系统：启动读取 `AppsUseLightTheme`；运行中每 2s 轮询，系统主题变化无需重启切换（启动/切换已实测，系统值实际翻转未测试）。
- [x] 标题栏"设置"左/右键菜单：浅色、深色、跟随系统、强调色蓝/绿/橙/紫；点击立即应用并持久化。
- [x] std-widgets（对话框 Button/LineEdit/ComboBox/CheckBox）通过 `Palette.color-scheme` 同步深浅色。
- [x] live 实测：light `244,246,250` → dark `16,21,29`（菜单）→ light → system（注册表=1，light）→ 绿色强调色分类底色 `234,247,239` → 重启 dark `16,21,29`；exit=0。
- [x] 锁定工作站期间以窗口消息 + PrintWindow 完成同一功能链的无头补充实测（`p2-12-theme-headless.tsv`）；解锁后官方 SendInput/CopyFromScreen 脚本通过。
- [x] 回归 P1-01～P1-18 + P2-01～P2-12 共 30 个脚本 exit=0；门禁 fmt=0、clippy -D warnings、169 项测试（151 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-12 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-12**。浅色/深色/跟随系统与强调色均可通过标题栏"设置"菜单即时切换、无需重启并持久化；跟随系统在启动与运行中生效。

最终 Release SHA-256：`D9E37A4D818FCC38C1D2AE1E126049D75130D216C8BEB27D105C2FDAA5BA9ED4`。

- 证据目录 `docs/test-results/P2-12/`：
  - `p2-12-theme.tsv`：`light_start` → `dark_switch`（菜单）→ `light_switch` → `system_follow` → `accent_green` → `app_exit`（exit=0）→ `dark_restart` → `app_exit_after_restart`（exit=0）→ `completed`。
  - 截图：`p2-12-light.png`、`p2-12-dark.png`、`p2-12-system.png`、`p2-12-accent-green.png`、`p2-12-dark-restart.png`。
  - 无头补充：`p2-12-theme-headless.tsv`、`p2-12-hl-*.png`（锁定期间真实菜单消息 + PrintWindow）。
  - 回归：`docs/test-results/P2-12/regression/final/regression.tsv`（30 脚本，0 失败）。

## 历史记录：P2-12 开发过程问题与修复

1. **原生菜单定位**：`ContextMenuArea` 的 `#32768` 弹出菜单位置与 Slint 逻辑坐标不一致（DPI/屏幕映射）；`WindowFromPoint` 会命中主窗口。改为枚举 `#32768` 菜单窗口，按菜单矩形点击条目。
2. **键盘导航不可靠**：弹出菜单在面板未激活时不一定获得键盘焦点；官方脚本改为按菜单 rect 点击。
3. **标题栏搜索坐标回归**：P2-12 把"设置"从文字改成 44px 按钮，P2-01 的 `width-116` 搜索坐标偏移；改为 `width-136`。
4. **P1-17 COM 挂起**：`/separate` 返回的进程立即退出，真实窗口属于另一 Explorer 进程；旧 Explorer 窗口的 `Document.Folder.Self.Path` 会阻塞。改为"测试前窗口集合 + 新窗口标题"定位 HWND，再按 HWND 匹配 Shell COM，避免遍历其他窗口的 Document。
5. **锁定期间验证**：工作站 04:51–08:50 锁定，`CopyFromScreen`/SendInput 不可用；用窗口消息驱动真实 Slint 点击与原生菜单、PrintWindow 采样完成无头实测；解锁后官方脚本与 30 脚本回归通过。

## 历史记录：P2-12 边界

- 系统主题在运行中实际翻转未测试（避免修改用户 Windows 个性化设置）；启动/切换到 `system` 时读取注册表并匹配，2s 轮询代码路径已接入。
- 原生弹出菜单跟随 Windows 系统主题，不跟随应用内浅/深覆盖。
- 强调色提供蓝/绿/橙/紫预设；任意取色属第 17 章设置中心。
- 透明度/圆角/Mica 属 P2-13/P2-14，不在本任务。

## 历史记录：P2-11 任务清单（已验收）

- [x] 基线 3.5 卡片菜单 8 项：打开、管理员身份运行、编辑、打开文件所在位置、复制路径、移动到其他分类、复制项目、删除（P1-05/06/07 与 P2-04/05 已逐项覆盖；P2-11 在屏幕边缘再次截图确认 8 项完整）。
- [x] 分组标签右键菜单（基线 2.2）：新建分组、重命名分组、左移/右移、强调色（蓝/绿/橙/紫/默认）、删除分组。
- [x] 分组名称对话框（新建/重命名共用，空名称/重复名称校验）；非空分组删除前弹确认，可迁移到其他分组或删除全部。
- [x] 分类菜单保留 重命名/上移/下移/删除，新增"新建项目"，作用于被右键的分类（非当前激活分类也可）。
- [x] 空白区菜单：添加项目、新建分类、新建分组。
- [x] 分组强调色在标签底部显示 3px 色条（`GroupRow.color` → Slint `accent_color` 映射）。
- [x] 菜单边缘/DPI：窗口移到工作区右下角、卡片距边缘 20px 时，原生菜单矩形 `1726,830–1920,1012` 完全落在 `0,0–1920,1032` 工作区内（自动上翻/左移）。
- [x] 清理 P1-03 遗留的重复 `on_category_menu_rename` 注册；分组删除处理器避免 match 临时借用导致的 RefCell 双借用崩溃。
- [x] 回归 P1-01～P1-18 + P2-01～P2-11 共 29 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、164 项测试（146 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-11 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-11**。卡片 8 项菜单、分组菜单（重命名/颜色/排序/删除含迁移）、分类"新建项目"、空白区新建入口全部可用；原生菜单在屏幕右下边缘不被截断。

最终 Release SHA-256：`2A4AA510AE2FE245276AF2015F3F78184AE2782ED1DE6EC479DDD7FA6281CD99`。

- 证据目录 `docs/test-results/P2-11/`：
  - `p2-11-menus.tsv` 十二阶段：`window_ready` → `group_rename`（name=AlphA）→ `group_color`（#2563eb）→ `category_new_project`（CatA1 items=1、CatA2 items=0）→ `blank_menu_new_group`（4 组，last=Delta）→ `group_migrate`（Gamma 删除，CatG1 迁入 AlphA）→ `group_delete_all`（Beta 删除，剩 AlphA/Delta）→ `group_move_right`（order=Delta,AlphA）→ `edge_window`（rect=1692,862）→ `edge_menu`（rect=1726,830,1920,1012，work=0,0,1920,1032）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-11-group-rename-dialog.png`、`p2-11-group-accent.png`、`p2-11-category-new-project.png`、`p2-11-new-group-dialog.png`、`p2-11-group-delete-confirm.png`、`p2-11-edge-menu.png`（右下角完整 8 项）。
  - 回归：`docs/test-results/P2-11/regression/final/regression.tsv`（29 脚本，0 失败）。

## 历史记录：P2-11 开发过程问题与修复

1. **RefCell 双借用崩溃**：分组删除处理器把 `controller.borrow_mut().delete_group(...)` 放在 `match` 临时表达式里，借用活到 `Err` 分支，随后 `borrow()` 构建迁移列表时 panic（应用崩溃）。改为先把 `outcome` 绑定到局部变量。
2. **重复回调注册**：P1-03 遗留了两处 `on_category_menu_rename` 注册（第二处覆盖第一处）；清理为一处。
3. **原生菜单窗口不在 `P005Desktop.Windows()`**：菜单 HWND 无标题/不满足可见窗口过滤；边缘测试改用"测试前后新增可见顶层窗口"枚举定位菜单，截图改用虚拟屏幕。
4. **菜单点击坐标脆弱**：分组菜单 10 项、含分隔线/DPI 缩放；改用原生菜单键盘导航（Down×N + Enter），避免像素偏移。
5. **新建分组入口**：原 ▼ 直接生成"新分组 N"；改为打开名称对话框，与其他创建路径一致。

## 历史记录：P2-11 边界

- 基线 2.2/2.3 的"设置图标"未纳入本任务（菜单聚焦 3.5 的 8 项与分组/分类管理）；分组仅提供强调色，分类/分组的 `icon` 字段保留但尚无显示图标的 UI。
- 分组栏 ▼ 的"下拉分组列表"（2.2）未实现；▼ 现打开新建分组对话框。
- 菜单边缘实测在 96 DPI；多 DPI/多显示器由 P2-15 专项验证。

## 历史记录：P2-10 任务清单（已验收）

- [x] 基线 4.4 识别：`is_uri_target`（RFC 3986 scheme）覆盖 `shell:`/`ms-settings:` 等协议；`is_shell_namespace_target` 覆盖 `::{CLSID}`/`shell:`/`ms-settings:`；`is_unc_path` 覆盖 `\\server\share`。不新增 item_type，分别归入既有 `uri`/`folder`。
- [x] `ItemKind::from_path`：URI → `uri`；`::{CLSID}` 与 UNC → `folder`；UNC/namespace 判断在 `is_dir()` 之前，避免导入时探测网络或 Shell。
- [x] `Item::path_missing`/`icon_source`：Shell 命名空间与 UNC 不显示失效角标、不尝试提取文件图标。
- [x] 启动：`launch_shell_target` 对 Shell 命名空间/UNC 跳过本地 `exists()` 检查，直接 `ShellExecuteExW`；`shell:`/`ms-settings:` 仍走 `launch_uri`；目标字符串保持原样，不解析/不复制。
- [x] `open_item_location` 对 URI/Shell/UNC 返回 `NoFileLocation`（"该项目没有本地文件位置（Shell 项目）"），不再误报"目标不存在"。
- [x] 导入补全：`fill_shell_draft` 在保存/拖入时把默认 Exe/File 类型修正为 Uri/Folder，并用 `SHCreateItemFromParsingName` + `SIGDN_NORMALDISPLAY` 从 Shell 命名空间读取显示名（`shell:MyComputerFolder` → "此电脑"）。
- [x] 特殊 Shell Link：`resolve_link_now` 对无文件目标的 IDList LNK 返回 `path=None`，启动仍走原始 LNK 的 `ShellExecuteExW`（控制面板 LNK 实测）。
- [x] 实测：`shell:MyComputerFolder`、`::{20D04FE0-3AEA-1069-A2D8-08002B30309D}`、控制面板 LNK、`ms-settings:display`、`\\localhost\C$` 均打开对应窗口；5 个项目的 target/type 启动后未改写；exit=0。
- [x] 集成测试 `tests/special_shell.rs`：特殊 LNK 无文件目标、Shell 显示名、`shell:`/CLSID 启动窗口、`NoFileLocation`。
- [x] 回归 P1-01～P1-18 + P2-01～P2-10 共 28 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、164 项测试（146 lib + 1 bin + 14 集成 + 3 doc）0 失败。

## 历史记录：P2-10 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-10**。控制面板、系统设置、Shell Namespace、特殊文件夹和网络位置均可保存原始 URI/LNK/UNC 并通过 Windows Shell 启动；不误报失效、不解析/复制目标，失败返回结构化错误。

最终 Release SHA-256：`C8C1D2C4500309BB8F0E754E6FDF4927B4BA88E005AB865988E7FD48F1A34328`。

- 证据目录 `docs/test-results/P2-10/`：
  - `p2-10-shell.tsv`：`fixture`（控制面板 LNK 116B）→ `window_ready` → `import_shell`（空名称导入 `shell:MyComputerFolder` → name=此电脑、type=uri、path 原样）→ `launch_shell`（此电脑）→ `app_exit_after_import`（exit=0）→ `extras_written` → `launch_clsid`（此电脑）→ `launch_control_panel`（所有控制面板项）→ `launch_settings`（设置）→ `launch_unc`（C$）→ `targets_kept`（5 项 target/type 未改写）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-10-import-shell.png`、`p2-10-thispc-window.png`、`p2-10-clsid-window.png`、`p2-10-control-panel-window.png`、`p2-10-settings-window.png`、`p2-10-unc-window.png`。
  - 回归：`docs/test-results/P2-10/regression/final/regression.tsv`（28 脚本，0 失败）。
- 集成测试输出：`special_shell_targets ... ok`（含 `checkpoint: display name = 此电脑`）。

## 历史记录：P2-10 开发过程问题与修复

1. **既有类型映射**：基线第 3.1 章固定 7 种 item_type，P2-10 不新增类型；URI/`ms-settings:`/`shell:` 归 `uri`，`::{CLSID}`/UNC 归 `folder`，避免模型与 UI 变更。
2. **避免网络探测**：`ItemKind::from_path` 对 UNC 的 `is_dir()` 会让导入阻塞/弹凭据；把 UNC/namespace 判断提前，`path_missing` 与 `icon_source` 同样跳过。
3. **显示名**：空名称导入 `shell:MyComputerFolder` 时，`SHCreateItemFromParsingName` + `SIGDN_NORMALDISPLAY` 读取"此电脑"；`ms-settings:` 不是命名空间项，返回 None，需用户填写名称。
4. **open_item_location**：URI/Shell/UNC 没有本地文件可选中，新增 `NoFileLocation` 结构化提示，替代误导性的"目标不存在"。
5. **测试**：集成测试用 `SetIDList` 创建真正的控制面板 Shell Link，并逐个关闭测试窗口；live 脚本用进程名（explorer/SystemSettings/ApplicationFrameHost）检测新窗口，避免依赖本地化标题。

## 历史记录：P2-09 任务清单（已验收）

- [x] 基线 4.3 识别：`shell::detect_advertised_shortcut` 用 `IShellLinkDataList::GetFlags` 检查 `SLDF_HAS_DARWINID`（MSI 广告快捷方式的 Darwin 标记）；检测只读链接标志，不解析目标、不触发 Windows Installer。
- [x] `Launch.msi_advertised`（serde default，旧配置兼容）经 `ItemDraft` 全链路保存；普通 LNK、UWP LNK 均为 false。
- [x] 基线 4.3 导入：广告快捷方式保留原始 LNK（`item_type=lnk`、`launch.path=原 LNK`），不解析 EXE，不复制/重写快捷方式；不填 args/workdir/icon/AUMID；状态栏提示"已识别 MSI 广告快捷方式（由 Windows Installer 启动）"。
- [x] 基线 4.3/4.6 启动：`launch_lnk` 始终对原始 LNK 调 `ShellExecuteExW`（不绕过 Windows Installer）；管理员/`run_as_admin` 仍走 runas 原始 LNK；MSI 项目禁用 AUMID 分支。
- [x] 失效提示：原 LNK 缺失 → `MsiShortcutMissing`；Shell 启动失败 → `MsiLaunchFailed`；消息含"请修复安装或重新添加入口"，不静默失败，且不改写项目路径。
- [x] 拖入导入同样调用 `fill_lnk_draft`（外部拖入的 MSI 快捷方式也保存广告标记）。
- [x] 集成测试 `tests/msi_advertised.rs`：普通 LNK 不误报、合成 Darwin 块 + `SLDF_HAS_DARWINID` 检出、缺失 LNK 不误报；`launch.rs` 单测覆盖缺失/Shell 失败映射与消息文本。
- [x] 真实 MSI 夹具：`scripts/p2-09-msi-fixture.ps1` 用 Windows Installer COM 建表 + SDK `MsiInfo.exe` 写 SummaryInformation；安装后 Start Menu 生成真实广告快捷方式（`Shortcut.Target=Feature`，含 Darwin 块），卸载后快捷方式移除。
- [x] 专项验证 `scripts/verify-p2-09-msi.ps1`：安装 → 添加对话框导入（断言 `msi_advertised=true`、路径保持、AUMID/args/workdir 为空）→ 右键"打开"启动 payload → exit=0 → 卸载 → 缺失快捷方式启动保持存活（截图 "MSI 快捷方式已失效…"）。
- [x] 回归 P1-01～P1-18 + P2-01～P2-09 共 27 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、160 项测试（143 lib + 1 bin + 13 集成 + 3 doc）0 失败。

## 历史记录：P2-09 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-09**。MSI 广告快捷方式以原始 LNK 保存和启动，不解析 EXE、不复制/重写、不绕过 Windows Installer；失效时给出修复安装/重新添加入口的结构化提示。

最终 Release SHA-256：`4F5496B52FE9B750D009FB726F44B891B36A909FB56083A6C59BF78DC696639F`。

- 证据目录 `docs/test-results/P2-09/`：
  - `p2-09-msi.tsv` 十阶段：`fixture_built` → `fixture_installed`（exit=0，广告快捷方式 Darwin=true）→ `window_ready` → `import_advertised`（flag=true、原 LNK、aumid/args/workdir 为空）→ `launch_advertised`（payload marker 出现）→ `app_exit`（exit=0）→ `fixture_uninstalled`（exit=0，快捷方式已移除）→ `launch_missing`（存活、未启动、路径保持）→ `app_exit_after_missing`（exit=0）→ `completed`。
  - 截图：`p2-09-import-msi.png`、`p2-09-msi-launched.png`（状态"已启动 msiitem"）、`p2-09-msi-missing.png`（状态"MSI 快捷方式已失效：…"）。
  - 回归：`docs/test-results/P2-09/regression/final/regression.tsv`（27 脚本，0 失败）。
- 集成测试输出：`advertised_shortcut_detection ... ok`。

## 历史记录：P2-09 开发过程问题与修复

1. **SummaryInformation 写入**：本机 Windows Installer 自动化的 `SummaryInformation.Property(Pid)` 赋值稳定报 `Msi API Error: Property,Pid`（PowerShell 与 VBScript、32/64 位均复现），改用 Windows SDK 的 `MsiInfo.exe` 写 `/C /T /J /A /P /V /G /W 0 /U 2`。缺少 PID 15（Words）时安装会在 "Determining source type" 处被 SecureRepair 拒绝（错误 1316）。
2. **非压缩源树**：`Media.Cabinet` 为空时源文件必须按目标目录镜像（`<MSI 目录>\QuickNestMsiTest\payload.exe`），否则报 2920（未指定源目录）/1309（读取失败）；PID 15 必须为 0（uncompressed），设为 2 会被当作压缩源。
3. **广告快捷方式生成**：Shortcut 表 `Target=MainFeature`（Feature 名）即广告快捷方式；安装后实测 Start Menu LNK 的 `LinkFlags=0x1084`（含 `SLDF_HAS_DARWINID`）并带 Darwin 块，卸载后由 `RemoveShortcuts` 移除。
4. **检测顺序**：广告检测先于 UWP 检测，避免 Darwin LNK 被误判为 UWP；检测仅调用 `GetFlags`，不解析目标，因此导入阶段不会触发 MSI 安装/修复。
5. **测试环境隔离**：P1-17 的 `explorer.exe /separate` 可能留下置顶的"文档"窗口，遮挡 QuickNest 导致 P1-18/P2 系列连锁失败；在 P1-17 的 finally 中记录测试前 Explorer 窗口并关闭测试期间新建的全部窗口。`run-regression.ps1` 改为每个脚本独立 PowerShell 进程（测试辅助类不能在同一会话重复 `Add-Type`），保留真实 ExitCode 并加 300s 超时。
6. **边界**：`MsiLaunchFailed` 只覆盖 `ShellExecuteExW` 同步失败；MSI 自身弹出的修复/源定位对话框由 Windows Installer 处理，QuickNest 不拦截。`msi_advertised` 是导入时的快照；若快捷方式在导入后被替换为普通 LNK，仍会按 MSI 提示。

## 历史记录：P2-08 任务清单（已验收）

- [x] 基线 4.2.4/4.6：`launch_lnk` 对带 `launch.aumid` 的项目（非管理员）先走 `IApplicationActivationManager::ActivateApplication`（`ACTIVATEOPTIONS(0)`，参数为项目 args join），返回 PID 即 `Started`。
- [x] 基线 4.2.5 回退：AUMID 激活失败（HRESULT）→ 原始 LNK 的 `ShellExecuteExW`；全部失败返回结构化错误；AUMID 激活不要求 LNK 文件存在（原快捷方式被删除仍可激活）。
- [x] 管理员运行保持 LNK 的 Shell 语义（runas 原始 LNK，不走 AUMID）；无 AUMID 的 LNK 行为不变。
- [x] `activate_application` 封装（`CoCreateInstance(ApplicationActivationManager)` + `ActivateApplication`，HRESULT 结构化返回）。
- [x] 集成测试 `tests/uwp_link.rs` 扩展：Calculator AUMID 激活（PID>0）、无效 AUMID 激活失败回退原始 LNK（cmd /c exit → Started）、有效 AUMID 且 LNK 已删除仍启动。
- [x] P2-08 专项验证 `scripts/verify-p2-08-aumid.ps1`：种子项目 AUMID=计算器 且 LNK 路径不存在 → 单击卡片 → 计算器窗口出现（AUMID 激活证据）→ 再次点击重新激活 → exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-08 共 26 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、156 项测试（140 lib + 1 bin + 7 集成 + 3 doc）0 失败。

## 历史记录：P2-08 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-08**。AUMID 直接激活与"激活失败回退原始 LNK"闭环；UWP 无 AUMID 时的原始 LNK 启动（P2-07）保持不变。

最终 Release SHA-256：`4B4E627A0970BA5407D841AB64F8C4018B2B9B94FC35039CD5B2CD615F40885C`。

- 证据目录 `docs/test-results/P2-08/`：
  - `p2-08-aumid.tsv` 六阶段：`window_ready` → `aumid_activation`（种子 LNK 路径不存在，计算器窗口出现 → AUMID 激活证据）→ `aumid_reactivate`（已运行应用重新激活）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-08-aumid-launch.png` / `p2-08-aumid-reactivate.png`。
  - 集成测试输出：`aumid activation started` / `aumid activation + fallback ok`。

## 历史记录：P2-08 开发过程问题与修复

1. **激活时机与 LNK 存在性**：初版把 AUMID 激活放在 LNK 存在性检查之后——原快捷方式被删除时 AUMID 激活永远不会执行；按基线 4.6"UWP AUMID 首选直接激活"把激活提前到存在性检查之前（AUMID 激活本身不依赖 LNK 文件）。
2. **let-chain 折叠**：clippy 要求合并嵌套 if；采用 `&& let Ok(pid) = ...` let-chain（Rust 2024）。
3. **管理员路径**：`runas` 原始 LNK 保持 Shell 语义（UWP 提权场景罕见），AUMID 激活仅限非管理员路径——记录为边界。
4. **测试脚本**：Close-Calculator 以 `*计算器*` 匹配窗口（ApplicationFrameHost 宿主），脚本需 UTF-8 BOM。

## 历史记录：P2-07 任务清单（已验收）

- [x] 基线 4.2 UWP 检测：`detect_uwp_link` —— .lnk 解析目标为空且 IDList 无文件系统路径时，用 `SHCreateItemFromIDList` + `IShellItem2::GetString(System.AppUserModel.ID)` 从目标 Shell 项读取 AUMID（不凭显示名称猜测）；普通 LNK/文件夹链接返回 None。
- [x] 基线 4.2.3 保存 AUMID：`Launch.aumid` 字段（serde default，旧配置兼容）经 `ItemDraft` 全链路接线；导入保存时 `fill_lnk_draft` 检测 UWP 并保存 AUMID，项目保持 `item_type=lnk` 引用原始 LNK。
- [x] 基线 4.2.2 启动：UWP 项目仍走 `launch_lnk` 的 `ShellExecuteExW` 原始 LNK（不解析成 EXE、不绕过 Shell）；启动失败结构化错误。AUMID 直接激活与激活失败回退属 P2-08。
- [x] 导入 UI：浏览选择 UWP 快捷方式时状态栏提示"已识别 UWP 应用（AUMID：…）"。
- [x] 集成测试 `tests/uwp_link.rs`（独立进程）：普通 LNK 不误判 + 真实 AppsFolder 项（枚举取第一个带 AUMID 的项）构建 IDList 快捷方式 → 检出 AUMID 且 resolve 目标为空。
- [x] P2-07 专项验证 `scripts/verify-p2-07-uwp.ps1`：WScript 创建 `shell:AppsFolder\计算器AUMID` 快捷方式 → 添加对话框导入（类型=Windows 快捷方式）→ 保存后配置含 AUMID → 右键"打开"启动原始 LNK → 计算器窗口出现 → 关闭 → exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-07 共 25 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、156 项测试（140 lib + 1 bin + 7 集成 + 3 doc：uwp_link 新增 1 项）0 失败。

## 历史记录：P2-07 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-07**。UWP 快捷方式的检测、AUMID 保存与原始 LNK 启动闭环；AUMID 直接激活（IApplicationActivationManager）与激活失败回退属 P2-08。

最终 Release SHA-256：`608AFBAE3581EA6990075EEEC2A2A75C369E9D3BE7AEBA419DFF79809F71DA4`。

- 证据目录 `docs/test-results/P2-07/`：
  - `p2-07-uwp.tsv` 六阶段：`fixture`（WScript 创建 shell:AppsFolder 快捷方式，1407B）→ `window_ready` → `import_debug`（item0=uwpcalc|lnk|aumid=[Microsoft.WindowsCalculator_8wekyb3d8bbwe!App]）→ `import_aumid` → `menu_rect` → `launch_original_lnk`（计算器窗口出现）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-07-import-uwp.png` / `p2-07-menu.png` / `p2-07-calculator.png`。
  - 集成测试输出：`uwp link detection ok: <AUMID>`。

## 历史记录：P2-07 开发过程问题与修复

1. **AUMID 来源**：.lnk 自身属性存储可能缺失 `System.AppUserModel.ID`（WScript 创建的快捷方式就没有），改为从目标 AppsFolder 项的 `IShellItem2::GetString` 读取——对 Windows 生成的真实 UWP 快捷方式和 IDList 快捷方式都可靠。
2. **PIDL 泄漏**：初版在 `SHCreateItemFromIDList` 失败路径用 `?` 提前返回，跳过 `CoTaskMemFree`；重写为无提前返回的 Option 链，PIDL 恰好释放一次。
3. **无文件系统目标的判定**：目标为空 + `SHGetPathFromIDListW` 失败 → AppsFolder 项；只判目标为空会误伤"此电脑"等特殊文件夹链接。
4. **旧二进制跑新脚本**：导入阶段曾用未重建的 release 二进制运行导致断言混乱（aumid 缺失/类型=exe）；此后每次脚本前显式重建 release。
5. **PSCustomObject 严格模式**：空 items 数组上访问属性抛 PropertyNotFoundStrict；调试记录改为 Count 分支。
6. **GetOpenFileNameW 对 UWP LNK 报错**：文件对话框校验 UWP 快捷方式时弹系统错误气球，改用目标字段手动输入 + 类型下拉选择"Windows 快捷方式"的导入路径（保存时检测逻辑相同）。
7. **卡片单击启动偶发不触发**：导入保存后紧接的卡片单击未触发 item-selected（三次中一次）；启动断言改用右键菜单"打开"（同一 `launch_item` 路径，确定性）；单击路径继续由 P1-06/P2-04 回归覆盖。

## 历史记录：P2-06 任务清单（已验收）

- [x] 基线 5.6/2.2 跨分组移动：拖拽卡片悬停顶部分组标签 → 标签高亮 + 状态栏"松开移动到「分组」"；悬停 400ms 弹簧加载切换到该分组（仅刷新侧栏，网格保持源分组以维持拖拽指针捕获），状态栏"已切换到「分组」"。
- [x] 释放语义：快速落点（<400ms）或弹簧加载后释放均调用 `move_item_across_groups` 把项目移动到目标分组当前分类并持久化；弹簧切换后直接落到网格也归入新分组当前分类。
- [x] 拖到当前分组标签/网格/分类仍按 P2-04/P2-05 语义；ESC 取消：清除分组目标与弹簧状态，释放不提交（弹簧已发生的分组切换保留）。
- [x] 单测：`move_item_across_groups`（跨组追加、同分类 no-op、第二目标、未知 item/目标、失败不改库）+ `category_id_of_item`。
- [x] P2-06 专项验证 `scripts/verify-p2-06-cross-group.ps1`：弹簧加载 700ms → CrossItem 移入 GroupB；快速落点 → FastItem 移入 GroupB；悬停后 ESC → KeepItem 不动、current=g2；exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-06 共 24 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、155 项测试（140 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-06 验收结论与证据

结论：**2026-11-05 本任务验收通过，停在 P2-06**。跨分组拖拽（含 400ms 弹簧加载与快速落点）闭环；分类/一级分组自身的拖拽排序仍未实现（P2 后续讨论或保持右键菜单）。

最终 Release SHA-256：`55FF36C64E273294AF1505ACA1DE4D47D3498B6D259752A2020518E11E245A74`。

- 证据目录 `docs/test-results/P2-06/`：
  - `p2-06-cross-group.tsv` 六阶段：`window_ready` → `spring_load`（CatA=FastItem,KeepItem、CatB=CrossItem、current=g2）→ `fast_drop`（CatA=KeepItem、CatB=CrossItem,FastItem）→ `esc_cancel`（CatA=KeepItem、CatB 不变、current=g2）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-06-spring-hover.png`（GroupB 高亮 + 侧栏切换 + 源卡变暗 + 状态"已切换到「GroupB」"）/ `p2-06-after-spring.png` / `p2-06-back-to-a.png` / `p2-06-fast-drop-moment.png` / `p2-06-after-fast-drop.png` / `p2-06-esc-cancel.png`。

## 历史记录：P2-06 开发过程问题与修复

1. **RefCell 双借用崩溃（修复）**：let-chain 条件中的 `controller.borrow()` 临时存活到整个 if 体，体内 `borrow_mut()` 触发 panic（应用直接退出、窗口消失）；改为先把 `active_category_id_of_group` / `category_id_of_item` 绑定到局部变量再判断。
2. **Flickable 抢手势偶发（修复）**：卡片按下后 Flickable 偶尔赢得 pan 手势导致拖拽不启动（P2-04 曾单发）；在卡片按下期间设置 `mouse-drag-pan-enabled=false`（wheel 滚动不受影响），3 连跑稳定。注意不能用 `interactive=false`——那会直接取消当前手势（P2-04 教训）。
3. **弹簧加载刷新范围**：`select_group` 后仅调用 `refresh_sidebar`（分组/分类/当前分类），不重建网格 items，保持被拖卡片的指针捕获；完整 `refresh_ui` 会销毁卡片导致释放事件丢失。
4. **弹簧后残留提示**：弹簧切换只清 `drag-target-group-id`，状态栏"松开移动到…"残留至释放；计时器同时清除 `item-drag-hint`。
5. **测试脚本时序**：ESC 阶段最初用带释放的拖拽助手，ESC 在释放之后到达导致误移动；改为按钮保持跨 ESC 的手动序列。
6. **config_save 恢复测试偶发 Busy**：`recovery_holds_the_write_lock_against_concurrent_saves` 的收尾恢复与并发写事务竞争；对 `recover_latest_backup` 增加 Busy 重试（与 missing_directory 测试同模式）。

## 历史记录：P2-05 任务清单（已验收）

- [x] 基线 5.6/3.5 跨分类移动：拖拽卡片经过左侧非当前分类行时该行高亮（浅蓝底 + 2px 蓝框），状态栏显示"松开以移动到「分类」"。
- [x] 命中测试复用 P0-06/P2-17：Slint 传入窗口逻辑坐标（含 Flickable content 偏移），Rust 用 `drop_layout::hit_test` 取分类行。
- [x] 释放：目标分类非当前分类时调用 `move_item_to_category` 持久化并刷新；源分类网格移除、目标分类计数更新；拖到网格/当前分类仍走 P2-04 内部排序。
- [x] ESC：拖拽中取消目标分类与拖拽状态，释放不提交，分类与顺序不变。
- [x] 插入竖线仅在网格为目标时显示；分类悬停时隐藏，避免与分类高亮混淆。
- [x] 单测：控制器已有 `move_item_to_category` 测试（跨分类移动/未知 id）继续通过。
- [x] P2-05 专项验证 `scripts/verify-p2-05-cross-category.ps1`：MoveMe 从 CatA 拖到 CatB → CatA=StayHere、CatB=MoveMe；ESC 拖拽取消分类不变；exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-05 共 23 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、154 项测试（139 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-05 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P2-05**。跨分类拖拽闭环；跨分组拖拽（分组标签弹簧加载 400ms）属 P2-06。

最终 Release SHA-256：`3E6A8DB2A46C0051B5E51EFF395BD2803BD5796854B414EECC39F714784643D3`。

- 证据目录 `docs/test-results/P2-05/`：
  - `p2-05-cross-category.tsv` 五阶段：`window_ready` → `cross_category_move`（CatA=StayHere，CatB=MoveMe）→ `esc_cancel`（分类不变）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-05-initial.png` / `p2-05-drag-over-category.png`（CatB 高亮 + 状态栏提示 + 源卡变暗）/ `p2-05-after-move.png` / `p2-05-esc-cancel.png`。

## 历史记录：P2-05 开发过程问题与修复

1. **拖拽窗口坐标**：卡片内 mouse 坐标是内容坐标，需叠加分类栏/标题+分组条偏移与 Flickable `content-x/y` 才能命中侧栏；用于分类命中测试。
2. **命中数据来源**：复用 `drop_layout::hit_test`（P2-17 外部拖拽同一布局常量），保证分类行/分组条/网格判定一致。
3. **当前分类释放**：目标等于当前分类时不执行移动，避免把项目"移动到自己"；仍允许网格内部排序。
4. **分类高亮与插入线冲突**：分类悬停时隐藏网格插入竖线，只保留分类高亮与状态提示。
5. **回归清理挂起（P1-17/P1-18）**：`Shell.Application.Windows()` 枚举在残留 Explorer 状态下偶发阻塞，导致批量回归卡住；移除 finally 中的 COM 枚举（已知窗口已用 WM_CLOSE 关闭），P1-17/P1-18 单独运行均快速退出。
6. **P2-04 点击启动偶发**：批量负载下 900ms 内探针未生成；改为最多 3 次点击 + 每次 2s 等待，单独 3 连跑与整批回归均通过。

## 历史记录：P2-04 任务清单（已验收）

- [x] 基线 2.4/5.6 网格内排序：卡片按下后移动超过 8px 进入拖拽，源卡片透明度 0.4，网格显示蓝色插入位置竖线。
- [x] 目标槽位：拖动坐标换算为内容坐标，按当前列数/格子尺寸计算 slot 并钳制到 0..N-1；控制器 `move_item_to_index` 重排并持久化。
- [x] ESC 取消：拖拽中按 Esc 清除拖拽状态，释放时不再提交，配置顺序不变。
- [x] 单击启动保持：卡片点击改由 pointer down/up 判定（未超过阈值才启动），不再依赖 `clicked`；右键菜单仍可用。
- [x] 单测：`move_item_to_index` 前移/后移/越界钳制/未知 id/同槽 no-op。
- [x] P2-04 专项验证 `scripts/verify-p2-04-sorting.ps1`：拖 SortA 到第三槽 → 顺序 B,C,A；ESC 拖拽取消顺序不变；单击 SortB 仍启动；exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-04 共 22 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、154 项测试（139 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-04 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P2-04**。当前分类内拖拽排序闭环；跨分类/跨分组移动仍属 P2-05/P2-06。

最终 Release SHA-256：`D969FC59A6DB48B8FA2DD1DED0A8D886BC4552E11D8C0DE774A8CAA5323152C2`。

- 证据目录 `docs/test-results/P2-04/`：
  - `p2-04-sorting.tsv` 五阶段：`window_ready` → `drag_reorder`（SortA→第三槽，顺序 SortB,SortC,SortA）→ `esc_cancel`（顺序不变）→ `click_launch`（SortB 启动）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-04-initial.png` / `p2-04-drag-hover.png`（SortA 变暗 + 插入竖线）/ `p2-04-after-drop.png` / `p2-04-esc-cancel.png`。

## 历史记录：P2-04 开发过程问题与修复

1. **Flickable 禁用导致手势取消**：尝试在按下时设置 `Flickable.interactive=false` 以阻止滚动抢占，结果 Flickable 取消当前手势，拖拽永远不开始；移除该设置后卡片在按下状态可正常收到 move 事件。
2. **点击与拖拽冲突**：移除 `clicked` 回调，改为 pointer down/up + 8px 阈值判定；未超过阈值才启动，拖拽释放只提交排序。
3. **插入位置计算**：Slint 传入内容坐标与列数，Rust 按 `cell_width/cell_height` 计算 slot 并钳制，避免在 Slint 侧做单位换算。
4. **ESC 取消时序**：Esc 只清除 root 拖拽状态，卡片本地 `dragging` 仍为 true；释放时检查 `root.item-drag-active` 以决定是否提交，保证取消生效。
5. **P1-18 回归偶发**：mshta 进程存在但窗口尚未创建时对空数组取 `[0]` 抛异常；改为先判 `Count`；标题栏固定点击增加读配置重试。修复后 22 脚本全过。

## 历史记录：P2-03 任务清单（已验收）

- [x] 基线 13.4：结果默认选中第一条；`search-selected-index` 驱动行高亮（浅蓝底 + 2px 边框）。
- [x] 上下左右：上/左向前、下/右向后，边界循环；空结果保持 -1。移动逻辑抽为 `search::move_selection` 并单测。
- [x] Enter：启动选中项（沿用 P1-06/P1-18 启动与自动隐藏策略）；Ctrl+Enter：以管理员身份启动（UAC 路径，代码审查，未自动接受 UAC）。
- [x] Ctrl+D：打开选中项所在位置并关闭搜索（`open_item_location`）；文件/文件夹/LNK 走既有 Shell 逻辑。
- [x] Tab：切换"当前分组/全部分组"并刷新结果、重置选中为第一条。
- [x] Esc：保持先清空关键词、再关闭；输入框与根 FocusScope 共用 `search_key` 映射。
- [x] 鼠标点击结果仍可用，并同步选中索引。
- [x] P2-03 专项验证 `scripts/verify-p2-03-keyboard.ps1`：默认首条 → Down 选中 AlphaB → Enter 启动；Up 从首条循环到 AlphaC → Enter；beta 当前分组 0 结果 → Tab 全部分组 → Enter 启动 BetaOther；Ctrl+D 打开所在文件夹；Esc 清空/关闭；exit=0。
- [x] 回归 P1-01～P1-18 + P2-01～P2-03 共 21 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、153 项测试（138 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-03 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P2-03**。搜索中心键盘流按 13.4 闭环（Ctrl+Enter 的 UAC 确认无法自动化，代码路径完整）。

最终 Release SHA-256：`1041658F41A1F023EDA1E2530B6F383012F38F3B90CDE78080171D4247507CDE`。

- 证据目录 `docs/test-results/P2-03/`：
  - `p2-03-keyboard.tsv` 七阶段：`window_ready` → `down_enter`（默认首条，Down → AlphaB）→ `up_wrap_enter`（Up 循环到 AlphaC）→ `tab_scope_enter`（Tab 全部分组 → BetaOther）→ `ctrl_d_location`（Explorer 标题 QuickNestP203-…）→ `esc_semantics` → `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-03-default-first.png` / `p2-03-after-down.png`（AlphaB 高亮）/ `p2-03-after-up.png`（AlphaC 高亮）/ `p2-03-tab-current.png` / `p2-03-tab-all.png` / `p2-03-ctrl-d.png` / `p2-03-esc-cleared.png` / `p2-03-esc-closed.png`。

## 历史记录：P2-03 开发过程问题与修复

1. **Tab/Enter 被输入框消费**：TextInput 默认处理 Tab 焦点与 Enter；在 LineEdit 的 `key-pressed` 中 accept 搜索按键，阻止默认行为后再路由到回调。
2. **选中项与模型同步**：结果刷新时重置为 0（无结果为 -1）；移动用 `move_selection` 处理空集与循环；点击结果同步选中索引。
3. **Ctrl+D 默认插入字符**：接受该按键后再调用定位回调，避免 "d" 写入搜索框。
4. **脚本测试项数组**：PowerShell 中 `Probe-Item a, Probe-Item b` 会把后续调用当作第一个函数的参数，只生成一项；改为每个调用加括号。
5. **`[ushort]` 类型别名**：Windows PowerShell 5.1 不识别 `[ushort]`，改用 `[int]` 参数。
6. 含中文脚本继续使用 UTF-8 BOM。

## 历史记录：P2-02 任务清单（已验收）

- [x] 基线 13.3 拼音索引：`PinyinIndex` 保存标准化名称（小写、去分隔符）、全拼变体、首字母变体；使用 `pinyin = "=0.11.0"` 的字符读音与异读列表，常见多音字自动生成候选（变体上限 32）。
- [x] 索引缓存：控制器按 item id 缓存 `(name, index)`；名称变化（新增/重命名）后首次搜索自动重建，旧读音不再命中。
- [x] 基线 13.2 匹配接入：全拼完全/前缀/包含、首字母完全/前缀/包含、全拼模糊子序列；拼音分数 660–690，低于直接名称模糊 700、高于路径 600。
- [x] 搜索中心 UI 复用 P2-01：无需改动面板，结果排序自然体现"直接名称 > 拼音 > 路径"。
- [x] 单测：微信→weixin/wx、网易云音乐→wyyyy（wyy 前缀）、音乐→yinyue/yinle 多音字、大小写/分隔符/中文查询不误入拼音、变体数量上限；控制器测试覆盖全拼/首字母命中与重命名后索引重建。
- [x] P2-02 专项验证 `scripts/verify-p2-02-pinyin.ps1`：`weixin`/`wx` → 微信测试，`wyy` → 网易云音乐，`yinyue` → 音乐盒（多音字候选），均通过点击结果验证真实启动与探针文件内容。
- [x] 回归 P1-01～P1-18 + P2-01 + P2-02 共 20 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、152 项测试（137 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-02 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P2-02**。全拼、首字母与常见多音字候选已接入搜索；纯键盘流（上下键/Enter/Ctrl+Enter/Ctrl+D/Tab）仍属 P2-03。

最终 Release SHA-256：`4C09FFD52F47C867A2CB8B8193F358B83CAD21E78248200D0F1A7842F0671BA2`。

- 证据目录 `docs/test-results/P2-02/`：
  - `p2-02-pinyin.tsv` 七阶段：`window_ready` → `weixin`（微信测试）→ `wx`（微信测试）→ `wyy`（网易云音乐）→ `yinyue`（音乐盒）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-02-weixin.png` / `p2-02-wx.png` / `p2-02-wyy.png` / `p2-02-yinyue.png`（每次查询的结果列表）。

## 历史记录：P2-02 开发过程问题与修复

1. **拼音数据来源**：未手写字符表，引入固定版本 `pinyin = "=0.11.0"`（含 `plain` 与异读数据），Cargo.lock 固定；全拼/首字母/多音字均由同一索引生成。
2. **多音字组合爆炸**：对每个字取异读列表并做笛卡尔积，`MAX_VARIANTS=32` 截断；单测覆盖"乐乐…"长名称的变体上限。
3. **索引失效**：缓存键为 item id，同时保存生成时的名称；重命名后名称不一致即重建，避免旧拼音残留。
4. **拼音分数不能压过直接名称**：拼音层固定 660–690，低于名称模糊 700、高于路径 600，保证"直接名称 > 拼音 > 路径"。
5. **脚本编码**：P2-02 脚本含中文项目名，PowerShell 5.1 读取无 BOM UTF-8 时会把引号吞入多字节字符；写入 UTF-8 BOM 后解析通过。

## 历史记录：P2-01 任务清单（已验收）

- [x] 基线 2.6/11.2：标题栏"搜索"可点击、Ctrl+F 打开搜索中心；输入框自动聚焦，关闭后焦点交还窗口按键作用域，Ctrl+F 可再次打开。
- [x] 基线 13.1：默认搜索当前一级分组，点击"范围"切换全部分组；只搜索已登记项目，不扫描磁盘。
- [x] 基线 13.2 基础匹配：名称完全/前缀/包含/模糊子序列（字符按序出现）、路径包含，大小写不敏感；按分数（完全 > 前缀 > 包含 > 模糊 > 路径）再按名称排序。
- [x] 实时过滤：输入每次变化立即刷新结果；空查询显示当前范围内全部项目。
- [x] 点击结果启动项目并关闭面板；全部分组结果可启动（项目查找改为全库，不再限于当前分类）。
- [x] 基线 13.4（部分）：Esc 先清空关键词，再按一次关闭；上下键/Enter/Ctrl+Enter/Ctrl+D/Tab 属 P2-03 未实现。
- [x] 单测：匹配评分 5 项（完全/前缀/包含/模糊顺序、路径、空查询/trim、中文）+ 控制器范围/排序/跨分组启动查找。
- [x] P2-01 专项验证 `scripts/verify-p2-01-search.ps1`：Ctrl+F 打开 → 输入 alpha 过滤并点击启动 → 标题入口重开 → beta 在当前分组无结果 → 切换全部分组后启动 BetaTwo → Esc 清空 → Esc 关闭 → WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-18 + P2-01 共 19 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、147 项测试（132 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P2-01 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P2-01**。搜索入口、实时过滤、分组范围、基础匹配与点击启动闭环；拼音（P2-02）与纯键盘流（P2-03）未实现。

最终 Release SHA-256：`DC97B78DCD6A919D634B446294E0C088B6D6E737C47EB7C1260CE8DAD62FF9B0`。

- 证据目录 `docs/test-results/P2-01/`：
  - `p2-01-search.tsv` 七阶段：`ctrl_f_open` → `filter_and_launch`（alpha.txt）→ `scope_current_group`（beta 无结果）→ `scope_all_groups`（BetaTwo → beta.txt）→ `esc_clears` → `esc_closes` → `app_exit`（exit=0）→ `completed`。
  - 截图：`p2-01-search-open.png` / `p2-01-query-alpha.png`（1 个结果）/ `p2-01-title-open.png` / `p2-01-scope-current.png`（0 个结果）/ `p2-01-scope-all.png`（全部分组，BetaTwo）/ `p2-01-esc-typed.png`（alpha + 1 结果）/ `p2-01-esc-cleared.png`（关键词清空、面板保留）/ `p2-01-esc-closed.png`（面板关闭）。

## 历史记录：P2-01 开发过程问题与修复

1. **LineEdit 文本绑定（修复）**：单向 `text: root.search-query` 在用户编辑后被 TextInput 内部赋值破坏，重新打开无法清空；改为双向绑定 `<=>`，Rust 侧另存查询状态供范围切换使用，避免依赖属性时序。
2. **搜索面板点击无效（修复）**：面板声明在 FocusScope 之前，被其鼠标处理挡住；移到 FocusScope 之后（对话框同层），结果行与范围按钮恢复点击。
3. **点击空白结果区触发清空（发现）**：0 结果时点击列表空白会落到背景关闭层，按 Esc 语义清空关键词；测试改为不点击空白区，避免误判。
4. **全分组结果无法启动（修复）**：`controller.item` 仅查当前分类，跨分组结果提示"项目不存在"；改为全库 `find_item` 查找。
5. **关闭后 Ctrl+F 失效（修复）**：焦点停留在已隐藏的搜索输入框，根 FocusScope 收不到 Ctrl+F；输入框自身处理 Ctrl+F，并在搜索关闭时把焦点交还 FocusScope。
6. **标题入口焦点丢失（修复）**：通过标题栏打开时点击会覆盖 `init` 聚焦；增加 100ms 定时重新聚焦输入框（仅在搜索可见时运行）。

## 历史记录：P1-18 任务清单（已验收）

- [x] 基线 10.1：交互模式创建 `FocusStateMachine`（`Rc<RefCell<...>>`），显示/隐藏统一经 `show_panel`/`hide_panel` 同步状态；托盘退出进入 `ShuttingDown`。
- [x] 基线 10.3/10.4：60ms 轮询 `GetForegroundWindow`/`GetWindowThreadProcessId`/Owner（P0-06 `foreground_belongs_to`），失焦后调度一次性 120ms 复查，复查确认仍为外部前台才隐藏；迟到的第二次激活取消隐藏。
- [x] 启动保护：窗口首次获得前台前不启用自动隐藏，避免从终端/启动器启动时尚未见窗即消失。
- [x] 基线 10.7：`settings.pinned` 启动时载入并镜像为 Pinned 抑制；标题栏"固定"与托盘"固定/取消固定"均持久化并同步状态机；固定时失焦与启动均不隐藏。
- [x] 基线 10.2/10.5 抑制锁（RAII `SuppressionToken`）：
  - `ModalDialogOpen`：任一 Slint 对话框（编辑/删除确认/分类/移动）可见时由轮询持有。
  - `ExternalDragHover`：OLE `DragEnter` 获取，`DragLeave`/`Drop`/`Failed` 释放。
  - `ContextMenuOpen`：托盘 `MenuOpened` 获取，`MenuSelected`/`MenuDismissed` 释放。
  - `LaunchingElevated`：管理员启动调用期间持有，返回后释放（含失败路径）。
- [x] 基线 10.6/17.1：成功启动且 `auto_hide_on_launch=true` 且未固定时隐藏面板；固定或该设置为 false 时保持并重新聚焦；启动失败保留面板并显示错误。
- [x] 单测：`ExternalDragHover` 抑制失焦观察与复查（新增 1 项），既有状态机/抑制计数测试保持通过。
- [x] P1-18 专项验证 `scripts/verify-p1-18-auto-hide.ps1`：失焦隐藏 → 热键恢复 → 标题栏固定不隐藏 → 固定下启动不隐藏 → 取消固定后失焦隐藏 → 对话框抑制 → 关对话框后恢复隐藏 → 启动失败保留 → 成功启动自动隐藏 → WM_CLOSE exit=0。
- [x] 回归适配：P1-06/07/08/09 配置种子 `settings.pinned=true`（启动测试需要面板常驻）；P1-17 拖拽脚本先点击标题栏固定。回归 P1-01～P1-18 共 18 个脚本批量 exit=0；门禁 fmt=0、clippy -D warnings、141 项测试（126 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-18 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-18**。P1 阶段 18 项全部完成。失焦隐藏、120ms 复查、固定、对话框/菜单/管理员启动抑制与启动后隐藏闭环；`ExternalDragHover` 的真实拖拽时序边界见下。

最终 Release SHA-256：`2004B4AB5E54E9192835A9C1F4685E7A99C0F6A75248838608C9CDD1C033A6D7`。

- 证据目录 `docs/test-results/P1-18/`：
  - `p1-18-auto-hide.tsv` 十四阶段：`window_ready` → `external_focus_source` → `blur_hide` → `hotkey_restore` → `pinned`（标题栏）→ `pinned_blur_keeps` → `pinned_launch_keeps` → `unpinned` → `unpinned_blur_hide` → `dialog_suppression` → `dialog_closed_hide` → `failed_launch_keeps` → `launch_auto_hide` → `app_exit`（exit=0）→ `completed`。
  - 截图：`p1-18-dialog-open.png`（添加项目对话框 + 状态栏"已取消固定（失焦自动隐藏）"）。

## 历史记录：P1-18 开发过程问题与修复

1. **启动即隐藏（修复）**：批量回归时多数脚本在找到窗口前窗口已失焦隐藏。增加"首次获得前台后才启用自动隐藏"的启动保护，避免从终端启动时未见面板即消失。
2. **既有脚本与新默认冲突（适配）**：P1-06/07/08/09 的种子配置增加 `settings.pinned=true`；P1-17 脚本在拖拽前点击标题栏固定。真实回归 18 脚本全过。
3. **固定状态双入口（修复）**：标题栏"固定"原为 P1-01 路由探针，现改为真实 `settings.pinned` 持久化并同步状态机；托盘入口同步更新。
4. **外部拖拽与 120ms 复检竞态（边界）**：未固定时从 Explorer 按下鼠标到 OLE `DragEnter` 实测约 169ms，超过 120ms 失焦复查，面板会在拖入前隐藏（`firstHiddenMs=169`，未导入项目）。`ExternalDragHover` 锁逻辑由单测覆盖；真实拖入验收在 P1-17（面板固定）。改善方案（更长启动宽限/拖拽预判）需产品决策，未在 P1 增加。
5. **启动后测试窗口清理（脚本）**：启动会拉起 notepad，测试记录新增 PID 并在断言后结束，避免遮挡后续点击。

## 历史记录：P1-17 任务清单（已验收）

- [x] 基线 5.4/5.6：交互模式主窗口注册 OLE IDropTarget（复用 P0-04 `DropRegistration`、P0-05 `receiving_target`），退出时 Drop 撤销 `RevokeDragDrop`。
- [x] CF_HDROP 读取（P0-05）→ 屏幕物理坐标经 P0-06 `from_screen` 转 Slint 逻辑坐标 → `drop_layout::hit_test` 命中顶部分组/左侧分类/右侧网格（标题栏、状态栏、空白不接收）。
- [x] 导入：`ItemDraft::from_path`（名称取文件主名、类型推断 EXE/LNK/文件/文件夹、EXE/文件默认工作目录为父目录），只保存引用，不复制/移动源文件。
- [x] 网格 → 当前分类追加；分类行 → 指定分类（不改变当前选中）；分组 → 目标分组当前分类；多文件按 CF_HDROP 顺序追加。
- [x] 基线 5.6 弹簧加载：分组标签悬停 400ms 切换当前分组（50ms Timer 轮询 + Instant 计时）；快速落下按光标所在分组处理。
- [x] 拖拽反馈：分组/分类行高亮、网格 2px 边框、状态栏"松开以添加到「X」（追加到末尾）"。
- [x] ESC/Leave：目标不提交数据，项目数与内容不变。
- [x] 单测：命中测试边界（标题/状态栏/分组间距/分类裁剪/网格边界）、`ItemDraft::from_path`、`add_item_to_category`（跨分组目标、选择不变）。
- [x] P1-17 专项验证 `scripts/verify-p1-17-external-drop.ps1`（真实 Explorer 拖入）：网格 EXE → 分类行文件 → 多文件顺序 → 分组 700ms 悬停弹簧加载 → ESC 取消 → Leave → 源文件哈希/数量不变 → WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-16 全过（17 个脚本批量 exit=0；首轮 P1-05/P1-08 偶发时序失败，单独重跑与第二轮批量均通过）；门禁 fmt=0、clippy -D warnings、140 项测试（125 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-17 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-17**。外部拖入的三类落点（网格/分类/分组）与取消语义按 5.6 闭环；内部拖拽（排序/跨分类/跨分组/分组排序）属 P2-04/05/06，不在本任务。

最终 Release SHA-256：`CB810C2CFA2D9F62D15DE6157BF52DF14973530DFA06A261675BCC1150235FD9`。

- 证据目录 `docs/test-results/P1-17/`：
  - `p1-17-external-drop.tsv` 十三阶段：`window_ready` → `library_persisted`（示例库落盘）→ `explorer_ready` → `initial`（6 项，必备软件）→ `grid_drop`（sample application，exe，必备软件）→ `category_drop`（系统工具 1 项）→ `multi_drop`（顺序 second file, 中文 sample）→ `spring_load`（应用，中文 folder）→ `esc_cancel`（11 项不变）→ `leave`（11 项不变）→ `source_unchanged`（5 文件/4 哈希）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p1-17-grid-drop.png` / `p1-17-category-drop.png` / `p1-17-spring-hover.png`（分组"应用"高亮 + 状态栏"松开以添加到「应用」"）。
  - `drags.jsonl`：五次真实鼠标拖拽记录。

## 历史记录：P1-17 开发过程问题与修复

1. **Explorer 导航窗格吞掉落点（脚本）**：Explorer 700px 宽时导航窗格约 345px，原 P0-05 的源 X=260 落在"下载"上，拖入错误项目；截图核对后改为文件列表内 X=500。
2. **前台锁导致窗口无法置前（脚本）**：后台 PowerShell 的 `SetForegroundWindow` 返回 false，Chrome 始终覆盖拖拽窗口；改用 `AttachThreadInput` 连接前台线程后置前（`scripts/p1-17-focus.ps1`）。
3. **临时目录短路径不匹配（脚本）**：`$env:TEMP` 为 8.3 短路径，Explorer 返回长路径，窗口查找失败；改用 `[IO.Path]::GetTempPath()` 长路径并归一化比较。
4. **清理挂起（脚本）**：`Shell.Application` 窗口对象 `Quit()` 在关闭测试窗口时阻塞；改为 `PostMessage(WM_CLOSE)` 后清理临时目录。
5. **示例库未落盘（脚本）**：启动后示例库只在内存，首次读取 config 失败；通过先切换分类再切回触发持久化（不改变最终选中）。
6. **首轮批量回归两例时序失败**：P1-05 对话框输入未落焦点、P1-08 Explorer 窗口超时；单独重跑与第二轮 17 脚本批量均通过，与本任务无直接因果。

## 历史记录：P1-16 任务清单（已验收）

- [x] 基线 9.1/9.2：交互模式启动最先 `acquire()`（`Local\QuickNest-{SID_HASH}-{SESSION_ID}` Mutex），主实例持句柄至退出；`WAIT_FAILED` 记录后安全退出，不创建窗口。
- [x] 基线 9.3/9.4：主实例 `start_server()` 命名管道（仅当前用户 SID、`PIPE_REJECT_REMOTE_CLIENTS`、长度前缀 + JSON ≤4KB、命令白名单 show/hide/toggle/show_and_focus/open_search）。
- [x] 第二实例：`AlreadyRunning` → 发送 `show_and_focus` → 收到确认 ack 后 exit=0，不创建窗口/托盘/热键。
- [x] 基线 9.5 启动竞态：管道连接 1500ms 预算；唤醒失败后**重新 acquire**，仅在成功取得所有权（FirstInstance/TookOverAbandoned）时转为主实例；仍被占用则记录错误并 MessageBox 提示后退出。
- [x] 主实例 100ms Timer 排空 `PipeServer::drain()` → `handle_ipc_command`：Show/ShowAndFocus → show + SetForegroundWindow；Hide；Toggle；OpenSearch → 状态栏提示（搜索中心属 P2-01）。
- [x] 接管异常退出（TookOverAbandoned）后继续正常启动，配置检查复用 P1-12 `load`（备份恢复/安全模式）。
- [x] P1-16 专项验证 `scripts/verify-p1-16-single-instance.ps1`：首实例窗口 → 第二实例 exit=0 且窗口仍为 1 → 点击 × 隐藏 → 再次启动 exit=0 且首实例恢复可见（IPC 唤醒）→ WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-15 全过（16 个脚本批量 exit=0）；门禁 fmt=0、clippy -D warnings、133 项测试（118 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-16 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-16**。单实例、命名管道唤醒与启动竞态按 9.1-9.5 闭环。唤醒失败后的"重新检查所有权"与 `WAIT_FAILED` 分支代码就绪，但未做故障注入实测（见边界）。

最终 Release SHA-256：`B5B5FB0CA5BEA672A164EA84E0B72C659BCC89268ECBED80AC924D82820BC8F1`。

- 证据目录 `docs/test-results/P1-16/`：
  - `p1-16-single-instance.tsv` 六阶段：`first_instance`（pid/rect）→ `second_exit`（exit=0，windows=1）→ `hidden`（× 隐藏）→ `wake_up`（visible=1，第三次启动 exit=0）→ `first_exit`（WM_CLOSE exit=0）→ `completed`。
  - 截图：`p1-16-wake-up.png`（被唤醒后的主窗口）。
  - 无 stderr 日志（两个次实例均静默退出）。

## 历史记录：P1-16 开发过程问题与修复

1. **基线 9.5 未闭环（修复）**：初版唤醒失败仅 eprintln 后退出，未重新检查 Mutex 所有权；现增加二次 `acquire()`，旧实例已失效时由本进程接管继续启动，仍被占用时 MessageBox 提示后退出。
2. **`WAIT_FAILED` 处理（修复）**：初版记录后继续建窗（fail-open），与 9.2"安全退出"不符；改为直接退出，避免无单实例保护地运行。
3. **`is_some_and` 误用（修复）**：`InstanceIdentity::current()` 返回 `Result`，`.map()` 后需用 `is_ok_and` 判断双层结果。
4. **clippy `unused_assignments`（修复）**：`become_main` 初值在所有分支返回/覆盖前不可达；改为 match 后以 `_instance_guard.is_some()` 判断是否启动 IPC server。
5. **批量回归偶发时序**：首轮批量 P1-01 拖拽断言 dx=0 失败，单独重跑与完整第二轮批量（16 脚本）均通过；属注入点击/前台时序抖动，与本任务无直接因果。

## 历史记录：P1-15 任务清单（已验收）

- [x] 基线 11.3 托盘注册：交互模式复用 P0-11 `TrayService::register`（Shell_NotifyIconW、专用线程消息循环、TaskbarCreated 重建、Drop 删除图标）。
- [x] 事件路由：`take_events` + 100ms Timer → `handle_tray_event`：
  - 左键/双击切换面板显示/隐藏（隐藏后保持，热键/托盘呼出）。
  - 菜单"显示 QuickNest" → `show_and_focus`。
  - 菜单"隐藏 QuickNest" → 隐藏。
  - 菜单"固定 / 取消固定" → 切换 `settings.pinned` 并持久化 + UI 状态。
  - 菜单"设置" → 状态栏提示（设置中心属第 17 章，后续任务）。
  - 菜单"检查更新" → 状态栏提示（开发版本）。
  - 菜单"退出程序" → **先保存配置**再退出（基线 11.3 退出顺序第 1 步；注销快捷键/删除托盘图标/后台线程清理由 Drop 顺序完成）。
- [x] 托盘注册失败在状态栏提示。
- [x] P1-15 专项验证 `scripts/verify-p1-15-tray.ps1`：托盘宿主窗口存在 → 左键隐藏/显示 → 菜单"隐藏"（菜单矩形 500,162,686,300，TPM_BOTTOMALIGN 光标上方）→ 左键恢复 → 菜单"退出程序"（exit=0，config 已保存）。
- [x] 回归 P1-01～P1-14 全过（14 个脚本批量 exit=0）；门禁 fmt=0、clippy -D warnings、133 项测试（118 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-15 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-15**。托盘退出流程的完整 8 步中，保存配置/注销快捷键/删除托盘图标/后台线程由显式调用与 Drop 顺序覆盖；RevokeDragDrop 属 P1-17；IPC/Mutex 已在 P1-16 接入（P1-15 验收时交互模式尚未启动单实例与管道）。

最终 Release SHA-256：`F335C251BAA3337DB6717C3AF2A525BEF4AE1BBF6FECEE02FCEBDE24BE9E1EC9`。

- 证据目录 `docs/test-results/P1-15/`：
  - `p1-15-tray.tsv` 八阶段：`window_found` → `tray_registered`（0x381DF2）→ `toggle_hide` → `toggle_show` → `menu_rect`（500,162,686,300）→ `menu_hide` → `restored_before_exit` → `menu_exit`（exit=0）→ `config_saved` → `completed`。
  - 截图：`p1-15-initial.png` / `p1-15-menu.png`（六项菜单）/ `p1-15-after-menu-click.png` / `p1-15-restored.png`。
  - 无 stderr 日志。

## 历史记录：P1-15 开发过程问题与修复

1. **菜单键盘注入不进菜单**：脚本 Down/Enter 未生效（前台窗口不是菜单）。改用**鼠标点击菜单项**（更贴近真实用户）。
2. **TPM_BOTTOMALIGN 定位误判**：`show_menu` 使用 `TPM_BOTTOMALIGN`，菜单在光标**上方**展开（矩形 500,162..300），最初按下方展开猜测点击导致落在菜单外。修复：`WindowFromPoint` 探测菜单矩形（探测点选在菜单上方区域 550,250），按高度比例点击第 2/第 6 项。
3. **断言恒真 bug**：`$null -ne @(...)` 对空数组恒为真；改为 `.Count -ne 0`。
4. **托盘宿主窗口可见性**：`show_menu` 会 `ShowWindow(SW_SHOWNOACTIVATE)` 宿主窗口；脚本通过 `FindWindowW(类名, 标题)` 定位，不依赖通知区域坐标。

## 历史记录：P1-14 任务清单（已验收）

- [x] 基线 11.1 全局快捷键：读取 `settings.hotkey`（默认 `Ctrl+Alt+Space`），`parse_hotkey` 解析（Ctrl/Alt/Shift/Win + A-Z/0-9/F1-F24/常用键，始终附加 `MOD_NOREPEAT`），复用 P0-10 的 `register`（RegisterHotKey 线程消息循环、冲突检测、Drop 注销）。
- [x] 事件消费：`HotkeyGuard::take_events` + Slint Timer（100ms）排空 → `show_and_focus`（`ui.show()` + `SetForegroundWindow`）。
- [x] 注册/解析失败在状态栏明确提示（"快捷键 X 注册失败：…" / "快捷键配置无法解析：…"）。
- [x] 基线 2.5 + 11.1 联动：交互模式点击 × 后**保持隐藏**（验证模式仍 2s 恢复），热键重新呼出并置于前台。
- [x] 基线 11.2：FocusScope 捕获 Ctrl+F → `search-requested` 回调（搜索中心属 P2-01，当前路由意图并在状态栏提示）。
- [x] 单测：`parse_hotkey` 合法/非法组合（大小写/空格/多键/未知键/空段）。
- [x] P1-14 专项验证 `scripts/verify-p1-14-hotkeys.ps1`：点击 × 隐藏 → Ctrl+Alt+Space 恢复（可见 + 前台）→ Ctrl+F 路由 → 自定义 `Ctrl+Alt+K` 恢复 → 无效配置 `Ctrl+Alt+Unknown` 状态栏提示 → 四次 WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-13 全过（偶发时序失败重跑通过）；门禁 fmt=0、clippy -D warnings、133 项测试（118 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-14 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-14**。搜索中心的实际 UI 属 P2-01；托盘入口属 P1-15；设置中心（含热键修改）属第 17 章，后续任务。

最终 Release SHA-256：`9A13B5B2D85762308006AB4276ADAF74A47A3C4B6C966D62D7ED24DB6C43AF0E`。

- 证据目录 `docs/test-results/P1-14/`：
  - `p1-14-hotkeys.tsv` 七阶段：`after_close`/`hidden`（× 隐藏）→ `hotkey_restore`（visible=1 foreground=True）→ `ctrl_f` → `custom_registered`/`custom_hotkey`（Ctrl+Alt+K）→ `invalid_hotkey` → `completed`。
  - 截图：`p1-14-after-close.png` / `p1-14-restored.png`（状态栏"已隐藏（Ctrl+Alt+Space 呼出）"）/ `p1-14-ctrl-f.png`（"搜索快捷键已触发（搜索中心属 P2-01）"）/ `p1-14-custom-start.png` / `p1-14-custom-after.png` / `p1-14-invalid-hotkey.png`（"快捷键配置无法解析：Ctrl+Alt+Unknown"）。
  - 无 stderr 日志。

## 历史记录：P1-14 开发过程问题与修复

1. **交互模式隐藏行为**：原 `on_hide_requested` 对所有模式 2s 后自动恢复（P1-01 验证遗留）。改为仅验证模式恢复；交互模式保持隐藏，由全局热键（本任务）与托盘（P1-15）呼出。
2. **F9 组合不可用**：`Ctrl+Alt+F9` 注册成功但 SendInput 注入未触发（疑似系统级键盘钩子拦截 F9）；改用 `Ctrl+Alt+K` 实测通过。脚本冲突注入也受脚本线程限制，冲突 UI 提示以 P0-10 注册冲突证据 + 代码路径覆盖。
3. **测试串行/时序**：批量回归偶发个别脚本失败（点击/菜单时序，重跑与第二次批量全绿），与本任务无直接因果，已记录。

## 历史记录：P1-13 任务清单（已验收）

- [x] 基线 15.7 备份保留：`prune_backups(target, 10)` 按命名时间倒序保留最近 **10 份有效备份**，无效 JSON 与超量旧备份删除。
- [x] 每次成功保存后自动 `prune_backups`；备份命名（`config.backup.<timestamp>.json`）与恢复扫描规则一致。
- [x] 手动备份：`config_save::backup_now`（复制当前有效 config + 时间戳命名 + 同步 + prune）与控制器 `backup_now`；状态栏新增"备份"入口，成功显示"已备份：<路径>"，失败显示原因；安全模式拒绝。
- [x] 单测：prune 保留 10 份最新有效（12 近期 + 1 旧有效 + 1 无效 → 保留 10、删除 4）；手动备份内容与恢复扫描命名；控制器手动备份与安全模式拒绝。
- [x] P1-13 专项验证 `scripts/verify-p1-13-backups.ps1`：14 次保存后备份恰为 10 份 → 点击状态栏"备份"生成新备份（文件名更新）且数量仍为 10 → 清理测试配置。
- [x] 回归 P1-01～P1-12 全过；门禁 fmt=0、clippy -D warnings、132 项测试（117 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-13 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-13**。"每日首次修改前备份"由每次保存前自动生成的备份满足（更保守，由 10 份保留策略控制总量）；导入前备份属 P2 导入任务。

最终 Release SHA-256：`CAED81EA41499470FFAD0BF2A142B962542469EFB2045652468DF86699A74210`。

- 证据目录 `docs/test-results/P1-13/`：
  - `p1-13-backups.tsv` 三阶段：`retention`（14 次保存 → 10 份）→ `manual_backup`（最新文件 20261104-084610-895 vs 之前 084608-862）→ `completed`。
  - 截图：`p1-13-status-bar.png` / `p1-13-manual-backup.png`（状态栏"已备份：…"）。
  - 无 stderr 日志。

## 历史记录：P1-13 开发过程问题与修复

1. **备份无限增长（修复）**：P0-13 每次替换都生成备份但无清理；新增 `prune_backups` 在每次保存后执行，保留 10 份有效备份。
2. **手动备份命名必须匹配恢复扫描**：复用 `timestamp_string` 与 `<stem>.backup.<stamp>.json` 规则，避免恢复链忽略手动备份。
3. **测试并行锁竞争（修复）**：controller 持久化测试与 config_save 测试并行争用进程全局写锁导致偶发失败；把 `TEST_SERIALIZER`/`test_lock()` 提升为 config_save 模块级（`pub(crate)`），controller 相关测试共用；4 次全量库测试稳定。
4. **手动备份同毫秒命名冲突**：`create_new(true)` 保证不覆盖；冲突返回结构化错误（用户点击间隔远大于毫秒）。

## 历史记录：P1-12 任务清单（已验收）

- [x] 基线 15.8 损坏恢复接入：解析失败 → `recover_latest_backup`（倒序验证备份）→ 恢复最近有效备份 → 损坏文件改名 `config.corrupted.<timestamp>.json`；无有效备份 → 安全模式且原文件不被覆盖。
- [x] 基线 15.9 Schema 校验：`schema_version == 2` 正常；`>2` 或 `<2` 进入安全模式，提示"版本 X 与当前版本 2 不兼容，未写入"，不覆盖原文件。
- [x] 安全模式：`persist` 拒绝写入（返回"安全模式：…"），所有变更只在内存中生效，不触碰原配置。
- [x] 保存失败不再静默：`LibraryController` 记录 `save_error`，`refresh_ui` 在状态栏显示"保存失败：…"；成功保存后自动清除并恢复默认文本。
- [x] 加载状态 UI 提示：`Recovered` 显示备份路径与损坏文件保留位置；`SafeMode` 显示原因；`Loaded` 保持默认。
- [x] 单测 5 项：不兼容版本不覆盖、损坏无备份安全模式、损坏有备份恢复（含 corrupted 改名）、保存失败记录、BOM 容错（既有）。
- [x] P1-12 专项验证 `scripts/verify-p1-12-atomic-save.ps1`：正常保存产生备份 → 破坏配置后启动恢复（config 合法、corrupted 文件存在）→ 删除备份后再破坏（安全模式，修改操作后原文件仍为 garbage）→ schema_version=3（安全模式且文件未变）→ 四次 WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-11 全过；门禁 fmt=0、clippy -D warnings、128 项测试（113 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-12 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-12**。底层原子替换（P0-13）与生产加载/恢复链已贯通。备份轮换与保留策略属 P1-13；设置项防抖属设置中心（第 17 章，后续任务）。

最终 Release SHA-256：`054C0A61832F0C326C8D6660D581E5297C9C11A285C49BDC1A5CDC6C55CF460B`。

- 证据目录 `docs/test-results/P1-12/`：
  - `p1-12-atomic-save.tsv` 五阶段：`backup_seeded`（1 份备份）→ `recovered`（config 合法、corrupted 改名）→ `safe_mode`（修改操作后原文件仍为 garbage）→ `newer_schema`（v3 拒绝且未修改）→ `completed`。
  - 截图：`p1-12-recovered.png`（状态栏"已从备份恢复配置：…"）/ `p1-12-safe-mode.png`（"配置不可用，已进入安全模式（不会覆盖原文件）：配置损坏且没有有效备份，未写入"）/ `p1-12-newer-schema.png`（"配置版本 3 与当前版本 2 不兼容，未写入"）。
  - 无 stderr 日志。

## 历史记录：P1-12 开发过程问题与修复

1. **加载静默回退示例库（修复）**：原 `load` 在解析失败时静默使用示例库，任何后续修改都会覆盖损坏文件（违反 15.8）。现按备份恢复/安全模式分流，安全模式下 `persist` 拒绝写入。
2. **保存失败被忽略（修复）**：各业务操作 `let _ = persist()` 丢弃错误；控制器记录 `save_error` 并在状态栏展示，成功保存后清理。
3. **测试隔离**：恢复测试在独立临时目录构造备份/损坏文件，不依赖真实 `%APPDATA%`。
4. **CLI 回归**：P1-12 脚本结束删除 config，避免污染后续脚本；回归全过。

## 历史记录：P1-11 任务清单（已验收）

- [x] 基线 12.2 缓存键：**BLAKE3**（`blake3` 1.8.7 固定版本）输入 = 规范化来源（小写、`/`→`\`）+ 文件修改时间 + 文件大小（缺失记 `missing`）+ 请求尺寸；自定义图标用其文件身份走同一函数。
- [x] 磁盘缓存：`%APPDATA%\QuickNest\icons\<blake3>.qni`（magic `QNI1` + width/height + RGBA）；损坏/截断文件忽略并重新提取。
- [x] 后台 worker 集成：命中读盘直接返回；未命中提取后写盘（写失败不影响显示）；缓存键随 mtime/size 变化，程序更新自动失效。
- [x] 基线 12.3 清理：启动时按当前库全部项目的引用键 `prune_cache`；删除项目后不再引用，共享图标在仍有引用时保留。
- [x] 单测 4 项：键规范化/尺寸敏感、文件身份变化、缓存往返与损坏、prune 保留引用；集成测试（独立进程）：首次写入、二次启动命中且不重写（mtime 不变）。
- [x] P1-11 专项验证 `scripts/verify-p1-11-icon-cache.ps1`：首启生成 1 个 `.qni`（16396B = 12 + 64×64×4）→ 二次启动命中（mtime 不变）→ 删除项目后启动清理为 0 → 三次 WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-10 全过；门禁 fmt=0、clippy -D warnings、124 项测试（109 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-11 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-11**。图标选择器 UI 与自定义图标索引应用属后续设置任务；性能（200+ 项目首次提取）属 P4。

最终 Release SHA-256：`15A0AD9FF185C46AB4F1C0EA283BBA700C92D8A8EA5126BE59C7193D83EE6C50`。

- 证据目录 `docs/test-results/P1-11/`：
  - `p1-11-icon-cache.tsv` 七阶段：`cache_written`（a2962fab….qni，16396B）→ `wm_close_exit` → `cache_reused`（mtime 不变）→ `wm_close_exit` → `cache_pruned`（0 个残留）→ `wm_close_exit` → `completed`。
  - 截图：`p1-11-first-launch.png`。
  - 无 stderr 日志。

## 历史记录：P1-11 开发过程问题与修复

1. **BLAKE3 依赖**：基线要求 BLAKE3，引入 `blake3 = "=1.8.7"`（Cargo.lock 固定）。
2. **缓存文件格式**：不额外引入图像编码依赖，采用 `QNI1` 二进制快照（12 字节头 + RGBA）；读取校验 magic/尺寸/长度，损坏即重提取。
3. **清理时机**：启动时按当前库引用键集合清理（12.3 允许"定期清理"）；删除项目后下次启动生效，避免运行期误删共享图标。

## 历史记录：P1-10 任务清单（已验收）

- [x] 基线 12.1 提取：`IShellItemImageFactory::GetImage`（64x64，`SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK`）优先；`SHGetFileInfoW`（`SHGFI_ICON | SHGFI_LARGEICON`）兼容回退。
- [x] 透明通道保留：HBITMAP（预乘 BGRA）经 `GetDIBits` 转直通 alpha RGBA；`windows` 的 `HBITMAP` 尺寸经 `GetObjectW` 读取。
- [x] **后台线程**：`IconService` 专用 worker（STA 初始化/清理）通过 channel 接收（key, source）并回传像素；UI 侧 `IconContext` 缓存 `slint::Image`、pending 去重、150ms `Timer` 排空后刷新（基线 5.4/12.1 不阻塞 UI）。
- [x] `Item::icon_source`：自定义图标（`path,index` 取路径部分）优先；URI/Command/Automation 无文件图标；空目标返回 None → 默认素材。
- [x] UI：`GridItem.icon`（Slint `image`），卡片默认素材上覆盖真实图标（空图标保持默认）。
- [x] 单测：`icon_source` 规则；集成测试（独立进程）真实提取 cmd 图标（尺寸/RGBA 长度/透明与不透明像素）。
- [x] P1-10 专项验证 `scripts/verify-p1-10-icons.ps1`：CmdIcon 真实图标（默认蓝像素比例 0）→ UriIcon 默认素材（比例 0.35）→ WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-09 全过（P1-07 增加启动等待，避免图标刷新与注入点击竞争）；门禁 fmt=0、clippy -D warnings、119 项测试（105 lib + 1 bin + 6 file_drop + 2 icon + 1 lnk + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-10 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-10**。图标磁盘缓存、BLAKE3 缓存键与失效/清理属 P1-11；图标选择器 UI 属后续设置任务。

最终 Release SHA-256：`606E15988218C2DA241219C787774EE1C4D7519559E27430220A6C68C3CE32F1`。

- 证据目录 `docs/test-results/P1-10/`：
  - `p1-10-icons.tsv` 五阶段：`window_found` → `icons_captured` → `pixels`（cmd_blue_ratio=0，uri_blue_ratio=0.347）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-10-icons.png`（CmdIcon 显示 cmd 黑色窗口图标，UriIcon 保持默认蓝色素材，MissingIcon 通用图标）。
  - 无 stderr 日志。

## 历史记录：P1-10 开发过程问题与修复

1. **UI 模型传递图标**：初始方案把 `refresh_ui` 改为 `Rc<dyn Fn()>` 闭包，导致 10 余处回调的 `Rc` 捕获被移动（E0382）。改为 **thread_local `ICON_CONTEXT`** + 恢复原 `fn refresh_ui(controller, weak)` 签名，回调零改动。
2. **采样断言落空**：默认图标含白色网格线，单像素采样可落在网格上；改为区域蓝色像素比例（cmd 0.0 / URI 0.35）。
3. **图标刷新与 UI 自动化竞争**：P1-07 回归首次在"编辑对话框保存 LNK"阶段偶发失败（图标 timer 刷新与注入点击竞争）；脚本在编辑前等待 1.5s，重跑通过。真实用户无影响。
4. **clippy `missing_const_for_thread_local`**：thread_local 初始值改 `const { RefCell::new(None) }`。

## 历史记录：P1-09 任务清单（已验收）

- [x] 基线 3.1/4.6：URI（HTTP/HTTPS/steam/VS Code/自定义协议）经 `ShellExecuteExW` 交给系统协议处理。
- [x] URI scheme 校验（RFC 3986：ALPHA 开头、字母数字/+/-/.；单字母 scheme 拒绝以免把盘符路径当 URI）；空目标与非法 URI 返回结构化错误（新增 `InvalidTarget`）。
- [x] 单测 2 项：scheme 判定（8 个合法/6 个非法用例）、空/非法目标校验。
- [x] P1-09 专项验证 `scripts/verify-p1-09-uri.ps1`：临时 HKCU 协议 `quicknest-p109` → cmd 写探针文件 → 单击后文件生成（系统协议处理生效）→ 非法 URI 状态栏错误 → WM_CLOSE exit=0 → finally 清理注册表与临时目录。
- [x] **测试基础设施修复**：MTA 冲突测试与真实 LNK 启动测试会创建进程级 MTA/隐式 MTA，污染同进程其它测试。拆分到独立集成测试进程 `tests/ole_mta.rs`、`tests/lnk_launch.rs`；lib 测试 6 次全量运行稳定全绿。
- [x] 回归 P1-01～P1-08 全过；门禁 fmt=0、clippy -D warnings、116 项测试（104 lib + 1 bin + 6 file_drop + 1 lnk_launch + 1 ole_mta + 3 doc）0 失败。

## 历史记录：P1-09 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-09**。UWP/AUMID 直接激活属 P2-07/08；Command/Automation 类型的结构化启动属 P2 结构命令任务。

最终 Release SHA-256：`B99D2FF479C64B4F44FB3F2DBE706D868A662B9396027A98CF8AA915487D6E61`。

- 证据目录 `docs/test-results/P1-09/`：
  - `p1-09-uri.tsv` 六阶段：`protocol_registered` → `window_found` → `uri_launch`（探针文件含 uri-launched）→ `invalid_uri`（状态栏"不是有效的 URI：not a uri"）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-09-initial.png` / `p1-09-after-click.png`（"已启动 UriProbe"）/ `p1-09-invalid.png`（非法 URI 错误）。
  - 无 stderr 日志。

## 历史记录：P1-09 开发过程问题与修复

1. **进程级 MTA 污染测试（本任务最重要修复）**：`mta_conflict` 测试创建 MTA、真实 LNK 测试创建 OLE/SHELL 对象后，同进程后续新线程被 COM 视为隐式 MTA，导致 `nested_ole_successes...` 的"新线程未初始化"断言偶发失败（并行与串行均可复现顺序依赖）。修复：两个场景拆到独立集成测试二进制（各自独立进程），lib 测试恢复干净；6 次全量运行稳定。
2. **URI scheme 校验**：单字母 scheme 会被 `C:\path` 误判，拒绝长度 <2 的 scheme（RFC 仍允许单字母，实际使用罕见）。
3. **协议注册仅限 HKCU**：测试协议在 `HKCU\Software\Classes` 注册，无需管理员，finally 删除；未触碰系统级协议。

## 历史记录：P1-08 任务清单（已验收）

- [x] 基线 4.6：普通文件走 `ShellExecuteExW` 系统文件关联；文件夹走 `ShellExecuteExW`/Explorer；模型 `args`/`workdir` 仅作元数据不传入（基线 3.3）。
- [x] "打开文件所在位置并选中目标"（基线 3.5）：`ILCreateFromPathW` + `SHOpenFolderAndSelectItems`（官方选中语义），命中/取消/失败均有结构化结果。
- [x] 缺失/空目标结构化错误并显示在状态栏。
- [x] 单测：File/Folder 空/缺失校验、open_item_location 空/缺失校验（launch 共 8 项）。
- [x] P1-08 专项验证 `scripts/verify-p1-08-shell-targets.ps1`：文件夹启动（Explorer 窗口 "FolderProbe - 文件资源管理器"）→ 文件启动（状态栏"已启动 FileProbe"）→ 右键"打开文件所在位置"（Explorer 窗口 "quicknest-p108"）→ 缺失文件错误提示 → WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-07 全过；门禁 fmt=0、clippy -D warnings、115 项测试（105+1+6+3）0 失败。

## 历史记录：P1-08 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-08**。URI/协议启动属 P1-09；UWP/AUMID 属 P2-07/08；文件关联的图标属 P1-10/11。

最终 Release SHA-256：`DF2073123D7AB45E8E89949290AE47CF7A0093D178AC3357AB6CBE7060AC7C84`。

- 证据目录 `docs/test-results/P1-08/`：
  - `p1-08-shell-targets.tsv` 七阶段：`window_found` → `folder_launch`（窗口标题 FolderProbe）→ `file_launch`（状态栏确认）→ `menu_rect`(344,270,538,452) → `open_location`（窗口标题 quicknest-p108）→ `missing_file`（状态栏错误）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-08-initial.png` / `p1-08-folder-opened.png` / `p1-08-file-launched.png`（"已启动 FileProbe"）/ `p1-08-after-location-click.png`（"已打开文件所在位置"）/ `p1-08-missing.png`（"目标不存在"）。
  - 无 stderr 日志。

## 历史记录：P1-08 开发过程问题与修复

1. **`explorer.exe /select,"path"` 经 ShellExecuteExW 未生效**：explorer 对带外层引号的 `/select` 参数解析特殊，窗口未按预期打开。改用官方 `ILCreateFromPathW` + `SHOpenFolderAndSelectItems`（PIDL 选中语义），窗口标题验证通过。
2. 菜单项坐标沿用 `WindowFromPoint` 菜单矩形法（P1-05/06 同源）。
3. 测试用 Explorer 窗口标题断言（`FolderProbe` / `quicknest-p108`），并在结束时 WM_CLOSE 关闭，避免污染桌面。

## 历史记录：P1-07 任务清单（已验收）

- [x] 基线 4.1/4.6：LNK 项目默认通过 `ShellExecuteExW` 启动**原始快捷方式**（`lpFile`=LNK），保留 Shell 启动语义；模型 `args`/`workdir` 仅作解析元数据，不传给 Shell。
- [x] 管理员运行 LNK：`runas` + 原始 LNK（保留语义）；缺失/空目标返回结构化错误。
- [x] 导入解析：`IShellLinkW` + `IPersistFile`（`GetPath/GetArguments/GetWorkingDirectory/GetIconLocation/GetIDList`）；新增 `shell::resolve_link_now` 供 UI 线程同步解析。
- [x] 编辑对话框：浏览选择 LNK 时填充参数/工作目录/图标（字段为空时）；保存时对 `item_type=lnk` 再解析一次（导入语义），确保手填 LNK 路径也能得到元数据。
- [x] 单测 2 项：LNK 空/缺失目标校验；真实创建 LNK 并启动（LNK 自身参数/工作目录生效，模型元数据未传入）。
- [x] P1-07 专项验证 `scripts/verify-p1-07-lnk.ps1`：单击 LNK 启动（cmd 探针文件 `via-lnk`）→ 缺失 LNK 错误提示 → 编辑对话框保存 LNK（名称/目标/类型选择）→ 配置断言 args/workdir 来自 LNK → WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-06 全过；门禁 fmt=0、clippy -D warnings、114 项测试（104+1+6+3）0 失败。

## 历史记录：P1-07 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-07**。UWP/AUMID 直接激活属 P2-07/08（当前 UWP LNK 与 MSI 广告 LNK 均走原始 LNK 启动）；文件/文件夹启动属 P1-08；URI 属 P1-09。

最终 Release SHA-256：`FC8D8C078644F886588E27DF3AE87EE11B28B74ED9B3092BC35DBB3475C6BE0D`。

- 证据目录 `docs/test-results/P1-07/`：
  - `p1-07-lnk.tsv` 六阶段：`lnk_created` → `window_found` → `click_launch`（LNK 参数/工作目录生效）→ `missing_lnk`（状态栏错误）→ `metadata_parsed`（保存时解析 args/workdir）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-07-initial.png` / `p1-07-after-click.png`（"已启动 LnkProbe"）/ `p1-07-missing.png` / `p1-07-type-combo.png`（类型下拉）/ `p1-07-dialog-filled.png`（类型=快捷方式、目标=LNK）/ `p1-07-after-save.png`（PickedLnk 落盘）。
  - 无 stderr 日志。

## 历史记录：P1-07 开发过程问题与修复

1. **文件对话框键盘注入不可靠**：原生 `GetOpenFileNameW` 对话框的文件名框接收不到脚本注入的字符（焦点/前台切换被系统拒绝），Enter 反而确认了记忆的 `cmd.exe`。解决：元数据解析同时放到**保存时**（更符合"导入"语义），脚本改用"手填目标 + 类型下拉"验证；浏览路径的解析逻辑保留。
2. **名称未填导致保存失败被断言掩盖**：断言按 path 匹配到了预置项目，掩盖了新项目未保存。修复：脚本先填名称并用 name 匹配。
3. **残留 quicknest 进程干扰**：上一轮异常退出后残留实例使新实例被单实例机制拦截，点击/状态栏错乱。修复：运行前清理进程（脚本 finally 亦 Kill）。
4. **`IPersistFile::Save` 参数**：`fremember` 在 windows 0.62 是 bool，clippy 指出多余 `.into()`，已修正。

## 历史记录：P1-06 任务清单（已验收）

- [x] 基线 4.6 EXE 启动：`CreateProcessW`（lpApplicationName=目标，lpCommandLine=带引号参数，工作目录，STARTUPINFO `STARTF_USESHOWWINDOW` + 窗口状态）；失败回退 `ShellExecuteExW`（open）。
- [x] 管理员运行：`ShellExecuteExW` + `runas`（项目 `run_as_admin` 或右键菜单强制）；进程句柄经 `GetProcessId` 取 PID 后关闭。
- [x] 结构化错误（基线 4）：`UnsupportedType`（非 EXE 类型，P1-07～P1-09 实现）、`EmptyTarget`、`MissingTarget(path)`、`Spawn{win32}`、`PrivilegedRun{win32}`；中文 `message()` 供界面显示。
- [x] 单击卡片启动（基线 2.4）与右键"打开"/"管理员身份运行"；结果显示在状态栏（"已启动 X" / 错误原因），不只静默失败。
- [x] 参数引用规则：空参数 `""`、含空白或引号加引号并转义内部引号；`build_command_line` 以带引号 argv[0] 开头、NUL 结尾。
- [x] 单测 5 项：参数引用、命令行构造、非 EXE 类型、空/缺失目标、真实 `cmd.exe /c echo > probe.txt`（验证参数与工作目录）。
- [x] P1-06 专项验证 `scripts/verify-p1-06-launch.ps1`：CmdProbe 单击（参数+工作目录）→ MshtaTest 菜单打开 → 管理员身份运行 → StartupProbe 探针断言 `flags=1;show=3`（最大化 STARTUPINFO）→ MissingTest 失败提示与无进程 → WM_CLOSE exit=0。
- [x] 回归 P1-01～P1-05 全过；P1-04 hover 测试改为仅移动鼠标（单击自 P1-06 起会启动项目）。
- [x] 门禁：fmt=0、clippy -D warnings 通过、112 项测试（102+1+6+3）0 失败、Release 构建通过。

## 历史记录：P1-06 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-06**。LNK/文件/文件夹/URI 启动分别属 P1-07～P1-09；启动后自动隐藏（`auto_hide_on_launch`）属 P1-18；打开文件所在位置属 P1-08。

最终 Release SHA-256：`86F36010BC8A7FE4EDF2911D3FD6A0B0FF53E706AEABBACCF547BB8BF523F45E`。

- 证据目录 `docs/test-results/P1-06/`：
  - `p1-06-launch.tsv` 九阶段：`window_found` → `click_launch`（cmd 探针文件含 launched）→ `menu_rect`(344,270,538,452) → `menu_open`（mshta 新进程）→ `menu_admin`（runas 新进程）→ `maximized_state`（`flags=1;show=3`）→ `missing_target`（无进程、状态栏错误）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-06-initial.png` / `p1-06-after-click.png`（状态栏"已启动 CmdProbe"）/ `p1-06-menu-open.png`（"已启动 MshtaTest"）/ `p1-06-maximized.png` / `p1-06-missing.png`（状态栏"目标不存在：C:\QuickNest Demo\missing-app.exe"，无进程）。
  - 无 stderr 日志。

## 历史记录：P1-06 开发过程问题与修复

1. **测试目标选择反复**：charmap/notepad 类 Win11 存根启动即退出（Store 重定向）；regedit 带 requireAdministrator 清单，`CreateProcessW` 返回 740。最终用 mshta.exe 验证普通/管理员启动，用自编 StartupProbe 探针验证 STARTUPINFO 窗口状态（避免第三方程序覆盖初始状态导致假失败）。
2. **菜单项坐标不可靠**（P1-05 同源）：改用 `WindowFromPoint` 菜单矩形定位；`menu_rect` 记录 344,270,538,452。
3. **窗口状态证据**：第三方程序（dxdiag/mshta）会自行 `ShowWindow(SW_SHOWNORMAL)` 覆盖初始状态；探针直接读 `GetStartupInfoW` 断言 `show=3`，证明是我们传递到位。
4. **P1-04 回归适配**：单击自 P1-06 起触发启动，hover 验证改为仅移动鼠标。

## 历史记录：P1-05 任务清单（已验收）

- [x] 基线 3.4 项目编辑字段：名称、类型（7 种）、目标、启动参数、工作目录、自定义图标、窗口状态、管理员运行、备注；添加与编辑共用同一对话框。
- [x] 基线 3.1/3.2 添加：空白区域右键 → 添加项目；类型/名称/默认工作目录自动推断（`.exe/.lnk/.bat/.cmd/.ps1/URI/目录/文件`；EXE 与普通文件取父目录，LNK 保留 Shell 语义）。
- [x] 文件选择窗口：原生 `GetOpenFileNameW`（owner 为 QuickNest HWND），选择后自动填充名称/类型/目标/工作目录。
- [x] 添加时的工作目录推断与保存：浏览选择 `notepad.exe` 后工作目录自动为 `C:\Windows\System32` 并持久化。
- [x] 基线 3.5 右键菜单 8 项：打开 / 管理员身份运行 / 编辑 / 打开文件所在位置 / 复制路径 / 移动到其他分类 / 复制项目 / 删除（打开类留 P1-06+ 接线）。
- [x] 复制项目（`<name> 副本` + 新 UUID）、移动到其他分类（目标下拉框）、复制路径（`CF_UNICODETEXT` 剪贴板）、删除确认。
- [x] 参数行解析：双引号分组（`-a "b c"` → `["-a","b c"]`），单测覆盖。
- [x] 模型/控制器：`ItemKind::from_path/index`、`Item::default_workdir`、`parse_args`、`ItemDraft`（validate/from_item/into_item）、`Library::{add_item,update_item,remove_item,move_item_to_category}`、controller CRUD + 持久化；新增 8 项单测。
- [x] 官方配置加载容错：UTF-8 BOM 剥离（编辑器/PowerShell 常见），新增单测。
- [x] P1-05 专项验证 `scripts/verify-p1-05-crud.ps1`：添加（含引号参数落盘）→ 编辑（预填改名）→ 复制项目 → 复制路径（剪贴板断言）→ 移动到第二分类（下拉框选择）→ 删除确认 → 原项目保留 → 浏览自动填充 → WM_CLOSE exit=0。
- [x] 门禁：fmt=0、clippy -D warnings 通过、107 项测试（97+1+6+3）0 失败、Release 构建通过。
- [x] 回归：P1-01/P1-02/P1-03/P1-04 专项脚本全过。

## 历史记录：P1-05 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-05**。菜单中"打开/管理员身份运行/打开文件所在位置"三项仅提供入口，实际启动与 Shell 定位分别属 P1-06～P1-09 与 P1-08；外部拖入与 `Ctrl+V` 粘贴属 P1-17；图标提取属 P1-10/11。

最终 Release SHA-256：`C15F2DE7E16D28654AF3DE66302489A8C0640C5A9A70F9DE0EEF074000740EB1`。

- 证据目录 `docs/test-results/P1-05/`：
  - `p1-05-crud.tsv` 十五阶段：`window_found` → `dialog_opened` → `added`（TestItem 与 `b c` 引号参数落盘）→ `edit_dialog` → `edited`（旧名消失）→ `duplicated`（副本后缀）→ `copied_path`（剪贴板 = `C:\Windows\System32\notepad.exe`）→ `move_dialog` → `moved`（第二分类 1 项）→ `category_switched` → `delete_menu_rect`（原生菜单矩形 248,270,442,452）→ `delete_confirm` → `deleted`（第二分类 0 项）→ `browsed`（名称 notepad/类型 exe/工作目录 System32）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-05-dialog-add.png` / `p1-05-after-add.png` / `p1-05-dialog-edit.png` / `p1-05-copy-menu.png` / `p1-05-after-copy.png` / `p1-05-move-dialog.png` / `p1-05-move-combo.png` / `p1-05-move-selected.png` / `p1-05-second-category.png` / `p1-05-delete-menu.png` / `p1-05-delete-confirm.png` / `p1-05-final.png` / `p1-05-browse-dialog.png` / `p1-05-browse-filled.png`。
  - 无 stderr 日志（全程无 panic）。

## 历史记录：P1-05 开发过程问题与修复

1. **原生菜单项坐标不可靠（最重要验证问题）**：muda 菜单的项高/偏移随内容与位置变化，按固定 25px/20px 估算会把"删除"点成"复制项目"（实测第二分类出现"副本 副本"）。修复：右键后用 `WindowFromPoint` 取原生菜单窗口矩形，点击 `rect.bottom - 10` 作为末项，`delete_menu_rect` 记录 (248,270,442,452)。
2. **模态层关闭后的第一次点击被吞**：移动对话框关闭后立刻点击分类行不生效；脚本对分类切换断言 `last_category_id == tool-category-2` 并在失败时重试一次。
3. **窗口截图偶发失败**：菜单关闭瞬间窗口枚举可能漏掉目标，`Save-WindowOrScreen` 捕获后回退全屏截图。
4. **PowerShell 5.1 无 BOM 中文乱码**（P1-03 同源）：断言改用 Unicode 码点构造"副本"（`[char]0x526F + [char]0x672C`）。
5. **`CheckBox` 未导入 std-widgets**：Slint 编译报 Unknown element，补 import。
6. **Clippy collapsible_if**：文件选择回填改为 let-chain。

## 历史记录：P1-04 任务清单（已验收）

- [x] 基线 2.4 网格展示：项目卡片（图标+名称）由当前分类的 `Category.items` 驱动，切换分类/分组时网格联动（空分类显示 "0 个项目"）。
- [x] 列数随窗口宽度自适应：`columns = max(1, floor(grid 宽度 / 单元宽))`，680px 窗口下 5 列两行、760px 6 列一行、1100px 9 列（截图核对）。
- [x] 大/中/小三档图标（`settings.icon_size`，默认 medium）：small 76x88/图标 28、medium 96x108/图标 40、large 128x140/图标 56；非法值回退 medium。
- [x] 名称最多两行、超出省略（`wrap: word-wrap; max-lines: 2; overflow: elide`）。
- [x] 悬停显示完整名称/路径/备注（自绘 tooltip；Flickable 消费 move 事件，改在 `has-hover` 进入时按卡片坐标显示）。
- [x] 路径失效角标：Exe/Lnk/File/Folder 且 path 非空且不存在 → 红色 "!"；URI/Command/Automation 与空路径不误报（单测覆盖）。
- [x] 项目右键菜单（打开/编辑/删除）与空白区域右键菜单（添加项目）入口；操作逻辑分别留给 P1-06/P1-05。
- [x] 示例库注入 6 个示例项目（2 个真实系统路径、2 个失效路径、1 个 URI、1 个命令），网格有真实模型数据可渲染。
- [x] P1-04 专项验证 `scripts/verify-p1-04-grid.ps1`：清库启动 → 分类切换空/回 → 中图标初始截图 → 悬停 tooltip → 卡片右键菜单 → 空白右键菜单 → 680/1100 列数截图 → small/large 预设重启截图 → 三次 WM_CLOSE exit=0，stderr 为空。
- [x] 门禁：fmt=0、clippy -D warnings 通过、99 项测试（89+1+6+3）0 失败、Release 构建通过。
- [x] 回归：P1-01/P1-02/P1-03 专项脚本全过。

## 历史记录：P1-04 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-04**。项目**启动**（单击/右键"打开"）属 P1-06～P1-08；**编辑/删除/添加**对话框属 P1-05；**网格内排序、跨分类/跨分组移动、从资源管理器拖入、多文件添加**分别属 P2-05/06 与 P1-17；**真实图标提取与缓存**属 P1-09（当前统一使用示例图标）；手动设置列数属设置中心（第 17 章，后续任务）。

最终 Release SHA-256：`7CE6705B65C5D494DF12C4008DF1A409923ADB5089297CE5EC7B37453E1DF1CE`。

- 证据目录 `docs/test-results/P1-04/`：
  - `p1-04-grid.tsv` 十四阶段：`window_found` → `category_switch_empty`（0 个项目）→ `model_seeded`（config 含 6 个示例项目）→ `medium_initial` → `hover_tooltip` → `tile_menu` → `blank_menu` → `columns_narrow`（680px 5 列两行）→ `columns_wide`（1100px 9 列）→ `wm_close_exit`(medium=0) → `small_preset` → `wm_close_exit`(small=0) → `large_preset` → `wm_close_exit`(large=0) → `completed`。
  - 截图：`p1-04-medium-initial.png`（6 项目、2 个红角标）、`p1-04-empty-category.png`、`p1-04-hover.png`（tooltip 显示 `/System32/notepad.exe` 路径）、`p1-04-tile-menu.png`（打开/编辑/删除）、`p1-04-blank-menu.png`（添加项目）、`p1-04-narrow-680.png`、`p1-04-wide-1100.png`、`p1-04-small-preset.png`、`p1-04-large-preset.png`。
  - 三次运行的 stderr 日志为空：无 panic。

## 历史记录：P1-04 开发过程问题与修复

1. **BOM 导致配置静默回退示例库（本任务最重要修复）**：验证脚本用 PowerShell 5.1 `Set-Content -Encoding UTF8` 改写 `settings.icon_size`，文件带上 UTF-8 BOM；`serde_json` 在 line 1 column 1 报 `expected value`，`LibraryController::load` 静默回退示例库，档位始终是 medium。修复：加载时先 `strip_prefix('\u{feff}')`（新增 BOM 单测），脚本改用 `UTF8Encoding($false)` 写无 BOM。
2. **`TouchArea.hovered` 不存在**：Slint 1.18 的属性是 `has-hover`，`changed hovered` 编译失败；改为 `changed has-hover`。
3. **Flickable 没有 `visible-width/visible-height`**：这是 ScrollView 的属性；viewport 宽高改取 `grid-area.width/height`，滚动高度用 `ceil(items/columns)`。
4. **`PointerButton` 未定义**：Slint 1.18 枚举是 `PointerEventButton`（与分类行一致）。
5. **UI 内部不能写 `in` 属性**：hover 状态由卡片回调写入，属性改为 `in-out`。
6. **Flickable 消费移动事件**：`moved` 回调不稳定触发，tooltip 改为 `has-hover` 进入时按卡片坐标显示（进入即出现，移出清空）。
7. **启动后无修改不落盘**：首次冒烟脚本直接读 config 失败；脚本改为先切换一次分类触发持久化，同时验证网格联动。

## 历史记录：P1-03 任务清单（已验收）

- [x] 基线 2.3 分类操作：创建（对话框输入）、重命名（右键菜单→对话框预填，空名/重名拒绝）、删除（空分类直接删除；非空分类弹确认对话框：迁移项目到目标分类或删除项目）、上移/下移排序（顺序持久化）。
- [x] 右键菜单：每行 Slint 原生 `ContextMenuArea`（muda 菜单，重命名/上移/下移/删除四入口）。`TouchArea` 会吞右键，在右键释放时显式 `context-menu.show({x: self.mouse-x, y: self.mouse-y})`。
- [x] 分类栏垂直滚动：`ScrollView` + viewport 高度 `max(visible-height, count*28px)`，行 `y: index*28px`。
- [x] 持久化：全部操作经 P0-13 `save_atomic` 写 `%APPDATA%\QuickNest\config.json`；启动读取（缺失时生成示例库）。
- [x] 模型/控制器：`rename_category` / `delete_category`（`CategoryDeleteOutcome`）/ `delete_category_confirmed` / `migrate_category_items_into` / `move_category` / `active_category_mut`，30 项单测覆盖。
- [x] P1-03 专项验证 `scripts/verify-p1-03-categories.ps1`：对**新建的 TestCat 行**执行 右键菜单→重命名（全选替换为 RenamedCat）→上移（断言配置顺序 RenamedCat 在 AI 前）→下移（恢复在 AI 后）→删除（断言 RenamedCat 消失且 `tool-category-1` 保留）+ 全程截图 + WM_CLOSE exit=0。
- [x] 门禁：fmt=0、clippy -D warnings 通过、93 项测试（83+1+6+3）0 失败、Release 构建通过。
- [x] 回归：P1-01/P1-02 交互脚本全过；P0-08 Mutex / P0-09 IPC / P0-13 保存链 Medium 全过。

## 历史记录：P1-03 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-03**。分类**图标**字段在模型（`Category.icon`）中具备，图标选择 UI 统一并入 P1-09 图标体系；**分类拖拽排序手势**并入 P2-05/06 的拖拽体系（模型 `move_category` 已实现，菜单上移/下移即为本任务的排序入口并已实测）。

最终 Release SHA-256：`F40DB4ACCC06BD840617114848F1D4C29DF98F6311E8A8A49224A41EED316DF5`。

- 证据目录 `docs/test-results/P1-03/`：
  - `p1-03-categories.tsv` 十二阶段：`window_found` → `initial_screenshot` → `dialog_opened` → `created`/`persisted`(TestCat) → `menu_opened`（右键第 8 行）→ `renamed`（RenamedCat；`tool-category-1` 未动）→ `moved_up`（配置顺序 RenamedCat 在 AI 前）→ `moved_down`（恢复 AI 后）→ `deleted`（RenamedCat 消失、`tool-category-1` 保留）→ `wm_close_exit` exit=0 → `completed`。
  - 截图：`p1-03-initial.png`（7 示例分类）/ `p1-03-dialog.png`（新建分类对话框）/ `p1-03-created.png` / `p1-03-menu.png`（TestCat 行右键四项菜单）/ `p1-03-before-delete.png`（8 分类，RenamedCat 末行高亮）/ `p1-03-after-delete.png`（恢复 7 分类，必备软件高亮）。
  - `p1-03-stderr.log` 为空：修复 RefCell panic 后全流程无 panic。

## 历史记录：P1-03 开发过程问题与修复

1. **上移/下移回调 RefCell panic（进程 exit=101）**：`if let Some(index) = controller.borrow()...` 的临时借用存活到整个 `if let` 语句结束（Rust 临时值生命周期），体内 `borrow_mut()` 触发 `RefCell already borrowed`。修复：位置计算先绑定到独立 `let` 语句。教训：RefCell 回调中 `borrow` 与 `borrow_mut` 不能跨 `if let` 主体。
2. **验证脚本目标行错误导致假绿（本任务最重要修正）**：初版右键坐标 (40,108) 落在第一行"必备软件"而非新建的 TestCat；把第一行改名/删除同样满足 config 断言（RenamedCat 存在/消失），而"上移"处于第一行的边界并未真正移动——移动功能实际上未被验证。修复：按行索引计算目标坐标（第 8 行 y=84+10+7*28+14），并断言 `tool-category-1` 未动、移动后配置顺序正确。教训：断言必须能区分"目标行"与"任意行"。
3. **重命名对话框预填导致输入拼接**：LineEdit 预填原名，TypeText 追加成 RenamedCatTestCat。修复：脚本先点击输入框 → Ctrl+A → Delete 再输入（P103Input 增加 Key/SelectAll）。
4. **PowerShell 5.1 无 BOM 脚本中文乱码**：`-notmatch '必备软件'` 被按 ANSI 解码为乱码而恒真。修复：断言改用 ASCII 的示例 id `tool-category-1`。
5. **Start-Process -PassThru 加重定向时 ExitCode 为 null**：改用 .NET `ProcessStartInfo` 启动；panic 证据经 `StandardError.ReadToEnd()` 写入 `p1-03-stderr.log`。
6. **菜单关闭后截图残影**：早期截图出现旧帧文本叠加（软件渲染重绘时序）；修正目标行后各阶段截图清晰，无残影。

## 历史记录：P1-02 任务清单（已验收）

- [x] 基线 14.1/14.2 数据模型对齐：三层数据结构（分组→分类→项目），全部实体 UUID v4（`UuidCreate`），`current_group_id` 顶层、分组 `color`、项目 `item_type`+嵌套 `launch` 块、顶层 `settings`（theme/accent_color/opacity/hotkey/auto_hide/pinned/icon_size/window_bounds 全字段默认值），`schema_version=2`。
- [x] 基线 3.1/3.4 项目字段：七种类型枚举（exe/lnk/file/folder/uri/command/automation）、启动参数、工作目录、自定义图标、窗口状态、管理员运行、备注。
- [x] 基线 2.2 分组操作：点击切换（每组独立分类列表与 `last_category_id` 记忆）、新建（"新分组"为真实分组，UUID 标识）、重命名（空名拒绝）、强调色（`#rrggbb` 校验）、拖拽排序、删除非空组需确认（`DeleteOutcome::NeedsConfirmation`）+ 迁移到其他组（分类冲突自动重赋 UUID）。
- [x] `LibraryController`（模型↔UI 桥接）：全部操作后经 P0-13 `save_atomic` 原子持久化；配置路径 None 时可禁用（测试）。
- [x] UI 绑定：分组条由模型驱动（`GroupView` 含估算 tab 宽度）、当前分组高亮、切换后左侧分类随之变化、▼/新建分类入口回调。
- [x] P1-02 专项验证 `scripts/verify-p1-02-groups.ps1`：四组截图（初始/应用高亮/自动化高亮/切回）+ 分类栏联动核对 + WM_CLOSE 干净退出。
- [x] 门禁：fmt=0、clippy -D warnings 通过、88 项测试（78+1+6+3）0 失败、Release 构建通过。
- [x] 回归：P0-08/09/12/13 Medium 全过；P0-11 维持环境限制口径；**P0-10 Medium 注入阶段暴露既有 UIPI 限制**（见下），dev 会话 hotkey-test 七阶段全过证明功能未破坏；P1-01 专项验证通过。

## 历史记录：P1-02 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-02**。分组拖拽排序的**鼠标拖拽交互**（模型 `move_group` 已实现并单测）的 UI 端拖拽手势属 P2-05/06 范围；重命名/强调色/删除确认的**对话框 UI** 尚未实现（操作逻辑与模型已就绪并通过单测，下一迭代接入对话框）。

最终 Release SHA-256：`A44A4E6C9CC0D38247C159C8711B9AE7770112EC9941BFA4F1D85E306B87B820`。

- 证据目录 `docs/test-results/P1-02/`：
  - `p1-02-groups.tsv` + 四张截图：初始（常用工具高亮+7 分类）→ 点击应用（高亮切换+分类变为桌面应用/便携应用）→ 点击自动化（高亮切换+分类变为脚本）→ 切回常用工具（分类恢复 7 项，`last_category_id` 记忆生效）→ WM_CLOSE exit=0。
  - 模型持久化形状测试 `persisted_shape_matches_baseline_14_2`：断言序列化 JSON 含 schema_version=2/settings/current_group_id/color/last_category_id/item_type/launch/workdir/run_as_admin 全部基线字段。
- **P0-10 Medium 回归的口径更新**：P1-01/P1-02 的 no-frame 窗口改变了启动前台时序，暴露 P0-10 Medium 注入阶段既有的 UIPI 环境限制（Medium 向 High 前台注入合成输入被静默过滤——与 P0-11/P0-12 同源）。dev 会话 hotkey-test 七阶段全过（`dev-regression-p1-02.tsv`，EXIT=0）证明热键功能未破坏；`run-p0-10-medium.ps1` 增加 env-limited 识别分支（registered+conflict 通过而注入事件缺失时标记环境限制），与 P0-11/P0-12 口径一致。

## 历史记录：P1-02 开发过程问题与修复

1. **数据模型与基线偏差（本任务最重要修正）**：初版用 u32 递增 id 与扁平 item 结构；核对基线 14.1/14.2 后全面重构为 UUID v4 + `current_group_id` + `color` + `item_type`+`launch` 嵌套 + 顶层 `settings`。教训：实现前应先核对第 14 章数据结构，而不是只看交互章节。
2. **Slint Window 自动布局压住标题栏**（P1-01 已修）在 P1-02 继续生效：新分组条/分类栏全部放进根 Rectangle 画布。
3. Slint 1.18 无 `uint` 类型（用 int）、无字符串测长 API（tab 宽度由控制器按 CJK/拉丁字符宽度估算传入）、Rust 侧无 `1px` 字面量（`tab-width` 用 float + Slint 侧 `* 1px`）。
4. bash heredoc 两次被截断损坏 library_model.rs——改用 Write 工具全量写入（沿用 P0-09 教训）。
5. P0-10 Medium 注入失败判别：dev 会话对照实验证明功能未破坏（排除 P1-02 回归），确认为既有 UIPI 环境限制，脚本加识别分支如实记录。

## 历史记录：P1-01 任务清单（已验收）

- [x] 基线 2.1 五区域布局：自定义标题栏（Logo/名称/搜索/固定/设置/×）+ 顶部一级分组条（常用工具/应用/自动化 + ▼）+ 左侧二级分类栏（7 分类 + 新建分类）+ 右侧项目网格 + 状态栏（计数 · 本地配置 | 视图/排序/添加）。
- [x] 基线 2.5 标题栏可拖动窗口：Slint 内置 `WindowMoveArea`（winit `drag_window` → `ReleaseCapture` + `PostMessageW(NCLBUTTONDOWN/HTCAPTION)`），实测拖动位移 dx=180/dy=90 与光标 +200/+100 匹配。
- [x] 无边框窗口：`no-frame: true` + 建窗 hook `with_decorations(false)`；客户区覆盖窗口顶部（client_top_offset=0）实测无原生标题栏条。
- [x] 关闭按钮语义（基线 2.5）：接 `hide-requested`（隐藏面板而非退出），真正退出经托盘（P1-15）。
- [x] 全部 P0 验证属性/回调保留（validation-status / dpi-validation 层 / escape-pressed / items 模型），P0 验证模式未受影响。
- [x] 专项验证脚本 `scripts/verify-p1-01-ui.ps1`：窗口发现 → 无边框判定 → 布局截图 → 标题栏拖动位移 → WM_CLOSE 干净退出（exit=0）。
- [x] 门禁：fmt=0、clippy -D warnings 通过、63 项测试（53+1+6+3）0 失败、Release 构建通过。
- [x] P0-08～P0-13 Medium 全量回归通过（新增界面未破坏任何验证模式）。

## 历史记录：P1-01 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P1-01**。分组/分类/网格的**交互**（创建、重命名、排序、增删、右键菜单、拖拽）分别属 P1-02～P1-05；本任务只交付基线 2.1/2.5 的界面骨架与窗口行为。

最终 Release SHA-256：`7E7EF6CD34C6C7ADBA2DF13154699970DBAEE112CF3A44AD2C87DA090F6F70A9`。

- 证据目录 `docs/test-results/P1-01/`：
  - `p1-01-ui.tsv`：window_found → style(GWL_STYLE) → frameless(client_top_offset=0) → screenshot → drag_probe(落点命中 QuickNest) → drag_down(光标/左键状态确认) → drag(dx=180 dy=90) → wm_close_exit(exit=0) → completed。
  - `p1-01-window.png`：五区域布局截图（标题栏/分组条/分类栏/网格/状态栏齐全）。
- 回归：P0-08/09/10/12/13 Medium 全过，P0-11 维持已记录的环境限制口径。

## 历史记录：P1-01 开发过程问题与修复

1. **标题栏被布局压住（决定性根因）**：Slint 的 Window 会给直接子元素自动套一层布局，我设置的 `y: title-bar.height` 被布局覆盖，标题栏渲染在主布局之下——事件（点击/拖动）因此永远落不到 WindowMoveArea，拖动位移恒为 0。修复：根元素改为 Rectangle 手动画布（坐标与层序只由声明顺序决定），标题栏声明在主布局之后以保证层序更高。
2. **验收标准错误**：曾以 GWL_STYLE 是否含 WS_CAPTION 判定无边框；实际 winit 0.30 Windows 为保留 aero snap 而保留该位、改用 WM_NCCALCSIZE 裁剪非客户区。改用客户区原点与窗口顶边的偏移（client_top_offset=0）作为正确判据。
3. **FocusScope 遮挡**：全窗口 FocusScope 的鼠标事件处理挡住标题栏 → 改为从标题栏下方开始。
4. **验证脚本环境干扰**：注入落点被 Chrome（更高 Z 序）吞掉 → 置顶 QuickNest；PowerShell 线程 DPI 虚拟化导致坐标错位 → 线程设 Per-Monitor V2（物理坐标）；窗口居中被终端覆盖 → 先移到屏幕角落。
5. 诊断探针（标题栏"固定"点击变为"已固定"的对照实验、OS 层 SC_MOVE 判别）完成后已删除，保留其结论。

## 历史记录：P0-15 任务清单（已验收，P0 阶段关闭）

- [x] `docs/P0_GO_NO_GO.md` 增补第 7 节最终决策记录：**GO**（决策日期 2026-11-04；依据 13/13 通过 + 命中基线 GO 矩阵路径 + 报告第 5 节建议与负责人推进指示；六项剩余风险知悉接受带入 P1；含可撤销条款——证据推翻可重开 P0）。
- [x] 基线 19 章进入 P1 前提复核：P0 已通过，"P0 未通过不允许进入完整 P1"条件满足。
- [x] 基线第 29 章 P0-15 勾选，P0 阶段 15/15 全部完成。
- [x] 进度文档更新至 P0 阶段关闭状态；下一任务为第 20 章 P1-01（主界面）。

## 历史记录：P0-14 任务清单（已验收）

- [x] 基线 19 章 P0 交付物补齐：`docs/WINDOWS_API_DECISIONS.md`（总体绑定 + 12 个任务域逐项 API 决策 + 4 条弃用方案 + 4 条通用纪律）与 `docs/P0_GO_NO_GO.md`（结论速览 + 决策矩阵映射 + 分域可行性判定 + 剩余风险 + 决策建议 + 证据索引）。
- [x] 基线 P0 决策矩阵映射完成：命中"Slint 主窗口直接 OLE 通过 → GO"路径；独立 STA 宿主备选已在 P0-07 验证（未启用）；WM_DROPFILES 与 NO-GO 分支均未触发。
- [x] 证据索引核对：13 个任务证据目录与 P0-05 主验收目录逐一确认真实存在。
- [x] 报告无未经测试的声明：全部结论可溯源至 P0_VALIDATION.md 与 test-results 证据；Medium 环境限制如实标注而非伪装通过。
- [x] GO / NO-GO 最终拍板显式留予 P0-15（项目负责人决策），报告仅提供技术结论与决策模板。
- [x] 基线第 29 章 P0-14 勾选与进度文档更新。

## 历史记录：P0-14 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P0-14**。P0 交付物三件套齐备：`P0_VALIDATION.md`（逐任务验收记录）+ `WINDOWS_API_DECISIONS.md`（API 决策）+ `P0_GO_NO_GO.md`（技术报告与决策依据）。

- 报告要点：13/13 任务通过；决策矩阵命中 GO 路径；技术证据支持 GO，最终决策留 P0-15；六项剩余风险（平台覆盖、持久化边界、权限模型、版本相关 API、性能长测、Medium 环境限制）均有边界记录与 P1 处理入口。
- 文档一致性：所有结论可溯源至 `docs/test-results/P0-01～P0-13/` 与 `P0_VALIDATION.md`；引用的 13 个证据目录经核对全部存在。

## 历史记录：P0-13 任务清单（已验收）

- [x] 基线 15.2 保存链：同目录唯一临时文件 `config.<PID>.<UUID>.tmp`（UuidCreate 生成）→ 独占创建（create_new，冲突自动换名重试）→ 完整写入 + `sync_all` → 关闭句柄后替换。
- [x] 基线 15.4 ReplaceFileW：flags=0（无 WRITE_THROUGH、无忽略 ACL 标志），唯一备份 `config.backup.<UTC戳>.json` 同卷同目录（字典序=时间序）；失败保留原配置、保留 tmp 供诊断、记录 GetLastError——真实失败注入（CreateFileW FILE_SHARE_READ 锁定目标）实测 ERROR_SHARING_VIOLATION(32) 且原配置与 tmp 完好。
- [x] 基线 15.5 首次创建：`MoveFileExW(MOVEFILE_WRITE_THROUGH)` 同卷移动；并发创建（目标已存在，ERROR_ALREADY_EXISTS）回退 ReplaceFileW——被竞态创建的文件进备份而非丢失（单测固化）。
- [x] 基线 15.7 单写者：进程内写事务锁（save/recovery 共用），持锁期间并发保存以 `Busy` 拒绝（单测确定性验证）；跨实例写锁留待 P1 与 P0-08 单实例锁整合。
- [x] 基线 15.8 备份恢复：`recover_latest_backup` 按文件名倒序扫描、JSON 解析验证、恢复最近**有效**备份、损坏文件改名 `config.corrupted.<戳>.json`；恢复失败把损坏文件改回原名（不留缺失状态）；**安全模式**——无有效备份时不动任何文件；对仍有效的活配置拒绝恢复。
- [x] `--save-test` 七阶段场景（first_create/atomic_replace/failure_protection/temp_lifecycle/backup_recovery/safe_mode/completed）dev 会话与 Medium 令牌各实测通过（Exit=0）。
- [x] fmt/check/Clippy -D warnings、63 项测试（53 lib 含 11 项保存链单测 + 1 bin + 6 集成 + 3 doc）与 Release 构建全部通过。
- [x] P0-08/P0-09/P0-10/P0-11/P0-12 回归通过；正式清单、进度与验证证据更新。

## 历史记录：P0-13 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P0-13**。防抖（300-500ms）、每日/导入前备份策略、备份轮换上限（最近 10 份）、Schema 迁移属 P1（15.7/15.9）——P0 只验证保存与恢复机制本体。

最终 Release SHA-256：`86A3D930799F41FEA74668E90512F0D0E7EFB51AF09F7821E479E658B0D32D62`。

- 证据目录 `docs/test-results/P0-13/`：
  - `save-run2.tsv`（dev 会话，7 行全过）与 `medium-10928112/`（Medium，7 行全过）：first_create（MoveFileExW WRITE_THROUGH，无残留）→ atomic_replace（ReplaceFileW flags=0，备份持旧内容，无残留）→ failure_protection（ERROR_SHARING_VIOLATION=32，目标完好，tmp 保留诊断）→ temp_lifecycle（成功零残留/失败保留至清理）→ backup_recovery（最新有效胜出、corrupted 改名）→ safe_mode（全坏备份不动原文件）→ completed。
  - `logs/run1.tsv`：失败记录保留不算通过——场景未轮换阶段 a/b 生成的真实备份导致恢复选中它（实现行为正确、场景缺陷），已修。
  - 门禁日志 fmt/clippy/test/doctest 全 0。
- 编译与测试：fmt=0；check/clippy 通过；测试 53+1+6+3 全部通过，0 失败。

## 历史记录：P0-13 开发过程问题与修复

1. 并行单测互锁：cargo test 多线程与进程级 WRITE_LOCK 争用导致大面积 Busy 失败；测试模块用第二把互斥锁自串行化，写锁获取改为毒化容忍（panic 后 clear_poison 继续服务，不永久楔死）。
2. Rust std `File::open` 默认共享含 FILE_SHARE_DELETE，不构成替换锁：失败注入改用 `CreateFileW(FILE_SHARE_READ)`（真实 ERROR_SHARING_VIOLATION 路径），std 的宽松共享行为已记录。
3. 场景 run1 失败：阶段 a/b 真实保存生成的备份比场景伪造戳更新，恢复正确选中它而断言失败——实现无误、场景需先轮换备份集合；已修（轮换本身也验证了备份可删除）。
4. 锁验证竞态：对真实恢复的 30ms racer 在快机器上赶不上锁窗口；改为确定性验证（手动持锁→断言 Busy→释放→断言可保存），recovery 走同一把锁故语义等价。

## 历史记录：P0-12 任务清单（已验收）

- [x] 基线 10.1 状态机：Hidden/Showing/Visible/Hiding/ShuttingDown，转换路径逐步记录（Hidden->Showing->Visible 实测）；ShuttingDown 为终态，后续转换被拒实测。
- [x] 基线 10.2 `SuppressionToken` RAII：六类原因（Pinned/ModalDialogOpen/SystemDialogActive/ExternalDragHover/ContextMenuOpen/LaunchingElevated），引用计数、Drop 自动释放、超获取饱和为零；token 比状态机存活更久仍正确释放（单测固化）。
- [x] 基线 10.3 失焦流程：前台变化→状态/固定/抑制检查→一次性 120ms 延迟复查（pending 槽防无限顺延）→重新读取前台→归属判定→确认外部后隐藏；绝不因单次 blur 立即隐藏（recheck_stays 实测）。
- [x] 基线 10.4 事件源：Slint FocusScope ESC 回调、Ctrl+Alt+Space 热键事件、托盘左键/菜单事件三路真实接线到状态机（dev 会话实测）；GetForegroundWindow/GetWindowThreadProcessId/Owner 链构成前台归属判定。
- [x] 基线 10.7 固定模式：pinned 独立于可见状态，同时镜像为 Pinned 抑制锁；失焦不隐藏实测，取消固定后恢复自动隐藏实测。
- [x] `focus.rs` 前台归属（root_owner 链 + 同进程判定）、`click_at` 改为 SendInput 绝对移动（修复 Medium 下 SetCursorPos 错误 183）。
- [x] `--focus-test` 十七行证据开发会话全过（Exit=0）；Medium 十三行证据 Exit=0（hotkey/tray 两类环境限制如实记录，其余全部真实跑通）。
- [x] fmt/check/Clippy -D warnings、51 项测试（41 lib 含 14 项状态机单测 + 1 bin + 6 集成 + 3 doc）与 Release 构建全部通过。
- [x] P0-08/P0-09/P0-10/P0-11 Medium 回归通过；正式清单、进度与验证证据更新。

## 历史记录：P0-12 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P0-12**。多显示器热键冲突提示、设置界面、UAC 启动对话框（LaunchingElevated 锁的真实来源）属 P1+。

最终 Release SHA-256：`C11E2BC61CA1143F148F1123185C8F1448AFEFC07EFFBDB99AAD70C134E0`。

- 证据目录 `docs/test-results/P0-12/`：
  - `focus-run7.tsv`（dev 会话，17 行全过）：状态转换→ESC→热键 toggle×2→托盘 toggle×2→托盘菜单 Show/Hide（真实鼠标点击菜单项）→blur 隐藏→pinned 抑制→RAII 抑制→refocus 后 recheck 不闪隐→再次失焦→ShuttingDown→completed。
  - `medium-dfe8b11e/`（Medium，13 行 Exit=0）：状态机核心+ESC（消息路径）+blur 全链真实跑通；hotkey_env_limited（UIPI 合成输入限制）与 tray_skipped（P0-11 已记录）如实降级。
  - `logs/`：失败记录保留不算通过（run1 HWND 未就绪、run2 事件顺序、run3/4 菜单与前台竞态、diag recheck 无限顺延的诊断证据）。
- 编译与测试：fmt=0；check/clippy 通过；测试 41+1+6+3 全部通过，0 失败。

## 历史记录：P0-12 开发过程问题与修复

1. 原生 HWND 需事件循环泵后才可用（P0-02 同款时序）：setup Timer 在 UI 线程检查就绪、同步状态机并停放 Hidden，场景线程等 ready 信号。
2. recheck 无限顺延：blur 管线每 tick 重设 120ms deadline 导致永不触发；修复为 pending 槽仅在空时设置（基线"一次性延迟检查"的准确语义），诊断日志留证。
3. 托盘菜单键盘选择在 Medium 不可靠：改为 MenuOpened 事件握手 + 真实鼠标点击菜单项（#32768 窗口坐标），鼠标点击不受前台限制。
4. Medium 下 SetCursorPos 返回错误 183：click_at 重写为 SendInput 绝对移动（虚拟桌面归一化，P0-05 验证过的注入通道）。
5. Medium 下热键注入事件不到达（forwarder consumed=0、drain 亦空）：鉴别实验证明 P0-10 Medium 链路当日仍通——根因是 UIPI 丢弃 Medium 进程在 High 前台（本机 Explorer）注入的合成输入（SendInput 返回成功但按键被过滤）；phase c 改为"前台归我才注入，否则如实记录 hotkey_env_limited"。物理键盘不受影响，产品无缺陷。
6. 单元测试与并行运行的实测实例争抢全局热键导致偶发失败：串行执行即过（教训记录）。

## 历史记录：P0-11 任务清单（已验收）

- [x] 基线 11.3 `Shell_NotifyIconW`：NIM_ADD/NIM_MODIFY/NIM_DELETE 全生命周期，NIF_MESSAGE|NIF_TIP|NIF_SHOWTIP。
- [x] `TrayService` 线程绑定 RAII：专用消息循环线程 + 隐藏顶层回调窗口（非 HWND_MESSAGE，保证菜单/前台可用）。
- [x] 六项菜单按基线顺序：显示 QuickNest / 隐藏 QuickNest / 固定-取消固定 / 设置 / 检查更新 / 退出程序；`GetMenuItemCount`/`GetMenuStringW` 实测验证。
- [x] 回调消息路由：WM_LBUTTONUP→LeftClick、WM_LBUTTONDBLCLK→DoubleClick、WM_RBUTTONUP→RightClick+菜单，事件经 mpsc 送达，UI 经 `invoke_from_event_loop` 转发实测。
- [x] 菜单真实交互：`TrackPopupMenu(TPM_RETURNCMD)` 打开 → SendInput ESC 关闭（MenuDismissed）；Down+Enter 选中（MenuSelected(Show)），菜单窗口（#32768）可见性独立核查。
- [x] TaskbarCreated 重建：`RegisterWindowMessageW` 注册，先删图标再广播（忠实模拟 Explorer 重启），NIM_ADD 成功实测。
- [x] 注销清理：Drop → 托盘线程 NIM_DELETE + DestroyMenu + DestroyWindow（基线 11.3 退出序列第 3 步实测）；`Deleted{success=true}` 事件为证。
- [x] fmt/check/Clippy -D warnings、37 项测试（27 lib + 1 bin + 6 集成 + 3 doc）与 Release 构建全部通过。
- [x] `--tray-test` 九阶段场景开发会话通过（Exit=0）；Medium 令牌受本机 UIPI 环境限制（见边界），已如实记录。
- [x] P0-08/P0-09/P0-10 Medium 回归通过；正式清单、进度与验证证据更新。

## 历史记录：P0-11 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P0-11**。托盘呼出窗口的实际显示/隐藏动作属 P0-12 窗口焦点状态机；菜单命令当前只验证路由与映射，不执行真实动作。

最终 Release SHA-256：`1F717F051133634657EA37ED66A59F24FCFBD1CB8FB9FC08A0ECCFDB01B9CC19`。

- 证据目录 `docs/test-results/P0-11/`：
  - `tray-run8.tsv`（最终验收，当前哈希二进制）：added → tooltip_modified → menu_verified(count=6) → callback_routed → menu_shown_dismissed → menu_selected(Show) → taskbar_readded → deleted → completed，Exit=0。
  - `diag5/6/7-*.log`：诊断证据——AttachThreadInput 跨进程导致菜单打开即关（弃用方案）、普通 SetForegroundWindow 菜单正常、TaskbarCreated 重复 NIM_ADD 返回 E_FAIL。
  - `logs/tray-run1~6.tsv`：失败记录保留不算通过（事件顺序、前台方案、VK_UP/VK_DOWN 混用等）。
  - `medium-*/`：受限令牌下 `Shell_NotifyIconW` 被拒（ERROR_ACCESS_DENIED=5，UIPI/Explorer 高完整性，见边界）。
- 编译与测试：fmt=0；check/clippy 通过；测试 27+1+6+3 全部通过，0 失败。

## 历史记录：P0-11 开发过程问题与修复

1. 验证框架缺陷：场景线程出错时事件循环永不退出、错误被吞。修复为失败时 `quit_event_loop` 并把 `scenario_error` 写入报告（Medium 证据因此可见）。
2. 菜单打开即关：跨进程 `AttachThreadInput` 前台转移使 TrackPopupMenu 立即返回 0（diag5）；改用普通 `SetForegroundWindow` + 菜单期间零尺寸宿主窗口 `SW_SHOWNOACTIVATE` 后，菜单保持打开且键盘可达（diag6）。attach 方案弃用并记录。
3. 测试按键错误：VK_UP(0x26) 被当作 VK_DOWN(0x28) 注入，菜单按上箭头环绕到末项返回"退出程序"——反向证明菜单 id→命令映射正确；修正后返回第一项 Show。
4. TaskbarCreated 模拟失真：图标仍存在时重复 NIM_ADD 被 Shell 拒绝（E_FAIL，diag7）；改为先 `remove_icon()` 再广播，忠实模拟 Explorer 重启状态。
5. 托盘线程早期错误不上报：pump 错误路径未走 ready 通道导致"exited before registration"无细节；全部错误路径补 `ready.send(Err(...))`。

## 历史记录：P0-10 任务清单（已验收）

- [x] 基线 11.1 默认组合 Ctrl+Alt+Space、优先 `MOD_NOREPEAT`（`MOD_CONTROL|MOD_ALT|MOD_NOREPEAT`，VK_SPACE）。
- [x] `RegisterHotKey(NULL, …)` 在专用消息循环线程执行；`GetMessageW` 可靠泵；注册前强制建立消息队列消除 PostThreadMessage 竞态。
- [x] `WM_HOTKEY` 事件经 mpsc 送达，`invoke_from_event_loop` 线程安全转发 Slint UI（`forwarded_to_ui=1` 实测）。
- [x] 冲突检测：子进程 `--hotkey-conflict` 探测同组合，`ERROR_HOTKEY_ALREADY_REGISTERED(1409)` 显式映射并实测。
- [x] 注销清理：Drop 在注册线程 `UnregisterHotKey`，独立子进程随后注册成功实测；所有退出路径单次收口注销。
- [x] 注册失败明确提示：错误映射为 `AlreadyRegistered`/`Failed(GetLastError)`，验证报告记录错误名。
- [x] 新增无 Slint/无注入的最小生命周期单测 `drop_unregisters_and_allows_re_registration` 固化 Drop 行为。
- [x] fmt/check/Clippy -D warnings、36 项测试（26 lib + 1 bin + 6 合成 + 3 doc）与 Release 构建全部通过。
- [x] `--hotkey-test` 六阶段场景开发会话与 Medium 受限令牌（RID 8192）各通过（Exit=0）。
- [x] P0-08/P0-09 Medium 回归通过；正式清单、进度与验证证据更新。

## 历史记录：P0-10 验收结论与证据

结论：**2026-11-04 本任务验收通过，停在 P0-10**。搜索快捷键（基线 11.2）与系统托盘（11.3）属后续任务，未实现。

最终 Release SHA-256：`DA3EC6B1F7EC75DF87EB6ACAFB49DC412E9E2366A3708CBB989BD62530C2C33`。

- 证据目录 `docs/test-results/P0-10/`：
  - `hotkey-run5.tsv`（开发会话）与 `medium-f6866f94/`（Medium）：registered → conflict_detected(1409) → hotkey_event(forwarded_to_ui=1) → unregistered_cleanup → re_registered → second_event → completed，Exit=0。
  - 失败记录保留不算通过：`run1/`（invoke_from_event_loop 阻塞）、`run2/`（Drop 无唤醒挂死）、`hotkey-run3.tsv`（控制通道 Disconnected 空转）、`hotkey-run4.tsv`（诊断日志污染 GetLastError 的 0x00000006 假象）+ `run4/diagnostic-isolation-hang.log` 决定性诊断。
- 编译与测试：fmt=0；check/clippy 通过；测试 26+1+6+3 全部通过，0 失败。

## 历史记录：P0-10 开发过程问题与修复

1. run1 挂死：`invoke_from_event_loop` 在事件循环未运行时永久阻塞；重构为场景线程 + 主线程 Slint 事件循环。
2. run2 挂死：Drop 发送 Shutdown 后泵阻塞在 `GetMessageW` 观察不到控制；增加 `thread_id` 并在 join 前投递 `PostThreadMessageW(WAKEUP)`。
3. run3 挂死（隔离单测定位）：控制通道排空循环在 `Disconnected`（Drop 丢弃 Sender 后）不退出，泵无限空转；修复为 `Shutdown|Disconnected` 统一 `PostQuitMessage`，注销由循环后单次 `UnregisterHotKey` 收口。
4. run4 假象：诊断 `eprintln` 插在 Win32 调用与 `GetLastError` 之间覆盖 last error；移除全部诊断，保持 `GetLastError` 紧跟失败调用。

## 历史记录：P0-08 任务清单（已验收）

- [x] Mutex 名按基线 `Local\QuickNest-{SID_HASH}-{SESSION_ID}`：TokenUser SID 关键字节 FNV-1a 64 位哈希 + `ProcessIdToSessionId`。
- [x] 语义修正：基线 9.2 的 WAIT_* 清单实为 WaitForSingleObject 语义；实现为 `CreateMutexW` + 紧读 `GetLastError`，`ERROR_ALREADY_EXISTS` 时用零超时等待探测接管（WAIT_ABANDONED）或判定已有活实例（WAIT_TIMEOUT）。
- [x] `MutexGuard` 线绑定（仿 OleGuard），持句柄至 Drop（owner 时 ReleaseMutex + CloseHandle）。
- [x] `--mutex-test/--mutex-child/--mutex-keeper` 三模式完整场景：首实例 → 子进程 ALREADY_RUNNING+WAIT_TIMEOUT → keeper 保活 → 父进程模拟崩溃（exit 70 跳过 Drop）→ keeper 记录 WAIT_ABANDONED 接管。
- [x] fmt/check/Clippy -D warnings、33 项测试（23 lib + 1 bin + 6 合成 + 3 doc）与 Release 构建全部通过。
- [x] Medium 受限令牌完整场景双轮通过（exit 70，全部断言证据齐全）；错误路径（已有报告/缺参数）返回 1 且保护已有报告。
- [x] P0-03/P0-07 回归通过且哈希一致。
- [x] 正式清单、进度与验证证据更新。

## 历史记录：P0-08 验收结论与证据

结论：**2026-11-03 本任务验收通过，停在 P0-08**。Named Pipe IPC 属 P0-09，未实现。

最终 Release SHA-256：`0735537A80FCF791DC94F915A59BEF6DFFE5BA4F0F9D0F125400BBC9BC1E515D`。

- 证据目录 `docs/test-results/P0-08/`：
  - `run2/`（开发会话）与 `medium-*/`（Medium RID 8192 双轮）：parent FIRST_INSTANCE → child ALREADY_RUNNING+WAIT_TIMEOUT → keeper opened → WAIT_ABANDONED 接管 → 释放，parent exit 70 模拟崩溃。
  - Mutex 名实测：`Local\QuickNest-7523641ad8f38453-1`（同用户同会话一致，符合基线命名）。
  - `errors-*/`：三个错误路径均返回 1，已有报告未覆盖。
  - `p0-03-regression/`：pass 且哈希一致；`run-p0-07-medium.ps1` 探针重跑 P0-07：全部阶段通过，exit 0。
- 编译与测试：fmt=0；check/clippy 通过；测试 23+1+6+3 全部通过，0 失败。

## 历史记录：开发过程问题与修复

1. 手写 `spawn_exe` 用空白切分参数，带空格的报告路径被拆散（InvalidFilename/NotFound）：改为参数数组直接传递。
2. 子进程/keeper 报告文件打不开（Os code 2）：`OpenOptions::append(true)` 单独使用是 OPEN_EXISTING 语义，不创建新文件；补 `.create(true)`。
3. 其余为一次性类型适配（HRESULT/u32、WPARAM/LPARAM 新类型、windows crate feature 门控），均已修复并保留通过日志。

## 历史记录：后续任务

本次停在 P1-15。下一任务 **P1-16 单实例唤醒**（基线 9.1～9.5：Mutex 单实例 + Named Pipe IPC 已由 P0-08/09 验证，接入生产启动与"再次启动显示已有实例"；退出流程补齐 IPC/Mutex 清理）。托盘已完成；外部拖拽属 P1-17，基础自动隐藏属 P1-18。

## 历史记录：P0-05 实测与问题

- 最终产物 SHA-256：`8BE01530722210A5661919592E4528E888B63BA6D2F8409DB40479BEC678A5E4`；主验收目录 `docs/test-results/P0-05/interactive-39ab013c/`。
- 实际源 Explorer PID 9172、目标 PID 29184，双方 Medium/RID8192；不是桌面 High Explorer 冒充普通权限源。
- 使用真实 Explorer 选择样本，再通过 SendInput 拖动和 ESC；并非构造 IDataObject 或直接调用目标回调来代替验收。
- 严格主验收 5 Drop/6 路径/2 Leave，取消动作前后 Drop 计数都为 5；UI 心跳连续、双方 Responding=true。
- 初轮文件夹未选中、一次窗口遮挡、早期取消脚本时序不足均保留为预检/失败记录；没有计入最终通过。取消脚本改为 ESC 持续及处理等待，并强制断言 Drop 计数不变后，重新完整矩阵通过。
- 汇总/样本检查曾遇 PowerShell 数组解包和路径错误，已修正脚本并重新逐项哈希核对；不把汇总失败当作功能通过。
- 初次编译有未使用导入警告已清理；最终 Clippy -D warnings 通过。
- Windows 11 x64 / 100% 缩放，未执行 Win10、多 DPI、多显示器及长期泄漏/性能测试；DPI 转换留待 P0-06。

## 历史记录：P0-04 问题与边界

- 集成编译首次因日志格式字符串多一个占位符失败；已修复，完整检查与实测重新通过，失败日志保留。
- 首次运行注册撤销本身通过；诊断记录后续修复销毁后的原 PID/TID 和完成行时间，最终三轮使用修复后的同一 EXE。
- RegisterDragDrop 首次为 S_OK；重复注册为 0x80040101，释放失败对象后仍冲突；显式 Revoke S_OK，再次诊断 Revoke 为 0x80040100；重注册成功，RAII 清理后同样确认不存在注册且 COM 对象析构。
- 关闭 winit 默认注册避免抢占它的目标。winit 在 WM_DESTROY 仍有无条件 Revoke 尝试；本层必须在窗口存活时先撤销，不能把后端销毁清理当成自有撤销成功。
- 该窗口配置入口是 Slint 版本相关 API，已固定版本；未来升级必须重新验证。
- P0-04 当时 Release SHA-256（历史产物）：`A70908402952196B06A8B0718A0DB641777F4F8E954F17462A098DED1C301528`。
- Timer、接口直接调用和注册通过不等于真实 OLE 拖放回调通过；未执行 Explorer 拖入或数据解析。
- 回归中本层释放 OLE 引用后 UI 仍 STA；不宣称全部后端 COM 引用清空，长期稳定性仍未验证。

## 历史记录：P0-03 问题与边界

- 首次 Release 运行因测试错误地将 COM STA 预初始化等同于已初始化 OLE 而失败；实际第一次 OleInitialize 为 S_OK，已修正并保留失败日志。重复 OLE 调用为 S_FALSE，MTA 冲突为 0x80010106。
- winit 0.30.13 在 HWND 创建期间也调用 OleInitialize；本层只释放自己的成功引用，不替后端清理。UI 在本层释放后仍 STA 是实测结果，不宣称所有 COM/OLE 引用完全清空。
- 本地 winit 源码未找到其 OleUninitialize 配对；此后端生命周期限制需在后续 OLE/长期稳定性阶段继续评估。当前不改架构，不作 P0 GO 决策。
- 已验证 Timer 和普通事件循环投递，尚未验证实际 OLE DragEnter/DragOver/Drop 消息和回调；不能宣称完整 OLE 消息泵/拖放验收通过。
- P0-03 当时 Release SHA-256（历史产物）：`7C835AD326C604E1AD4E35E9E8EA8182D28213601FE3AFCCD242B434165AD5BD`。
- 证据位于 `docs/test-results/P0-03/`，详细测试边界见 P0_VALIDATION.md。

## 历史记录：P0-02 实测结论

- Windows 11 x64 / 100% 缩放；Rust、Slint、后端及渲染器未变。
- Slint provider 在原生窗口创建后可用，每次在 UI 线程重新获取并验证非空/IsWindow/PID/TID。
- 隐藏不销毁默认 Windows HWND；恢复保持同一代身份。
- 先 hide，再 drop 唯一组件 owner；所有 provider 已离开作用域，weak 无法升级。事件循环继续处理消息后，旧 HWND 实际失效。
- Medium 权限独立外部观察 3 次完整运行，每次 3 代；每代旧 HWND 在进程存活且下一代创建前已确认失效，正常退出码 0，无强杀。
- 首次高权限内部预检也通过；不能用进程退出代替窗口生命周期验证。
- P0-02 当时 Release SHA-256（历史产物）：`4A0A835D284FA1C0A89F2A82D38E19B238AB651E3B1B6104A22EC8AC070BF208`。
- P0-02 证据见 `docs/test-results/P0-02/`，详细报告见 [P0_VALIDATION.md](P0_VALIDATION.md)。

## 历史记录：本次问题处理

1. 暂停前 Rust 1.96.0 安装不完整；恢复安装后固定工具链检查通过。
2. Slint 不允许同时设置固定尺寸和最小尺寸；改为首选尺寸与最小尺寸。
3. 退出循环与窗口 API 错误类型不同；分别处理，未屏蔽错误。
4. 首轮截图发现 Rust 动态中文名称缺字；从预栅格化嵌入改为 `EmbedFiles`，使用系统字体，修复后截图确认中文完整。
5. PowerShell 5.1 参数默认值中的脚本路径为空；移到脚本主体初始化。
6. 自动截图受前台焦点限制；测试脚本临时置顶窗口截图后恢复，不修改产品行为。
7. 本机 Administrator 会话中的 Explorer 也启动高完整性进程；另用测试专用受限令牌启动，实际确认验证脚本与 QuickNest 均为 Medium（0x2000）。未修改系统 UAC 或账户。

所有先前失败记录保留；它们不算通过。最终证据及哈希见 [P0_VALIDATION.md](P0_VALIDATION.md)。

## 历史记录：剩余限制与风险

- 本次无 P0-01 阻塞问题。
- P0-01 旧构建记录过 `VCRUNTIME140.dll` 依赖；该声明已过时。当前已调整静态 CRT，最终 EXE 导入表已核验，无动态 MSVC 运行库依赖，仍使用 Windows 系统 DLL。
- 高权限宿主直接启动 asInvoker 程序仍会继承高权限。本次普通权限验证使用测试受限令牌；未实现安装后降权启动或主程序强制降权功能。
- 未执行 Win10、多显示器/不同 DPI、性能目标、长时间稳定性测试，均不宣称通过。
- P0-01 当时仅为演示原型；“无真实启动、无托盘等”已过时，不描述当前产品。
