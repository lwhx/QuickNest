# QuickNest 项目开发文档

## V1.2 正式开发基线（PRD + 技术方案 + 分阶段执行规范）

  文档属性       内容
  -------------- --------------------------------------------
  产品名称       QuickNest
  文档版本       V1.2
  文档类型       产品需求文档、技术开发规范、测试与验收规范
  目标平台       Windows 10 / Windows 11 x64
  开发语言       Rust
  UI 框架        Slint
  Windows 集成   Win32 API、Windows Shell、COM、OLE
  数据存储       JSON + 本地图标缓存
  运行模式       本地独立运行、悬浮启动面板
  发布形式       安装版 + 绿色便携版
  当前状态       正式开发基线；先完成 P0 技术验证

> 本文档是 QuickNest 后续开发的统一依据。AI 编程工具和开发者应按
> P0、P1、P2、P3、P4
> 的顺序推进。未经用户确认，不得改变已经确定的产品定位、主界面结构或技术路线。

------------------------------------------------------------------------

# 0. 项目决策摘要

## 0.1 已确认的产品方向

QuickNest 是一款 Windows 桌面快捷启动器，参考 Lily
的经典交互逻辑，采用：

-   顶部一级分组标签栏。
-   左侧二级分类列表。
-   右侧快捷项目图标网格。
-   悬浮窗口，通过全局快捷键呼出。
-   系统托盘后台运行。
-   本地配置和图标缓存。
-   Rust + Slint 技术路线。

**当前版本不开发常驻 Dock 模式。** 不显示 Dock 入口，不实现 Dock
代码；架构只允许未来扩展，不为 Dock 提前增加复杂度。

## 0.2 本项目的核心目标

1.  能够替代 Lily 完成日常快捷启动。
2.  支持多级分类和大量快捷项目管理。
3.  支持 EXE、LNK、UWP、MSI 快捷方式、文件、文件夹、网址和 URI 协议。
4.  支持资源管理器拖入和应用内部拖拽排序。
5.  支持全局快捷键、托盘、单实例和自动隐藏。
6.  支持中文全拼和拼音首字母搜索。
7.  支持浅色、深色、透明度和 Windows 背景材质。
8.  支持安装版和绿色便携版。
9.  配置保存失败时保护旧数据，具备备份和恢复能力。
10. 保持低资源占用和长期运行稳定性。

## 0.3 不纳入当前版本的功能

-   常驻 Dock 栏。
-   屏幕边缘常驻工具栏。
-   云端同步。
-   用户注册和登录。
-   内置 AI 助手。
-   全盘自建文件索引。
-   在线自动化脚本交易市场。
-   内置截图、OCR、系统清理工具。

------------------------------------------------------------------------

# 1. 产品定位与设计原则

## 1.1 产品定位

QuickNest 是一款轻量化、现代化、可定制的 Windows
桌面快捷启动器。它将用户的软件、文件、文件夹、网址和命令组织到分组与分类中，通过鼠标或键盘快速访问。

## 1.2 设计原则

1.  **轻量优先**：隐藏状态不进行不必要的轮询、扫描或动画。
2.  **本地优先**：核心功能离线可用，不要求登录。
3.  **操作直观**：保留 Lily 已经成熟的导航习惯。
4.  **数据安全**：配置自动保存，升级不覆盖用户数据。
5.  **权限最小化**：主程序始终以普通用户权限运行。
6.  **模块化**：UI、领域模型、业务服务、Windows 适配层和存储层分离。
7.  **可扩展**：后续可增加插件，但插件不能影响核心启动能力。
8.  **可验证**：Windows
    底层功能必须通过实际运行测试，不能只凭架构图认定可行。

------------------------------------------------------------------------

# 2. 主界面与交互规范

## 2.1 总体布局

采用 Lily 式三级视觉区域：

``` text
┌──────────────────────────────────────────────────────┐
│ QuickNest                       搜索  固定  设置  ×   │
├──────────────────────────────────────────────────────┤
│ 常用工具       应用       工作       自动化        ▼  │
├──────────────┬───────────────────────────────────────┤
│              │                                       │
│ 必备软件      │                                       │
│ 系统工具      │             图标网格区域              │
│ 浏览器        │                                       │
│ 办公软件      │   软件1      软件2      软件3         │
│ 视频音乐      │                                       │
│ 系统优化      │   软件4      软件5      软件6         │
│ AI           │                                       │
│              │   软件7      软件8      添加项目      │
│ + 新建分类    │                                       │
├──────────────┴───────────────────────────────────────┤
│ 9 个项目 · 本地配置              视图  排序  添加     │
└──────────────────────────────────────────────────────┘
```

## 2.2 顶部一级分组

默认示例：常用工具、应用、自动化。用户可自由创建、重命名、删除、排序、设置图标和可选强调色。

要求：

-   点击分组切换其二级分类。
-   每个分组独立维护分类列表。
-   每个分组记住上次选中的分类 `last_category_id`。
-   分组过多时支持横向滚动和下拉列表。
-   支持拖拽调整分组顺序。
-   删除非空分组时必须确认迁移或删除内容。
-   "新分组"是用户自定义的真实分组名称，不是固定按钮；新增分组使用独立入口。

## 2.3 左侧二级分类

仅显示当前一级分组下的分类。支持创建、重命名、删除、排序、图标、右键菜单、拖拽和垂直滚动。

删除非空分类时，必须提示迁移项目或确认删除，不得静默丢失数据。

## 2.4 右侧项目网格

每个项目显示图标和名称，可显示路径失效角标。支持大、中、小三档图标，默认中图标。列数随窗口宽度自适应，也允许手动设置。

项目名称最多两行，超出省略；悬停显示完整名称、路径和备注。

支持：

-   单击启动。
-   右键操作。
-   网格内排序。
-   跨分类和跨分组移动。
-   从资源管理器拖入。
-   多文件添加。
-   空白区域右键添加。

## 2.5 标题栏和窗口

标题栏包括 Logo、QuickNest
名称、搜索、固定、设置和关闭按钮。标题栏可拖动窗口。

关闭按钮默认隐藏面板，不退出后台进程。真正退出通过托盘菜单完成。设置中可修改关闭按钮行为。

## 2.6 搜索入口

主界面不常驻大型搜索框。提供标题栏搜索按钮和
`Ctrl+F`。搜索范围默认当前一级分组，可切换全部分组。只搜索已登记项目，不默认扫描全盘。

P2 支持中文全拼、拼音首字母、前缀、模糊匹配和纯键盘操作。

------------------------------------------------------------------------

# 3. 快捷项目模型与管理

## 3.1 支持的项目类型

  类型               枚举           说明
  ------------------ -------------- -----------------------------------------
  Windows 程序       `exe`          标准可执行程序
  Windows 快捷方式   `lnk`          普通及特殊 Shell Link
  普通文件           `file`         文档、表格、图片等
  文件夹             `folder`       本地路径和 UNC 路径
  网址/协议          `uri`          HTTP、HTTPS、Steam、VS Code、自定义 URI
  自定义命令         `command`      结构化程序和参数
  自动化动作         `automation`   后续阶段扩展

`.bat`、`.cmd`、`.ps1` 不作为普通 EXE 直接处理，统一进入命令执行模块。

