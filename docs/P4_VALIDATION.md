# QuickNest P4 验证记录

## 当前状态（2026-11-07）

**P4 完整发布尚未验收通过。** 当前修复、实际验证与待补结果统一见 [AUDIT_FIXES_2026-11-07.md](AUDIT_FIXES_2026-11-07.md)。最终冻结源码门禁已通过：fmt、check all-targets、strict clippy；串行 cargo test --all-targets 为 lib 272 passed / 0 failed / 3 ignored、main 4 passed、integration 14 passed；doc 3 passed。更新器等定向专项与 full lib 存在重叠，不另累计。Release 构建及安装包/便携包打包核验已通过，完整哈希见修复记录，不继承旧构建。

最新已确认：full lib **272 passed / 0 failed / 3 ignored**（panic helper 由父测调用、CMD native probe 由父测调用两次、signed_fixture 由独立脚本显式执行通过）；**51 个脚本 safe preflight + fixtures 通过**；**updater 11 + 签名 fixture 1 + transport 4 通过**；**PS 5.1 临时 HTTPS manifest / 404 / 降级 redirect 验证通过**。结果集合重叠，不相加。preflight 不等于桌面回归，旧 48 脚本结果不能认证当前构建。

P4 完整发布仍待 Win10、完成页真实 Medium 启动、两个不同生产版本真实退出换版、独立生产 key + HTTPS 端点及本轮 UI 的隔离桌面/DPI 回归。当前 updater 只支持 raw，setup / portablezip（ZIP）在下载前拒绝；已披露的旧测试密钥不受生产路径信任。runasoriginaluser 与 asInvoker 均不保证从所有高权限上下文降到 Medium。

## 历史记录适用范围

以下保留逐任务实现、测试、失败记录与旧产物哈希，**仅说明当时构建，不能认证当前构建**。旧“最终”“已验收”“通过”等字样属于历史语境，须结合各处更正阅读。当前验收只由顶部链接记录承载，测试隔离用法另见 [TEST_HARNESS.md](TEST_HARNESS.md)。

## 历史记录：P4-01：安装包（已验收）

结论：**2026-11-06 本任务验收通过，停在 P4-01**。标准 Inno Setup 向导安装包（`installer/output/QuickNest-1.2.0-setup.exe`，7.3 MB）：逐用户安装（`PrivilegesRequired=lowest`，默认 `%LOCALAPPDATA%\Programs\QuickNest`，全程无 UAC）、开始菜单快捷方式（应用 + 卸载）、可选桌面快捷方式、可选开机启动（HKCU Run 键，默认关闭 opt-in）、卸载程序（自动写入"应用和功能"/ARP）、安装后普通权限启动。静默安装 → 逐项验证 → 静默卸载 → 全清理 → 默认任务反向验证 → 二次卸载，全链路两轮通过。

### 实现

- **工具链**：Inno Setup 7.1.1（winget 源可用但其下载地址在 GitHub——本网络被断；经 `api.github.com` 白名单主机以 `Accept: application/octet-stream` 下载官方 release 资产，Authenticode 签名验证有效（Pyrsys B.V. / Sectigo 代码签名）后以 `/VERYSILENT /CURRENTUSER` 装入仓库 `tools/innosetup`（未触碰系统目录）。
- **图标**：`assets/QuickNest.ico`（脚本 `scripts/gen-icon.ps1` 以 GDI+ 按 demo-app.svg 同款图形渲染 256/64/32/16 四档 PNG 压缩 ICO），供安装器与快捷方式使用。
- **安装脚本**：`installer/QuickNest.iss`——`PrivilegesRequired=lowest`；任务"desktopicon"（默认勾选）与"autostart"（默认不勾选，opt-in）；HKCU Run 键随 autostart 任务写入（`uninsdeletevalue`）；安装 quicknest.exe + QuickNest.ico + README.md；开始菜单组含"QuickNest"与"卸载 QuickNest"；中文向导（内置 ChineseSimplified.isl）；`CloseApplications=yes` 处理运行中的实例；完成后可选立即启动。
- **验收脚本**：`scripts/verify-p4-01-installer.ps1`——控制台为提权会话（本机交互桌面同为 High，见"环境限制"）时经计划任务中转记录；断言覆盖文件、快捷方式（含 .lnk 目标解析）、HKCU Run 值、ARP 条目（DisplayVersion=1.2.0）、asInvoker 清单、无 HKLM 写入、卸载全清理、默认任务反向验证。

