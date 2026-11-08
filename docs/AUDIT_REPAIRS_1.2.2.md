# QuickNest 1.2.2 审核修复记录

更新日期：2026-11-08。基线提交：`946565d2098ec775cfc5087b4e1d93a806e7622e`。本地修复分支：`fix/1.2.2-audit-followup`。

状态：**阶段 1–7 实施与验收完成，阶段 8 统一门禁通过（fmt/严格 clippy/安全集合），1.2.2 本地交付进行中。** 工程版本已更新为 1.2.2（仅包版本与 lock 对应条目，普通依赖未变）。既有 GitHub v1.2.1、标签、附件与来源记录保持原样。

保持 Rust + Slint + Win32、配置 schema 2、原子保存、失败回滚、有效跨分类选择、命令确认、raw-only 更新及默认禁用生产更新 key。每阶段相关编译和回归通过后才进入下一阶段；未测试项目不记通过。

## 问题对照

### 已复现的数据与交互缺陷

- [x] A1：导出拒绝活动配置同文件目标，使用原子提交。（阶段 1）
- [x] A2：备份读取失败保留，不视为内容无效。（阶段 1）
- [x] A3：保护本次创建的备份与恢复/导入回退点。（阶段 1）
- [x] A4：全库替换后协调选择，拒绝失效 ID 空壳组合。（阶段 2）
- [x] A5：固定与窗口位置使用事务、失败反馈和正确重试。（阶段 2；位置失败后按 tick 重试最新观测，安全模式放弃）- [ ] A6：托盘设置显示窗口并进入正式设置面板。（阶段 3）
- [ ] A7：标题栏获得焦点后仍可使用 Ctrl+F/Esc。（阶段 3）

### 源码调用链问题

- [x] B1：互动模式原生关闭与自绘关闭统一隐藏。（阶段 3：验证模式保留退出路径）
- [x] B2：热区为完整屏幕矩形内的窄带，副屏内部不误触发。（阶段 3：hot_zone_at_edge + 四向副屏/带内/带外几何回归）
- [x] B3：图标成功缓存可重验、失效和有限逐出。（阶段 4：条目/字节双上限 LRU、每 tick 一次预算化重验、同像素零通知、编辑/删除精确失效、导入/恢复全量失效。注：网格行持有像素克隆，预算覆盖缓存侧；整行随刷新重建）（阶段 4）
- [x] B4：便携图标身份、磁盘键与实际提取对象一致。（阶段 4：cache_key 经 portable root 解析后再取元数据，替换文件即换键；相对/绝对/未解析三向反例）
- [x] B5：保存组合后的卡片、计数及菜单同步。（阶段 2：`sync_batch_selection_ui` 统一入口）
- [x] B6：自动排序禁止分类内重排，跨分类移动仍可用。（阶段 2：controller 拒绝 + 界面提示）
- [x] B7：键盘选中的搜索结果完整进入视口。（阶段 3：Rust 侧 reveal 几何 + 双向 scroll-offset 属性；揭示/翻页/跳转回归通过）
- [x] B8：脚本错误标型不能绕过确认和专用参数编码。（launch_item 分发前硬拒绝 ScriptTypeMismatch；导入重新标记 confirm）
- [x] B9：退出统一结算已完成的延时结果与统计。（托盘 Exit 与事件循环收尾均改为 cancel→drain→按批提交→persist；统计失败不遮蔽其余保存）
- [x] B10：覆盖安装取消自启时移除本应用旧 Run 值。（deletevalue 分支 + 注册表双分支静态契约）
- [x] B11：实际发布 profile 选项固定并进入构建证据。（九键配方 v2 指纹、CARGO_PROFILE_* 清除/固定/恢复、build.rs 与 PS 规范串逐字节一致）

### 更新条件路径

这些路径要求配置独立发布公钥并获得有效签名 raw 包。修复与受控验证不代表生产两版本换版验收。

- [x] C1：worker 准备及就绪等待离开 UI 线程。（prepare_in_background + 通道回传，确认按钮立即返回）
- [x] C2：交接后结果可见，失败与回滚位置可交付。（启动轮询 handoff/outcome/failed 标记并显示）
- [x] C3：按受验证的归属记录清理已结束交接的自有文件。（containment 校验 + worker 存活保护 + acknowledge 幂等）
- [x] C4：异步确认排队，不叠加编辑模态或穿透键盘焦点。（确认走独立 update-confirm 模态，any-modal-open 屏障覆盖）

### 使用体验

- [x] D1（Rust 侧完成）：TaskQueue/TaskView/cancel/seal 快照 API 就绪；UI 队列面板列为后续（当前延时任务保留既有结果提示语义）
- [x] D2：集中保留错误并提供重放原操作意图的重试。（阶段 2：checked 变体保留 pending；pin/位置/统计重试按各自意图；完整 status_ui 留待阶段 3 D3 汇合）
- [x] D3（部分）：设置页显示已配置热键与实时注册状态；隐藏提示跟随实际热键；数据目录/崩溃日志/备份入口已就位。**暂缓**：不重启即时换绑（候选注册→保存→切换的事务需要专门的热键冲突测试设施，记录为后续项）
- [x] D4：导入预览、删除和覆盖导入的有条件短时撤销。（阶段 2：预览两步确认 + 单槽撤销，合并导入仍走备份路径）
- [x] D5（部分）：滚动揭示、PageUp/PageDown/Ctrl+Home/End 导航、文字编辑快捷键保留已落地。**暂缓**：命中区间高亮——Slint 1.18 无文本测量 API，无法把字符区间映射到像素位置；需自绘或引入测量方案，另行评估
- [x] D6：完整冻结归档可迁移重验，当前源码检查独立且严格。（-Archive 凭 source-files 快照离线验证、移动/改名可验、-Legacy 明确 v1 限制）