## 3.2 添加方式

支持：

1.  点击"添加项目"。
2.  从 Windows 资源管理器拖入。
3.  通过文件选择窗口。
4.  通过空白区域右键菜单。
5.  复制文件或路径后使用 `Ctrl+V` 粘贴添加。

添加时自动提取名称、类型、路径、默认工作目录和图标。用户可以修改名称、目标和图标。

普通拖入只保存引用，不复制或移动用户原始文件。

## 3.3 默认工作目录

-   直接添加 EXE：默认工作目录为 EXE 所在父目录。
-   普通文件：记录父目录作为元数据；实际打开文件优先使用 Shell
    文件关联。
-   普通 LNK：优先保留快捷方式自带的工作目录和 Shell 启动语义。
-   LNK 工作目录为空时：默认仍通过原始 LNK
    启动；只有用户明确选择"直接启动解析出的目标程序"时，才回退到目标父目录。

不得为了自动填写工作目录而破坏快捷方式原始行为。

## 3.4 项目编辑字段

-   显示名称。
-   项目类型。
-   目标路径或 URI。
-   启动参数。
-   工作目录。
-   自定义图标。
-   窗口状态：normal、minimized、maximized。
-   是否管理员身份运行。
-   备注。

## 3.5 右键菜单

-   打开。
-   管理员身份运行。
-   编辑。
-   打开文件所在位置并选中目标。
-   复制路径。
-   移动到其他分类。
-   复制项目。
-   删除。

菜单必须避免被屏幕边缘截断，并正确处理 DPI。

------------------------------------------------------------------------

# 4. Windows 快捷方式和特殊项目启动策略

不得将所有快捷方式都当作普通 EXE 路径处理。

## 4.1 普通 LNK

导入时使用 COM：

-   `IShellLinkW`
-   `IPersistFile`

尽可能读取：

-   `GetPath`
-   `GetArguments`
-   `GetWorkingDirectory`
-   `GetIconLocation`
-   `GetIDList`

默认保存原始 LNK 路径及解析元数据。**默认通过原始 LNK 启动**，以保留
Windows Shell 的启动语义。

首选接口：`ShellExecuteExW`，`lpFile` 指向原始 LNK 路径。

## 4.2 UWP / Microsoft Store

UWP 可能没有普通 EXE 路径。

优先方案：

1.  保存用户添加的原始 LNK。
2.  通过 `ShellExecuteExW` 启动原始快捷方式。
3.  如果能够可靠取得 AUMID，可额外保存 AUMID。
4.  需要直接激活时使用
    `IApplicationActivationManager::ActivateApplication`。
5.  AUMID 激活失败时回退原始 LNK。

不得仅凭显示名称猜测 AUMID。

## 4.3 MSI 广告快捷方式

MSI 广告快捷方式可能依赖 Windows Installer 的修复、安装或组件激活行为。

处理原则：

-   保存原始 LNK 路径。
-   通过 Shell 执行原始快捷方式。
-   不强制解析为 EXE。
-   不绕过 Windows Installer。
-   不擅自复制或重写快捷方式内容。
-   失效时提示用户修复安装或重新添加入口。

## 4.4 特殊 Shell 项目

控制面板、系统设置、Shell Namespace、特殊文件夹和网络位置，优先保存原始
Shell Link 或 URI，通过 Windows Shell 启动。

## 4.5 特殊快捷方式副本策略

默认不复制原始 LNK。未来若增加"复制快捷方式到 QuickNest
数据目录"，必须验证目标、工作目录、PIDL、图标位置和启动参数是否完整保留。无法保证时继续引用原始文件。

## 4.6 启动接口选择

  ------------------------------------------------------------------------------------------
  类型                    首选方式                          回退方式
  ----------------------- --------------------------------- --------------------------------
  EXE                     `CreateProcessW` / Rust `Command` `ShellExecuteExW`

  普通 LNK                `ShellExecuteExW` 原始 LNK        用户明确选择后直接启动解析目标

  UWP LNK                 `ShellExecuteExW` 原始 LNK        AUMID 激活

  UWP AUMID               `IApplicationActivationManager`   原始 LNK

  MSI 广告 LNK            `ShellExecuteExW` 原始 LNK        提示修复

  普通文件                `ShellExecuteExW`                 系统文件关联

  文件夹                  `ShellExecuteExW` / Explorer      提示路径错误

  URI                     `ShellExecuteExW`                 系统协议处理

  自定义命令              结构化进程启动                    用户明确选择 CMD/PowerShell
  ------------------------------------------------------------------------------------------

所有启动失败均返回结构化错误，不得静默失败。

------------------------------------------------------------------------

# 5. Windows OLE 拖拽系统

## 5.1 技术目标

使用 Windows OLE Drag & Drop，优先支持：

-   `IDropTarget`
-   `IDataObject`
-   `CF_HDROP`
-   `RegisterDragDrop`
-   `RevokeDragDrop`
-   `OleInitialize`
-   `OleUninitialize`

Slint 负责 UI 和逻辑坐标，Windows OLE 负责接收资源管理器拖放数据。

## 5.2 OLE 线程约束

OLE 拖放要求正确的 STA 初始化和消息泵。

使用 `OleInitialize(NULL)`，不能简单使用 MTA 初始化代替。

`OleInitialize` 返回值：

-   `S_OK`：成功初始化。
-   `S_FALSE`：已经以兼容方式初始化，仍需匹配调用 `OleUninitialize`。
-   `RPC_E_CHANGED_MODE`：当前线程 COM Apartment 不兼容，不能强行切换。

Slint HWND 必须在窗口真正创建后取得。使用 Slint 提供的 `window_handle()`
和相应 `raw-window-handle` 功能，不得在窗口创建前假设 HWND 存在。

## 5.3 P0 必须验证的实现方案

### 方案 A：Slint 窗口直接注册 OLE

前提：

-   UI 线程为兼容 STA。
-   事件循环能正确处理 OLE 消息。
-   HWND 生命周期可控。
-   OLE 回调不阻塞 Slint UI。

优先验证。

### 方案 B：独立 STA 拖拽宿主

如果 Slint UI 线程不满足 OLE 条件，验证独立 STA 线程和原生 HWND
拖拽宿主。

必须测试：

-   窗口覆盖和鼠标命中。
-   与 Slint 主窗口位置同步。
-   拖拽反馈。
-   逻辑/物理坐标转换。
-   多 DPI。
-   线程安全事件传递。

不能只凭理论设计判定可行。

### 方案 C：调整窗口集成方式

如果 A、B 均不稳定，评估 Slint Windows 后端或宿主窗口架构。

`WM_DROPFILES` 可作为简化降级方案，但不能视为完整 OLE
拖拽能力，因为它不提供完整 DragEnter、DragOver、DragLeave
和弹簧加载交互。

## 5.4 拖拽数据处理

优先通过 `CF_HDROP` 读取单个或多个路径。

COM 回调只负责：

1.  判断数据格式。
2.  读取路径。
3.  复制必要数据。
4.  记录鼠标位置。
5.  向 UI 发送事件。
6.  返回拖拽效果。

图标提取、文件属性读取和 Hash 计算必须在后台线程执行。

## 5.5 坐标转换