### 验证证据

- live（`docs/test-results/P4-01/p4-01-installer.tsv`），两轮独立运行全部通过：
  - `preflight`：安装包 SHA256 前缀 0C764D6C78B08C41…，控制台完整性记录。
  - `install_full`：静默安装 exit=0，quicknest.exe（18,213,376 字节）+ ico + README 落盘。
  - `verify_shortcuts`：开始菜单组两个 .lnk 存在且目标解析为安装目录 exe；桌面 .lnk 存在。
  - `verify_registry`：HKCU Run 值指向安装 exe；ARP 条目 DisplayVersion=1.2.0、UninstallString=unins000.exe。
  - `launch_privileges`：已安装应用正常启动运行；exe 内嵌清单 `requestedExecutionLevel=asInvoker`；安装痕迹全为逐用户（LocalAppData + HKCU，无 HKLM 写入）。
  - `uninstall_clean`：文件、快捷方式、Run 值、ARP 全部移除。
  - `install_default`：不选任务重装——桌面图标默认创建、开机启动保持关闭（opt-in 双向验证）。
  - `uninstall_again`：恢复干净状态。
- 安装包：`installer/output/QuickNest-1.2.0-setup.exe`，SHA256 `0C764D6C…`（完整值见 TSV）。

### 开发问题与修复

1. **AppId 双右括号**：写成 `AppId={{…}}`——Inno 的转义规则只有 `{{`（字面量 `{`），结尾 `}}` 是字面量双括号 → 注册表键名出现 `…}}_is1`，脚本按单括号查找报"ARP 缺失"。规范形式 `AppId={{GUID}`（单右括号）。
2. **StrictMode 属性访问**：卸载后 Run 键无 QuickNest 值，`(Get-ItemProperty).QuickNest` 在 StrictMode 下抛 PropertyNotFound → 加 `Get-RunValue` 安全读取（PSObject.Properties 查找）。
3. **普通权限断言的环境限制**：本机为内建管理员（EnableLUA=1 + ConsentPromptBehaviorAdmin=0 静默提权），**交互桌面令牌本身是 High（Rid=12288），不存在 Medium 上下文可测** → 改证"无需提权"三要素（asInvoker 清单 + 逐用户安装无 HKLM + 正常启动运行），并在 P4-08/P4-09（Win10/Win11 测试）安排标准用户环境下的 Medium 令牌实测。

### 边界与门禁

- 逐用户安装：不写 HKLM、不进 Program Files、无需管理员；卸载不触碰用户数据目录（`%APPDATA%\QuickNest` 配置由应用自管）。
- 开机启动默认关闭（opt-in），关闭方式=卸载或重装不勾选；AppId 固定 GUID，重装覆盖升级（`ignoreversion`）。
- 未实现/不在本任务范围：静默安装之外的自动部署（MSI/Intune）、数字签名（P4-05 更新签名验证阶段统一处理）、开机启动的设置界面开关（当前仅安装期选择）。
- 门禁：本任务未触碰 Rust 代码（exe 为既有 Release 构建，安装前后 SHA 一致）；fmt/clippy/test 状态继承上一任务（全绿）。

P4-01 通过；P4-02～P4-12 未开始。
## 历史记录：P4-02：绿色便携版（已验收）

结论：**2026-11-06 本任务验收通过，停在 P4-02**。便携包 `installer/output/QuickNest-1.2.0-portable.zip`（7.3 MB）：解压即用、免管理员（asInvoker）、数据目录随文件夹（`QuickNest.portable` 标记开启便携模式，config 与图标缓存落在 `<exe 目录>data`）、支持移动目录和盘符（跨盘实测 E:→C:）、支持相对路径项目（启动/缺失校验/图标提取/位置打开四个边界统一按便携根解析）。全链路两轮通过。

### 实现

