# QuickNest 1.2.1 本地优化记录

更新日期：2026-11-08。工程版本 **1.2.1**，当前为本地优化候选；七项实施已完成并通过统一源码门禁、安全回归、性能对照与本地交付校验。本文不继承历史构建通过数、快测统计、包大小或哈希，全部最终结果见下文实际证据。

当前入口：[README](../README.md)、[候选说明](../RELEASE_NOTES.md)、[开发进度](DEVELOPMENT_PROGRESS.md)、[P4 验证记录](P4_VALIDATION.md)。测试设施、安全命令与排障统一见 [TEST_HARNESS.md](TEST_HARNESS.md)。已公开 v1.2.0 的附件及历史哈希保持不变，原发布说明完整保留于 [releases/1.2.0.md](releases/1.2.0.md)；此前 [五项后续修复](AUDIT_FOLLOWUP_2026-11-08.md) 和 [发布构建](AUDIT_FIXES_2026-11-07.md) 只认证各自历史批次。

## 七项实施范围

以下描述本轮拟保持的产品行为及实现范围，验收结论由后文最终证据补充。

| 项目 | 本轮变化 | 必须保持的语义 |
| --- | --- | --- |
| 1. 配置写入、启动记录与备份 | 无变化的分组/分类选择零写入；成功启动记录用 `LaunchRecord` 批量事务提交；备份内容使用有界 BLAKE3 校验缓存 | 失败事务回滚；仅实际成功启动计 usage；继续校验 schema/BOM，最近 10 份有效备份，不用元数据代替内容校验 |
| 2. 排序 | manual 快速路径；其余模式使用 first-rank map | 保留首次位置和稳定并列；最近使用、高频、收藏及手动排序语义不变 |
| 3. 图标 | 完成结果只用 `set_row_data` 更新对应行；容量 32 的有界队列、非阻塞提交；校对当前 model generation 并加入失败冷却 | 迟到结果不写入已切换模型；失败保留默认图标并按冷却重试；不因图标完成重建整个列表 |
| 4. 搜索 | 每次查询复用 `PreparedQuery`，借用拼音 cache，跳过无效拼音查询 | 名称、拼音、路径及模糊匹配优先级与结果排序保持既有语义 |
| 5. 键盘与无障碍 | `PillButton`、设置入口完善焦点、键盘激活与 a11y 信息 | 正式控件可聚焦、激活，禁用态与可访问名称一致 |
| 6. 模态界面 | 正式 `modal_ui` 共用注册；覆盖 `NameDialog` 和可滚动 editor | 正式回调、模态屏障、输入焦点与长表单操作保持一致 |
| 7. 版本与本地交付 | Cargo 单一版本 1.2.1；冻结实际输入的来源指纹内嵌 EXE；setup/ZIP 共用 staging；独立核验三件本地产物 | 准确记录 dirty 工作树；基准 HEAD 与实际指纹区分；各层证据与未执行项分别记录 |

## 统一验证

<!-- FINAL_VALIDATION_1_2_1：2026-11-08 冻结输入实际输出。 -->

以下为 2026-11-08 对冻结输入执行的实际结果；完整命令、逐目标统计、ignored/filtered 记账与日志见 [验证记录](test-results/2026-11-08-optimizations-1.2.1/VALIDATION.md)。定向测试与统一集合重叠处不累计。