基本流程：

``` text
OLE 屏幕物理坐标
    ↓ ScreenToClient
HWND 客户区物理坐标
    ↓ DPI / Scale Factor 换算
Slint 逻辑坐标
    ↓ 命中测试
顶部一级分组 / 左侧分类 / 右侧网格
```

必须在 100%、125%、150%、175% 缩放环境实测。200% 不纳入 V1.2 必测范围。

## 5.6 拖拽规则

外部拖入：

-   右侧网格：加入当前分类。
-   左侧分类：加入指定分类。
-   顶部一级分组：悬停切换目标分组。
-   多文件：按拖入顺序添加。

内部拖拽：

-   项目排序。
-   跨分类移动。
-   跨分组移动。
-   分类排序。
-   一级分组排序。

顶部一级分组弹簧加载默认 400ms。

拖拽过程中显示目标高亮和插入位置。按 ESC 取消，取消后原数据不变。

------------------------------------------------------------------------

# 6. COM 线程和系统接口架构

## 6.1 线程职责

  线程               职责
  ------------------ --------------------------------------------------
  Slint UI 线程      UI 事件循环、窗口状态、用户交互
  OLE 拖拽承载线程   按 P0 结果承载 IDropTarget
  Shell STA 线程     IShellLinkW、ShellExecuteEx、UWP 激活等 COM 操作
  后台任务线程池     图标提取、文件检查、Hash、搜索索引
  IPC 服务线程       Named Pipe 消息处理

## 6.2 Shell STA 服务

单独建立 ShellService。

初始化：

`CoInitializeEx(NULL, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)`

负责：

-   `IShellLinkW`
-   `IPersistFile`
-   `ShellExecuteExW`
-   `IApplicationActivationManager`
-   Shell 项目解析

该线程必须正确处理消息泵。

COM 接口不能未经 Marshaling 随意跨线程传递。线程间通过普通 Rust
数据结构和消息队列通信。

## 6.3 UI 更新

耗时操作不得阻塞 UI。

后台任务完成后使用：

`slint::invoke_from_event_loop`

将结果交回 UI 线程。

------------------------------------------------------------------------

# 7. UIPI 与权限隔离

## 7.1 主程序权限

QuickNest 主程序始终以普通用户权限运行。

Manifest：

``` xml
<requestedExecutionLevel level="asInvoker" uiAccess="false" />
```

不得默认使用 `requireAdministrator`。

## 7.2 拖拽兼容边界

重点保证普通权限 Explorer → 普通权限 QuickNest 的拖拽。

不承诺以下场景全部可用：

-   提升权限 Explorer → 普通权限 QuickNest。
-   安全桌面。
-   受保护桌面。
-   不同完整性级别之间的任意窗口消息。

文档不得写"拖拽 100% 可用"，应明确权限边界。

## 7.3 ChangeWindowMessageFilterEx

不允许为规避 UIPI 而全局开放窗口消息。

只有确有特定消息互操作需求、经过安全评估后，才可对单个窗口使用
`ChangeWindowMessageFilterEx`。

它不能代替 OLE，也不能绕过所有权限隔离。

## 7.4 管理员启动

只有用户明确选择某个项目以管理员身份运行时，才调用：

`ShellExecuteExW`，设置 `lpVerb = "runas"`。

UAC 由 Windows 正常处理。QuickNest 主进程保持普通权限。

## 7.5 安装包降权启动

安装程序可能以管理员权限运行。

安装结束启动 QuickNest 时必须恢复到原始登录用户权限。

Inno Setup 优先使用
`ExecAsOriginalUser`。其他安装方案必须提供经过验证的降权启动实现。

无法可靠降权时，不得直接以管理员权限启动主程序。

------------------------------------------------------------------------

# 8. 命令执行安全

## 8.1 命令数据模型

命令不得全部保存成未经区分的 Shell 字符串。

建议：

``` json
{
  "item_type": "command",
  "program": "C:\\Windows\\System32\\notepad.exe",
  "args": [
    "C:\\Users\\User\\Desktop\\test.txt"
  ],
  "workdir": "C:\\Users\\User\\Desktop",
  "execution_mode": "direct",
  "run_as_admin": false,
  "confirm_on_first_run": false
}
```

## 8.2 执行模式

### Direct（默认）

-   使用 `CreateProcessW` 或 Rust `Command`。
-   程序路径独立保存。
-   参数数组独立保存。
-   工作目录独立设置。
-   不经过 cmd.exe。
-   不拼接未经处理的 Shell 字符串。

### CMD（用户明确选择）

通过 `cmd.exe /d /s /c` 执行。

界面必须显示实际执行内容。

### PowerShell（用户明确选择）

优先使用：

`powershell.exe -NoProfile -File`

通过脚本文件路径和参数执行。不默认运行不明来源的内联脚本。

## 8.3 安全确认

首次执行自定义 Shell 命令时：

-   展示程序。
-   展示参数。
-   展示工作目录。
-   说明可能修改系统或文件。
-   允许用户取消。

不把 `del`、`format`、`Remove-Item` 等关键词黑名单当作安全边界。

安全依赖用户明确添加、执行模式明确、首次确认、最小权限和不自动运行未知脚本。

## 8.4 配置导入安全

导入配置时：

-   不自动执行命令。
-   不自动执行脚本。
-   不自动打开未知 URI。
-   命令项目标记为未验证。
-   首次执行时按设置提示确认。

------------------------------------------------------------------------

# 9. 单实例与 IPC

## 9.1 单实例目标

同一用户、同一登录会话只运行一个主实例。

防止：

-   多实例同时写配置。
-   多个托盘图标。
-   多个窗口。
-   重复注册全局快捷键。

## 9.2 Mutex

使用 `CreateMutexW`。

命名：

`Local\\QuickNest-{SID_HASH}-{SESSION_ID}`

其中 SID_HASH 为当前用户 SID 的稳定 Hash，SESSION_ID 为 Windows 登录会话
ID。

返回值处理：

-   `WAIT_OBJECT_0`：获得主实例所有权。
-   `WAIT_ABANDONED`：原实例异常退出，当前实例接管并先检查数据。
-   `WAIT_TIMEOUT`：已有主实例。
-   `WAIT_FAILED`：记录系统错误并安全退出。

主实例必须持有 Mutex 句柄直至退出。

## 9.3 Named Pipe

管道名称：

`\\\\.\\pipe\\QuickNest-{SID_HASH}-{SESSION_ID}`

使用：

-   `CreateNamedPipeW`
-   `ConnectNamedPipe`
-   `CreateFileW`
-   `ReadFile`
-   `WriteFile`

设置显式安全描述符：

-   仅当前用户 SID。
-   必要时允许 SYSTEM。
-   使用 `PIPE_REJECT_REMOTE_CLIENTS`。
-   限制消息长度。
-   验证客户端身份。
-   不允许远程连接。

## 9.4 IPC 消息

使用长度前缀 + JSON，单条消息上限 4KB。

示例：

``` json
{
  "version": 1,
  "command": "show_and_focus",
  "source": "secondary_instance"
}
```

允许命令：

-   show。
-   hide。
-   toggle。
-   show_and_focus。
-   open_search。

IPC 不允许传入任意 Shell 命令让主实例执行。