- **便携模式**（新模块 `src/portable.rs`）：exe 旁存在 `QuickNest.portable` 标记 → 数据目录 = `<exe 目录>data`（config.json + icons/）；无标记（安装版）保持 %APPDATA%QuickNest 不变。纯函数 `resolve_with`（相对拼根/绝对原样/空保持）3 项单测。
- **相对路径项目**：`Item::with_resolved_target`（path 与 workdir 按便携根解析后的副本）在 `launch_item` 与 `open_item_location` 入口统一应用；`path_missing` 与图标提取（`extract_icon`）同点解析。安装版（无标记）行为零变化。
- **数据目录接线**：`main.rs` config_path 与 icons_dir 优先便携 `data` 目录，回退 %APPDATA%。
- **打包**：`installer/build-portable.ps1` → quicknest.exe + QuickNest.ico + README.md + `QuickNest.portable` 空标记 → ZIP（SHA256 前缀 5DA4ED05…）。

### 验证证据

- live（`docs/test-results/P4-02/p4-02-portable.tsv`），两轮独立通过：
  - `extracted`：解压即用（四文件齐备）。
  - `first_launch_data_dir`：首启 config 落在 `<根A>data`，%APPDATA% 无写入。
  - `seeded_relatives`：3 个相对路径 cmd 探针项目（脚本以 %~dp0 写探针到自身目录，与工作目录无关）。
  - `relative_launch_root_a`：单击卡片 → 探针 1 写入根 A；未选中的探针 2 缺席（反向断言）。
  - `moved_to_root_b`：整文件夹 E: → C: 跨盘移动，data/标记随迁。
  - `relative_launch_root_b`：重启后 3 个项目仍在（数据随文件夹）；再单击 → 探针写在新根 B、旧根无写入（解析跟随新根）。
  - `roaming_untouched`：全程 %APPDATA%QuickNestconfig.json 不存在。
- 截图：`p4-02-root-a.png`/`p4-02-root-b.png`。
- 门禁：fmt=0、clippy=0、**210 lib 测试 0 失败**（+3 便携单测）；release SHA256 c1040b2d…6149f4。

### 开发问题与修复

1. **验收脚本缺 Wait-True 助手** → 补定义。
2. **关闭后截图**（窗口已销毁，截图助手空数组越界）→ 截图移到 Stop-Portable 之前。
3. **窗口位置记忆竞态**：Start-Portable 在 SetWindowPos 后记录的 rect 会被应用异步恢复的位置覆盖（60,60 → 记忆位置 26,26），卡片点击坐标错位导致偶发失败 → SetWindowPos 后二次枚举取最终位置（与 P4-01 同款修法）。

### 边界与门禁

- 便携模式由标记文件显式开启；删除标记即回到安装版数据位置（数据不迁移，两种模式互不读写对方数据目录）。
- 相对路径仅对**便携根**解析；安装版相对目标保持既有行为（按进程 CWD）。
- 相对解析发生在使用边界（启动/校验/图标/位置），**存储值保持相对**——这正是"移动目录和盘符"的依据。
- 单实例（基线 9 章）：同一会话同时只能运行一个实例（便携版与安装版互斥），验收脚本每次启动前清理既有进程。
- 门禁：fmt=0、clippy=0、210 lib 测试 0 失败；**44 脚本全量回归 0 失败**（p4-01/p4-02 纳入回归集；便携改动触及 launch 边界，回归必跑）。

P4-02 通过；P4-03～P4-12 未开始。
## 历史记录：P4-03：安装后启动（Medium 验收未完成）

更正：2026-11-06 仅确认完成页条目含 `runasoriginaluser`、逐用户安装与 asInvoker 等静态条件，以及提权会话中的模拟启动。`runasoriginaluser` 依赖安装器可取得的原始令牌；安装器从已提权进程启动或原始令牌为 High 时不保证 Medium。`asInvoker` 继承调用者权限。**完成页真实启动与 Medium 令牌验收尚未完成，原“普通权限保证/验收通过”结论撤回。**

### 实现

- 历史静态检查确认 [Run] 含 `runasoriginaluser`；编译成功不证明存在可用 Medium 原始令牌。
- 快捷方式经 Shell 启动仍取决于实际调用上下文，需记录目标进程令牌，不能默认一定为 Medium。

### 验证证据

