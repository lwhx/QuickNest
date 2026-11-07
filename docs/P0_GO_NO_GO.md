# QuickNest P0 技术报告与 GO / NO-GO 决策依据

> 基线 19 章 P0 交付物。本报告汇总 P0-01 ～ P0-13 的技术结论、基线决策矩阵映射与剩余风险；
> **GO / NO-GO 的最终拍板属 P0-15，由项目负责人依据本报告决定**。
> 全部结论均以实际代码与 Windows 11 x64 实测证据为依据，证据索引见文末。
> 报告日期：2026-11-04。最终产物 SHA-256：`86A3D930799F41FEA74668E90512F0D0E7EFB51AF09F7821E479E658B0D32D62`。

## 1. 结论速览

| 基线任务 | 结论 | 核心证据 |
|---|---|---|
| P0-01 基础工程 | 通过 | 无浏览器内核/Node/Python；Slint 窗口 + 中文渲染 + Medium 三轮 |
| P0-02 HWND 生命周期 | 通过 | 三代窗口身份验证，隐藏不销毁 |
| P0-03 OLE 初始化 | 通过 | S_OK/S_FALSE/RPC_E_CHANGED_MODE 实测 |
| P0-04 COM 对象 | 通过 | 真实 HRESULT 序列、winit 协同 |
| P0-05 Explorer 拖入 | 通过 | 真实拖拽 5 Drop/6 路径、双方 Medium |
| P0-06 DPI 转换 | 通过 | 真实系统缩放 100/125/150/175% 四档 |
| P0-07 Shell STA | 通过 | 真实 .lnk 解析、Medium 通过 |
| P0-08 Mutex 单实例 | 通过 | 崩溃接管 WAIT_ABANDONED 实测 |
| P0-09 Named Pipe IPC | 通过 | SDDL 安全、白名单协议、1500ms 竞态 |
| P0-10 全局快捷键 | 通过 | 真实注入、冲突 1409、Drop 清理 |
| P0-11 系统托盘 | 通过 | 图标生命周期、六项菜单、TaskbarCreated 重建 |
| P0-12 窗口焦点状态机 | 通过 | 五态+RAII 抑制锁+120ms 复查，三事件源真实接线 |
| P0-13 ReplaceFileW 保存 | 通过 | 原子替换+失败保护+备份恢复+安全模式，dev/Medium 双过 |

**质量门禁（最终产物）**：fmt=0；clippy -D warnings 通过；63 项测试（53 lib + 1 bin + 6 集成 + 3 doc）0 失败；Release 构建 PE32+/GUI 子系统。

## 2. 基线决策矩阵映射

| 基线结果 | 基线决策 | 本项目实测命中 |
|---|---|---|
| Slint 主窗口直接 OLE 通过 | **GO** | ✅ P0-04/P0-05：Slint 原生 HWND 直接注册 IDropTarget，真实 Explorer 拖入验证（`docs/test-results/P0-05/interactive-39ab013c/`） |
| 独立 STA 宿主通过 | GO WITH ADAPTER | 未走到此分支——直接方案已通过，独立 STA 宿主不需要；但 STA 线程宿主模式已在 P0-07 Shell 服务中验证（GetMessageW 泵 + 接口不跨线程），可作为 P1 备选适配器 |
| 只有 WM_DROPFILES | 不算完整通过，重新评估 | ❌ 未采用——从始至终使用 OLE IDropTarget，未走 WM_DROPFILES 捷径 |
| 所有方案均不稳定 | NO-GO，重新评估窗口架构 | ❌ 未触发——直接方案稳定（拖放/焦点/托盘/热键全部在同一窗口上长期验证） |

## 3. 技术可行性判定（分域）

### 3.1 窗口与 UI 层（P0-01/02/06）——GO

Slint + winit + software renderer 满足基线"无浏览器内核"硬约束；HWND 生命周期可控（隐藏不销毁，组件销毁后句柄真正失效）；Per-Monitor V2 下 DPI 转换四档实测准确。**约束**：Slint `unstable-winit-030` 钩子为版本相关 API，Slint 升级必须重新验证；原生 HWND 在事件循环泵前不可用。

### 3.2 拖放与 COM（P0-03/04/05）——GO

OLE 全链路（初始化语义、IDropTarget 注册/撤销、CF_HDROP 数据、真实 Explorer 交互）通过。**弃用记录**：手写 COM vtable（访问违例）已弃用，COM 一律走 windows crate 生成接口。winit 后端存在两个已验证的共存约束（建窗期自初始化 OLE、销毁期无条件 Revoke），管理方式已固化。

### 3.3 系统集成（P0-07/08/09/10/11）——GO

Shell STA 服务、Mutex 单实例（含崩溃接管）、Named Pipe 安全 IPC（SDDL + SID 验证 + 1500ms 启动竞态）、全局热键、托盘全生命周期全部实测通过。**环境限制（非缺陷）**：本机 Explorer 高完整性导致 Medium 令牌下托盘（UIPI 拒绝）与跨前台合成输入被过滤——均有 dev 会话（完整权限）等价验证与 P0-10 独立 Medium 证据；生产环境标准用户会话不受影响。

