# P0-06 DPI 实测复核（已验收）

坐标转换实现、编译测试和 V1.2 规定的真实系统缩放验证均通过。2026-11-03 用户明确确认 200% 不属于正常使用范围，正式基线 §5.5/§16.4 已由 100/125/150/175/200% 调整为 **100/125/150/175% 四档**；任务清单 P0-06 已勾选。

Windows 系统缩放由 Settings UI 实际选择，运行时 DPI、Slint scale 和 PMv2 均由真实 HWND 查询。没有使用 SLINT_SCALE_FACTOR、伪造 WM_DPICHANGED、私有 DPI 协议或注册表改动代替系统实测。

## 环境、产物和恢复

- Windows 11 专业版 25H2 / Build 26200.6899 / x64；一个活动显示器 DISPLAY1。
- Rust 1.96.0、Slint 1.18.1、winit/software，架构未变。
- Release SHA-256：`73096799DB451B4520DB8C12312AC5A44ED2FC9577F84BF19CE4F897F906FD5B`，12,380,160 字节。
- 初始与最终系统缩放均为 100%；Settings 实际选择 `100% (推荐)`，恢复后真实目标窗口 DPI96、PMv2=true，并再次完成真实 Drop。证据：restored-scale-final.json、restored-window-dpi.json。
- 两个最终源码会话均正常退出码 0，COM 目标析构一次、介质 acquired/released=2/2、errors=0，样本未变。

## 四档真实实测

| 系统缩放 | HWND DPI | Slint scale | 独立 Slint pointer-event 参照 | 真实 Explorer Drop |
|---|---:|---:|---|---|
| 100% | 96 | 1.00 | 稳定多点差值 0 | 通过 |
| 125% | 120 | 1.25 | 稳定多点差值 0 | 通过；逻辑点 (519.2,375.2) |
| 150% | 144 | 1.50 | 稳定多点差值 0 | 通过；逻辑点 (471.3333,357.3333) |
| 175% | 168 | 1.75 | 稳定多点差值 0 | 通过；逻辑点 (420.5714,314.2857) |

日志同时记录 screen/client/logical、Win32 DPI、Slint scale、实际 PMv2 context、客户区/Slint 物理尺寸、half-open inside 和分区。每次转换使用 UI 线程新取得的 provider，不缓存裸 HWND。两轮共 749 条 Slint 独立参照，最大 delta=0。

`final-100/interactive-4fd078cb/`：365 条参照，150/175% 真实 Drop；`physical-recheck/interactive-fff6ee10/`：384 条参照，100/125% 真实 Drop。一条窗口调整时 Win32 尺寸先于 Slint 更新的 sizes_match=false 快照作为瞬态诊断保留，不计稳定命中。

负数、非整数、半开边界、空区域和非法因子由单元测试覆盖。真实负原点屏和混合 DPI 多屏未执行，不属于本次单显示器四档验收；后续多显示器阶段仍须验证。

## 测试工具问题与处理

- TouchArea.moved 只在按住拖动时触发，改为 pointer-event Move 后重新构建并重测。
- 早期 UIA 文件行含水平裁剪部分，PowerShell/COM DPI 上下文也可能变化。最终工具把每次原生几何调用限定在 PMv2，按 UIA 行与实际客户区交集选可见物理起点，并用 SetPhysicalCursorPos。
- 旧测试 Explorer、Settings、ZCode 和浮层曾遮挡点位；WindowFromPoint 检查均拒绝操作，失败尝试不计通过。125% 后续临时移开测试遮挡窗口，恢复后完成真实 Drop；没有绕过命中检查。
- 一张 175% 原始截图含系统悬浮布局/输入浮层，仅保留为事件时原始证据；正式坐标结论来自实际命中、日志和独立参照。

## 编译、测试和回归

fmt/check/Clippy、21 项库单元＋1 项二进制单元、6 项合成拖放、3 项文档测试及 Release 构建全部通过。P0-01～P0-04 自动回归对应当前 EXE 哈希并通过。系统最终恢复 100%，无遗留 quicknest/cargo/rustc 进程。

机器摘要 `validation-summary.json` 状态为 Passed，记录四档矩阵、回归、哈希和用户范围决策。P0-06 已验收，下一任务是 P0-07 Shell STA 服务；尚未开始。