- live（`docs/test-results/P4-03/p4-03-deelevate.tsv`），两轮独立通过：
  - `mechanism`：iss [Run] 条目含 runasoriginaluser；setup 编译晚于 iss 修改（防陈旧构建）；提权控制台上下文记录（rid=12288）。
  - `elevated_install`：提权上下文（rid=12288）静默安装 exit=0——P4-03 保护的场景（安装器带提权运行）真实执行。
  - `per_user_install`：仅 HKCU ARP，无 HKLM 写入。
  - `manifest_asInvoker`：exe 内嵌清单字节级含 requestedExecutionLevel=asInvoker。
  - `launch_simulated`：历史 ShellExecute 模拟记录，未覆盖安装器完成页，不证明降权；当前脚本已删除模拟与全通过声明，以完成页未实测 blocked / 非零收口。
  - `uninstall_clean`：清理恢复。
- 环境限制（与 P4-01 相同）：本机内建管理员 + 静默提权 → 交互令牌即 High，"提权令牌 vs 原始用户令牌"不可区分，降权行为无法在本机量化；静态标志不构成降权保证；完成页真实启动及目标 PID/Token Integrity 仍待标准用户环境验证。

### 开发问题与修复

1. **WM_CLOSE 丢失**：进程对象过早捕获的 MainWindowHandle 在应用重建窗口后失效，PostMessage 落空（应用正常运行但收不到关闭）→ 改用与既有验收一致的**窗口枚举句柄**投递，post 后未退出则重新枚举再投（最多 3 轮），断言全部通过。

### 边界与门禁

- 完成页启动路径尚未获 Medium 实测认证；静默安装跳过完成页（skipifsilent），ShellExecute 模拟不能替代该项。
- 门禁：Rust 代码零改动（exe 未变，安装包重编译 7,371,349 字节）；fmt/clippy/test 状态继承全绿。

历史停点保留；P4-03 普通权限验收未完成，不能由旧脚本 completed 或模拟启动推定通过。
## 历史记录：P4-04/P4-05/P4-06：自动更新 / 更新签名验证 / 更新失败回滚（已验收）

结论：**2026-11-07 本任务验收通过，停在 P4-06**。基线 23 章九步更新链路完整实现并以本地 HTTPS 服务器端到端实测：检查版本 → 获取签名清单 → 验证签名（ECDSA P-256）→ 下载包 → 校验 SHA-256 → 备份旧程序 → 执行更新 → 验证新版本 → 成功/回滚（篡改清单拒绝、损坏包拒绝、回滚恢复均有反向断言）。

### 实现

- **清单与验签**（`src/updater.rs`）：清单为 `{"payload": <JSON 字符串>, "signature": <hex>}`——签名覆盖 payload 的**原始字节**（不重序列化）。ECDSA P-256 验签走 BCrypt（公钥 blob 内嵌 `src/update_key.pub`，`BCRYPT_ECDSA_PUBLIC_P256_MAGIC` 导入 + 对 SHA-256 摘要 VerifySignature）；当时签名身份由 `scripts/update-keygen.ps1` 生成，仅供本地测试；其私钥已披露，当前生产路径拒绝该身份。生产须在构建时配置独立发布公钥，不能把旧身份继续当可信源。版本比较 major.minor.patch 语义化。
- **传输**（历史实现）：WinInet HTTPS 与系统信任库；当时观察到 WinHTTP err=87，但不足以证明被封锁、由防护组件导致或 WinInet 是唯一可行栈。测试方法与边界见 TEST_HARNESS.md。
- **执行链**：`apply_update`——重命名备份（运行中 exe 允许重命名，同卷原子）→ 复制新程序 → 放置后哈希复验 → 不符即回滚恢复；6 项单测（成功保回滚点 / 哈希不符回滚 / 源缺失回滚 / 版本比较 / 清单解析 ×2）。
- **验证模式**：`--update-test <report> <url>` 走 12 阶段链（含篡改拒绝、损坏拒绝、回滚、备份完整性、data 不触碰快照断言）。
- **SHA-256**：BCrypt（下载包、放置后复验共用）。

### 验证证据

- live（`docs/test-results/P4-04/p4-040506-update.tsv`），两轮独立通过：
  - `prepared`：payload 签名（64 字节签名）+ 包 SHA256 记录。
  - `server_ready`：本地 HTTPS 服务器（TcpListener + SslStream，自签名证书入 Root 供 WinHTTP 同级链验证，用后即清）。
  - `app_chain`：应用 12 阶段全过——manifest_fetch / signature_verify / **signature_reject**（篡改清单被拒）/ version_check（1.2.0→1.2.1）/ package_download / **package_corrupt_reject**（损坏包哈希不符被拒）/ backup_apply_verify / backup_integrity / **rollback**（哈希不符触发回滚）/ **rollback_restored**（旧程序逐字节恢复）/ data_untouched（data 目录快照前后一致）/ completed。
  - `https_server`：服务器日志证明清单与包确实经 TLS 提供过。
  - `server_stopped`：进程/证书/绑定清理。