| 验证域 | 最终证据 |
| --- | --- |
| fmt / check all-targets / strict clippy | 三项均退出码 0：`cargo fmt --all -- --check`、`cargo check --locked --offline --all-targets`、`cargo clippy --locked --offline --all-targets -- -D warnings` |
| 安全库、主程序、所选集成及 doc 测试 | lib 324（3 ignored）、bin 29、ui_layout 8、icon_queue 13、file_drop_data 6、icon_cache 1、icon_extract 2、lnk_launch 1、msi_advertised 1、ole_mta 1、doc 3，全部 0 失败；显式跳过 clipboard_round_trip，未选择 special_shell/uwp_link |
| 配置零写入、批量 usage 事务、备份内容/schema/BOM/有效保留 | `repeated_valid_selections_do_not_write`、`launch_batch_*`、`failed_launch_batch_rolls_back`、备份内容变化/坏备份过滤/恢复保护等 controller 与 config_save 回归通过 |
| 排序、搜索结果语义 | `sort_mode_tests`（首项/并列/重复/缺失 ID/rank map 对照）与 `search`/`pinyin_index`（Unicode 小写、拼音资格、路径竞争、改名/导入/恢复失效）回归通过 |
| 正式图标增量、队列/generation/冷却 | icon_ui 13 项（有界预算、补投、断连、过期 generation、共享 source、仅回填 icon、失败重提默认图标单次回退）与 worker/cache 8 项通过 |
| 正式控件键盘/a11y、modal/NameDialog/滚动 editor | ui_layout 8 项（PillButton 焦点/单次激活/禁用、设置入口、共用 NameDialog、编辑器双尺寸滚动）与正式 modal/delay 12 项通过 |
| 脚本安全 preflight 与产物验证 Fixture | 96 入口解析分类通过（36 BLOCKED 保持）、7 个旧 runner 执行前非零拒绝；产物校验 39 项拒绝反例 + 来源映射正例通过 |

Slint 保留一条非阻塞提示（导出的非 Window `PillButton` 不生成独立 Rust 组件）；正式界面使用与导入组件测试正常，Rust 严格 Clippy 通过。本轮发现并修正一处发布脚本缺陷：PowerShell 自动变量 `$input` 与循环变量重名导致首次构建在读取暂存来源清单时失败；已改为命名变量、补入来源映射正例与 2 项拒绝反例后重建。首次失败日志保留于 [build-release.log](test-results/2026-11-08-optimizations-1.2.1/build-release.log)，不作为交付。

## 前后性能

<!-- BEFORE_AFTER_1_2_1：2026-11-08 成对实际测量。 -->

基线 `7b7c313`（工程版本 1.2.0，经 `git archive` 隔离提取）与优化后工作树使用同一测量工具 `examples/performance.rs`、相同合成输入（BLAKE3 指纹逐 case 一致）和相同样本口径（clone 15、serialize 7、record/save 与 prune 各 5、重复选择 3、行构建 3、冷搜索 3、热搜索 7，取中位数）。环境为 Windows 11 x64、Rust 1.96.0、Release、文件系统缓存预热；"冷"指进程内拼音缓存未准备，非冷磁盘。完整表格、局限与复跑命令见 [性能对照](test-results/2026-11-08-optimizations-1.2.1/PERFORMANCE.md)。

| 比较项 | before | after | 结论 |
| --- | --- | --- | --- |
| 无变化选择（三次重复选择新增备份） | 每规模 3 份 | 每规模 0 份 | 零写入，配置字节不变；近零耗时只报零写入，不算加速倍数 |
| 记录一次启动并保存（5,000 项） | 149.0761 ms | 68.7576 ms | 备份维护同步受益：87.0 → 26.0 ms |
| 手动顺序行构建 | 1,000 项 4.6221 ms；10,000 项 438.5755 ms | 1,000 项 0.3404 ms；10,000 项 5.1943 ms | 平方级排序消除 |
| 冷空查询 | 10,000 项 74.1964 ms | 3.4503 ms | 空查询跳过索引准备 |
| 冷拼音 / 热拼音查询 | 10,000 项 76.006 / 12.7961 ms | 68.295 / 8.9624 ms | 一次归一化与借用缓存；拼音构建本身占比最大 |

数据层生产实现测量后未改动（仅修正两处新增测试的预期分数），上述 after 数据无需重测。结果只覆盖隔离合成工作负载，不代表真实 UI 端到端延迟，不保证其他硬件相同耗时。原始数据 [before.json](test-results/2026-11-08-optimizations-1.2.1/before.json)、[after.json](test-results/2026-11-08-optimizations-1.2.1/after.json)、逐 case 对比 [comparison.json](test-results/2026-11-08-optimizations-1.2.1/comparison.json)。

## 本地交付

<!-- LOCAL_DELIVERY_1_2_1：2026-11-08 实际 run 与三件产物证据。 -->