## 9.5 启动竞态

如果 Mutex 已被占用，但 Pipe 尚未就绪：

1.  等待 Pipe。
2.  重试连接。
3.  最长等待 1500ms。
4.  重新检查 Mutex 所有者状态。
5.  主实例失效时重新竞争所有权。
6.  无法唤醒时记录错误并提示。

不得在未确认原实例失效时启动第二个主实例。

------------------------------------------------------------------------

# 10. 窗口焦点和自动隐藏状态机

## 10.1 窗口状态

``` text
Hidden
  ↓
Showing
  ↓
Visible
  ↓
Hiding
  ↓
Hidden

任意状态 → ShuttingDown
```

固定状态与窗口可见状态分离管理。

## 10.2 抑制锁

使用可追踪的 `SuppressionToken`，避免简单全局 Boolean 导致永久锁定。

抑制原因：

-   Pinned。
-   ModalDialogOpen。
-   SystemDialogActive。
-   ExternalDragHover。
-   ContextMenuOpen。
-   LaunchingElevated。

每个临时锁必须有明确释放路径，优先使用 Rust RAII Guard。

必要时增加异常超时清理，但不得用超时机制掩盖资源生命周期错误。

## 10.3 失焦隐藏流程

1.  收到窗口激活状态变化。
2.  检查当前窗口状态。
3.  检查固定状态。
4.  检查抑制锁。
5.  启动一次性延迟检查，默认 120ms。
6.  重新获取前台窗口。
7.  检查是否为 QuickNest 或其拥有的弹窗。
8.  确认用户切换到外部窗口后隐藏。

不能仅凭一次 `WM_KILLFOCUS` 立即隐藏。

## 10.4 事件来源

综合使用：

-   Slint WindowActiveChanged。
-   `WM_ACTIVATEAPP`。
-   `WM_KILLFOCUS`。
-   `GetForegroundWindow`。
-   `GetWindowThreadProcessId`。
-   Owner / Owned 窗口关系。

## 10.5 系统弹窗和拖拽

打开文件选择器、编辑弹窗、图标选择器、颜色选择器和原生菜单前建立抑制锁。

OLE：

-   DragEnter：建立锁。
-   DragOver：保持锁。
-   DragLeave：释放锁。
-   Drop：处理完成后释放锁。
-   RevokeDragDrop：强制清理。

## 10.6 管理员启动

-   建立 LaunchingElevated 锁。
-   异步执行管理员启动请求。
-   UAC 取消时保留面板。
-   成功时按设置决定是否隐藏。
-   失败时提示并保留面板。
-   所有路径都必须释放抑制锁。

## 10.7 固定模式

固定时：

-   不因失焦隐藏。
-   不因启动项目自动隐藏。
-   只有用户取消固定后恢复自动隐藏。

------------------------------------------------------------------------

# 11. 全局快捷键和系统托盘

## 11.1 全局快捷键

默认：`Ctrl + Alt + Space`。

使用 `RegisterHotKey`，支持自定义和冲突检测。优先使用 `MOD_NOREPEAT`。

如果注册失败，显示明确的占用提示。

快捷键消息处理必须有可靠的 Windows 消息循环，并通过线程安全事件通知
Slint UI。

## 11.2 搜索快捷键

`Ctrl + F` 激活搜索。

## 11.3 系统托盘

使用 `Shell_NotifyIconW`。

菜单：

-   显示 QuickNest。
-   隐藏 QuickNest。
-   固定 / 取消固定。
-   设置。
-   检查更新。
-   退出程序。

左键单击或双击切换面板状态；右键显示菜单。

退出时：

1.  保存配置。
2.  注销快捷键。
3.  删除托盘图标。
4.  RevokeDragDrop。
5.  释放 COM 资源。
6.  关闭 IPC。
7.  释放 Mutex。
8.  清理后台线程。

------------------------------------------------------------------------

# 12. 图标提取与缓存

## 12.1 提取

优先使用：

`IShellItemImageFactory::GetImage`

请求 64x64 或 256x256 图标。

必要时使用 `SHGetImageList` 的 EXTRALARGE / JUMBO，`SHGetFileInfoW`
作为兼容回退。

保留透明通道。

所有提取和编码在后台线程执行。

## 12.2 Hash 缓存

使用 BLAKE3。

缓存键建议包含：

-   规范化图标来源。
-   目标路径或 AUMID。
-   文件修改时间。
-   文件大小或文件标识。
-   图标资源位置。
-   图标索引。
-   请求尺寸。

自定义图标使用自定义文件身份及修改信息作为缓存键。

## 12.3 缓存规则

-   同一来源尽可能复用。
-   程序更新后识别缓存失效。
-   删除项目不立即删除共享图标。
-   定期清理无引用缓存。
-   提取失败使用默认图标。
-   图标加载不阻塞 UI。

------------------------------------------------------------------------

# 13. 搜索中心

## 13.1 搜索范围

默认当前一级分组，可切换全部分组。只搜索已登记项目。

## 13.2 匹配

支持：

-   名称完全匹配。
-   前缀匹配。
-   模糊匹配。
-   路径匹配。
-   中文全拼。
-   拼音首字母。

示例：

  输入        匹配
  ----------- ------------
  wx          微信
  weixin      微信
  wyy         网易云音乐
  chrome      Chrome
  photoshop   Photoshop

## 13.3 拼音索引

新增或重命名时生成：

-   标准化名称。
-   全拼。
-   首字母。
-   常见多音字候选。

不承诺所有多音字均能自动准确识别，允许配置词典。

## 13.4 键盘流

-   上下左右：移动选中项。
-   Enter：普通启动。
-   Ctrl+Enter：管理员启动。
-   Ctrl+D：打开文件位置。
-   Tab：切换搜索范围。
-   Esc：先清空关键词，再退出搜索。

默认选中第一条匹配结果。

------------------------------------------------------------------------

# 14. 数据结构与存储

## 14.1 存储原则

第一版使用 JSON，不急于引入 SQLite。

数据层级：

`Workspace -> Groups -> Categories -> Items`

所有实体使用 UUID v4。

顺序由 JSON 数组自然顺序决定，不维护冗余 order 字段。

## 14.2 配置结构

``` json
{
  "schema_version": 2,
  "settings": {
    "theme": "system",
    "accent_color": "#2563EB",
    "opacity": 0.95,
    "hotkey": "Ctrl+Alt+Space",
    "auto_hide_on_blur": true,
    "auto_hide_on_launch": true,
    "pinned": false,
    "icon_size": "medium",
    "window_bounds": {
      "x": 420,
      "y": 200,
      "width": 860,
      "height": 580,
      "monitor_id": "DISPLAY1"
    }
  },
  "current_group_id": "group-uuid",
  "groups": [
    {
      "id": "group-uuid",
      "name": "常用工具",
      "icon": "wrench",
      "color": "#3B82F6",
      "last_category_id": "category-uuid",
      "categories": [
        {
          "id": "category-uuid",
          "name": "必备软件",
          "icon": "star",
          "items": [
            {
              "id": "item-uuid",
              "name": "Everything",
              "item_type": "exe",
              "launch": {
                "kind": "executable",
                "path": "C:\\Program Files\\Everything\\Everything.exe",
                "args": ["-startup"],
                "workdir": "C:\\Program Files\\Everything",
                "run_as_admin": false,
                "window_state": "normal"
              },
              "icon_ref": "icons/example.png",
              "launch_count": 15,
              "last_launched_at": 1762140000
            }
          ]
        }
      ]
    }
  ]
}
```