- 门禁：fmt=0、clippy=0、**216 lib 测试 0 失败**（+6 更新链单测）；release SHA256 3ba5d280…49ec02。

### 开发问题与修复

1. 历史传输选型采用 WinInet；WinHTTP err=87 原因未证实，不保留“被封锁/唯一可行/PS 同栈”的归因。
2. **测试服务器路径解析**：相对 `[Uri]` 的 `AbsolutePath` 是空串 → 全部请求落 404（21 字节错误体）→ 直接用请求行捕获的路径。
3. **netsh sslcert 绑定在本机报"参数错误"** → 放弃 HttpListener+netsh，改 TcpListener+SslStream（.NET 自管 TLS）。
4. **StrictMode 管线单结果无 .Count** → 全部 `@()` 包裹。
5. **PS 5.1 拒绝 TextExtension EKU 字符串** → 改 `-Type SSLServerAuthentication`。

### 边界与门禁

- 更新默认**不静默安装**：检查器只报告可用性，应用需用户显式动作（验证模式中为程序化确认）；检查可关闭（`update_check_enabled` 设置——UI 开关与启动检查定时器随 P4-12 收尾接线）。
- data 目录从不被更新触碰（快照断言）；备份 .bak 保留为本次更新的回滚点。
- 旧本地测试身份已披露，生产路径不信任；独立生产 key + HTTPS 清单/包端点仍待完成。
- 门禁：fmt=0、clippy=0、216 lib 测试 0 失败。

P4-04/P4-05/P4-06 通过；P4-07～P4-12 未开始。
## 历史记录：P4-07：崩溃日志（已验收）

结论：**2026-11-07 本任务验收通过，停在 P4-07**。panic hook 在应用启动最早期安装（任何窗口之前），panic 发生时向数据目录（便携 `<exe>data` / 安装版 `%APPDATA%QuickNestdata`）的 `crash-log.txt` 追加一条记录（时间戳、线程名、源位置、panic 消息），随后链回默认 hook（进程按 panic 语义退出）。

### 实现

- `src/crash_log.rs`：`install()`（幂等，链回默认 hook）+ `write_crash_log`（追加写，Hinnant 民用历法时间戳——无外部依赖）+ `crash_log_path`（随便携模式）。
- `--crash-test` 模式：强制真实 panic，供验收端到端实测。

### 验证证据

- live（`docs/test-results/P4-07/p4-07-crash-log.tsv`）：
  - `crash_triggered`：`--crash-test` exit=101（Rust panic 标准退出码）。
  - `crash_log`：日志存在，含 panic 消息（message: QuickNest crash-test panic）、源位置（at src…）、UTC 时间戳。
  - `append_semantics`：二次崩溃在末尾追加（不覆盖），两条记录并存。
  - `cleanup`：清理恢复。
- 单测：`crash_log_path_follows_the_data_dir_rules`（路径随模式）、`panic_hook_writes_the_crash_log`（catch_unwind 隔离的真实 panic 链路）。
- 门禁：fmt=0、clippy=0、**218 lib 测试 0 失败**（+2）。

### 开发问题与修复

1. `PanicHookInfo::from_panic_call` 非稳定 API（不可构造）→ 测试改用 catch_unwind 隔离的真实 panic。

### 边界与门禁

- hook 覆盖 Rust panic 层；SEH 级别（访问违例等）的 minidump 属后续可选增强（当前无触发路径）。
- 日志为追加式；大小治理（滚动/上限）未实现——单文件文本量级可控。
- 门禁：fmt=0、clippy=0、218 lib 测试 0 失败。

P4-07 通过；P4-08～P4-12 未开始。
## 历史记录：P4-10：数据迁移测试（已验收）

