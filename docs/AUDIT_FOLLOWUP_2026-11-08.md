# 2026-11-08 审核后续修复

起点：`483ecda`。本轮处理后续审核确认的五项问题；沿用既有 Rust + Slint + Win32 架构及配置格式。未确认的候选项不作为修复依据，前次已撤回的拖放边界与外观提示顺序误报没有修改。

## 修复清单

| 问题 | 修复后的行为 | 对应验证 |
| --- | --- | --- |
| 组合启动失败反馈不完整 | 摘要显示失败数量和项目名称；未确认命令单独计为跳过；延时任务显示“已排队等待启动”；结果支持换行、滚动 | 正式 UI 回调实际启动两个不存在的临时 EXE，检查失败数、名称、跳过数、错误标志和 usage 不增加 |
| 跨分类/分组批量选择丢失 | 保留全局选择。当前可见项按网格顺序优先，其余仍存在的已选项按原选择顺序处理；不再静默变成 0/0 后清空 | 4 条控制器回归覆盖视图切换、四种排序、失效 ID 和空选择；UI 回调检查隐藏分类项目仍计入结果 |
| 恢复/覆盖导入后运行状态不一致 | 成功提交后同步固定抑制、标题栏固定状态、主题、强调色、透明度、圆角、背景与网格；失败不改变运行状态 | 实际写入隔离备份和导入文件，验证调色板、固定、选项值与失焦策略；非法导入保持原状态 |
| 组合删除保存失败却提示成功 | 保留组合、列表和选择，显示“删除失败”；仅成功提交后显示“已删除”并清除失效选择 | Windows 文件共享锁阻止原子替换，检查内存/磁盘不变和错误状态；释放锁后再次删除成功 |
| auto_hide_on_blur 设置未生效 | 启动、配置恢复和轮询均同步开关；关闭时忽略失焦并取消待执行复核，显式显隐与其它抑制规则仍保留 | 20 条状态机测试，包括禁用、重新启用、过期复核、pinned、其它 suppression、显式显隐和关机 |

恢复或导入后，热区计时器读取当前配置，即使启动时未开启，也可在恢复后启用；关闭或切换边缘时清除旧驻留状态。此项本轮未实测真实光标触边。快捷键内容变化会明确提示“快捷键变更需重启后生效”，不暗示现有系统注册已自动替换。

## 验证

新增正式 UI 回调回归均使用独立临时目录、真实配置文件与 Slint 无桌面后端，不启动用户应用、不修改用户配置、剪贴板或共享 Shell。组合删除失败使用真实 Windows 文件锁，不以伪造返回值替代。

定向结果：`ui_regression_tests` **8 passed / 0 failed**，跨分类选择专项 **4 passed / 0 failed**，`window_state` **20 passed / 0 failed**。这些集合将包含在统一测试结果内，不重复累计。

统一门禁实际结果：

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo check --locked --offline --all-targets` | 通过 |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | 通过 |
| 库测试安全集合 | **291 passed / 0 failed / 3 ignored / 1 filtered out** |
| 主程序测试 | **12 passed** |
| 所选集成测试 | **16 passed**，包括 4 条正式 UI 布局与焦点回归 |
| 文档测试 | **3 passed** |

统一测试命令：

```powershell
cargo test --locked --offline --lib --bin quicknest --test ui_layout --test file_drop_data --test icon_cache --test icon_extract --test lnk_launch --test msi_advertised --test ole_mta -- --test-threads=1 --skip clipboard_round_trip
cargo test --locked --offline --doc
```

唯一主动过滤项为旧剪贴板 round-trip；未选择 `special_shell` 和 `uwp_link` 两个共享 Shell 集成测试。3 个默认 ignored 中，panic 和 argv 子探针由父测试实际调用，外部签名 fixture 本轮未单独重跑。未执行的项目不计为通过。

## 本地产物与证据

`cargo build --locked --offline --release` 通过，退出码 0。本地 [quicknest.exe](../target/release/quicknest.exe) 为 **19,668,480 字节**，SHA-256：

```text
c27baa3c68667d2aee848556d4ebb2499ffe4c69f285a561bde31ee3d95fe517
```

PE 检查：AMD64 / PE32+ / Windows GUI，清单 asInvoker，普通及延迟导入表无动态 MSVC 运行库依赖。完整输出保存在 [本轮证据目录](test-results/audit-followup-2026-11-08/README.md)。

## 交付范围

本记录按用户后续要求随修复代码一同提交到仓库；提交源码不替换 GitHub Release 附件。测试后端验证不代替实机 DPI、多显示器、共享 Shell 和生产更新端到端验收。