实际实现采用 Serde tagged enum，`item_type` 与 `launch.kind`
必须校验一致。

## 14.3 LaunchSpec

``` rust
enum LaunchSpec {
    Executable {
        path: String,
        args: Vec<String>,
        workdir: Option<String>,
        run_as_admin: bool,
    },
    ShellLink {
        link_path: String,
        metadata: Option<ShellLinkMetadata>,
    },
    File {
        path: String,
    },
    Folder {
        path: String,
    },
    Uri {
        uri: String,
    },
    AppUserModelId {
        aumid: String,
        args: Option<String>,
    },
    Command {
        program: String,
        args: Vec<String>,
        workdir: Option<String>,
        execution_mode: String,
        confirm_on_first_run: bool,
    },
    Automation {
        action_id: String,
    }
}
```

## 14.4 环境变量和相对路径

支持：

-   `%USERPROFILE%`
-   `%APPDATA%`
-   `%LOCALAPPDATA%`
-   `%WINDIR%`
-   `%PROGRAMFILES%`
-   `${APP_DIR}`

只在明确支持的路径字段展开，不得破坏 URI 编码。

`${APP_DIR}` 表示 QuickNest.exe 所在目录。

相对路径以 APP_DIR 解析。不同卷之间不得生成无效相对路径。

## 14.5 安装版和便携版

安装版：

`%APPDATA%\\QuickNest\\`

便携版：

``` text
QuickNest/
├── QuickNest.exe
├── .portable
├── data/
│   ├── config.json
│   ├── backups/
│   ├── icons/
│   └── shortcuts/
└── logs/
```

通过 `.portable` 标记明确识别。

若发现旧版 data 目录但无标记，应提示迁移，不得静默改变配置位置。

安装版和便携版共用同一数据格式。

------------------------------------------------------------------------

# 15. Windows JSON 原子保存与容灾

## 15.1 目标

避免配置写成 0
字节、写入中断损坏、并发覆盖、备份覆盖、升级覆盖用户数据。

## 15.2 标准流程

``` text
业务状态变更
    ↓
序列化
    ↓
Schema 校验
    ↓
同目录创建唯一临时文件
    ↓
完整写入
    ↓
FlushFileBuffers
    ↓
关闭临时文件句柄
    ↓
ReplaceFileW / MoveFileExW
    ↓
验证最终配置
    ↓
更新保存状态
```

## 15.3 临时文件

与 config.json 放在同一目录，文件名例如：

`config.<PID>.<UUID>.tmp`

要求：

-   独占创建。
-   完整写入。
-   调用 Rust `File::sync_all()`。
-   关闭句柄后替换。
-   不跨卷替换。

## 15.4 目标文件已存在

优先使用 `ReplaceFileW`：

参数：

-   原配置文件路径。
-   新临时文件路径。
-   唯一备份文件路径。
-   Replace Flags。

示意：

``` text
ReplaceFileW(
    config.json,
    config.<UUID>.tmp,
    config.backup.<timestamp>.json,
    0,
    NULL,
    NULL
)
```

重要约束：

-   不使用 `REPLACEFILE_WRITE_THROUGH`，该标志不受支持。
-   不默认忽略 ACL 错误。
-   备份与原文件必须位于同一卷。
-   替换失败时保留原配置。
-   保留临时文件用于诊断。
-   记录 `GetLastError`。
-   禁止先删除原配置再重命名新文件。

## 15.5 首次创建

目标文件不存在时使用 `MoveFileExW` 同卷移动，按验证结果使用
`MOVEFILE_WRITE_THROUGH`。

必须处理目标文件在并发期间被创建的情况。

## 15.6 备用策略

若当前文件系统不支持预期替换：

1.  检查原文件和临时文件。
2.  验证备份。
3.  记录文件系统和错误码。
4.  使用经过测试的同卷备用方案。
5.  仍失败则保留旧配置和临时文件。
6.  提示用户保存失败。

禁止删除旧配置来强行完成保存。

网络共享和特殊可移动介质不承诺与本地 NTFS 完全相同的故障恢复保证。

## 15.7 防抖与备份

-   窗口位置、尺寸和滑块：300--500ms 防抖。
-   项目增删改：尽快保存。
-   同一时间只允许一个写入事务。
-   每日首次修改前备份。
-   导入前备份。
-   支持手动备份。
-   默认保留最近 10 份有效备份。

## 15.8 损坏恢复

启动时配置解析失败：

1.  停止自动写入。
2.  按时间倒序检查备份。
3.  验证 JSON 和 Schema。
4.  恢复最近有效备份。
5.  原损坏文件改名为 `config.corrupted.<timestamp>.json`。
6.  提示用户恢复结果。
7.  无有效备份时进入安全恢复状态，不得静默覆盖原文件。

## 15.9 Schema 迁移

当前 `schema_version = 2`。

-   v1 → v2：执行明确迁移。
-   未知旧版本：拒绝破坏性迁移。
-   高于当前版本：提示版本不兼容，不得覆盖。
-   迁移前备份。
-   迁移后完整校验。

------------------------------------------------------------------------

# 16. 外观、DPI 和 Windows 系统集成

## 16.1 主题

-   浅色。
-   深色。
-   跟随系统。

## 16.2 透明度

范围 50%--100%，默认 95%。

## 16.3 背景材质

Windows 11：

-   Mica。
-   Acrylic / Desktop Acrylic。
-   普通纯色背景。

`DWMWA_SYSTEMBACKDROP_TYPE` 需要按 Windows
版本和系统能力检测，不能假设所有 Win10/Win11 均支持。

Windows 10：

-   默认纯色。
-   可选半透明。
-   旧式模糊作为经过验证的兼容增强。
-   不可用时降级，不影响功能。

## 16.4 DPI

Manifest 声明 Per-Monitor V2 DPI Awareness。

测试 100%、125%、150%、175%。200% 不纳入 V1.2 必测范围。

窗口跨屏时根据显示器工作区和 DPI 修正位置，防止恢复到屏幕外。

------------------------------------------------------------------------

# 17. 设置中心

## 17.1 常规

-   开机自启动。
-   开机后静默隐藏。
-   默认一级分组。
-   默认二级分类。
-   启动项目后隐藏。
-   失焦自动隐藏。
-   固定状态。
-   关闭按钮行为。

开机自启动使用当前用户范围，不触发 UAC。

## 17.2 快捷键

-   呼出/隐藏。
-   搜索。
-   冲突检测。
-   恢复默认。

## 17.3 外观

-   主题。
-   强调色。
-   透明度。
-   圆角。
-   背景材质。
-   图标大小。
-   网格间距。
-   字体大小。
-   窗口尺寸。

## 17.4 数据管理

-   配置路径。
-   安装/便携模式。
-   导出全部配置。
-   导出单独分组。
-   合并/覆盖导入。
-   立即备份。
-   历史备份恢复。
-   清理未使用图标。
-   打开数据目录。

