# 再次审核修复验证证据

本目录对应从 `84c95cf` 开始的 12 项问题修复；完整说明见 [修复记录](../../AUDIT_REPAIRS_2026-11-07.md)。

- `final-gates.log`：格式、全目标编译、严格 Clippy、安全测试集合、文档测试及 Release 构建。
- `script-preflight.log`：全文件系统脚本清点及隔离 fixtures。
- `release-pe.json`：修复后本地 EXE 的结构、依赖与 SHA-256。

安全测试集合显式不运行旧的真实剪贴板 round-trip，以及会操作共享 Shell 的 special_shell、uwp_link 集成测试；未执行旧桌面回归、安装或卸载。测试计数与跳过项目分别记录，不将跳过计作通过。

Slint 行为回归使用正式 UI 组件的无桌面后端，检查指针、Tab/Space/Esc、焦点、回调、状态和几何；不使用截图存在作为验收条件，也不代替实机多显示器和系统 DPI 测试。