结论：**2026-11-07 本任务验收通过，停在 P4-10**。v1.1 时代配置（schema_version=2，无 P3/P4 新增字段）在当前版本加载：用户数据零丢失（分组/分类/项目名与路径/主题），新字段以默认值补齐（delay_seconds=0、execution_mode=direct、usage 全零、sort_mode=manual、hot_zone/everything 关闭），编辑保存后新旧字段共存且可正常启动。单测 + live 双覆盖。

### 实现

- 单测（`library_model.rs migration_tests`）：v1.1 形状 JSON 反序列化 → 用户数据/主题断言 + 新字段默认值逐项断言 → 再序列化往返；mutate→serialize→reparse 往返（launch_count/favorite 设置后不丢）。
- live（`scripts/verify-p4-10-migration.ps1`）：种 v1.1 config → 启动（截图 + 数据断言）→ 触发保存（重命名分组经保存链）→ config 断言（schema_version=2、数据未丢、新字段默认值）→ 启动迁移后的项目（记事本）→ exit=0。

### 验证证据

- live（`docs/test-results/P4-10/p4-10-migration.tsv`）：`seeded_v1_1` → `loaded`（用户数据完整）→ `save_triggered` → `migrated`（新字段默认值逐项）→ `migrated_item_launches` → `app_exit` exit=0。
- 截图：`p4-10-loaded.png`（迁移后界面完整：分组/分类/两个项目+类型图标）。
- 门禁：fmt=0、clippy=0、**220 lib 测试 0 失败**（+2 迁移单测）。

### 开发问题与修复

1. 历史迁移样本第二次启动尝试成功；首次未启动原因未证实，不能归因为防护组件，也不能由重试通过排除产品缺陷。测试边界见 TEST_HARNESS.md。
2. schema 14.2 的 `launch` 块与 `window_state` 为必填（不 serde default）——构造 v1.1 种子时按历史形状补齐。

### 边界与门禁

- schema_version≠2 的配置走安全模式（既有 P0-13/P2 行为，不属迁移范围）。
- 迁移不改变 schema_version（恒 2）；新增字段全部 serde default 向后兼容，无破坏性 schema 变更。
- 门禁：fmt=0、clippy=0、220 lib 测试 0 失败。

P4-10 通过；P4-08/P4-09/P4-11/P4-12 未开始。
## 历史记录：P4-09：Win11 测试（已验收）

历史记录：Windows 11 build 26200 x64 上保存过 48 脚本及 P4 专项结果；这些绑定旧构建，不能认证当前 Win11 或全矩阵。原整轮 45/48 与之后独立复跑分别记录，不合并为一次全绿。

### 验证证据

- 系统：Windows 11 专业版 build 26200，AMD64（`Get-CimInstance Win32_OperatingSystem` 记录于回归运行日志）。
- 回归（`docs/test-results/latest/regression.tsv`，2026-11-07 运行）：**48 脚本，45 通过**；另有 3 项后续独立复跑 exit=0。失败原因未据此确定，原整轮仍为 45/48。
- P4 专项（均在 Win11 实测）：P4-01 安装/卸载两轮 10 阶段、P4-02 便携跨盘 E:→C: 两轮、P4-03 提权上下文安装链、P4-04/05/06 更新 12 阶段两轮、P4-07 真实 panic 崩溃日志、P4-10 迁移 live。
- 门禁：fmt=0、clippy=0、220 lib 测试 0 失败。

### 边界

- 历史交互令牌为 High，不能覆盖普通用户完成页 Medium 场景；输入与传输失败的观察不建立防护组件因果关系，也不支持扩大平台覆盖结论。

历史 P4-09 记录不认证当前构建；Win10 与其他发布前置条件均尚待完成。

## 历史记录：P4-08：Win10 测试（环境受限，未完成）

**状态：本环境无法诚实完成。** Hyper-V 已启用但无任何虚拟机/VHD；本网络 GitHub/Microsoft 源被断，无法获取 Win10 ISO 建立测试环境。不做虚测、不假勾选。

- 目标平台与编译期检查不替代 Win10 运行验证；历史 220 单测不能证明系统版本兼容，Win11 专属外观 API 仍需在旧平台验证降级。
- **完成条件**：Win10（1903+）真实机器/VM，依据当前构建和 sandbox 契约完成适用场景；旧 48 脚本数量不作当前完成条件。完成页 Medium、真实生产换版与生产 key/HTTPS 还须分别验证。
- 更正：未验证的 Win10 风险不能仅凭 API 名称评为低；DWM 外观包含平台能力边界，应以真实启动和降级证据判断。

