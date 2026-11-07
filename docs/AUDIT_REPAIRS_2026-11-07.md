# 2026-11-07 再次审核修复记录

起点：`84c95cf`。本轮按复审清单修复，不修改既定 Rust + Slint + Win32 架构，不更改配置格式。GitHub 上已发布的 v1.2.0 附件不自动替换；源码修复与发布附件的状态分开记录。

## 修复与验收清单

| 序号 | 审核问题 | 已修复并通过对应回归的行为 |
| --- | --- | --- |
| 1 | 非空分类删除在确认前崩溃 | 释放控制器可变借用后生成确认信息，目标分类不进入迁移列表 |
| 2 | 未知 schema 的新结构被当损坏恢复 | 先识别版本；不兼容版本进入只读安全模式，不恢复覆盖原配置 |
| 3 | 旧主题脚本删除真实配置 | 封堵危险入口；所有可执行 PowerShell 入口纳入安全分类，旧坐标测试不得冒报通过 |
| 4 | 设置两个关闭按钮无效 | 右上关闭与底部关闭均隐藏面板并解除模态屏障 |
| 5 | 子对话框被设置遮挡 | 打开备份、导入导出、更新确认时隐藏设置，显示对应目标 |
| 6 | 拖放命中滚动视口外分组 | 先裁剪可交互视口，再补偿滚动；固定下拉区、留白和间距不得作为分组目标 |
| 7 | 迁移当前分组后活动 ID 悬空 | 将当前活动引用切换至迁移目标；保存及重载后仍有效 |
| 8 | 外观选项高亮不同步 | 保存成功后同步外观与选中快照；保存失败回滚并明确显示错误 |
| 9 | Tab/输入框取得焦点后 Esc 无效 | 模态控件的共同祖先接收未消费的 Esc，不依赖兄弟节点抢焦点 |
| 10 | 分组下拉按钮越过窗口边界 | 分组滚动区按剩余空间分配，固定下拉完整落在窗口内 |
| 11 | IPC 客户端提前断开终止服务 | 将提前断开作为可恢复会话错误，后续连接仍可正常处理 |
| 12 | IPC 接受错误版本与未知字段 | 仅接受版本 1 的严格协议帧；错误帧返回拒绝 ACK 且不得派发命令 |

## 实际验证结果

| 检查 | 结果与范围 |
| --- | --- |
| 格式、全目标编译、严格 Clippy | `cargo fmt --all -- --check`、`cargo check --locked --offline --all-targets`、`cargo clippy --locked --offline --all-targets -- -D warnings` 全部通过 |
| 库测试（安全集合） | **282 passed / 0 failed / 3 ignored / 1 filtered out**。唯一主动过滤项为旧的真实剪贴板 round-trip，避免覆盖用户剪贴板 |
| 主程序测试 | **8 passed**，包含 4 条新增回归：空/非空分类删除，真实设置关闭/子面板切换，五类外观同步与落盘/失败回滚 |
| 集成测试（安全集合） | **16 passed**，覆盖文件拖放介质、图标、专属 LNK 探针、MSI 检测、COM 以及 4 条新 UI 行为测试 |
| 文档测试 | **3 passed** |
| 配置定向回归 | controller 39、model 58、import 14、save 20 全部通过，包含未知结构/BOM/大版本号零写入、迁移后重载和真实文件锁回滚；已包含在上述库测试中，不重复累计 |
| IPC | 原有 13 + 新增 3 = **16 passed**，错误帧不派发，提前断开后后续请求成功；已包含在库测试中 |
| 脚本入口 | 本机实际枚举 **91 个脚本**，36 BLOCKED、41 verification-contract、3 support-library、9 utility、2 infrastructure；静态检查及隔离 fixtures 通过，非桌面验收通过数量 |
| 旧截图助手 | `target/ui-smoke.ps1` 已无条件拒绝，实测非零退出，不能再报告伪通过 |

统一测试命令：

```powershell
cargo test --locked --offline --lib --bin quicknest --test ui_layout --test file_drop_data --test icon_cache --test icon_extract --test lnk_launch --test msi_advertised --test ole_mta -- --test-threads=1 --skip clipboard_round_trip
cargo test --locked --offline --doc
```

未执行 `special_shell`、`uwp_link` 两个会操作共享 Shell 的旧集成测试，未执行桌面、安装或卸载脚本。这些排除项不计为通过。3 个默认 ignored 中，panic 和 CMD argv 子探针由父测试实际调用；外部签名 fixture 本轮未单独重跑，保留 ignored。

UI 回归使用正式组件的 Slint 无桌面后端，实际派发指针/Tab/Space/Esc，验证两个关闭按钮、输入框初始焦点与输入、焦点控件销毁后的面板切换、搜索清空后关闭，以及 0/2/20 分组在 680/760/1200 宽度下的布局。Rust 回调测试使用独立临时配置文件，检查真实落盘与失败回滚。验证不以截图存在代替行为成立。

证据：[本轮测试日志目录](test-results/audit-repairs-2026-11-07/README.md)。测试入口与隔离约束另见 [TEST_HARNESS.md](TEST_HARNESS.md)。

## 本地构建产物

`cargo build --locked --offline --release` 通过（退出码 0）。修复后的 [quicknest.exe](../target/release/quicknest.exe) 大小为 **19,619,840 字节**，SHA-256：

```text
16061497f87761444ef055ca93e41bc346f073243b971d057e7dcfae8449fa40
```

PE 检查为 AMD64 / PE32+ / Windows GUI，清单 asInvoker；普通及延迟导入表无动态 MSVC 运行库依赖。这个本地 EXE 与已发布安装包/便携包分开记录，本轮未重新打包或上传发布物。

## 发布与实机覆盖边界

本轮不安装、卸载或替换远端 Release 附件。Win10、实际多显示器、100/125/150/175% 系统 DPI，以及生产更新端到端验证仍需独立环境验收；无桌面后端布局及键盘回归不替代这些实机项目。