本地构建命令为 `scripts/build-release.ps1`，默认禁用更新 key。实际 run 目录：`installer/output/release-1.2.1-7ac1990619d44342bc857cdc984763f2`。该目录含 `source-manifest.json`、`build-manifest.json`、`staging-manifest.json`、`setup-evidence.json`、`release-manifest.json`、`cargo-build.log`、`inno-compile.log` 及冻结只读 `staging/`。

| 证据项 | 最终记录 |
| --- | --- |
| 实际 run 目录 | `installer/output/release-1.2.1-7ac1990619d44342bc857cdc984763f2` |
| 准确 manifest 名称及相互引用 | 见上列七个文件；独立校验逐项核对引用路径与哈希 |
| base HEAD / dirty / 实际输入来源指纹 / 构建指纹 | `7b7c3133b239c3c7115f6f63c9e6cf45ddbb8be7` / `true` / `e79865610ac646e2cc893b4d039a4cbdcb28047a898d978c348545bc2806e610` / `35b45410fb803a4be53b26fc121129389381f335eadd6fed972d40ee41725487`；冻结输入 191 个文件（含 16 个未提交源码与构建脚本） |
| EXE 内嵌版本及来源核对、提前 health 入口 | `--quicknest-build-info` JSON 与清单逐字段一致（update_key `disabled`）；`--quicknest-update-health 1.2.1 <nonce>` 协议保持；5 组错误参数非零拒绝；非法相对数据根下零文件初始化。9 项探针见 [release-cli-probes.json](test-results/2026-11-08-optimizations-1.2.1/release-cli-probes.json) |
| 本地 EXE、setup、ZIP 路径/大小/SHA-256 | `staging/quicknest.exe` 19,884,544 字节 `a0c21f5d37f80cf82ae1982774f7b5fbe978e20a1d63ef157bfaa3c7b4c0668d`；`QuickNest-1.2.1-portable.zip` 8,232,528 字节 `c5e4d666a70b8ed1ea0dd9616050088cd7051d7b1250432bb2851291c7cb5d79`；`QuickNest-1.2.1-setup.exe` 7,679,667 字节 `5f45379a60abfc5a0b3ea8f5822db01f6e3c242e3c76229ece08fb60275c8763` |
| ZIP 内部文件、标记、重复项及字节一致性 | 独立校验确认 4 个规范条目、空 portable 标记、无重复/额外条目、逐字节哈希与冻结 staging 一致 |
| setup 冻结 staging 输入及编译器/输出证据 | `setup-evidence.json` 记录冻结 staging manifest、ISCC.exe 身份与参数、编译日志及输出哈希；编译退出码 0 |
| 独立验证命令、退出码与日志 | `scripts/verify-release-artifacts.ps1 -ReleaseDirectory <run>` 退出码 0，日志 [verify-release-final.log](test-results/2026-11-08-optimizations-1.2.1/verify-release-final.log)；构建入口内部亦执行同一校验 |
| setup 安装/独立解包/桌面验收 | 未执行，不能由编译与输入一致性代替 |

实际冻结输入、EXE 内嵌来源与共享 staging 用来核对本地过程一致性。允许 dirty 工作树并准确标记实际指纹；base HEAD 仅为基准提交，不能冒称未提交源码构建来源。该来源记录不构成第三方签名或跨机器可复现构建证明。

ZIP 内部验证和 setup 的输入冻结/编译记录具有不同覆盖范围：setup 尚未安装、未独立解包，不能标其内容已实机验收。最终本地交付记录也不替代完整 P4 发布矩阵。

## 既有未验收项目

Win10 实测、安装完成页真实 Medium 启动、两个不同生产版本真实退出换版、独立生产 key + HTTPS 端点，以及当前 UI 的实机 DPI/多显示器回归仍待完成。`runasoriginaluser` 取决于可用原始令牌，`asInvoker` 继承调用者权限，均不保证所有上下文都降到 Medium。

更新只支持 raw，setup/portablezip 在下载前拒绝；旧测试密钥已披露，不受生产路径信任。测试后端、fixture、临时 HTTPS 和本地来源一致性各按实际覆盖范围记录，P4 完整发布不据此提前放行。