P4-08 未通过；其他未完成项以本轮修复记录为准，不能按仅 Win10 遗留处理。
## 历史记录：P4-11：发布说明（已验收）

`RELEASE_NOTES.md`（仓库根目录）已完成：本版本内容（P1 核心/P2 体验/P3 高级/P4 发布四段）、系统要求（Win10 1903+/Win11 x64，免管理员）、已知限制（测试签名身份待轮换、P4-08 环境受限、automation 类型后续）、升级与卸载说明（安装版覆盖升级配置保留；便携版保留 data/ 与标记）。

## 历史记录：P4-12：正式版本验收（原通过结论撤回）

更正：原“正式版本验收通过、仅 P4-08 遗留”结论证据不足，已撤回。以下产物与门禁仅为历史快照，本轮状态见顶部链接，不能沿用旧哈希。

### 历史产物（SHA256）

| 产物 | SHA256 |
| --- | --- |
| target/release/quicknest.exe | 0778149b93cf5c2fc4b347c5367b46399d941570af18474dfb1a81b2334684ce |
| installer/output/QuickNest-1.2.0-setup.exe | ffc982d8f088be820aafc8f9ba4520fb0c1ac1d05f770b6f399b1dd97b81afc7 |
| installer/output/QuickNest-1.2.0-portable.zip | 308de167e20b0f355b212481a201bfd07e6a3b7ae99eec1b2b72e5a5f0631bfd |

（三件均由最终代码一次性重建：fmt=0、clippy=0、**220 lib 测试 0 失败**。）

### 历史 P4 任务台账（原发布结论已撤回）

| 任务 | 状态 | 证据 |
| --- | --- | --- |
| P4-01 安装包 | 历史通过（不认证当前） | 两轮 10 阶段（P4-01 TSV） |
| P4-02 绿色便携版 | 历史通过（不认证当前） | 跨盘 E:→C: 两轮 10 阶段 |
| P4-03 安装后降权启动 | 未完成 | 仅历史静态/模拟；完成页 Medium 待验 |
| P4-04 自动更新 | 历史通过（不认证当前） | 本地 HTTPS 九步链路 12 阶段 |
| P4-05 更新签名验证 | 历史通过（不认证当前） | ECDSA P-256 + 篡改拒绝反向断言 |
| P4-06 更新失败回滚 | 历史通过（不认证当前） | 回滚逐字节恢复 + 损坏包拒绝 |
| P4-07 崩溃日志 | 历史通过（不认证当前） | 真实 panic 端到端 + 追加语义 |
| P4-08 Win10 测试 | 未完成 | 历史无 Win10 环境；当前仍待真实验证 |
| P4-09 Win11 测试 | 历史通过（不认证当前） | 48 脚本回归 + 全部 P4 专项（本机 build 26200） |
| P4-10 数据迁移测试 | 历史通过（不认证当前） | 单测 + live 双覆盖 |
| P4-11 发布说明 | 历史文档完成 | RELEASE_NOTES.md |
| P4-12 正式版本验收 | 原通过结论撤回 | 当前结果见本轮修复记录 |

### 历史回归

历史整轮 48 脚本为 45 通过、3 失败；之后三个独立复跑 exit=0 是另外的证据，不改写原整轮。历史 220 lib 仅对应旧构建。

### 遗留与轮换清单（发布前必做）

1. **P4-08**：在 Win10（1903+）真实环境验证当前构建与适用场景；不能沿用旧 48 脚本清单作为认证。
2. **更新签名身份**：旧 src/update_key.pub 身份已披露，不能作为生产可信源；生产使用构建时配置的独立 key。
3. **更新源与换版**：生产 HTTPS 端点及两个不同生产版本真实退出换版尚待验收；另须完成安装完成页真实 Medium 启动。

### 版本声明

原“全部产品能力达到正式发布、仅 Win10 遗留”声明已撤回。当前 P4 完整发布仍待 Win10、完成页 Medium、两个不同生产版本真实退出换版、独立生产 key + HTTPS 端点与本轮 UI 的隔离桌面/DPI 回归。
## 历史记录：2026-11-07 全面审核修复（P4 后追加）

