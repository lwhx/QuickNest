# QuickNest 1.2.0 发布说明（待发布验收）

更新日期：2026-11-07 ｜ 对齐基线：《QuickNest V1.2 正式开发基线》

QuickNest 是使用 Rust + Slint（software renderer、backend-winit）开发的 Windows 原生快捷启动器，无浏览器内核、无 Node/Python 运行时依赖。工程版本为 1.2.0。

**本轮代码修复、本地构建与打包核验已完成，P4 完整发布尚未通过。** 当前修复范围、最新验证与待补结果以 [本轮修复记录](docs/AUDIT_FIXES_2026-11-07.md) 为准；历史任务验收与旧产物哈希不能认证当前构建。最终源码门禁、Release 构建和打包核验均通过，完整哈希见本轮修复记录与 artifacts.json。

## 产品能力

- 启动 EXE、原始 LNK、文件/文件夹、URI、UWP（AUMID）及 MSI 广告快捷方式；支持项目 CRUD、Explorer 拖入和分组/分类管理。
- Ctrl+F 搜索、中文拼音与纯键盘操作；全局热键、托盘、固定与失焦隐藏；浅色/深色/跟随系统主题、透明度、圆角与按平台能力应用的背景材质。software renderer 的不透明客户区限制 Mica/Acrylic 的可见范围。
- 批量、命名组合、可取消的延时启动；仅实际启动成功更新 usage，支持最近使用/高频/收藏置顶排序；Everything 为可选集成。
- direct / CMD / PowerShell 命令模式，首次执行确认；编辑执行内容后重新确认，批量和组合跳过未确认命令。参数遵循 Windows CRT 规则，CMD 拒绝无法安全表达的字面输入。
- 原子保存、结构与 schema 校验、损坏恢复/安全模式、备份恢复及导入导出；合并导入同步组合 ID 引用，兼容 UTF-8 BOM，失败事务回滚。
- 配置、图标、备份与崩溃日志使用统一 data root，实例 namespace 与 IPC 隔离一致；IPC 交付包含 deadline 与 ACK。
- 本轮收口编辑 AUMID/MSI 元数据、drag scroll reset、模态屏障、焦点及 tooltip 状态。详细范围见修复记录。

## 打包与更新边界

安装版采用 Inno Setup 逐用户安装，提供开始菜单、可选桌面图标、可选开机启动与卸载入口。便携版由 `QuickNest.portable` 标记启用，数据位于 `<exe 目录>\data`，文件系统相对路径按便携根解析。本轮产物均已重建；构建通过不代表安装或完整桌面验收通过。完整 SHA-256 见本轮修复记录。

| 产物 | 路径 | 本轮状态 |
| --- | --- | --- |
| 安装包 | `installer/output/QuickNest-1.2.0-setup.exe` | 已重建并核验，完整哈希见修复记录 |
| 便携包 | `installer/output/QuickNest-1.2.0-portable.zip` | 已重建并核验，完整哈希见修复记录 |
| 裸程序 | `target/release/quicknest.exe` | Release 构建及 PE/提前 health 核对通过，完整哈希见修复记录 |

`runasoriginaluser` 依赖安装器可取得的原始上下文，不能保证所有启动方式都降到 Medium；`asInvoker` 会继承调用者权限。安装完成页的 Medium 启动实测仍待完成。

更新器采用 HTTPS 签名清单、ECDSA P-256、SHA-256 校验和 typed worker，应用更新需用户显式确认，并提供启动检查与设置开关。**当前仅支持 `raw` 更新包；`setup` 与 `portablezip`（ZIP）在下载前明确拒绝。** 仓库旧测试密钥已披露，生产路径不信任该身份；须在构建时配置独立发布公钥。生产 key、HTTPS 端点及两个不同生产版本之间真实退出换版仍待验收，不能用签名 fixture 或本地文件替换测试宣称生产更新完成。

## 最新验证与发布前置条件

已回传结果为 full lib **272 passed / 0 failed / 3 ignored**（panic helper 由父测调用、CMD native probe 由父测调用两次、signed_fixture 由独立脚本显式执行通过）、**51 个脚本 safe preflight + fixtures 通过**、**updater 11 + 签名 fixture 1 + transport 4 通过**，以及 **PS 5.1 临时 HTTPS manifest / 404 / 降级 redirect 验证通过**。这些集合存在重叠，不能相加；安全 preflight 不能改写为桌面回归通过，旧“48 脚本全绿”不能沿用为当前结果。

最终冻结源码门禁已通过：fmt、check all-targets、strict clippy；串行 cargo test --all-targets 为 lib 272 passed / 0 failed / 3 ignored、main 4 passed、integration 14 passed；doc 3 passed。更新器等定向专项与 full lib 存在重叠，不另累计。Release 构建、安装包与便携包打包及产物一致性核验已通过。P4 完整发布还须完成 Win10 验证、安装完成页 Medium 实测、两个不同生产版本真实退出换版、独立生产 key 与 HTTPS 端点验证；当前不宣称全矩阵验收。

## 系统目标与已知限制

- 目标平台为 Windows 10（1903+）/ Windows 11 x64；目标兼容范围不等于已完成平台矩阵验证。
- 安装和常规使用按逐用户权限设计；显式管理员启动与继承权限仍遵循 Windows 令牌语义。
- automation 项目类型仍为后续扩展；Rust panic 日志不等同于 SEH/minidump 全覆盖。

## 升级与卸载

- 安装版：手动运行新版 setup.exe 覆盖升级；卸载经“应用和功能”，用户数据默认位于 `%APPDATA%\QuickNest` 并保留。
- 便携版：退出程序后手动替换程序，保留 `data/` 与 `QuickNest.portable`；删除便携文件夹会一并删除其中的数据。