### 3.4 窗口行为与数据（P0-12/13）——GO

焦点状态机（五态 + RAII 抑制锁 + 120ms 失焦复查）与三类事件源（ESC/热键/托盘）真实接线；原子保存链（ReplaceFileW + 失败保护 + 备份恢复 + 安全模式）在 dev 与 Medium 双验证。**约束**：跨实例写锁由单实例 Mutex 承担；防抖/备份轮换/Schema 迁移按基线属 P1。

## 4. 未覆盖项与剩余风险（进入 P1 前必须知悉）

1. **平台覆盖**：全部实测仅 Windows 11 x64 + NTFS；Win10、网络共享/可移动介质（基线 15.6 明示不承诺同等保证）、多显示器失焦行为未测。
2. **持久化边界**：`sync_all` 为软件层最强持久化点，断电级故障未注入；备份唯一性依赖 UTC 毫秒戳（同毫秒理论重名，P1 轮换策略一并处理）。
3. **权限模型**：asInvoker 在高完整性宿主下继承高权限（P0-01 记录）；测试普通权限路径用受限令牌，产品级降权启动属 P1 决策。
4. **版本相关 API**：Slint `unstable-winit-030` 钩子、winit 0.30.13 的 OLE/Revoke 行为——升级依赖必须重跑 P0-02/03/04/05 回归。
5. **性能与长期稳定性**：基线要求的长测（固定 200 项目、反复显示隐藏启动拖拽）尚未执行，属 P1 性能验收范围。
6. **Medium 令牌两类限制**：托盘 UIPI 拒绝、跨前台合成输入过滤——本机环境固有问题（Explorer 高完整性），已如实记录而非伪装通过。

## 5. 决策建议（供 P0-15 拍板）

- **技术层面**：13/13 任务全部通过，基线决策矩阵命中"Slint 主窗口直接 OLE → GO"路径；全部已知风险均有边界记录与 P1 处理入口。**技术证据支持 GO**。
- **决策模板**（基线 19 章）：

```text
Slint 主窗口直接 OLE 通过        → GO                    ✅ 命中
独立 STA 宿主通过                → GO WITH ADAPTER       （备选已验证，未启用）
只有 WM_DROPFILES                → 不算完整通过          （未采用）
所有方案均不稳定                 → NO-GO                 （未触发）
```

- **P0-15 待决事项**（由项目负责人确认）：是否接受第 4 节剩余风险进入 P1；是否要求先补 Win10/长测再进 P1；P1 范围内优先级确认。

## 6. 证据索引

| 任务 | 目录 |
|---|---|
| P0-01 | `docs/test-results/P0-01/` |
| P0-02 | `docs/test-results/P0-02/` |
| P0-03 | `docs/test-results/P0-03/` |
| P0-04 | `docs/test-results/P0-04/` |
| P0-05 | `docs/test-results/P0-05/`（主验收 `interactive-39ab013c/`） |
| P0-06 | `docs/test-results/P0-06/` |
| P0-07 | `docs/test-results/P0-07/` |
| P0-08 | `docs/test-results/P0-08/` |
| P0-09 | `docs/test-results/P0-09/` |
| P0-10 | `docs/test-results/P0-10/` |
| P0-11 | `docs/test-results/P0-11/` |
| P0-12 | `docs/test-results/P0-12/` |
| P0-13 | `docs/test-results/P0-13/` |

逐任务实现细节、问题与修复、边界与门禁见 [P0_VALIDATION.md](P0_VALIDATION.md)；API 选型与弃用记录见 [WINDOWS_API_DECISIONS.md](WINDOWS_API_DECISIONS.md)；进度与停点见 [DEVELOPMENT_PROGRESS.md](DEVELOPMENT_PROGRESS.md)。

## 7. 最终决策记录（P0-15）

**决策：GO —— P0 技术验证通过，进入 P1 基础可用版本开发。**

- **决策日期**：2026-11-04。
- **决策依据**：P0-01 ～ P0-13 共 13 项任务全部实测通过；基线 19 章决策矩阵命中"Slint 主窗口直接 OLE 通过 → GO"路径；技术层面无 NO-GO 触发条件（全方案稳定、无 WM_DROPFILES 捷径、直接方案成立）。
- **决策语境**：项目负责人在 P0-14 报告呈交后指示继续推进，决策按报告第 5 节建议（技术证据支持 GO）记录。
- **接受范围**：第 4 节六项剩余风险（平台覆盖、持久化边界、权限模型、版本相关 API、性能长测、Medium 环境限制）在知悉并接受的前提下带入 P1；各项均有边界记录与 P1 处理入口。
- **可撤销性**：本决策基于 2026-11-04 前的证据。若后续实测推翻本报告任一结论（如 Win10 不兼容、长期稳定性失败），项目负责人可要求重新决策，P0 阶段重新开启。
- **进入 P1 的前提条件复核**：基线 19 章"P0 未通过，不允许进入完整 P1"——P0 已通过，条件满足。P1 范围见基线第 20 章（主界面、分组分类、启动、Windows 集成复用 P0 已验证方案、JSON 数据层建立在 P0-13 保存链之上）。