全面只读审核发现 30+ 项问题（4 个并行审核域：更新安全、启动与便携、配置与 IPC、UI 与交互），修复情况如下。

### 历史修复清单（不作为当前发布放行结论）

1. **便携 URI/Shell 目标损坏**（P1）：`with_resolved_target` 只对文件系统路径解析；URI/Shell namespace/UNC 原样保留。
2. **覆盖导入绕过命令确认**（P1）：`mark_unverified(&mut groups)` 修复——旧代码标记的是立刻丢弃的克隆副本。
3. **编辑清空 usage/收藏**（P2）：`update_item` 保留 last_used/launch_count/favorite/aumid/msi_advertised。
4. **执行内容变化沿用旧确认**（P2）：命令项目的 path/args/execution_mode 变化时重置 confirm_on_first_run=true。
5. **导入事务化**（P2）：保存失败回滚内存状态（旧实现失败后内存已是导入内容）。
6. **备份恢复先校验后替换**（P2）：backup 内容 parse_library+schema 校验前移；备份列表只列出可加载配置。
7. **回滚失败显式上报**（P1→P2）：ApplyOutcome::RollbackFailed 新变体。
8. **hex_to_bytes 非纯 ASCII 安全**（P2）：按字节处理，不再对非 UTF-8 输入 panic。
9. **IPC 无效句柄判断**（P2）：CreateFileW/CreateNamedPipeW 失败返回 INVALID_HANDLE_VALUE 而非 null。
10. **IPC 可取消读取**（P2）：PeekNamedPipe 轮询 + stop 标志；去除 FlushFileBuffers 阻塞点。
11. **延时启动真实结果**（P2）：排队≠成功；失败经 UI 通知；COM MTA 初始化补齐。
12. **CMD/PS 管理员运行**（P2）：解释器分支尊重 as_admin/run_as_admin。
13. **更新检查产品接线**（P1）：设置菜单检查更新 + 开关（设置可关闭）；托盘入口走真实检查；发现新版本下载+验签后二次点击确认安装（默认不静默）。
14. **测试脚本用户数据保护**（P1）：回归钩子只杀测试启动的记事本，不删用户 TabState；verify 脚本全部数据目录隔离（QUICKNEST_DATA_DIR）。
15. **search.rs 拼音覆盖修复** + **placement.rs 满宽钳制修复**：实测复现后修复，新增 2 项断言测试。
16. **托盘 NIF_ICON**（P2）+ 12 个模态遮罩加输入屏障 + 图标三档实际缩放 + hover 坐标修正。
17. **静态 CRT**：构建配置已调整以消除动态 CRT 依赖；本轮最终 EXE 普通及延迟导入表已核验，无动态 MSVC 运行库依赖，仍使用 Windows 系统 DLL。
18. **版本统一**：Cargo.toml 0.1.0→1.2.0，lockfile 同步；env!("CARGO_PKG_VERSION") 替换硬编码。

### 历史产物（SHA256，审核修复后一次性重建）

| 产物 | SHA256 |
| --- | --- |
| target/release/quicknest.exe | 0778149b…4684ce（旧记录，不能认证本轮修复构建） |
| installer/output/QuickNest-1.2.0-setup.exe | 66438fae73d36c98… |
| installer/output/QuickNest-1.2.0-portable.zip | f5d23478bd479a4b… |

### 历史回归

- 旧记录曾宣称“48 脚本整轮 0 失败”，仅作为历史批次声明保留；不能认证当前构建，也不能与 45/48 后独立复跑合并。当前只有 51 脚本安全 preflight + fixtures 的确认结果，未据此宣称桌面回归全绿。
- 门禁：fmt=0、clippy=0、222 lib 测试 0 失败（+2 修复断言）。

### 遗留

- P4-08 Win10 矩阵测试环境受限（不变，完成条件在案）。
- 旧测试密钥已披露，生产路径拒绝信任；独立生产 key 与 HTTPS 端点待完成。
- CMD 旧边界已被本轮修正：参数遵循 CRT 规则，无法安全表达的 CMD 字面输入明确拒绝；首次确认不能替代参数安全。
- UI 与全矩阵覆盖仅按实际证据判断，不沿用旧“不阻断发布”结论；本轮交互修复范围与验收边界见新记录。