## 17.5 关于

-   版本和构建 Hash。
-   检查更新。
-   开源许可证。
-   项目主页。
-   反馈地址。
-   日志目录。

------------------------------------------------------------------------

# 18. Rust + Slint 工程架构

## 18.1 技术选型

  模块          选型
  ------------- ---------------------------
  主语言        Rust Stable
  UI            Slint
  Windows API   `windows` / `windows-sys`
  COM           Windows COM 接口
  配置          serde / serde_json
  ID            uuid
  Hash          blake3
  拼音          pinyin
  日志          tracing
  安装包        Inno Setup
  更新校验      SHA-256 + 签名清单

依赖版本必须固定在 Cargo.toml / Cargo.lock。Rust 工具链通过
rust-toolchain.toml 固定。

## 18.2 分层架构

``` text
QuickNest
├── UI Layer
│   ├── MainWindow
│   ├── TitleBar
│   ├── GroupTabs
│   ├── CategorySidebar
│   ├── ItemGrid
│   ├── ItemCard
│   ├── SearchBar
│   └── Settings / Dialogs
│
├── Application Layer
│   ├── GroupService
│   ├── CategoryService
│   ├── ItemService
│   ├── SearchService
│   ├── LauncherService
│   ├── FocusManager
│   └── WindowController
│
├── Domain Layer
│   ├── Group
│   ├── Category
│   ├── Item
│   ├── LaunchSpec
│   └── Settings
│
├── Infrastructure
│   └── Windows
│       ├── InstanceManager
│       ├── NamedPipeServer
│       ├── GlobalHotkey
│       ├── SystemTray
│       ├── ShellService
│       ├── OleDropTarget
│       ├── IconExtractor
│       ├── WindowManager
│       └── DwmManager
│
└── Storage
    ├── ConfigRepository
    ├── AtomicWriter
    ├── BackupManager
    ├── MigrationManager
    └── IconCache
```

## 18.3 项目目录

``` text
quicknest/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── build.rs
├── assets/
│   ├── app.ico
│   ├── app.rc
│   ├── QuickNest.manifest
│   └── themes/
├── ui/
│   ├── main_window.slint
│   ├── components/
│   │   ├── title_bar.slint
│   │   ├── group_tabs.slint
│   │   ├── category_sidebar.slint
│   │   ├── item_grid.slint
│   │   ├── item_card.slint
│   │   ├── search_bar.slint
│   │   ├── status_bar.slint
│   │   └── context_menu.slint
│   └── dialogs/
├── src/
│   ├── main.rs
│   ├── app/
│   ├── domain/
│   ├── application/
│   ├── infrastructure/
│   │   └── windows/
│   ├── storage/
│   └── utils/
├── tests/
├── spikes/
│   └── slint_ole_drop/
└── docs/
```

------------------------------------------------------------------------

# 19. P0：技术可行性验证

**P0 未通过，不允许进入完整 P1。**

## P0-01：基础工程

-   创建 Rust 工程。
-   引入 Slint。
-   显示主窗口。
-   创建基础图标网格。
-   实现显示/隐藏。
-   Release 编译。
-   验证 Windows x64 EXE。

验收：无浏览器内核、无 Node/Python 运行时依赖，窗口稳定显示。

## P0-02：HWND

-   启用 `raw-window-handle-06`。
-   窗口实际创建后获取 HWND。
-   `IsWindow` 验证。
-   验证窗口销毁后的句柄生命周期。

验收：不使用空句柄或失效句柄。

## P0-03：OLE 拖拽

-   检测 UI 线程 COM Apartment。
-   验证 OleInitialize。
-   注册 IDropTarget。
-   拖入 EXE、多个文件、文件夹和 LNK。
-   测试 DragEnter、DragOver、DragLeave、Drop。
-   验证消息泵。
-   验证坐标转换。
-   验证 RevokeDragDrop。

验收：

-   不死锁。
-   不挂起 Explorer。
-   不阻塞 UI。
-   路径正确。
-   中文路径正常。
-   DPI 坐标准确。
-   OLE 资源正确释放。

如果直接注册方案不成立，必须验证独立 STA 宿主。不能只用 WM_DROPFILES
就宣布完整 OLE 验证通过。

## P0-04：COM

-   测试 S_OK、S_FALSE、RPC_E_CHANGED_MODE。
-   验证 STA 消息泵。
-   验证 Shell STA 服务。
-   验证跨线程数据传递。
-   验证 COM 生命周期。

## P0-05：单实例

-   创建 Mutex。
-   创建 Named Pipe。
-   第二实例发送 Show。
-   原实例唤醒。
-   原实例退出后新实例正常接管。
-   异常退出后检查配置。

## P0-06：窗口行为

-   显示/隐藏。
-   固定。
-   失焦。
-   ESC。
-   快捷键。
-   托盘呼出。

## P0-07：JSON 保存

-   首次创建。
-   ReplaceFileW 替换。
-   替换失败保护。
-   临时文件残留。
-   备份恢复。

## P0 交付物

``` text
spikes/
└── slint_ole_drop/
    ├── Cargo.toml
    ├── src/
    ├── README.md
    └── test-results/

docs/
├── P0_VALIDATION.md
├── WINDOWS_API_DECISIONS.md
└── P0_GO_NO_GO.md
```

## P0 决策

  结果                        决策
  --------------------------- -------------------------
  Slint 主窗口直接 OLE 通过   GO
  独立 STA 宿主通过           GO WITH ADAPTER
  只有 WM_DROPFILES           不算完整通过，重新评估
  所有方案均不稳定            NO-GO，重新评估窗口架构

P0 必须提供实际代码和测试结果。

------------------------------------------------------------------------

# 20. P1：基础可用版本（MVP）

## 实现范围

### UI

-   主窗口。
-   顶部一级分组。
-   左侧二级分类。
-   右侧图标网格。
-   标题栏。
-   基础状态栏。

### 管理功能

-   分组增删改和排序。
-   分类增删改和排序。
-   项目增删改。
-   当前状态记忆。

### 启动功能

-   EXE。
-   普通 LNK。
-   文件。
-   文件夹。
-   URI。

### Windows 集成

-   全局快捷键。
-   系统托盘。
-   Mutex。
-   Named Pipe。
-   窗口显示隐藏。
-   基础焦点状态机。
-   P0 已验证的外部拖拽方案。

### 数据

-   JSON。
-   原子保存。
-   自动保存。
-   基础备份。
-   便携版识别。

## P1 验收

1.  创建多个分组和分类。
2.  项目增删改正常。
3.  EXE、LNK、文件、文件夹和 URI 正常启动。
4.  图标正确显示。
5.  外部拖拽正常。
6.  全局快捷键正常。
7.  第二实例唤醒原实例。
8.  重启后数据完整。
9.  保存失败不会破坏旧配置。
10. 没有明显焦点误隐藏。

P1 目标：真正能够替代 Lily 完成日常基础启动工作。

------------------------------------------------------------------------

# 21. P2：完整体验版本

## 搜索

-   实时搜索。
-   拼音全拼。
-   首字母。
-   模糊匹配。
-   全分组搜索。
-   纯键盘启动。

## 拖拽增强

