# 2026-11-07 审核修复与当前验收状态

本轮代码修复、源码检查、本地 Release 构建和打包核验已完成。P4 完整发布尚未验收通过：Win10、完成页真实 Medium 启动、两个不同生产版本的完整换版，以及独立生产更新身份和端点仍待验证。历史任务记录、旧截图和旧哈希不能认证当前构建。

证据见 [本轮证据目录](test-results/audit-fixes-2026-11-07/README.md)：`final-gates.log`、`sandbox-preflight.log`、`release-pe.json`、`artifacts.json` 和 `packaging.log`。

## 修复清单

| 范围 | 当前行为 |
| --- | --- |
| 数据根和实例 | 配置、图标、备份和 crash 日志使用统一数据根。QUICKNEST_DATA_DIR 必须是非空绝对路径，无效时明确报错；未指定则选便携 data 或 APPDATA/QuickNest。Mutex 与 Pipe 共享测试 namespace，隔离用户实例。 |
| 测试隔离 | 每轮独立 sandbox；路径删除防越界、junction/reparse 和真实数据路径；进程与窗口清理核验登记 PID 和 StartTime。默认拒绝危险桌面回归，不删除 Notepad TabState、不按进程名强杀用户程序。用法见 [TEST_HARNESS.md](TEST_HARNESS.md)。 |
| 导入与组合 | 导出组合，导入建立项目 ID 映射并重写组合成员；覆盖导入替换旧组合，拒绝歧义与失效引用。 |
| 配置和备份 | BOM、Library 结构及 schema=2 校验统一；跳过坏备份寻找可恢复快照。保存、恢复、导入、设置和组合失败时回滚内存，保留错误与选择。 |
| 参数与命令 | Windows CRT 参数解析/序列化互逆，保留空串、引号和尾反斜杠。CMD 专用编码，拒绝无法保证字面语义的输入；Direct 拒绝隐式执行脚本。 |
| 编辑 | 仅同一 LNK 目标保留 AUMID/MSI 信息；执行类型、路径、参数、模式、工作目录和管理员设置变化后重新确认。 |
| 延时启动 | 排队只计待启动，实际成功才记录一次 usage；失败回传状态，退出取消未执行工作。 |
| IPC | 连接、发送和 ACK 共用 deadline；overlapped I/O 可取消，服务端 ACK 后等待客户端关闭或期限，不立即断管丢响应。 |
| crash 测试 | 独立子进程安装 panic hook，不修改测试进程全局环境或遗留 hook；日志遵循统一数据根。 |
| UI | 拖放目标随当前位置清空，命中计算包含侧栏滚动；模态 gate、Esc 取消、弹簧加载收尾、真实键盘焦点及互斥 tooltip 已接线。 |
| 更新 | 私有字段 VerifiedUpdate 才能授权 worker；启动检查、开关和显式确认已注册。UI 回调拒绝取消或过期请求；设置按实际保存状态生效。worker 等父进程退出、原子替换、精确版本 health、失败恢复，并按结果重启新/恢复旧程序，回滚失败不启动。 |

## 更新和安装边界

当前仅支持 raw 更新包；setup 和 portablezip 在下载前明确拒绝。仓库旧测试私钥已删除，已披露身份不被生产构建信任。仅构建时独立发布公钥可启用更新，默认清单 URL 为空；缺少配置时拒绝更新，不联系猜测域名。生产 key、私钥保管及 HTTPS 清单/包服务仍待部署与验收。

runasoriginaluser 使用安装器可取得的原始上下文；asInvoker 继承调用者权限。两者不能保证所有管理员启动场景变为 Medium。P4-03 脚本已删除普通 ShellExecute 模拟通过的结论；无法观察真实完成页令牌时记录 blocked 并非零退出。本轮未运行安装或桌面验收。

## 实际验证结果

| 检查 | 结果 |
| --- | --- |
| cargo fmt --all -- --check | 通过 |
| cargo check --locked --offline --all-targets | 通过 |
| cargo clippy --locked --offline --all-targets -- -D warnings | 通过 |
| cargo test --locked --offline --all-targets -- --test-threads=1 | lib 272 passed / 0 failed / 3 ignored；main 4 passed；integration 14 passed |
| cargo test --locked --offline --doc | 3 passed |
| cargo build --locked --offline --release | 通过，退出码 0，版本 1.2.0 |
| 安全脚本预检及 fixtures | 51 个脚本解析/契约通过；路径、环境保留、独立根及归属清理反例通过 |
| 更新专项 | updater/worker 原专项 11、独立签名 fixture 1、transport 4 通过；后续重启分支测试包含在最终 lib 汇总内 |
| PS5.1 临时 HTTPS | manifest、404、降级重定向不跟随验证通过，无系统证书库变更 |
| 本地产物 | Inno Setup 编译和便携 ZIP 打包通过；ZIP 四文件和空标记核对通过，内含 EXE SHA-256 与最终 Release 相同 |
| Release 程序核对 | AMD64 / PE32+ / Windows GUI / asInvoker；普通及延迟导入表无 VCRUNTIME/MSVCP/CONCRT 动态依赖；提前 health 入口在非法数据根环境下仍正确返回版本与 nonce |

3 个 ignored 是专用入口：panic helper 由父测试在独立进程调用；CMD argv probe 由父测试调用两次；签名 fixture 由独立脚本显式执行。保持默认 ignored 计数，专项与全量汇总重叠，不累计为额外通过数量。

开发过程中曾有 strict lint 与改名期间旧测试符号编译失败，均修复后重跑。上表对应最终冻结源码结果。旧“48 脚本全绿”不沿用；历史输入未送达或 WinHTTP err=87 不能无证据归因为防护组件。

## 本轮产物

| 产物 | 大小（字节） | SHA-256 |
| --- | ---: | --- |
| target/release/quicknest.exe | 18881536 | c38a699dc1b6a2abbc602fd9b1cd92419ee64e6ee67231af4db167755eceb624 |
| installer/output/QuickNest-1.2.0-setup.exe | 7530655 | aef57ca3168ac2d20a4f7035e41e7a5ffc4bc9b581ea5ad2f09de06b166c082e |
| installer/output/QuickNest-1.2.0-portable.zip | 7933645 | c72061cf719b438290e675bc625b27cad1d3f2537f6c9a59e96d6aa752dec41c |

本轮没有执行安装或卸载。源码已推送到 <https://github.com/lwhx/QuickNest>；上面两个产物已作为 **正式 Release `v1.2.0`** 发布（<https://github.com/lwhx/QuickNest/releases/tag/v1.2.0>，published_at 2026-11-07T03:06:42Z，标签指向提交 `2544ccb`）。两个附件经匿名公开接口下载回比对，与本地产物字节完全一致。

Release 的发布说明已列出下列尚未实测项，公开页面不作全平台验收声明。

## 仍待验收

- Win10（1903+）机器或虚拟机上的当前构建及平台降级行为。
- 标准用户环境中，提权安装器完成页真实启动应用的 PID / Medium 完整性证据。
- 两个不同生产版本之间完整的父进程退出、替换、正常重启及失败回滚链路；隔离文件与子进程反例测试不能替代该项。
- 独立生产发布公钥、HTTPS 端点及生产 WinInet 证书链端到端验证。
- 本轮 UI 修改后的完整隔离桌面交互及 DPI 回归。