## 阶段与验收

| 阶段 | 状态 | 实际验证证据 |
|---|---|---|
| 1. 配置与备份保护 | **通过 2026-11-08** | [stage-01-config-backup.log](test-results/2026-11-08-audit-repairs-1.2.2/stage-01-config-backup.log)：`cargo test --locked --offline --lib -- library_controller:: infrastructure::windows::config_save:: infrastructure::windows::file_identity:: --test-threads=1` 退出码 0，**91 通过 / 0 失败**（含 9 项本阶段新增回归），fmt 全绿 |
| 2. 事务、选择、预览与撤销 | **通过 2026-11-08** | [stage-02a](test-results/2026-11-08-audit-repairs-1.2.2/stage-02a-selection-transactions.log) controller 64/64；[stage-02c](test-results/2026-11-08-audit-repairs-1.2.2/stage-02c-preview-undo.log) controller 67/67（预览/提交/取消/撤销）；[stage-02d](test-results/2026-11-08-audit-repairs-1.2.2/stage-02d-ui-gate.log) 主程序 29/29 + ui_layout 8/8；fmt 全绿 |
| 3. 窗口、键盘、搜索与设置 | **通过 2026-11-08（高亮暂缓）** | [stage-03a](test-results/2026-11-08-audit-repairs-1.2.2/stage-03a-window-keys.log) 33/33 + [stage-03b](test-results/2026-11-08-audit-repairs-1.2.2/stage-03b-search-navigation.log) 35/35 + [stage-03c](test-results/2026-11-08-audit-repairs-1.2.2/stage-03c-hotkey-maintenance.log) 36/36 + ui_layout 8/8，fmt 全绿。A6/A7/B1/B2/B7/D3/D5-nav 落地；D5 命中高亮因 Slint 1.18 无文本测量 API 暂缓（理由见 D 条目），D3 即时换绑暂缓 |
| 4. 图标身份与内存预算 | **通过 2026-11-08** | [stage-04a](test-results/2026-11-08-audit-repairs-1.2.2/stage-04a-icons.log) bin 38/38 + icon_queue 15/15；[stage-04b](test-results/2026-11-08-audit-repairs-1.2.2/stage-04b-icons-lib.log) lib 343/343（含便携键一致性）+ icon_cache/extract，fmt 全绿 |
| 5. 启动、任务队列与退出 | **通过 2026-11-08** | [stage-05a](test-results/2026-11-08-audit-repairs-1.2.2/stage-05a-launch-tasks.log) 定向 104/104、交叉 lib 363/363；B8 守卫/确认/导入标记 + TaskQueue/shutdown_settle 落地，B9 退出结算接线（托盘 Exit 与事件循环收尾） |
| 6. 更新交接与模态 | **通过 2026-11-08（C4 模态排队随既有模态屏障覆盖）** | [stage-06a](test-results/2026-11-08-audit-repairs-1.2.2/stage-06a-update-handoff.log) updater 23/23 + update_net 4/4；C1 异步准备（UI 线程不再阻塞）、C2 启动消费交接/失败可见、C3 受限清理（containment+worker 存活保护）接线 |
| 7. 安装、构建与归档校验 | **通过 2026-11-08** | [stage-07-fixtures](test-results/2026-11-08-audit-repairs-1.2.2/stage-07-fixtures.log) 69 项负例 + 快照直验；[stage-07-preflight](test-results/2026-11-08-audit-repairs-1.2.2/stage-07-preflight.log) 96 入口含新 release 分类。构建中断由主线程接管修复（fixture 裸调用污染返回流 → [void] 抑制；缺直接快照负例 → 补充） |
| 8. 统一回归、性能与本地交付 | 进行中 | [gate-clippy](test-results/2026-11-08-audit-repairs-1.2.2/gate-clippy.log) -D warnings 通过；[gate-safe-tests](test-results/2026-11-08-audit-repairs-1.2.2/gate-safe-tests.log) lib 362+main 38+ui_layout 8+icon_queue 15+集成 12 全绿；[gate-doc](test-results/2026-11-08-audit-repairs-1.2.2/gate-doc.log) doc 通过。1.2.2 打包进行中 |

原审核基线的反例日志与正常回归记录保留在 [本轮证据目录](test-results/2026-11-08-audit-repairs-1.2.2/README.md)。修复后的结果按阶段追加，不能以历史通过数替代。

## 验证边界

只使用自有临时目录、明确归属进程、无桌面测试后端和受控效果端口。继续排除真实剪贴板 round-trip、共享 Shell 集成、旧桌面坐标脚本和当前用户环境的安装卸载、UAC、证书、系统 DPI 或用户进程清理。

Win10、真实 Medium、实机四档 DPI/多屏、真实覆盖安装和生产换版仍按实际执行情况记账；编译、静态契约、headless 或 fixture 不替代相应平台验收。测试设施说明单独维护于 TEST_HARNESS，本文只保留产品行为及真实结果。
