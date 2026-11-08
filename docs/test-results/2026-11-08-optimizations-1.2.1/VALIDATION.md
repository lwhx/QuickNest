# 1.2.1 冻结源码验证

日期：2026-11-08。环境：Windows 11 Professional x64（build 26200）、Rust/Cargo 1.96.0、Slint 1.18.1、MSVC x64。工程版本 1.2.1，配置 schema 仍为 2。所有下表最终命令退出码均为 0。

| 门禁 | 实际结果 | 日志 |
|---|---|---|
| `cargo fmt --all -- --check` | 通过 | [fmt-frozen.log](fmt-frozen.log) |
| `cargo check --locked --offline --all-targets` | 通过 | [check-frozen.log](check-frozen.log) |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | 通过 | [clippy-all-targets.log](clippy-all-targets.log) |
| 最终安全测试集合 | 下表全部通过 | [safe-tests-final.log](safe-tests-final.log) |
| `cargo test --locked --offline --doc` | 3 项 compile-fail 文档测试通过 | [doc-tests.log](doc-tests.log) |
| `verify-release-artifacts.ps1 -Fixture` | 39 项拒绝反例及分发文档/图标来源映射正例通过，不运行应用或安装器 | [fixture 日志](release-fixtures-final.log) |
| `verify-sandbox-preflight.ps1 -Fixture` | 96 入口解析与分类通过；7 个旧 runner 在执行前非零拒绝；路径/归属反例通过 | [preflight 日志](preflight-final.log) |
| build.rs SHA-256 独立已知向量 | 4 组通过 | [向量日志](build-info-sha256-vectors.stdout.log) |
| `build-release.ps1`（冻结输入离线构建） | 退出码 0；run `installer/output/release-1.2.1-7ac1990619d44342bc857cdc984763f2`，来源指纹 `e7986561…`、构建指纹 `35b45410…`、update_key `disabled` | [build-release-final.log](build-release-final.log)；首次失败尝试保留于 [build-release.log](build-release.log) |
| `verify-release-artifacts.ps1 -ReleaseDirectory` | 独立执行退出码 0：来源链、EXE 内嵌信息、ZIP 条目/标记/字节、setup 编译证据全部一致 | [verify-release-final.log](verify-release-final.log) |
| 早期入口 CLI 探针（暂存 EXE） | 9 项通过：build-info 与清单一致、health 协议保持、5 组错误参数非零拒绝、非法相对数据根零初始化 | [release-cli-probes.json](release-cli-probes.json) |

最终安全集合的实际命令：

```powershell
cargo test --locked --offline --jobs 2 --lib --bin quicknest --test ui_layout --test icon_queue --test file_drop_data --test icon_cache --test icon_extract --test lnk_launch --test msi_advertised --test ole_mta -- --test-threads=1 --skip clipboard_round_trip
```

| 目标 | 通过 | 失败 | ignored | filtered |
|---|---:|---:|---:|---:|
| lib | 324 | 0 | 3 | 1 |
| bin quicknest | 29 | 0 | 0 | 0 |
| ui_layout | 8 | 0 | 0 | 0 |
| icon_queue | 13 | 0 | 0 | 0 |
| file_drop_data | 6 | 0 | 0 | 0 |
| icon_cache | 1 | 0 | 0 | 0 |
| icon_extract | 2 | 0 | 0 | 0 |
| lnk_launch | 1 | 0 | 0 | 0 |
| msi_advertised | 1 | 0 | 0 | 0 |
| ole_mta | 1 | 0 | 0 | 0 |
| doc | 3 | 0 | 0 | 0 |

不将上述数字解释为全部互异场景：13 项 icon_ui 用例在 bin 与 icon_queue 两个目标中重复执行。数据层定向 159 项和正式 UI 定向结果已包含在最终集合，不再累计；父测试启动的 argv/panic 子探针亦不重复累计。

lib 的 3 个 ignored 是 `crash_log::tests::panic_hook_child`、`infrastructure::windows::launch::tests::cmd_arguments_native_probe_child` 和 `updater_validation::tests::signed_fixture_acceptance`。前两项是父测试按需要启动的专用子入口；本轮未另行生成签名 fixture 或单独运行其验收。唯一主动过滤项为旧 `clipboard_round_trip`。未选择 `special_shell` / `uwp_link` 两个共享 Shell 集成目标。36 个 BLOCKED 脚本和 7 个 runner 拒绝不计为产品验收通过。

本轮行为覆盖包括：重复选择零写入、持久化原始选择字段修复、安全模式拒写、重复成功事件及最后时间/饱和计数、单批提交和真实文件锁回滚、备份内容变化/schema/BOM/有效保留、排序首项/并列/重复及缺失 ID、拼音查询缓存与改名/导入/恢复失效；图标请求/结果有界、有限预算/补投/断连/过期 generation、共享 source、只回填 icon 并保留模型/焦点/滚动/手势，以及失败重提后默认图标单次回退；正式模态取消与 Esc 状态清理、搜索两阶段关闭、NameDialog 重开、键盘单次激活和禁用语义、680×480 与 760×520 下编辑器正文及长错误滚动。

Slint 编译器保留一条非阻塞提示：导出的非 Window `PillButton` 不生成独立 Rust 组件。正式 MainWindow 使用和导入组件测试正常；Rust 严格 Clippy 已通过。没有以抑制 Rust warning 的方式放宽门禁。

性能采用独立合成工作负载，结果和局限见 [PERFORMANCE.md](PERFORMANCE.md)。最终 EXE 与包的版本、来源、大小和 SHA-256 已记录于 [优化记录](../../OPTIMIZATIONS_1.2.1.md) 及 run 目录内的 `release-manifest.json`；本页测试数只认证当前冻结源码，不认证历史产物。

未执行 setup 安装、卸载或独立解包，未操作真实剪贴板、共享 Shell、系统 DPI、证书或用户进程清理。Win10、完成页真实 Medium、两个不同生产版本真实退出换版、独立生产更新 key + HTTPS 端点及本轮实机 DPI/多显示器仍待验收；本地自动化验证不代替 P4 完整发布矩阵。
