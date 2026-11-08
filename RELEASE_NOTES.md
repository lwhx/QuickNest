# QuickNest 1.2.1 本地优化候选说明

更新日期：2026-11-08 ｜ 对齐基线：《QuickNest V1.2 正式开发基线》

QuickNest 是使用 Rust + Slint（software renderer、backend-winit）开发的 Windows 原生快捷启动器，无浏览器内核、无 Node/Python 运行时依赖。工程版本为 **1.2.1**。

**v1.2.1 已于 2026-11-08 发布到 GitHub Releases（tag `v1.2.1`，附件 setup 与便携 ZIP 的哈希见优化记录交付表）；七项优化通过统一门禁与成对性能验证。P4 完整发布矩阵尚未验收通过。** 本轮范围与最终证据入口统一见 [1.2.1 优化记录](docs/OPTIMIZATIONS_1.2.1.md)。旧测试数量、快测结果和旧产物哈希不能认证当前源码或产物。

已公开 **v1.2.0** 继续保留历史发布身份，附件和历史哈希不变。替换前的根发布说明已逐字节保留于 [1.2.0 历史说明](docs/releases/1.2.0.md)，其中旧“待发布”“本轮”等措辞仅记录当时语境；历史发布不意味着 P4 全矩阵验收完成。历史构建证据见 [2026-11-07 记录](docs/AUDIT_FIXES_2026-11-07.md)，后续五项修复见 [2026-11-08 记录](docs/AUDIT_FOLLOWUP_2026-11-08.md)。

## 产品能力

- 启动 EXE、原始 LNK、文件/文件夹、URI、UWP（AUMID）及 MSI 广告快捷方式；支持项目 CRUD、Explorer 拖入和分组/分类管理。
- Ctrl+F 搜索、中文拼音与纯键盘操作；全局热键、托盘、固定与失焦隐藏；浅色/深色/跟随系统主题、透明度、圆角与按平台能力应用的背景材质。software renderer 的不透明客户区限制 Mica/Acrylic 的可见范围。
- 批量、命名组合、可取消的延时启动；仅实际启动成功更新 usage，支持最近使用/高频/收藏置顶排序；Everything 为可选集成。
- direct / CMD / PowerShell 命令模式，首次执行确认；编辑执行内容后重新确认，批量和组合跳过未确认命令。参数遵循 Windows CRT 规则，CMD 拒绝无法安全表达的字面输入。
- 原子保存、结构与 schema 校验、损坏恢复/安全模式、备份恢复及导入导出；合并导入同步组合 ID 引用，兼容 UTF-8 BOM，失败事务回滚。
- 配置、图标、备份与崩溃日志使用统一 data root，实例 namespace 与 IPC 隔离一致；IPC 交付包含 deadline 与 ACK。

## 1.2.1 七项优化范围

以下七项已实现，行为回归、性能效果及覆盖边界见优化记录中的实际证据。

1. 重复选择且状态无变化时零写入；成功启动记录以 `LaunchRecord` 批量事务保存；备份内容使用有界 BLAKE3 校验缓存，保留 schema、BOM 与最近 10 份有效备份语义。
2. 手动排序快速返回，其余模式以 first-rank map 保留首次位置、稳定并列与既有排序语义。
3. 图标完成只用 `set_row_data` 更新对应行；后台队列容量 32、非阻塞提交，并核对当前 model generation，失败按冷却策略重试。
4. 搜索每次查询复用 `PreparedQuery`，借用拼音 cache，跳过不适用于拼音的查询，保持匹配和排序语义。
5. `PillButton` 与设置入口完善键盘焦点、激活和无障碍信息。
6. 正式 `modal_ui` 共用注册路径，覆盖 `NameDialog` 与可滚动项目编辑器，保持正式界面的模态屏障及焦点行为。
7. Cargo 单一版本来源 1.2.1，实际冻结输入的来源指纹内嵌 EXE；两种包装共用 staging，以独立入口核验本地 EXE、setup 和 ZIP。

## 本地构建与验证入口

在仓库根目录执行，先备妥本机依赖和 Inno Setup 编译器。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-release.ps1
# <run> 使用构建入口实际返回的目录。
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-release-artifacts.ps1 -ReleaseDirectory <run>
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-release-artifacts.ps1 -Fixture
```

默认禁用更新 key。输出目录规则为 `installer/output/release-1.2.1-<guid>/`；实际目录、manifest 名称、大小、哈希和验证结果统一记录在 [优化记录](docs/OPTIMIZATIONS_1.2.1.md)。

构建记录允许 dirty 工作树，准确标识实际文件指纹与基准 HEAD；HEAD 本身不能冒称未提交源码的构建来源。来源指纹说明本地过程的一致性，不构成第三方签名或跨机器可复现构建证明。

独立核验区分 EXE 内嵌来源、ZIP 内部文件与字节一致性，以及 setup 的冻结输入和编译证据。setup 尚未安装、未独立解包，不能据此宣称其安装内容已实机验收。安全测试命令及测试设施排障见 [TEST_HARNESS.md](docs/TEST_HARNESS.md)，发布前置条件见 [P4 验证记录](docs/P4_VALIDATION.md)。

## 打包、更新与待验边界

安装版采用 Inno Setup 逐用户安装，提供开始菜单、可选桌面图标、可选开机启动与卸载入口。便携版由 `QuickNest.portable` 标记启用，数据位于 `<exe 目录>\data`，文件系统相对路径按便携根解析。

`runasoriginaluser` 依赖安装器可取得的原始上下文，不能保证所有启动方式都降到 Medium；`asInvoker` 会继承调用者权限。安装完成页真实 Medium 启动仍待验收。

更新器采用 HTTPS 签名清单、ECDSA P-256、SHA-256 校验和 typed worker，应用更新需用户显式确认，并提供启动检查与设置开关。**当前仅支持 `raw` 更新包；`setup` 与 `portablezip`（ZIP）在下载前明确拒绝。** 仓库旧测试密钥已披露，生产路径不信任该身份。独立生产 key、HTTPS 端点及两个不同生产版本之间真实退出换版仍待验收，签名 fixture 不替代生产更新。

目标平台为 Windows 10（1903+）/ Windows 11 x64；Win10 与当前 UI 的实机 DPI、多显示器验证仍未完成。automation 项目类型仍为后续扩展；Rust panic 日志不等同于 SEH/minidump 全覆盖。

## 升级与卸载

- 安装版：退出程序后手动运行新版 setup.exe 覆盖升级；卸载经“应用和功能”，用户数据默认位于 `%APPDATA%\QuickNest` 并保留。
- 便携版：退出程序后手动替换程序，保留 `data/` 与 `QuickNest.portable`；删除便携文件夹会一并删除其中的数据。