-   内部排序。
-   跨分类移动。
-   跨分组移动。
-   多文件拖入。
-   一级分组 400ms 弹簧加载。
-   取消拖拽恢复。

## 特殊快捷方式

-   UWP。
-   AUMID。
-   MSI 广告快捷方式。
-   特殊 Shell Namespace。
-   控制面板项目。

## 外观

-   浅色/深色/跟随系统。
-   透明度。
-   圆角。
-   Mica/Acrylic。
-   自定义主题色。
-   图标尺寸和网格密度。

## 数据管理

-   自动备份轮转。
-   损坏恢复。
-   导入导出。
-   配置迁移。

## P2 验收

-   拼音首字母准确。
-   搜索无明显卡顿。
-   拖拽无数据错乱。
-   特殊快捷方式能启动或给出明确错误。
-   双屏不同 DPI 正常。
-   主题切换无需重启。
-   配置恢复成功。

------------------------------------------------------------------------

# 22. P3：高级功能

-   批量启动。
-   启动组合。
-   顺序、并行和延时启动。
-   鼠标边缘/角落热区。
-   最近使用。
-   高频排序。
-   收藏置顶。
-   自定义命令管理。
-   Everything 可选集成。
-   轻量插件接口。

插件必须由用户主动安装和启用。插件异常不得影响主程序。插件不得默认拥有全部数据和命令执行权限。

------------------------------------------------------------------------

# 23. P4：正式发布

## 安装版

-   标准安装向导。
-   开始菜单快捷方式。
-   可选桌面快捷方式。
-   开机启动。
-   卸载程序。
-   安装后普通权限启动。

## 便携版

-   解压即用。
-   不要求管理员权限。
-   本地数据目录。
-   支持移动目录和盘符。
-   支持相对路径项目。

## 自动更新

``` text
检查版本
    ↓
获取签名更新清单
    ↓
验证签名
    ↓
下载更新包
    ↓
校验 SHA-256
    ↓
备份旧程序
    ↓
执行更新
    ↓
验证新版本
    ↓
成功 / 回滚
```

更新要求：

-   HTTPS。
-   SHA-256。
-   签名更新清单。
-   更新失败回滚。
-   不覆盖 data。
-   默认不静默安装。
-   用户可关闭更新检查。

------------------------------------------------------------------------

# 24. 性能和稳定性标准

以下是工程目标，必须通过 Release 实测验证，不得未经测试直接承诺。

## 24.1 测试环境

参考：

-   Windows 11 x64。
-   6 核 CPU 或以上。
-   16GB RAM。
-   SSD。
-   集成显卡。
-   100% 缩放。

兼容：

-   Windows 10 22H2 x64。
-   4 核 CPU。
-   8GB RAM。
-   SSD。
-   100% / 150% 缩放。

## 24.2 指标

  指标                      目标
  ------------------------- -------------------------
  隐藏状态平均 CPU          \<0.5%
  空闲 Private Bytes        目标 \<60MB
  空闲 Private Bytes 上限   100MB
  冷启动目标                1 秒以内
  冷启动 P95                2 秒以内
  热键呼出目标              100ms
  热键响应 P95              200ms 以内
  UI 帧率                   目标 60 FPS
  200 项目网格              无明显卡顿
  长期运行                  7 天无明显内存/句柄泄漏

## 24.3 测量规范

-   内存同时记录 Private Bytes 和 Working Set，不混用。
-   CPU 隐藏并预热后采样至少 60 秒。
-   启动时间从进程创建测到窗口可交互，至少 10 次，记录 P50/P95。
-   热键从收到 WM_HOTKEY 到窗口首帧可见，使用
    QueryPerformanceCounter，至少 100 次。
-   长期测试固定 200 个项目，反复显示、隐藏、启动和拖拽。

------------------------------------------------------------------------

# 25. 测试计划

发布前必须覆盖：

1.  全新安装与首次运行。
2.  安装后降权启动。
3.  托盘已有实例时再次启动。
4.  分组创建、重命名、排序、删除。
5.  分类创建、重命名、排序、删除。
6.  200+ 项目网格性能。
7.  EXE、普通 LNK、UWP、MSI、文件、文件夹、URL、URI。
8.  绿色软件工作目录。
9.  中文、空格和特殊字符路径。
10. 目标文件被移动或删除。
11. 快捷键冲突。
12. 外部拖拽。
13. 内部拖拽取消。
14. 文件选择器、编辑弹窗和 UAC 焦点测试。
15. 中文拼音搜索。
16. 多显示器及不同 DPI。
17. 配置文件损坏恢复。
18. 强制结束进程后的配置完整性。
19. 开机自启动无 UAC。
20. 安装版与便携版迁移。
21. 便携版跨盘符移动。
22. Win10 透明效果关闭降级。
23. Win11 Mica/Acrylic。
24. Named Pipe 权限与异常客户端测试。
25. ReplaceFileW 失败保护。

------------------------------------------------------------------------

# 26. 错误处理规范

统一结构化错误：

``` rust
enum QuickNestError {
    ConfigReadError,
    ConfigWriteError,
    ConfigCorrupted,
    MigrationFailed,
    InvalidTarget,
    TargetNotFound,
    PermissionDenied,
    LaunchCancelled,
    LaunchFailed,
    HotkeyConflict,
    IpcUnavailable,
    InstanceAlreadyRunning,
    OleInitializationFailed,
    DragDropRegistrationFailed,
    UnsupportedDropFormat,
    IconExtractionFailed,
    UnsupportedItemType,
    InvalidCommand,
    SecurityConfirmationRequired,
}
```

要求：

-   用户提示易理解。
-   日志记录详细错误码。
-   不向普通用户展示大量堆栈。
-   单项目失败不能导致主程序崩溃。
-   保存失败必须明确提示。
-   拖拽失败不能丢失原数据。

------------------------------------------------------------------------

# 27. 工程质量与 Git 规范

## 27.1 Rust 规范

-   使用 Rust Stable。
-   固定工具链。
-   锁定依赖版本。
-   提交 Cargo.lock。
-   尽量减少 unsafe。
-   unsafe 必须说明安全前提。
-   COM 生命周期明确。
-   禁止未经 Marshaling 跨线程传递 COM 接口。

## 27.2 自动化检查

每次合并前执行：

``` powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

Windows API 代码必须在 Windows x64 实际编译和运行。Linux
编译通过不能证明 Windows 集成正确。

## 27.3 Git 分支

-   `main`：稳定版本。
-   `develop`：集成开发。
-   `feature/*`：功能分支。
-   `fix/*`：修复分支。

每阶段记录完成内容、测试结果、未解决问题和下一阶段任务。

------------------------------------------------------------------------

# 28. AI 编程工具执行规范

本节是交给 AI 编程工具时必须遵循的工作协议。

## 28.1 总体规则

1.  AI 必须先完整阅读本开发文档。
2.  必须按 P0 → P1 → P2 → P3 → P4 顺序推进。
3.  当前阶段未验收，不得直接跳到下一阶段。
4.  不得擅自删除已确认功能。
5.  不得擅自改为 Electron、Tauri 或其他技术栈。
6.  不得擅自增加常驻 Dock。
7.  不得在没有实际测试的情况下声称功能完成。
8.  不得为了通过编译而删除测试或屏蔽错误。
9.  不得一次性生成大量未经验证的代码。
10. 每次修改必须检查现有项目结构，避免覆盖已有实现。

## 28.2 每个任务的执行流程

每次收到"继续"或阶段任务时：

1.  检查 Git 当前分支和工作区。
2.  阅读相关源码和已有文档。
3.  确认当前阶段及未完成任务。
4.  明确本次修改范围。
5.  制定最小可验证实现。
6.  编写代码。
7.  执行格式化、编译、测试。
8.  对 Windows API 进行实际运行验证。
9.  修复本次引入的问题。
10. 更新进度文档。
11. 汇报完成内容和剩余问题。

## 28.3 阶段报告格式

每次开发后输出：

``` text
QuickNest 开发进度报告

当前版本：
当前阶段：
当前任务：

一、本次完成
- ...

二、修改文件
- ...

三、测试结果
- cargo fmt:
- cargo check:
- cargo clippy:
- cargo test:
- cargo build --release:
- Windows 实际测试:

四、未解决问题
- ...

五、当前风险
- ...

六、下一步
- ...
```

没有执行的测试必须写"未执行"，不能写"通过"。

## 28.4 P0 特殊规则

P0 必须优先完成 Slint + HWND + OLE 技术验证。

不得先开发完整 UI 再回头解决拖拽底层问题。

P0 未通过时：

-   暂停依赖 OLE 的完整拖拽开发。
-   输出具体失败日志。
-   提供替代架构。
-   等待用户确认重大架构变化。

## 28.5 代码提交

每个阶段形成可审查的提交。

提交信息示例：

``` text
feat(p0): add slint windows bootstrap
test(p0): validate hwnd lifecycle
feat(p0): implement ole drop target spike
feat(p1): add group and category model
feat(p1): implement item launcher
fix(p1): protect config atomic replacement
```

禁止未经用户确认直接覆盖用户已有代码或强制推送远程主分支。

------------------------------------------------------------------------

# 29. 分阶段开发任务清单

## P0：技术验证

-   [x] P0-01 Rust + Slint 工程。
-   [x] P0-02 HWND 生命周期。
-   [x] P0-03 OLE STA 初始化。
-   [x] P0-04 IDropTarget 注册和撤销。
-   [x] P0-05 Explorer 文件拖入。
-   [x] P0-06 DPI 坐标转换。
-   [x] P0-07 Shell STA 服务。
-   [x] P0-08 Mutex 单实例。
-   [x] P0-09 Named Pipe IPC。
-   [x] P0-10 全局快捷键。
-   [x] P0-11 系统托盘。
-   [x] P0-12 窗口焦点状态机。
-   [x] P0-13 ReplaceFileW 保存验证。
-   [x] P0-14 输出 P0 技术报告。
-   [x] P0-15 GO / NO-GO 决策。

## P1：MVP

-   [x] P1-01 主界面。
-   [x] P1-02 一级分组。
-   [x] P1-03 二级分类。
-   [x] P1-04 项目网格。
-   [x] P1-05 项目 CRUD。
-   [x] P1-06 EXE 启动。
-   [x] P1-07 普通 LNK 启动。
-   [x] P1-08 文件和文件夹启动。
-   [x] P1-09 URI 启动。
-   [x] P1-10 图标提取。
-   [x] P1-11 图标缓存。
-   [x] P1-12 JSON 原子保存。
-   [x] P1-13 基础备份。
-   [x] P1-14 快捷键。
-   [x] P1-15 托盘。
-   [x] P1-16 单实例唤醒。
-   [x] P1-17 外部拖拽。
-   [x] P1-18 基础自动隐藏。

## P2：完整体验

-   [x] P2-01 搜索中心。
-   [x] P2-02 中文拼音。
-   [x] P2-03 纯键盘搜索。
-   [x] P2-04 内部排序。
-   [x] P2-05 跨分类拖拽。
-   [x] P2-06 跨分组拖拽。
-   [x] P2-07 UWP 启动。
-   [x] P2-08 AUMID 激活。
-   [x] P2-09 MSI 快捷方式。
-   [x] P2-10 特殊 Shell 项目。
-   [x] P2-11 右键菜单。
-   [x] P2-12 主题切换。
-   [x] P2-13 透明度和圆角。
-   [x] P2-14 Mica/Acrylic。
-   [x] P2-15 DPI 和多显示器。
-   [x] P2-16 备份恢复。
-   [x] P2-17 配置导入导出。

## P3：高级功能

-   [x] P3-01 批量启动。
-   [x] P3-02 启动组合。
-   [x] P3-03 延时启动。
-   [x] P3-04 鼠标热区。
-   [x] P3-05 最近使用。
-   [x] P3-06 高频排序。
-   [x] P3-07 收藏置顶。
-   [x] P3-08 自定义命令管理。
-   [x] P3-09 Everything 集成。
-   [ ] P3-10 插件接口。

## P4：正式发布

-   [x] P4-01 安装包。
-   [x] P4-02 绿色便携版。
-   [x] P4-03 安装后降权启动。
-   [x] P4-04 自动更新。
-   [x] P4-05 更新签名验证。
-   [x] P4-06 更新失败回滚。
-   [x] P4-07 崩溃日志。
-   [ ] P4-08 Win10 测试。
-   [x] P4-09 Win11 测试。
-   [x] P4-10 数据迁移测试。
-   [x] P4-11 发布说明。
-   [x] P4-12 正式版本验收（P4-08 环境受限遗留，见 docs/P4_VALIDATION.md）。

------------------------------------------------------------------------

# 30. 官方技术参考

开发时优先使用官方文档和实际编译验证。

## Slint

-   https://docs.slint.dev/latest/docs/rust/slint/struct.Window
-   https://docs.slint.dev/latest/docs/rust/slint/

## Windows OLE / COM

-   https://learn.microsoft.com/en-us/windows/win32/shell/dragdrop
-   https://learn.microsoft.com/en-us/windows/win32/api/ole2/nf-ole2-oleinitialize
-   https://learn.microsoft.com/en-us/windows/win32/api/ole2/nf-ole2-registerdragdrop
-   https://learn.microsoft.com/en-us/windows/win32/com/single-threaded-apartments

## Windows 文件系统

-   https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew
-   https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw

## Windows Shell

-   https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw
-   https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw
-   https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iapplicationactivationmanager

## IPC

-   https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipes
-   https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights

## DWM / DPI

-   https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
-   https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows

------------------------------------------------------------------------

# 31. 最终开发基线

QuickNest V1.2 是正式工程开发基线。

所有后续代码、测试、架构调整和功能实现均以本文件为准。

核心开发顺序：

1.  先验证底层技术。
2.  再完成基础界面。
3.  再完成启动和数据管理。
4.  再完善 Windows 系统集成。
5.  再增加搜索、拖拽和高级功能。
6.  最后完成安装、更新和正式发布。

核心原则：

-   不为功能数量牺牲启动速度。
-   不为视觉效果牺牲稳定性。
-   不绕过 Windows 权限机制。
-   不破坏原始快捷方式语义。
-   不允许配置保存失败时静默丢失数据。
-   不允许高级功能影响基础启动器。

**V1.2 文档结束。**
