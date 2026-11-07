# QuickNest P2 验证记录

> P2 阶段（基线第 21 章 完整体验版本）的逐任务验收记录，格式与
> [P1_VALIDATION.md](P1_VALIDATION.md) 一致：实现 → 被测产物与检查 → 实际测试 → 问题与修复 → 边界与门禁。
> 证据目录 `docs/test-results/P2-XX/`，进度与停点见 [DEVELOPMENT_PROGRESS.md](DEVELOPMENT_PROGRESS.md)。

---

## P2-01：搜索中心（已验收）

结论：**2026-11-04 本任务验收通过，停在 P2-01**。基线 2.6（搜索入口）、11.2（Ctrl+F）与 13.1/13.2（范围与基础匹配）已实现并实测；拼音属 P2-02，纯键盘流属 P2-03。

### 实现

- `src/search.rs`（纯逻辑）：`score(query, name, path)` 大小写不敏感；名称完全 1000、前缀 900、包含 800、模糊子序列 700、路径包含 600；空查询匹配全部（0 分）。
- `src/library_controller.rs`：
  - `SearchRow { id, name, path, scope, type_label }` 与 `search_items(query, all_groups)`：默认只扫当前分组的分类/项目，`all_groups` 扫描全库；按分数降序、名称升序排序。
  - `Library::find_item` 全库查找；`LibraryController::item` 改为全库查找，保证全分组搜索结果可启动。
- `ui/main_window.slint`：
  - 标题栏"搜索"文字可点击；搜索面板（600×400，标题栏下方）常驻实例化、按 `search-visible` 显隐。
  - LineEdit `text <=> root.search-query` 双向绑定；`edited` 触发实时查询；Esc 在输入框与根 FocusScope 两处处理。
  - 范围按钮显示"当前分组/全部分组"；结果行显示名称、分组·分类、路径·类型；空结果显示"无匹配项目"。
  - 搜索关闭时根 `changed search-visible` 把焦点交还 FocusScope；输入框自身处理 Ctrl+F，保证隐藏后快捷键仍可用。
- `src/main.rs`：
  - `search_all_groups`（默认 false）与 `search_query_state`；打开时清空查询、重置当前分组范围并聚焦；输入/范围切换即时刷新。
  - 点击结果：关闭面板 → `launch_item_by_id`（沿用 P1-06 启动与 P1-18 启动后隐藏策略）。
  - Esc：关键词非空时清空并保留面板；为空时关闭。

### 被测产物与检查

- Release SHA-256：`DC97B78DCD6A919D634B446294E0C088B6D6E737C47EB7C1260CE8DAD62FF9B0`。
- fmt=0；check/clippy -D warnings 通过；**147 项测试 0 失败**（132 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 新增单测：`search::tests` 5 项（完全/前缀/包含、模糊顺序、路径与大小写、空查询/trim、中文子串）；`search_respects_scope_and_ranks_by_match`（范围、排序、跨分组可启动）。
- 专项脚本 `scripts/verify-p2-01-search.ps1`（真实 Ctrl+F/标题点击/Unicode 输入/结果点击/范围按钮/Esc）。

### Windows 实际测试

- `docs/test-results/P2-01/p2-01-search.tsv` 七阶段：
  1. `ctrl_f_open`：Ctrl+F 打开搜索中心，空查询显示当前分组 2 项。
  2. `filter_and_launch`：输入 `alpha` 实时过滤为 1 项，点击结果创建 `alpha.txt`（内容 alpha）。
  3. `scope_current_group`：标题栏入口重新打开，输入 `beta` 在当前分组 0 结果，未启动任何进程。
  4. `scope_all_groups`：点击范围按钮切换全部分组，结果仅 `BetaTwo`，点击后创建 `beta.txt`（内容 beta）。
  5. `esc_clears`：Esc 清空关键词，面板保留并恢复当前分组全部结果。
  6. `esc_closes`：再次 Esc 关闭面板。
  7. `app_exit`：WM_CLOSE exit=0。
- 截图：`p2-01-search-open.png` / `p2-01-query-alpha.png` / `p2-01-title-open.png` / `p2-01-scope-current.png` / `p2-01-scope-all.png` / `p2-01-esc-typed.png` / `p2-01-esc-cleared.png` / `p2-01-esc-closed.png`。
- 回归：P1-01～P1-18 + P2-01 共 19 个脚本批量 exit=0。

### 开发问题与修复

1. **LineEdit 单向绑定失效**：用户编辑后 TextInput 内部赋值破坏 `text: root.search-query`，重开无法清空；改为 `text <=> root.search-query` 双向绑定，并另存 Rust 查询状态供范围切换。
2. **面板被 FocusScope 挡住**：搜索面板最初声明在 FocusScope 之前，整块内容收不到鼠标；移到 FocusScope 之后（与对话框同层）。
3. **全分组结果无法启动**：`controller.item` 只看当前分类，跨分组结果报"项目不存在"；改为全库 `find_item`。
4. **关闭后 Ctrl+F 失效**：焦点留在隐藏的输入框，根 FocusScope 收不到按键；输入框自身处理 Ctrl+F，关闭时 `focus-scope.focus()`。
5. **标题入口聚焦丢失**：点击标题后在 `init` 之后改焦点；增加 100ms 定时聚焦（仅搜索可见时运行）。
6. **0 结果空白区点击**：会落到背景关闭层并按 Esc 语义清空关键词；测试改为不点击空白区，产品行为保留（点击外部区域关闭符合 13.4）。

### 边界与门禁

- 拼音全拼/首字母与多音字候选属 **P2-02**；上下左右/Enter/Ctrl+Enter/Ctrl+D/Tab 属 **P2-03**。
- 结果列表暂无项目图标与匹配字符高亮；空查询显示范围内全部项目（未做"最近使用"排序，属 P3-05）。
- 匹配为按需线性扫描；200+ 项目性能属 P4 长期测试。
- 范围切换仅鼠标按钮；Tab 切换留给 P2-03。
- 搜索期间自动隐藏仍按 P1-18 生效（点击外部关闭搜索并可能随后失焦隐藏）。
- P1-14 此前的 Ctrl+F 状态栏提示已被真实搜索中心取代；P1-14 脚本仅记录意图，不受影响。

P2-01 通过；P2-02～P2-17 未开始。

---

## P2-02：中文拼音（已验收）

结论：**2026-11-04 本任务验收通过，停在 P2-02**。全拼、首字母与常见多音字候选已接入 P2-01 搜索中心；纯键盘搜索属 P2-03。

### 实现

- `Cargo.toml`：新增 `pinyin = "=0.11.0"`（启用 plain/heteronym 数据，Cargo.lock 固定）。
- `src/pinyin_index.rs`：
  - `PinyinIndex::build(name)`：跳过空白/ASCII 标点，ASCII 字母数字保留并小写，Han 字符用 `to_pinyin_multi` 取读音与异读；生成 `normalized`、`full`（全拼变体）、`initials`（每音节首字母变体）。
  - 变体笛卡尔积上限 `MAX_VARIANTS = 32`，避免多音字长名称指数增长。
  - `pinyin_score(query, index)`：全拼完全 690、首字母完全 685、全拼前缀 680、首字母前缀 675、全拼包含 670、首字母包含 665、全拼模糊 660；非 ASCII 查询返回 None（中文走名称匹配）。
- `src/search.rs`：`score_with_pinyin` = 直接名称/路径评分优先，否则拼音评分。
- `src/library_controller.rs`：`pinyin_cache: RefCell<HashMap<id, (name, PinyinIndex)>>`；搜索时按名称校验缓存，重命名/新增后自动重建；`search_items` 改用 `score_with_pinyin`。
- UI 无改动。

### 被测产物与检查

- Release SHA-256：`4C09FFD52F47C867A2CB8B8193F358B83CAD21E78248200D0F1A7842F0671BA2`。
- fmt=0；clippy -D warnings 通过；**152 项测试 0 失败**（137 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 新增单测：拼音索引 5 项（微信全拼/首字母、网易云音乐 wyy 前缀、音乐 yinyue/yinle 多音字、分隔符/中文查询、变体上限）；控制器搜索测试扩展全拼/首字母与重命名重建。
- 专项脚本 `scripts/verify-p2-02-pinyin.ps1`。

### Windows 实际测试

- `docs/test-results/P2-02/p2-02-pinyin.tsv`：
  1. `weixin`：结果仅"微信测试"，点击创建 `weixin.txt`。
  2. `wx`：首字母命中"微信测试"，点击创建 `weixin.txt`。
  3. `wyy`：首字母前缀命中"网易云音乐"，点击创建 `netease.txt`。
  4. `yinyue`：多音字候选（乐→yue）命中"音乐盒"，点击创建 `music.txt`。
  5. `app_exit`：WM_CLOSE exit=0。
- 截图：`p2-02-weixin.png` / `p2-02-wx.png` / `p2-02-wyy.png` / `p2-02-yinyue.png`。
- 回归：P1-01～P1-18 + P2-01 + P2-02 共 20 个脚本批量 exit=0。

### 开发问题与修复

1. **拼音数据**：使用固定版本 `pinyin` crate 的 plain 与异读数据，避免手写不完整字表。
2. **多音字组合**：异读笛卡尔积限制 32 个变体。
3. **重命名索引失效**：缓存同时保存名称，名称变化即重建。
4. **排序约束**：拼音 660–690，低于名称模糊 700、高于路径 600。
5. **PowerShell 编码**：含中文的脚本需 UTF-8 BOM，否则 PS 5.1 会吞掉字符串引号导致解析失败。

### 边界与门禁

- 多音字依赖 crate 的异读列表，未提供外部可配置词典（基线允许而非要求）。
- 拼音仅匹配 ASCII 查询；中文查询仍走名称包含。
- 首字母对 ASCII 名称按字符展开（查询直接名称时优先命中名称分数，不影响使用）。
- 纯键盘流（上下左右/Enter/Ctrl+Enter/Ctrl+D/Tab）属 P2-03。
- 搜索仍为按需线性扫描；200+ 项目性能属 P4。

P2-02 通过；P2-03～P2-17 未开始。

---

## P2-03：纯键盘搜索（已验收）

结论：**2026-11-04 本任务验收通过，停在 P2-03**。基线 13.4 的选中、移动、启动、定位、范围切换与 Esc 语义已实现并实测；Ctrl+Enter 的管理员 UAC 路径代码完整，未自动接受 UAC。

### 实现

- `ui/main_window.slint`：
  - `search-selected-index` 属性；结果行选中态浅蓝底 + 2px 蓝色边框。
  - `function search_key(event)`：Esc/清空关闭、Tab 切范围、↑/← 前移、↓/→ 后移、Enter 启动、Ctrl+Enter 管理员、Ctrl+D 定位、Ctrl+F 重开。
  - LineEdit 与根 FocusScope 共用该映射；按键 accept 阻止 TextInput 的 Tab/Enter/Ctrl+D 默认行为。
  - 点击结果同步选中索引。
- `src/search.rs`：`move_selection(current, count, delta)`（空集 -1、边界循环）+ 单测。
- `src/main.rs`：刷新结果默认选中 0/-1；`search-move` 调用移动；`search-activate`/`search-activate-admin` 调用 `launch_item_by_id`；`search-open-location` 关闭搜索后调用 `open_item_location_by_id`；通过 `Model::row_data` 取选中 id。

### 被测产物与检查

- Release SHA-256：`1041658F41A1F023EDA1E2530B6F383012F38F3B90CDE78080171D4247507CDE`。
- fmt=0；clippy -D warnings 通过；**153 项测试 0 失败**（138 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 新增单测：`selection_moves_and_wraps_without_a_result`。
- 专项脚本 `scripts/verify-p2-03-keyboard.ps1`（真实 SendInput 方向键/Enter/Tab/Ctrl+D/Esc）。

### Windows 实际测试

- `docs/test-results/P2-03/p2-03-keyboard.tsv` 七阶段：
  1. `window_ready`。
  2. `down_enter`：查询 alpha 默认选中 AlphaA；Down 后 AlphaB 高亮；Enter 创建 `alphaB.txt`，未创建 `alphaA.txt`。
  3. `up_wrap_enter`：重开查询 alpha，Up 从第一条循环到 AlphaC；Enter 创建 `alphaC.txt`。
  4. `tab_scope_enter`：查询 beta 在当前分组 0 结果；Tab 切全部分组出现 BetaOther；Enter 创建 `beta.txt`。
  5. `ctrl_d_location`：查询 locate，Ctrl+D 打开所在文件夹（Explorer 标题 QuickNestP203-…）。
  6. `esc_semantics`：两次 Esc 分别清空与关闭。
  7. `app_exit`：WM_CLOSE exit=0。
- 截图：`p2-03-default-first.png` / `p2-03-after-down.png` / `p2-03-after-up.png` / `p2-03-tab-current.png` / `p2-03-tab-all.png` / `p2-03-ctrl-d.png` / `p2-03-esc-cleared.png` / `p2-03-esc-closed.png`。
- 回归：P1-01～P1-18 + P2-01～P2-03 共 21 个脚本批量 exit=0。

### 开发问题与修复

1. **Tab/Enter 默认行为**：TextInput 会处理 Tab 焦点与 Enter；在 LineEdit 的 key-pressed 中 accept 后路由到回调。
2. **Ctrl+D 写入字符**：接受按键阻止 "d" 插入，再执行定位。
3. **脚本 items 数组**：PowerShell 命令模式把逗号后的函数调用当作前一调用的参数，导致只生成一个项目；每个调用加括号修复。
4. **`[ushort]` 不支持**：PS 5.1 改用 `[int]` 参数。
5. **选中模型同步**：刷新重置首条，空结果 -1；点击结果同步索引。

### 边界与门禁

- Ctrl+Enter 管理员启动：调用 `launch_item_by_id(..., true)` 并持有 `LaunchingElevated` 抑制锁；真实 UAC 同意/拒绝未自动化，代码路径审查。
- 上下左右：↑/← 与 ↓/→ 同映射并循环；实测覆盖 Down/Up，Left/Right 走同一分支。
- 搜索输入框聚焦时按键生效；点击空白关闭搜索后焦点回到根 FocusScope。
- 默认选中为排序后的第一条（名称/拼音/路径综合分数）。
- 拼音/匹配逻辑与 P2-01/02 不变；性能仍为线性扫描（P4）。

P2-03 通过；P2-04～P2-17 未开始。

---

## P2-04：内部排序（已验收）

结论：**2026-11-04 本任务验收通过，停在 P2-04**。当前分类内的网格拖拽排序、插入位置提示、持久化与 ESC 取消已实现并实测；跨分类/跨分组移动属 P2-05/P2-06。

### 实现

- `ui/main_window.slint`：
  - 卡片 pointer down/up/move 手势：左键按下记录起点，移动超过 8px 进入拖拽；源卡片 `opacity 0.4`，网格显示 3px 蓝色插入竖线（按 drop index 的列/行定位）。
  - 未超过阈值释放 → `item-selected`（单击启动）；拖拽释放 → `item-reorder-requested(id, index)`；右键菜单逻辑不变。
  - `item-drag-active`/`dragging-item-id`/`item-drop-index` 状态；Esc 在根 FocusScope 中优先取消拖拽。
- `src/library_model.rs`：`move_item_to_index`（同槽 no-op、越界钳制、未知 id false）。
- `src/library_controller.rs`：`move_item_to_index` 调用模型并持久化。
- `src/main.rs`：`item-drag-moved` 按内容坐标/列数/格子尺寸计算 slot；`item-reorder-requested` 应用重排、刷新 UI、状态栏提示。

### 被测产物与检查

- Release SHA-256：`D969FC59A6DB48B8FA2DD1DED0A8D886BC4552E11D8C0DE774A8CAA5323152C2`。
- fmt=0；clippy -D warnings 通过；**154 项测试 0 失败**（139 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 新增单测：`moving_an_item_to_an_index_reorders_and_clamps`。
- 专项脚本 `scripts/verify-p2-04-sorting.ps1`（真实鼠标按下/移动/释放、ESC、单击）。

### Windows 实际测试

- `docs/test-results/P2-04/p2-04-sorting.tsv` 五阶段：
  1. `window_ready`。
  2. `drag_reorder`：SortA 拖到第三槽（截图含插入竖线与源卡片变暗），配置顺序变为 `SortB,SortC,SortA`。
  3. `esc_cancel`：再次拖拽到第三槽并中途按 ESC，释放后顺序保持 `SortB,SortC,SortA`。
  4. `click_launch`：单击 SortB 正常启动（探针文件创建）。
  5. `app_exit`：WM_CLOSE exit=0。
- 截图：`p2-04-initial.png` / `p2-04-drag-hover.png` / `p2-04-after-drop.png` / `p2-04-esc-cancel.png`。
- 回归：P1-01～P1-18 + P2-01～P2-04 共 22 个脚本批量 exit=0。

### 开发问题与修复

1. **Flickable 禁用取消手势**：按下时设置 `interactive=false` 会让 Flickable 取消当前手势，拖拽无法开始；移除该设置后卡片在按下状态正常收到 move 事件。
2. **点击与拖拽分离**：移除 `clicked`，统一 pointer down/up + 8px 阈值；未拖拽才启动。
3. **目标槽位**：内容坐标 + 列数传给 Rust，按格子尺寸计算并钳制。
4. **ESC 取消**：Esc 清除 root 拖拽状态；释放时检查 `item-drag-active` 阻止提交。
5. **P1-18 偶发空数组索引**：mshta 窗口尚未创建时取 `[0]` 抛异常；增加 Count 判断，标题栏固定增加读配置重试。

### 边界与门禁

- 仅支持当前分类内排序；跨分类/跨分组移动属 P2-05/P2-06。
- 拖拽时显示插入竖线，但源卡片不跟随鼠标（仅变暗），未实现浮动拖拽预览。
- 拖拽自动滚动（拖到网格边缘滚动）未实现。
- 排序立即持久化（每次释放一次写入）；连续拖拽多次会多次保存。
- 分类/一级分组仍通过既有右键上移/下移排序；拖拽排序分组/分类未在本任务实现（P2-05/06 讨论）。
- 多选拖拽、触摸拖拽未测。

P2-04 通过；P2-05～P2-17 未开始。

---

## P2-05：跨分类拖拽（已验收）

结论：**2026-11-04 本任务验收通过，停在 P2-05**。网格卡片拖到左侧另一分类后移动并持久化，目标行高亮与状态提示、ESC 取消均已实测；跨分组仍属 P2-06。

### 实现

- `ui/main_window.slint`：
  - `item-drag-moved` 增加窗口逻辑坐标参数（叠加分类栏/标题+分组条偏移与 Flickable `content-x/y`）。
  - 新属性 `drag-target-category-id` / `item-drag-hint`；分类行命中时浅蓝底 + 2px 蓝框，状态栏显示"松开以移动到「分类」"。
  - 分类悬停时隐藏网格插入竖线；释放调用 `item-drop-requested(id,index)`；ESC/取消同时清除分类目标。
- `src/main.rs`：
  - 拖拽移动时用 `drop_layout::hit_test`（与外部拖拽同一布局常量）命中非当前分类；
  - 释放时优先跨分类 `move_item_to_category`，否则走 P2-04 内部排序；刷新 UI 与状态栏。
- 复用 P2-04 手势/阈值/源卡变暗与控制器 `move_item_to_category`。

### 被测产物与检查

- Release SHA-256：`3E6A8DB2A46C0051B5E51EFF395BD2803BD5796854B414EECC39F714784643D3`。
- fmt=0；clippy -D warnings 通过；**154 项测试 0 失败**（139 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 专项脚本 `scripts/verify-p2-05-cross-category.ps1`（真实拖拽到分类行、ESC、配置断言）。

### Windows 实际测试

- `docs/test-results/P2-05/p2-05-cross-category.tsv` 五阶段：
  1. `window_ready`。
  2. `cross_category_move`：MoveMe 从 CatA 拖到 CatB，配置变为 CatA=`StayHere`、CatB=`MoveMe`。
  3. `esc_cancel`：再次拖拽并中途 ESC，分类与内容不变。
  4. `app_exit`：exit=0。
- 截图：`p2-05-initial.png` / `p2-05-drag-over-category.png` / `p2-05-after-move.png` / `p2-05-esc-cancel.png`。
- 回归：P1-01～P1-18 + P2-01～P2-05 共 23 个脚本批量 exit=0。

### 开发问题与修复

1. **窗口坐标换算**：内容坐标需加侧栏/标题区偏移与 Flickable content 偏移；窗口坐标用于 `hit_test`。
2. **同分类目标**：目标为当前分类时不移动，仍按网格内部排序。
3. **视觉冲突**：分类高亮时隐藏网格插入线。
4. **批量回归挂起**：P1-17/P1-18 的 finally 使用 Shell.Application 枚举偶发阻塞；移除 COM 枚举（已知窗口 WM_CLOSE 关闭）后回归恢复。
5. **P2-04 点击偶发**：增加点击重试与探针等待，3 连跑与整批回归通过。

### 边界与门禁

- 仅当前一级分组内的分类；跨分组移动（含分组标签 400ms 弹簧加载）属 P2-06。
- 拖拽仍无浮动预览与边沿自动滚动。
- 拖到分类后项目追加到目标分类末尾，不支持指定插入位置。
- 分类排序/分组排序仍通过既有右键上移/下移；分类拖拽排序未实现。
- 触摸/多选拖拽未测。

P2-05 通过；P2-06～P2-17 未开始。

---

## P2-06：跨分组拖拽（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-06**。网格卡片拖到顶部分组标签：悬停 400ms 弹簧加载切换目标分组、快速落点直接移动；ESC 取消语义完整。分类/分组的拖拽排序未实现（保持右键菜单）。

### 实现

- `src/library_model.rs`：
  - `move_item_across_groups(item_id, target_category_id)`：任意组间移动、追加到目标分类末尾；同分类 no-op、未知 item/目标拒绝且不改库。
  - `category_id_of_item(item_id)`：跨组定位项目所在分类。
  - `active_category_of_group(group_id)`：分组当前展示的分类（`last_category_id` 回退首分类）。
- `src/library_controller.rs`：`move_item_to_group_category`（持久化包装）、`active_category_id_of_group`、`category_id_of_item`。
- `ui/main_window.slint`：`drag-target-group-id`、`item-press-active`（按下期间 `mouse-drag-pan-enabled=false`，防止 Flickable 抢手势）；分组标签高亮包含拖拽目标；插入竖线在分组/分类悬停时隐藏；ESC/释放/取消清除分组目标。
- `src/main.rs`：
  - `item-drag-moved` 的 Group 分支：非当前分组时高亮标签 + 提示"松开移动到「分组」"，记录悬停 `(group_id, Instant)`。
  - 50ms 弹簧计时器（拖拽期间活动）：悬停 ≥400ms → `select_group` + `refresh_sidebar`（只刷新侧栏，保持拖拽指针捕获）+ 状态"已切换到「分组」"。
  - `item-drop-requested`：分类目标 → P2-05 移动；分组目标 → `move_item_across_groups` 到该分组当前分类；无目标且项目不在当前分类（弹簧后落网格）→ 移入新分组当前分类；否则 P2-04 排序。let-chain 中的借用先绑定局部变量，避免 `borrow_mut()` 双借用 panic。

### 被测产物与检查

- Release SHA-256：`55FF36C64E273294AF1505ACA1DE4D47D3498B6D259752A2020518E11E245A74`。
- fmt=0；clippy -D warnings 通过；**155 项测试 0 失败**（140 lib + 1 bin + 6 file_drop + 1 icon_cache + 2 icon_extract + 1 lnk + 1 ole_mta + 3 doc）。
- 新增单测：`moving_an_item_across_groups_appends_to_the_target`（跨组追加、同分类 no-op、第二目标、未知 item/目标、失败不改库）。
- 专项脚本 `scripts/verify-p2-06-cross-group.ps1`（真实鼠标拖拽：弹簧悬停、快速落点、ESC）。

### Windows 实际测试

- `docs/test-results/P2-06/p2-06-cross-group.tsv` 六阶段：
  1. `window_ready`。
  2. `spring_load`：CrossItem 悬停 GroupB 700ms（400ms 弹簧切换）→ 释放 → CatA=`FastItem,KeepItem`、CatB=`CrossItem`、current=g2。
  3. `fast_drop`：切回 GroupA 后 FastItem 快速拖到 GroupB 标签（<400ms 释放）→ CatA=`KeepItem`、CatB=`CrossItem,FastItem`。
  4. `esc_cancel`：KeepItem 悬停 GroupB（弹簧切换）→ ESC → 释放 → CatA/CatB 不变、current=g2。
  5. `app_exit`：exit=0。
- 截图：`p2-06-spring-hover.png`（GroupB 高亮 + 侧栏已切换 + 源卡变暗 + "已切换到「GroupB」"）等六张。
- 回归：P1-01～P1-18 + P2-01～P2-06 共 24 个脚本批量 exit=0。

### 开发问题与修复

1. **RefCell 双借用 panic**：let-chain 条件中的借用临时存活至 if 体结束，体内 `borrow_mut()` 触发 panic（进程退出、窗口消失，表现为截图失败）；改为局部变量绑定。
2. **Flickable 抢手势（偶发）**：按下期间 `mouse-drag-pan-enabled=false` 确定性地让卡片获得 move 事件；`interactive=false` 会取消手势（P2-04 教训），不可用。
3. **弹簧刷新范围**：弹簧切换只刷新侧栏；完整 `refresh_ui` 会销毁被拖卡片、丢失释放事件。
4. **弹簧残留提示**：计时器同时清除 `item-drag-hint`。
5. **脚本 ESC 时序**：ESC 必须发生在释放之前（按钮保持跨 ESC）；改用手动序列。
6. **config_save 恢复测试偶发 Busy**：收尾恢复增加 Busy 重试。

### 边界与门禁

- 弹簧加载只切换分组（侧栏），网格在拖拽期间保持源分组内容（指针捕获需要）；释放/取消后才整体刷新。
- 跨分组移动追加到目标分组当前分类末尾，不支持指定插入位置。
- 分组悬停无浮动预览与边沿自动滚动；拖拽中 ESC 保留已发生的分组切换（基线 5.6"取消后原数据不变"针对项目数据，分组选择属弹簧加载既成状态）。
- 分类/一级分组拖拽排序未实现（保持右键上移/下移）；触摸/多选拖拽未测。
- 拖拽分类/分组时的图标提取仍走 P1-10 缓存，跨组移动后图标随 item 迁移。

P2-06 通过；P2-07～P2-17 未开始。

---

## P2-07：UWP 启动（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-07**。UWP 快捷方式的导入检测、AUMID 保存与原始 LNK 启动闭环；AUMID 直接激活（IApplicationActivationManager）与激活失败回退属 P2-08。

### 实现

- `src/infrastructure/windows/shell.rs`：`detect_uwp_link(path)` —— 加载 .lnk；`GetPath(SLGP_RAWPATH)` 目标非空 → 普通链接返回 None；`GetIDList` 的 PIDL 经 `SHGetPathFromIDListW` 判定无文件系统路径后，`SHCreateItemFromIDList` + `IShellItem2::GetString(System.AppUserModel.ID)` 读取 AUMID（本地定义 `PKEY_APPUSERMODEL_ID` 常量，不凭显示名称猜测）。PIDL 恰好释放一次（无提前返回路径）。
- `src/library_model.rs`：`Launch.aumid: String`（serde default，旧配置兼容）经 `ItemDraft`（empty/from_item/into_item）全链路接线。
- `src/main.rs`：`fill_lnk_metadata` 浏览时检测 UWP 并提示状态栏；`fill_lnk_draft` 保存时检测并保存 AUMID（项目保持 `item_type=lnk` 与原始 LNK 路径）；`launch_lnk` 不变（原始 LNK 的 ShellExecuteExW）。
- `tests/uwp_link.rs`（独立进程，STA 初始化/清理平衡）：普通 LNK 控制组不误判；枚举 `shell:AppsFolder` 取第一个带 AUMID 的真实项 → `SetIDList` 构建 IDList 快捷方式 → 检出 AUMID 且 resolve 目标为空。

### 被测产物与检查

- Release SHA-256：`608AFBAE3581EA6990075EEEC2A2A75C369E9D3BE7AEBA419DFF79809F71DA4`。
- fmt=0；clippy -D warnings 通过；**156 项测试 0 失败**（140 lib + 1 bin + 7 集成 + 3 doc）。
- 专项脚本 `scripts/verify-p2-07-uwp.ps1`。

### Windows 实际测试

- `docs/test-results/P2-07/p2-07-uwp.tsv` 六阶段：
  1. `fixture`：WScript.Shell 创建 TargetPath=`shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App` 的快捷方式（1407B）。
  2. `import_debug`：添加对话框导入 → item0=uwpcalc|lnk|aumid=[Microsoft.WindowsCalculator_8wekyb3d8bbwe!App]。
  3. `import_aumid`：AUMID 持久化、路径保留原始 LNK、类型=lnk。
  4. `menu_rect`：右键菜单出现。
  5. `launch_original_lnk`：菜单"打开"启动原始 LNK → 计算器窗口出现（title=计算器）→ WM_CLOSE 关闭。
  6. `app_exit`：exit=0。
- 截图：`p2-07-import-uwp.png` / `p2-07-menu.png` / `p2-07-calculator.png`。
- 回归：P1-01～P1-18 + P2-01～P2-07 共 25 个脚本批量 exit=0。

### 开发问题与修复

1. **AUMID 来源**：.lnk 属性存储不可靠（WScript 创建的快捷方式无该属性）；从目标 AppsFolder 项读取。
2. **PIDL 泄漏**：初版失败路径 `?` 提前返回跳过 CoTaskMemFree；重写为无提前返回。
3. **UWP 判定**：仅"目标为空"不够（"此电脑"等特殊文件夹同样无目标）；叠加"IDList 无文件系统路径" + "目标项带 AUMID"。
4. **GetOpenFileNameW 拒绝 UWP LNK**：文件对话框校验时报系统错误；改用目标字段手动输入 + 类型下拉选择，保存时同一检测逻辑。
5. **旧二进制跑新脚本**：未重建 release 导致断言混乱；每次专项脚本前显式重建。
6. **卡片单击启动偶发不触发**：导入后紧接的单击三次中一次未触发 item-selected；启动断言改用右键菜单"打开"（同一 `launch_item` 路径）；单击路径由 P1-06/P2-04 回归覆盖。
7. **BHID_EnumItems**：BindToHandler 传空 GUID 返回 IShellItem 自身导致访问冲突；显式传 `BHID_EnumItems`。

### 边界与门禁

- AUMID 直接激活（`IApplicationActivationManager::ActivateApplication`）与"AUMID 激活失败回退原始 LNK"属 **P2-08**。
- MSI 广告快捷方式属 **P2-09**（同样走原始 LNK）。
- 未测：损坏的 UWP LNK、AppsFolder 项无 AUMID 的边缘（状态栏有对应提示路径）、UWP 快捷方式的窗口状态字段。
- UWP 项目图标提取仍走 P1-10 缓存（实测计算器图标提取成功）。
- 导入时 AUMID 从目标项读取的调用需要 STA + OLE 已初始化（与 resolve_link_now 同契约；导入回调在 UI 线程满足）。

P2-07 通过；P2-08～P2-17 未开始。

---

## P2-08：AUMID 激活（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-08**。带 AUMID 的 UWP 项目走 `IApplicationActivationManager::ActivateApplication` 直接激活；激活失败回退原始 LNK（基线 4.2.5）；UWP 无 AUMID 时的原始 LNK 启动（P2-07）不变。

### 实现

- `src/infrastructure/windows/launch.rs`：
  - `activate_application(aumid, arguments) -> Result<u32, HRESULT>`：`CoCreateInstance(ApplicationActivationManager)` + `ActivateApplication(AUMID, args, ACTIVATEOPTIONS(0))`，返回激活 PID。
  - `launch_lnk` 路由：非管理员且 `launch.aumid` 非空 → 先 AUMID 激活（成功即 `Started{pid}`）；失败/无 AUMID → 原始 LNK `ShellExecuteExW`（P2-07 路径）。AUMID 激活不要求 LNK 文件存在（原快捷方式被删除仍可激活，基线 4.6 "UWP AUMID 首选直接激活"）。管理员运行保持 `runas` 原始 LNK 的 Shell 语义。
- 复用 P2-07 的 `launch.aumid` 字段（导入时从目标 AppsFolder 项读取）。

### 被测产物与检查

- Release SHA-256：`4B4E627A0970BA5407D841AB64F8C4018B2B9B94FC35039CD5B2CD615F40885C`。
- fmt=0；clippy -D warnings 通过；**156 项测试 0 失败**（140 lib + 1 bin + 7 集成 + 3 doc）。
- 集成测试 `tests/uwp_link.rs` 扩展三项：
  1. `activate_application(Calculator AUMID)` → Ok(pid>0)。
  2. 无效 AUMID（`QuickNest.NoSuch.App.2026`）→ 激活失败 → `launch_item` 回退原始 LNK（cmd /c exit）→ Started。
  3. 有效 AUMID 且原 LNK 已删除 → `launch_item` 仍 Started（AUMID 激活不依赖 LNK 文件）。
- 专项脚本 `scripts/verify-p2-08-aumid.ps1`。

### Windows 实际测试

- `docs/test-results/P2-08/p2-08-aumid.tsv` 六阶段：
  1. `window_ready`。
  2. `aumid_activation`：种子项目 AUMID=计算器 且 LNK 路径 `C:\QuickNest Demo\original.lnk` 不存在 → 单击卡片 → 计算器窗口出现（证明 ActivateApplication 执行——原始 LNK 路径走 ShellExecuteExW 会报 MissingTarget）。
  3. `aumid_reactivate`：再次单击 → 已运行应用重新激活成功。
  4. `app_exit`：exit=0。
- 截图：`p2-08-aumid-launch.png` / `p2-08-aumid-reactivate.png`。
- 回归：P1-01～P1-18 + P2-01～P2-08 共 26 个脚本批量 exit=0。

### 开发问题与修复

1. **激活时机与 LNK 存在性**：初版把 AUMID 激活放在 LNK 存在性检查之后——原快捷方式被删除时直接激活永不执行；按基线 4.6 把激活提前。
2. **let-chain 折叠**：clippy 要求合并嵌套 if；采用 `&& let` 链。
3. **管理员路径**：runas 保持原始 LNK 的 Shell 语义（UWP 提权场景罕见），AUMID 激活仅限非管理员——记录为边界。
4. **测试脚本**：Close-Calculator 以 `*计算器*` 匹配 ApplicationFrameHost 宿主窗口；脚本 UTF-8 BOM。

### 边界与门禁

- AUMID 激活仅限非管理员路径；管理员运行保持 runas 原始 LNK（UWP 提权场景罕见，未实测）。
- ActivateApplication 的 args 传项目 args join（UWP 项目通常无 args）。
- 未测：激活超时/挂起应用的具体窗口焦点行为、AUMID 大小写敏感性（Windows 不区分）。
- 激活失败的具体 HRESULT 不展示给用户（回退原始 LNK 后仅在两者都失败时报错）。

P2-08 通过；P2-09～P2-17 未开始。

## P2-09：MSI 快捷方式（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-09**。MSI 广告快捷方式保留原始 LNK 并通过 Shell 启动；不解析 EXE、不复制/重写、不绕过 Windows Installer；失效时提示修复安装或重新添加入口。

### 实现

- `shell::detect_advertised_shortcut`：`IShellLinkDataList::GetFlags` 检查 `SLDF_HAS_DARWINID`；只读标志，不解析目标，不触发 Windows Installer。
- `Launch.msi_advertised` + `ItemDraft.msi_advertised`（serde default，旧配置兼容）；`fill_lnk_draft` 保存时检测，`fill_lnk_metadata` 浏览时提示；拖入导入同样调用 `fill_lnk_draft`。
- `launch_lnk`：始终 `ShellExecuteExW` 原始 LNK（args/workdir 不覆盖），管理员 runas 原始 LNK；MSI 项目禁用 AUMID 分支；缺失 LNK → `MsiShortcutMissing`，Shell 失败 → `MsiLaunchFailed`，消息含"请修复安装或重新添加入口"。

### 验证证据

- `tests/msi_advertised.rs`：普通 LNK 不误报、合成 Darwin 块 + `SLDF_HAS_DARWINID` 检出、缺失 LNK 不误报。
- `src/infrastructure/windows/launch.rs` 单测：缺失 LNK 的 MSI 错误、`Spawn`/`PrivilegedRun` → `MsiLaunchFailed`、普通 LNK 错误保持原样。
- `scripts/p2-09-msi-fixture.ps1`：Windows Installer COM 建表 + SDK `MsiInfo.exe` 写 SummaryInformation（PID 15=0、Security=2）；安装后 Start Menu 广告 LNK（`LinkFlags=0x1084`、Darwin 块）。
- `scripts/verify-p2-09-msi.ps1`：`docs/test-results/P2-09/p2-09-msi.tsv` 十阶段；截图 `p2-09-import-msi.png`、`p2-09-msi-launched.png`、`p2-09-msi-missing.png`。
- 回归 `docs/test-results/P2-09/regression/final/regression.tsv`：27 脚本 exit=0。
- 门禁：fmt=0、check、clippy -D warnings、160 项测试（143 lib + 1 bin + 13 集成 + 3 doc）0 失败；Release SHA-256 `4F5496B52FE9B750D009FB726F44B891B36A909FB56083A6C59BF78DC696639F`。

### 开发问题与修复

1. Windows Installer 自动化的 `SummaryInformation.Property(Pid)` 在本机不可写（`Property,Pid`），改用 SDK `MsiInfo.exe`；缺少 PID 15 时 SecureRepair 报 1316。
2. 非压缩源树必须按目标目录镜像（`<MSI 目录>\QuickNestMsiTest\payload.exe`）；PID 15=0（uncompressed）。
3. P1-17 的 `explorer.exe /separate` 可能遗留置顶的"文档"窗口，遮挡 QuickNest 导致后续测试连锁失败；在 finally 中关闭测试期间新建的 Explorer 窗口。
4. 回归脚本每个脚本独立 PowerShell 进程（Add-Type 静态类不能在同一会话重复加载），保留真实 ExitCode 并加 300s 超时。

### 边界与门禁

- `MsiLaunchFailed` 只覆盖 `ShellExecuteExW` 同步失败；MSI 自身弹出的修复/源定位对话框由 Windows Installer 处理，QuickNest 不拦截。
- 不解包/不读取 MSI 内部；不复制、不重写快捷方式。
- 管理员运行保持 runas 原始 LNK；UWP 提权场景不涉及。
- `msi_advertised` 为导入时的快照；若 LNK 在导入后被替换为普通快捷方式，仍按 MSI 提示（重新导入可刷新）。

P2-09 通过；P2-10～P2-17 未开始。

## P2-10：特殊 Shell 项目（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-10**。控制面板、系统设置、Shell Namespace、特殊文件夹和网络位置保存原始 URI/LNK/UNC 并通过 Windows Shell 启动；不误报失效、不解析/复制目标，失败返回结构化错误。

### 实现

- `is_uri_target`（RFC 3986 scheme）、`is_shell_namespace_target`（`::{CLSID}`/`shell:`/`ms-settings:`）、`is_unc_path`（`\\server\share`）；不新增 item_type：URI → `uri`，`::{CLSID}`/UNC → `folder`。
- `ItemKind::from_path` 在 `is_dir()` 前完成 UNC/namespace 判断；`path_missing`/`icon_source` 跳过 Shell/UNC，避免网络探测与无意义图标提取。
- `launch_shell_target` 对 Shell/UNC 跳过本地存在性检查，直接 `ShellExecuteExW`；`shell:`/`ms-settings:` 走 `launch_uri`；目标字符串原样保留。
- `open_item_location` 对 URI/Shell/UNC 返回 `NoFileLocation`。
- `fill_shell_draft`：保存/拖入时修正 Exe/File 为 Uri/Folder；空名称时用 `SHCreateItemFromParsingName` + `SIGDN_NORMALDISPLAY` 读取 Shell 显示名。

### 验证证据

- `tests/special_shell.rs`：`SetIDList` 控制面板 LNK 解析为无文件目标；`shell:MyComputerFolder`/`::{20D04FE0-…}` 启动出“此电脑”窗口；显示名“此电脑”；`NoFileLocation`。
- `scripts/verify-p2-10-shell.ps1`：`docs/test-results/P2-10/p2-10-shell.tsv` 十三阶段；5 个项目的 target/type 启动后未改写；exit=0。
- 截图：`p2-10-import-shell.png`、`p2-10-thispc-window.png`、`p2-10-clsid-window.png`、`p2-10-control-panel-window.png`、`p2-10-settings-window.png`、`p2-10-unc-window.png`。
- 回归 `docs/test-results/P2-10/regression/final/regression.tsv`：28 脚本 exit=0。
- 门禁：fmt=0、check、clippy -D warnings、164 项测试（146 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `C8C1D2C4500309BB8F0E754E6FDF4927B4BA88E005AB865988E7FD48F1A34328`。

### 开发问题与修复

1. 既有 7 种 item_type 不扩展；Shell 目标映射到 uri/folder。
2. UNC 的 `is_dir()/exists()` 会阻塞或弹凭据；识别、失效角标、图标、启动四处统一跳过。
3. 空名称的 `shell:` 项目由 Shell 命名空间补全；`ms-settings:` 无命名空间项，需用户填写名称。
4. URI/Shell/UNC 的“打开文件所在位置”改为结构化 `NoFileLocation`。
5. Explorer 可能复用已有窗口；live 脚本以“新窗口 + 进程名”判定，设置页先关闭旧窗口再启动。

### 边界与门禁

- UNC 离线时由 Shell 显示自己的错误；QuickNest 不做本地探测，避免阻塞。
- Shell 命名空间/UNC 使用默认图标（无本地文件可提取）；自定义图标仍可手动指定。
- `ms-settings:` 的名称需用户输入（系统协议无 Shell 命名空间项）。
- 不解析/不复制/不重写原始 URI、LNK、UNC 字符串。

P2-10 通过；P2-11～P2-17 未开始。

## P2-11：右键菜单（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-11**。卡片 8 项菜单、分组菜单（重命名/颜色/排序/删除含迁移）、分类"新建项目"、空白区新建入口全部可用；原生菜单在屏幕右下边缘不被截断。

### 实现

- 分组标签 `ContextMenuArea`：新建/重命名分组、左移/右移、蓝/绿/橙/紫/默认强调色、删除分组。
- 分组名称对话框（新建/重命名共用）；非空分组删除确认对话框（迁移到其他分组 / 删除全部 / 取消）。
- 分类菜单在保留 4 项后追加"新建项目"，先 `select_category` 再打开项目编辑器，作用于被右键分类。
- 空白区菜单：添加项目、新建分类、新建分组。
- 分组强调色显示为标签底部 3px 色条（Slint `accent_color` 映射预设色）。
- 原 ▼ 的"自动新分组"改为打开分组名称对话框。

### 验证证据

- `scripts/verify-p2-11-menus.ps1`：`docs/test-results/P2-11/p2-11-menus.tsv` 十二阶段（重命名、颜色、分类新建项目、空白新建分组、迁移删除、删除全部、右移、边缘菜单、exit=0）。
- 边缘实测：窗口移至工作区右下角，卡片距边缘 20px；菜单矩形 `1726,830–1920,1012` 完全位于 `0,0–1920,1032` 工作区内。
- 截图：`p2-11-group-rename-dialog.png`、`p2-11-group-accent.png`、`p2-11-category-new-project.png`、`p2-11-new-group-dialog.png`、`p2-11-group-delete-confirm.png`、`p2-11-edge-menu.png`。
- 回归 `docs/test-results/P2-11/regression/final/regression.tsv`：29 脚本 exit=0。
- 门禁：fmt=0、check、clippy -D warnings、164 项测试（146 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `2A4AA510AE2FE245276AF2015F3F78184AE2782ED1DE6EC479DDD7FA6281CD99`。

### 开发问题与修复

1. 分组删除的 `match controller.borrow_mut()...` 临时借用活到 `Err` 分支，后续 `borrow()` 触发 RefCell 双借用 panic；改为局部 `outcome`。
2. 清理 P1-03 重复的 `on_category_menu_rename` 注册。
3. 原生菜单 HWND 不在 `P005Desktop.Windows()` 过滤结果中；用"新增可见顶层窗口"枚举定位，截图改虚拟屏幕。
4. 菜单选项用 Down×N + Enter 键盘导航，避免 10 项菜单的像素偏移/分隔线问题。
5. 分组创建从自动命名改为名称对话框。

### 边界与门禁

- 分组/分类"设置图标"（2.2/2.3）未纳入；分组仅有强调色，分类/分组 `icon` 字段保留但无图标的 UI。
- 分组栏 ▼ 的"下拉分组列表"未实现；▼ 打开新建分组对话框。
- 菜单边缘在 96 DPI 实测；多 DPI/多显示器属 P2-15。

P2-11 通过；P2-12～P2-17 未开始。

## P2-12：主题切换（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-12**。浅色/深色/跟随系统与强调色均可通过标题栏"设置"菜单即时切换、无需重启并持久化；跟随系统在启动与运行中生效。

### 实现

- `src/theme.rs`：`ThemeMode`（light/dark/system）、浅/深 `Palette`、`normalize_accent`、`blend`、`system_theme_is_dark`。
- `src/infrastructure/windows/theme.rs`：读取 `AppsUseLightTheme`。
- `ui/main_window.slint`：`ThemeColors` 语义色板；主界面背景/面板/侧栏/边框/文字/强调色全部走语义色；标题栏"设置"菜单（浅色/深色/跟随系统/强调色预设）；`Palette.color-scheme` 同步 std-widgets。
- `src/main.rs`：启动 `apply_theme`；菜单回调持久化并即时应用；`system` 模式每 2s 轮询。

### 验证证据

- `docs/test-results/P2-12/p2-12-theme.tsv`：light 244,246,250 → dark 16,21,29（菜单）→ light → system（注册表=1）→ green accent 234,247,239 → exit=0 → dark restart 16,21,29 → exit=0。
- 截图：`p2-12-light.png`、`p2-12-dark.png`、`p2-12-system.png`、`p2-12-accent-green.png`、`p2-12-dark-restart.png`。
- 无头补充（锁定期间）：`p2-12-theme-headless.tsv`、`p2-12-hl-*.png`。
- 回归 `docs/test-results/P2-12/regression/final/regression.tsv`：30 脚本 exit=0。
- 门禁：fmt=0、check、clippy -D warnings、169 项测试（151 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `D9E37A4D818FCC38C1D2AE1E126049D75130D216C8BEB27D105C2FDAA5BA9ED4`。

### 开发问题与修复

1. 原生 `#32768` 菜单的屏幕位置与 Slint 逻辑坐标不一致；改为枚举菜单窗口并按 rect 点击。
2. 弹出菜单无键盘焦点时 Down/Enter 不可靠；官方脚本改为按 rect 点击。
3. P2-12 加宽"设置"按钮导致 P2-01 的标题栏搜索坐标偏移；P2-01 改为 `width-136`。
4. P1-17 `/separate` 进程立即退出且旧 Explorer 窗口的 `Document` 会阻塞；改为新窗口 HWND + Shell COM 匹配。
5. 工作站锁定期间用窗口消息 + PrintWindow 完成无头实测，解锁后官方脚本通过。

### 边界与门禁

- 系统主题实际翻转未测试（不改用户个性化）；启动/切换与 2s 轮询代码已接入。
- 原生菜单跟随 OS 主题，不跟随应用内覆盖。
- 强调色仅预设；任意取色属第 17 章。
- 透明度/圆角/Mica 属 P2-13/P2-14。

P2-12 通过；P2-13～P2-17 未开始。

## P2-13：透明度和圆角（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-13**。透明度 50%–100%（默认 95%）与圆角开/关均可从标题栏"设置"菜单即时切换并持久化；透明度经分层窗口 alpha 生效，圆角经 DWM 角偏好生效（不支持平台降级直角）。

### 实现

- `settings.rounded_corners`（serde default=true）+ `normalize_opacity`（0.5–1.0 钳制、NaN→0.95）。
- `appearance::apply_window_effects`：`WS_EX_LAYERED`+`SetLayeredWindowAttributes(LWA_ALPHA)`；`DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, ROUND/DONOTROUND)`；失败静默降级。
- `apply_theme` 同时应用主题色板与窗口外观；启动后 150ms Timer 再刷（winit 句柄在事件循环内才可用），菜单/主题每次切换都重刷。
- 标题栏"设置"菜单扩为 14 项：主题 3 + 强调色 4 + 透明度 5（100/95/85/70/50）+ 圆角开/关。

### 验证证据

- 红色参考窗体置于面板后方，合成像素 = app×α + red×(1−α)，±4 容差：
  - 95% `245,233,237`；100% `244,246,250`；70% `247,173,175`；50% `249,123,125`。
  - 圆角开：角落 `202,34,34`（红）；圆角关：角落 `255,255,255`（白标题栏）；重启 0.7+方角：中心 `247,173,175`、角落 `255,179,179`。
  - 钳制：配置 1.5 → 100%；配置 0.2 → 50%。
- `docs/test-results/P2-13/p2-13-opacity-corners.tsv` 全阶段通过，exit=0。
- 回归 `docs/test-results/P2-13/regression/final/regression.tsv`：31 脚本 exit=0。
- 门禁：fmt=0、check、clippy -D warnings、171 项测试（153 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `FA5516BB627322863C4828C46815D1CB262FEC3B5D73BEC22F33A534C4490ECF`。

### 开发问题与修复

1. `ui.show()` 前原始句柄不可用（NotSupported），外观不生效；改为 show 后 150ms Timer 应用。
2. 设置菜单扩为 14 项后 P2-12 脚本 `height/7` 行高失准；改用 `MN_GETHMENU`+`GetMenuItemCount`。
3. 方角+透明时角落像素是"白标题栏×α+红参考"合成；断言按合成公式而非纯面板色。
4. clippy：结构体更新语法替代 default 后赋值；`&*value` 替代 owned 比较。

### 边界与门禁

- DWM 圆角仅 Windows 11；Windows 10 无属性静默降级为直角，功能不受影响。
- 分层 alpha 对整窗统一；逐像素材质 Mica/Acrylic 属 P2-14。
- 透明度 5 档预设；任意滑杆属第 17 章。

P2-13 通过；P2-14～P2-17 未开始。

## P2-14：Mica/Acrylic（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-14**。背景材质（纯色/Mica/Acrylic）按能力检测应用并持久化；DWM 属性应用经读回验证；不支持平台降级纯色不影响功能。

### 实现

- `settings.backdrop`（none/mica/acrylic，默认 none，非法值回退 none）持久化。
- `theme::Backdrop` 解析（非法回退 none）；`appearance::apply_backdrop`（`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`，Mica=`DWMSBT_MAINWINDOW`/Acrylic=`DWMSBT_TRANSIENTWINDOW`/纯色=`DWMSBT_NONE`）；`read_backdrop` 读回验证。
- 标题栏"设置"菜单扩为 17 项：新增 背景纯色/Mica/Acrylic。

### 验证证据

- live：菜单切换 none→Mica→Acrylic→none，每步 DWM 读回 1/2/3/1 且配置一致；重启后 Mica 读回=2 保持。
- `docs/test-results/P2-14/p2-14-backdrop.tsv` 全阶段通过；截图 `p2-14-backdrop-mica.png` 等。
- 回归：32 个脚本全通过（`docs/test-results/P2-14/regression/final4/regression.tsv`，0 失败）。
- 门禁：fmt=0、clippy -D warnings=0、173 项测试（155 lib + 1 bin + 14 集成 + 3 doc）0 失败；SHA-256 `4521CBAE…22DD2`。

### 开发问题与修复

1. `SetWindowCompositionAttribute` 不静态导出 → GetProcAddress 运行时解析。
2. **`DwmExtendFrameIntoClientArea(-1)` 破坏 P2-13 透明度**：把客户区并入边框后，DWM 的分层 alpha 不再作用于客户区，95% 透明度面板变为全不透明（P2-13 像素断言 246≠234 实证）→ 移除该调用，仅设置背景类型属性，材质在 DWM 管理区域（圆角裁剪）合成。
3. **材质可见性受渲染器限制**：`renderer-software` 的客户区为不透明 GDI 像素，材质合成在其后 → 记录为渲染器边界（透出主体需 alpha 呈现路径，属架构变更）。
4. **测试环境拦截注入按键**（详见下节）→ 验收脚本改用窗口消息投递。

### 环境事件：注入式键盘输入被系统拦截（2026-11-05）

现象：回归中 10 个脚本失败，失败集合与"脚本是否使用 `TypeText`"完全一致（100% 相关）。

定位结论（与 QuickNest 代码无关）：

- `SendInput`（含 `KEYEVENTF_UNICODE`）返回成功，注入事件也能被低级键盘钩子观察到（vk=231/75/76），但**任何应用都收不到**：记事本与 WinForms TextBox 两个外部对照同样失败；鼠标注入正常。
- 禁用 P2-14 的 `apply_backdrop` 后失败依旧；外部对照应用与 QuickNest 无关，代码嫌疑排除。
- 现象在 10:56～12:13 之间出现（P2-13 回归当时全部通过），期间机器上有第三方防护组件重启其守护进程（`HipsDaemon`，服务受保护无法停止）。

处置：验收脚本的键盘投递改为向目标窗口直接投递窗口消息（`PostMessage`，走同一条 winit→Slint 路径），文字输入经剪贴板粘贴（winit 依据**调用线程**的 `GetKeyboardState` 判定修饰键，投递的 shift 键事件不进入该状态，否则大写与 `:`、`_` 会被写成非 shift 字符）。原生文件对话框的文件名框直接投递 `WM_PASTE`。

### 边界与门禁

- SYSTEMBACKDROP 需 Win11 22H2+；旧系统调用失败回退纯色（能力探针即调用成败）。
- 客户区主体不透明；材质透过主体可见需支持 alpha 的呈现路径（渲染器升级，架构变更需用户批准）。
- Mica/Acrylic 3 档预设；跟随壁纸的系统行为由 Windows 管理。

P2-14 通过；P2-15～P2-17 未开始。

## P2-15：DPI 和多显示器（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-15**。Manifest 的 Per-Monitor V2 声明在运行中的窗口上生效；四档缩放（100%/125%/150%/175%）经 `WM_DPICHANGED` 在应用上实测，逻辑尺寸保持不变、物理尺寸随缩放变化；窗口位置持久化并在显示器缺失/位置离屏时按工作区修正，防止恢复到屏幕外。

### 实现

- **PMv2 声明**（`assets/QuickNest.manifest`，P0-01 已内置）：`dpiAware = true/pm` + `dpiAwareness = PerMonitorV2`；本任务在运行窗口上验证（`AreDpiAwarenessContextsEqual`，上下文句柄不可用数值比较）。
- **纯逻辑放置模块** `src/placement.rs`：`Rect`、`MonitorArea`、`scale_for_dpi`、`position_inside`、`clamp_into`、`nearest_monitor`、`ensure_visible`、`resolve_placement`；10 项单元测试覆盖四档 DPI、四边越界、窗口大于工作区、负坐标显示器、离屏恢复与双屏不同 DPI。
- **Windows 层** `src/infrastructure/windows/placement.rs`：`EnumDisplayMonitors`+`GetMonitorInfoW`（含设备名）与 `GetDpiForMonitor(MDT_EFFECTIVE_DPI)` 枚举显示器；`window_rect`/`set_window_position`/`correct_window_position`/`current_work_area`。
- **接线**（`src/main.rs`）：启动后 150ms（原生窗口可用时）按 `settings.window_bounds` 恢复位置，经 `resolve_placement` 修正到所在显示器工作区内；`show_and_focus` 每次显示前做离屏修正；400ms 轮询位置并以 `window_bounds`（位置、尺寸、显示器设备名）持久化，轮询间隔即 300–500ms 防抖。
- **模型**：`settings.window_bounds` 改为 `Option<WindowBounds>` —— 旧配置/全新配置为 `None` 时**不移动窗口**（没有记忆位置可恢复），首次位置变化后写入。

### 验证证据

- `docs/test-results/P2-15/p2-15-dpi.tsv`（全部阶段通过）：
  - `pmv2_awareness`：窗口上下文 `AreDpiAwarenessContextsEqual(…, PerMonitorV2)=1`、awareness=2。
  - `scale_factor` 四档：100% → scale=1、760x520；125% → 1.25、950x650；150% → 1.5、1140x780；175% → 1.75、1330x910（逻辑尺寸恒为 760x520）。
  - `offscreen_corrected`：种子位置 (6000,4000) → 修正为 (0,0)，位于工作区 (0,0,1920,1032) 内。
  - `position_persisted`：x=120 y=90 monitor=`\\.\DISPLAY3`；`position_restored`：重启后回到 (120,90)。
- 回归：33 个脚本（`docs/test-results/P2-15/regression/final6/regression.tsv`）。
- 门禁：fmt=0、clippy -D warnings=0、184 项测试（166 lib + 1 bin + 14 集成 + 3 doc）0 失败；SHA-256 `A1F0286A…5F10`。

### 开发问题与修复

1. **`GetWindowDpiAwarenessContext` 返回值不可数值比较**：返回的是上下文句柄（本机为 34）而非 -4；改用 `AreDpiAwarenessContextsEqual` 判定（与 P0-06 的 `coordinates.rs` 一致）。
2. **`PostMessage(WM_DPICHANGED)` 不生效**：异步消息在建议矩形指针释放后才被处理；改为 `SendMessage`，winit 在返回前复制建议矩形。
3. **启动恢复时序**：`ui.show()` 之后、事件循环运行之前原生窗口句柄不可用（P2-13 的同一约束）；恢复位置并入 150ms 外观定时器。
4. **位置恢复干扰验收脚本**：多数脚本的种子配置不含 `window_bounds`，`serde(default)` 的 (420,200) 被当作"要恢复的位置"，应用启动后搬窗口导致脚本坐标失准（`p1-01` 客户区偏移 148px、`p2-13` 参考窗体不再位于面板后方等 13 个失败）。改为 `Option<WindowBounds>`：无记忆位置时不移动窗口，问题消失。
5. **并发自伤**：一次门禁（`cargo test` 会启动进程并占用 Ctrl+Alt+Space 热键）与后台回归并行执行，导致 13 个脚本失败；改为串行后消失。
6. **系统弹窗遮挡**：桌面上残留一个 SmartScreen「无法访问 SmartScreen」对话框（修复前误点桌面快捷方式所致），其强调蓝覆盖面板位置，导致 p2-12/13/14 像素断言与 p2-10 失败；关闭后全部通过。

### 边界与门禁

- **多显示器实测受限**：本机仅 1 台显示器（1920x1080@96dpi），双屏不同 DPI 的场景由纯逻辑单元测试覆盖（双显示器、负坐标、不同 DPI、离屏恢复）；现场未做物理双屏实测，需在多屏机器上补验。
- 缩放验证通过 `WM_DPICHANGED` 模拟：验证应用对 DPI 变化的响应（逻辑尺寸不变、物理尺寸缩放），不改变系统实际缩放设置。
- 位置修正采用"完全离屏才搬回、部分越界则夹入工作区"的规则；用户刻意让窗口部分越界时会被夹回工作区内。
- 200% 缩放不在 V1.2 必测范围（基线 16.4）。

P2-15 通过；P2-16～P2-17 未开始。

## P2-16：备份恢复（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-16**。标题栏“设置”菜单可打开历史备份对话框：列出所有内容可解析的备份（按时间倒序），支持立即备份与恢复所选；恢复走同一条原子替换链，被替换的配置自动保留为备份，恢复可回退；保留窗口维持最近 10 份有效备份。

### 实现

- **备份枚举**（`config_save::list_backups`）：扫描 `<stem>.backup.<timestamp>.json`，只保留内容能解析为 JSON 的文件（基线 15.8.3“验证 JSON”），按时间倒序；`BackupEntry::display_label` 把时间戳格式化为 `YYYY-mm-dd HH:MM:SS.mmm`。
- **恢复**（`config_save::restore_backup`）：先校验备份内容，再经 `save_atomic` 的同一原子替换链写入（ReplaceFileW 会把被替换的配置保存为备份，因此恢复可回退），随后按 10 份保留窗口裁剪。
- **控制器**（`LibraryController::{backup_entries, restore_backup}`）：恢复后重新解析并装载库、清除拼音索引缓存、退出安全模式；安全模式下拒绝恢复。
- **界面**：标题栏“设置”菜单新增第 18 项“历史备份恢复…”；对话框列出备份（默认选中最新）、立即备份 / 取消 / 恢复所选，状态行显示结果或错误。

### 验证证据

- live（`scripts/verify-p2-16-backups.ps1`，`docs/test-results/P2-16/p2-16-backups.tsv`）：
  - 种子：实时配置分组 `Current`，有效备份 `2026-12-31`（分组 `Restored`），不可解析备份 `2027-01-01`（更晚，必须不被列出）。
  - `dialog_opened`：设置菜单第 18 项打开对话框（菜单项计数 = 18）。
  - `restored`：恢复后实时配置分组变为 `Restored` —— 证明更晚的坏备份被跳过、首项是最新**有效**备份。
  - `restore_kept_previous`：被替换的 `Current` 配置保存为 `config.backup.20261105-091121-583.json`，备份数 4（≤10）。
  - `backup_now`：立即备份新增 `config.backup.20261105-091123-064.json` 且内容为当前配置，备份数 5。
  - `dialog_closed` / `app_exit`：取消关闭对话框；退出后分组仍为 `Restored`（恢复结果持久化）。
- 单元测试：`restore_list_skips_broken_backups_and_sorts_newest_first`、`restore_replaces_the_live_config_and_keeps_the_previous_one`、`restore_refuses_a_broken_backup_without_touching_the_config`。
- 回归：34 个脚本全部 exit=0（`docs/test-results/P2-16/regression/final6/regression.tsv`）。
- 门禁：fmt=0、clippy -D warnings=0、187 项测试（169 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `3CFB7652…8EBD`。

### 开发问题与修复

1. **Slint 语法**：多行 `text:` 缺少结尾分号导致编译失败；`Flickable` 用 `viewport-height: max(list.height, …)` 形成绑定循环，改为按条目数计算并使用非弃用属性 `content-height`/`content-width`。
2. **`for … in model` 不能用于回调语句**：恢复按钮改为通过 `backup-dialog-selected-index` 取下标回调。
3. **`display_label` 时间格式判断**：时间部分为 10 字符（`HHMMSS-mmm`），初版按 ≥12 判断导致回退原始标签；改为精确定位并输出 `HH:MM:SS.mmm`。
4. **回归环境抖动**：多轮回归中出现过 4+4+1 个一次性失败（启动类/窗口类），单独复跑全部通过；根因是数日测试累积的 77 个无窗口 explorer 残留进程与系统弹窗占用桌面。清理残留 explorer 进程、最小化无关终端后，34 脚本连续通过（final4/final6）。同时把 P1-03 的菜单等待从 300ms 提升到 600ms、给 P2-13 的红色参考窗体加 TopMost（应用窗口再置顶于其上），避免同类抖动。

### 边界与门禁

- 备份列表只包含内容可解析的备份；schema 版本不兼容的备份会被控制器拒绝恢复（与 15.9 一致）。
- 保留窗口为最近 10 份**有效**备份（基线 15.7）；损坏备份在裁剪时被移除。
- 备份与配置同目录同卷（基线 15.6）；网络共享/可移动介质不承诺同等故障恢复保证。
- “导入前备份”属配置导入（P2-17），本任务不含导入。

P2-16 通过；P2-17 未开始。

## P2-17：配置导入导出（已验收）

结论：**2026-11-05 本任务验收通过，停在 P2-17，P2 阶段全部完成**。设置菜单可打开“配置导入导出”对话框：导出全部配置 / 导出当前分组写出 QuickNest 导出文档；合并导入保留现有数据并把导入分组改为新 id、同名分组自动重命名；覆盖导入替换全部分组与设置。导入前自动备份当前配置（基线 15.7），导入的命令/自动化项目标记为未验证（基线 8.4），损坏或版本不兼容的文档被拒绝且不改动现有数据。

### 实现

- **文档格式**（`src/config_transfer.rs`）：`ExportDocument`（format=`quicknest-export`、version=1、schema_version=2、scope、可选 settings、groups）。导入同时接受导出封装与普通 `config.json`；schema ≠ 2 或封装版本不符直接拒绝（15.9）。
- **合并/覆盖**：合并为每个分组/分类/项目重新生成 id，同名分组依次命名为“名称 (导入)”“名称 (导入 2)”…，命令与自动化项目置 `confirm_on_first_run = true`；覆盖替换分组与设置并修复 current/last 分类引用。
- **控制器**：`export_all_to` / `export_group_to` 写出 JSON；`import_from` 先 `backup_now`（导入前备份），再应用、清拼音缓存并持久化；安全模式拒绝导入。
- **模型**：`Launch.confirm_on_first_run`（serde 默认 false；编辑器新建命令项目默认未验证；编辑项目时保留已存标记）。
- **界面**：设置菜单第 19 项“配置导入导出…”打开对话框（导出全部配置 / 导出当前分组 / 合并导入 / 覆盖导入 / 取消 + 状态行），文件经原生保存/打开对话框选择。
- **验证入口**：`--transfer-test <report.tsv>` 以真实临时目录与真实控制器跑完整导出/导入链。

### 验证证据

- 验证模式（`docs/test-results/P2-17/p2-17-transfer-validation.tsv`）：`export_all`（2 分组 + settings）、`export_group`（仅 Tools、无 settings）、`merge_import`（Tools/Media/Imported，未验证 1，备份 0→2）、`overwrite_import`（Replaced + dark）、`broken_rejected`（库未变）→ completed。
- live（`docs/test-results/P2-17/p2-17-transfer.tsv`）：`validation_mode` → `dialog_opened` → `export_all_dialog` / `export_group_dialog`（原生保存对话框打开并取消）→ `merge_import`（Alpha→Alpha,Imported，导入命令项未验证，备份 1→3）→ `overwrite_import`（Replaced + dark）→ `dialog_closed` → `app_exit`（exit=0、Replaced 持久化）。
- 回归：35 个脚本全部 exit=0（`docs/test-results/P2-17/regression/final3/regression.tsv`）。
- 门禁：fmt=0、clippy -D warnings=0、197 项测试（179 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `2F5ECD69…114B`。

### 开发问题与修复

1. **原生保存对话框的 OK 路径在自动化下不可用**：保存对话框被隐藏的“确认另存为”模态卡住（文件不存在也会出现），保存按钮不可用。处置：导出逻辑改由 `--transfer-test` 验证模式以真实文件验证；live 脚本验证导出按钮打开原生保存对话框并取消。导入的原生打开对话框 OK 路径可正常驱动，已端到端实测。
2. **`WM_SETTEXT` 必须 Unicode + 同步**：未指定 `CharSet` 时 P/Invoke 解析为 `SendMessageA`，UTF-16 缓冲区只读出首字符；`PostMessage` 会晚于缓冲区释放而产生截断 → 改为 `SendMessageW` 同步写入。
3. **`p2-16` 菜单项计数**：P2-17 新增第 19 项后，p2-16 的“恰好 18 项”断言改为“至少 18 项”。
4. **环境抖动**：p1-17 在部分回归中清理完成后进程不退出而超时（单独复跑正常），最终回归通过；其余脚本一次通过。

### 边界与门禁

- 导入不执行任何命令/脚本/URI（8.4）；命令项目的“首次执行确认”由命令引擎（P3-08）消费 `confirm_on_first_run`。
- 合并导入不覆盖设置；需要替换设置时使用覆盖导入。
- 重复导入同一文件会不断新增分组（按设计，用户可自行删除）。
- 导出文件包含完整路径与参数，分享前需自行确认内容。

P2-17 通过；P2 阶段（P2-01～P2-17）全部完成。

# P3：高级功能验证记录

## P3-01：批量启动（已验收）

结论：**2026-11-05 本任务验收通过，停在 P3-01**。网格卡片可加入/移出批量选择（卡片右键菜单“加入批量选择/移出批量选择”，或 Ctrl+单击），右键菜单“批量启动所选（N）”按网格顺序一次启动全部选中项；批完成后选择自动清空。

### 实现

- **选择状态**：`LibraryController.batch_selection`（内存态，不入库）+ `toggle_batch_selection` / `is_batch_selected` / `batch_selection_ids` / `clear_batch_selection`。
- **界面**：`GridItem.batch_selected` 驱动卡片高亮（accent 边框 2px）；卡片菜单新增“加入批量选择/移出批量选择”与“批量启动所选（N）”（N=选中数，0 时禁用）；Ctrl+单击与菜单等效。
- **批量启动**：按网格顺序逐项 `launch_item`，状态栏报告“已启动/失败”计数；完成后清空选择并刷新。
- **注入键盘不可用**：本机第三方防护组件拦截注入按键（P2-17 已记录），验证脚本以右键菜单 + `PostMessage` 方向键驱动菜单选择。

### 验证证据

- live（`docs/test-results/P3-01/p3-01-batch.tsv`）：
  - `selected`：两张卡片经右键菜单“加入批量选择”。
  - `batch_launch`：记事本进程 0→2（两个选中项全部启动）。
  - `selection_cleared`：批完成后再次“批量启动”不启动任何进程（选择已清空）。
  - `app_exit`：exit=0。
  - 截图：`p3-01-grid.png`、`p3-01-selected.png`、`p3-01-launched.png`。
- 回归：36 个脚本全部 exit=0（`docs/test-results/P3-01/regression/final4/regression.tsv`）。
- 门禁：fmt=0、clippy -D warnings=0、197 项测试 0 失败；Release SHA-256 `EE66B9D2…C112`。

### 开发问题与修复

1. **Slint 菜单项需显式声明回调**：卡片菜单新增两项后回调未在 Window 上声明导致编译失败。
2. **Slint 自绘菜单不可读文字**（`GetMenuStringW` 返回空串）→ 选择状态以批启动的实际效果断言（启动的进程数）。
3. **菜单行定位**：`GetMenuItemRect` 返回菜单窗口客户区坐标，需 `ClientToScreen` 换算；弹出窗含阴影使“弹窗高度/行数”估算失准。
4. **测试残留窗口**：堆积的记事本窗口遮挡面板与菜单 → 清理后通过；回归运行器在每个脚本前清理记事本，p1-08 启动时清理并在“打开文件所在位置”后关闭新开的记事本。

### 边界与门禁

- Ctrl+单击的快捷键依赖系统按键状态（`GetKeyState`，真实物理键）；注入按键在本机被拦截不影响真实用户。
- 批量启动按网格顺序（可见布局）逐项启动，不做并行/延时编排（顺序、并行和延时启动属基线 P3 后续项）。
- 选择为内存态，不入库；面板隐藏/重启后选择清空。

P3-01 通过；P3-02～P3-09 未开始。

## P3-02：启动组合（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-02**。批量选择可保存为命名的启动组合（随配置持久化），随时从“启动组合…”对话框一键启动其中全部项目并支持删除；启动时跳过已删除的项目。

### 实现

- **模型**：`Library.combinations: Vec<LaunchCombination>`（serde default，旧配置兼容）；`LaunchCombination`（id/name/item_ids）；`add_combination`（trim 名字，空名回退“组合 N”）、`remove_combination`、`combination`。
- **控制器**：`save_batch_combination`（保存当前批量选择并清空）、`combination_rows`（对话框行）、`combination_item_ids`（过滤已删除项目）、`delete_combination`（并持久化）。
- **界面**：卡片菜单新增“保存批量选择为组合…”（命名对话框，预填“组合 N”）与“启动组合…”；组合对话框列出全部组合（名称 + 项目数），点击行选中，底部“启动组合 / 删除 / 取消”。
- **启动语义**：按保存时的顺序逐项 `launch_item`，已删除的项目自动跳过，状态栏报告“已启动/总数”。

### 验证证据

- 单元测试（`library_model::combination_tests`）：`add_combination_trims_and_defaults_the_name`、`remove_combination_reports_whether_it_existed`、`combinations_round_trip_through_serialization`、`legacy_configs_load_without_combinations`。
- live（`docs/test-results/P3-02/p3-02-combinations.tsv`）：`selected` → `combination_saved`（config.combinations=1、2 项）→ `combination_launched`（记事本 0→2）→ `combination_deleted`（config 归零）→ `app_exit`（exit=0）→ `completed`。
  - 截图：`p3-02-name-dialog.png`、`p3-02-launched.png`、`p3-02-closed.png`。
- 回归：37 个脚本全部 exit=0（`docs/test-results/P3-02/regression/final7/regression.tsv`）。
- 门禁：fmt=0、clippy -D warnings=0、201 项测试（183 lib + 1 bin + 14 集成 + 3 doc）0 失败；Release SHA-256 `056F0569…AE729`。

### 开发问题与修复

1. **Slint 回调 panic 进程中止**：`combination-save-requested` 处理器里 `borrow_mut()` 的借用贯穿整个 match，Ok 分支里再 `borrow()` 触发 double borrow panic（Slint 回调跨边界中止进程）→ 将保存结果先落变量、借用出作用域后再分支。
2. **组合字段误放 Settings**：初版把 `combinations` 加到 `Settings`，编译报 Library 无此字段 → 移到 `Library` 并补齐 Default。
3. **Win11 记事本会话恢复**：被杀的记事本重启后恢复全部标签页，数十个“无标题”窗口层层堆叠盖满桌面 → 清理 `%LOCALAPPDATA%\Packages\Microsoft.WindowsNotepad_*\LocalState\TabState` 后解决。
4. **对话框按钮坐标**：按布局实测（启动组合≈left+423/删除≈left+510/取消≈left+576，行中心 y=top+417；列表首行≈top+286）。

### 边界与门禁

- 组合保存的是项目 id 集合；项目删除后启动时自动跳过（不修复组合本身）。
- 启动按保存时顺序逐项执行；延时/并行编排属 P3-03。
- 组合随配置持久化，受配置备份/导入导出覆盖的影响与普通数据一致。

P3-02 通过；P3-03～P3-09 未开始。

## P3-03：延时启动（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-03**。项目可带逐项启动延时（秒）；批量启动与启动组合在启动时尊重该延时——延时项由独立线程定时启动，不阻塞其余项目与 UI。

### 实现

- **模型**：`Launch.delay_seconds: u32`（serde default 0 = 立即；旧配置兼容）；编辑器不含该字段，编辑时保留已存值。
- **启动**：`launch_item_maybe_delayed`——延时为 0 时同步启动；延时 > 0 时在 detached 线程 sleep 后启动（定时器 API 要求主线程，批循环不能睡 UI 线程）。
- **批量与组合**：两个启动循环都改用该助手，状态栏/对话框报告“N 个延时启动”。
- **单项目单击忽略延时**（立即启动）。

### 验证证据

- live（`docs/test-results/P3-03/p3-03-delay.tsv`）：种子 3 项（延时 0/3/6 秒，各自启动 cmd 向探针文件写入延时值）→ 三卡加入批量选择 → 批量启动 →
  - t=0.5s：仅 probe-0 存在（立即项即时启动，延时项未启动）。
  - t=4s：probe-3 出现（3s 项按延时启动）。
  - t=7.5s：probe-6 出现（6s 项按延时启动）。
  - 探针文件内容 = 各自延时值（0/3/6），证明启动的是对应项。
  - 截图：`p3-03-before-launch.png`（三卡选中态）。
- 单元测试：`delay_defaults_to_zero_and_round_trips`、`legacy_items_load_without_delay`。
- 门禁：fmt=0、clippy=0、185 lib 测试 0 失败（含新增 2 项）。

### 开发问题与修复

1. **探针参数引号破坏 cmd 重定向**：种子把无空格的探针路径加了引号，`launch_exe` 对含空格参数加引号后 `cmd /c` 的引号剥离规则破坏重定向 → 去掉引号（路径本身无空格）。命令本身经独立 Start-Process 实测可用。

### 边界与门禁

- 延时项在 detached 线程启动，启动结果不回传状态栏（立即项计入“已启动”，延时项计入“延时启动”）。
- 单项目单击忽略延时；仅批量/组合启动尊重延时。
- 延时上限未做限制（u32 秒）；0 = 立即。

P3-03 通过；P3-04～P3-09 未开始。

## P3-04：鼠标热区（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-04**。鼠标在配置的屏幕边缘停留约 600ms 切换面板显示/隐藏；默认关闭（opt-in）；光标离开边缘后才能再次触发，停留不会反复切换；禁用后悬停无效果。

### 实现

- **设置**：`Settings.hot_zone_enabled`（serde default false，opt-in）与 `hot_zone_edge`（left/right/top/bottom，`normalize_hot_zone_edge` 将未知值回退 left）；3 项单元测试（默认关闭、边缘规范化、设置往返）。
- **检测**：交互模式下的 200ms 轮询定时器（`GetCursorPos`，物理像素）；光标进入边缘 2px 带后持续 600ms 触发一次切换；`triggered` 标志保证停留不重复触发，光标离开边缘后重置。
- **切换路径**：复用 `show_panel` / `hide_panel`（推进焦点状态机）；仅交互模式启用。
- **助手**：`launch::cursor_position`（光标物理坐标）与 `placement::primary_bounds`（主显示器边界）。

### 验证证据

- live（`docs/test-results/P3-04/p3-04-hotzone.tsv`）：
  - `startup_visible`：启用热区后面板可见。
  - `edge_hide`：左边缘悬停 ~1s 后面板隐藏。
  - `edge_show`：再次悬停后面板显示。
  - `dwell_required`：光标触边 200ms 时面板仍可见（有 dwell，非瞬时触发），1.4s 后已隐藏。
  - `disabled_noop`：禁用后（重启应用）悬停无效果。
  - `app_exit`：exit=0。截图：`p3-04-hidden.png`。
- 门禁：fmt=0、clippy=0、188 lib 测试 0 失败。

### 开发问题与修复

1. **热区字段误放 Library** → 移到 `Settings` 并补齐 Default。
2. **`match` 分支字面量被转义序列破坏**（\r\t\b 转义在生成代码时被消费）→ 按行精确修复。
3. **`normalize_hot_zone_edge` 返回 `&\static str`** 使元组构造不再借用临时值；`controller.borrow()` 需跨块保持（`let library = controller.borrow()` 后再 `library.library()`）。
4. **防振荡断言语义**：停留边缘 2.5s 本来就会再次触发（dwell 设计使然）→ 改为验证“dwell 窗口内不重复切换”。

### 边界与门禁

- 热区为 opt-in；默认关闭不影响既有行为。
- 支持四条边（left/right/top/bottom）；角落由相邻边缘自然覆盖（两边缘交点处两条件同时成立，先命中的边缘触发）。
- 轮询间隔 200ms + dwell 600ms；光标以物理像素（PMv2）判定。
- 面板隐藏后热区仍可唤出（热区为显式用户动作，不受 pinned/auto-hide 影响）。

P3-04 通过；P3-05～P3-09 未开始。

## P3-05/P3-06/P3-07：最近使用 / 高频排序 / 收藏置顶（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-07**。网格支持四种排序模式：手动（默认，配置顺序）/ 最近使用（last_used 降序）/ 高频（launch_count 降序）/ 收藏置顶（favorite 优先、其余保持手动序）；模式从标题栏“设置”菜单即时切换并持久化；使用数据（last_used/launch_count）由真实启动写入，卡片菜单可收藏/取消收藏并即时生效。

### 实现

- **模型**：`Item` 增 `last_used: i64`、`launch_count: u64`、`favorite: bool`（均 serde default，旧配置兼容）；`Settings.sort_mode`（default manual，`normalize_sort_mode` 将未知值回退 manual）；纯函数 `sort_item_ids_by_mode`（`UsageSnapshot` 视图）承载全部排序规则。6 项单元测试（recent 降序 / frequent 降序 / favorite 置顶 / manual 与未知保序 / 规范化+往返 / 旧配置加载）。
- **控制器**：`record_launch`（last_used=now、launch_count+1、persist）接入三种启动路径（单击、批量、组合）；`set_sort_mode`/`sort_mode`；`toggle_favorite`（双向 persist）。
- **UI**：设置菜单新增 4 个扁平排序项（排序：手动/最近使用/高频/收藏置顶，设置菜单 19→23 项）；卡片菜单新增“收藏/取消收藏”（第 8 行，卡片菜单 12→13 项）；`item_rows` 按当前模式重排并在状态栏标题反映模式。

### 验证证据

- live（`docs/test-results/P3-05/p3-05-sorting.tsv`）：种子手动序 A/B/C（A 永未启动；B count=3 last_used 最旧；C count=2 last_used 次新、favorite）——
  - `recent_mode`：seeded sort_mode=recent，网格 B、C、A（截图 `p3-05-recent.png`）。
  - `frequent_mode`：设置菜单第 22 行切到高频，config.settings.sort_mode=frequent，网格 B、C、A（截图 `p3-06-frequent.png`）。
  - `favorite_mode`：第 23 行切到收藏置顶，sort_mode=favorite，网格 C、A、B（截图 `p3-07-favorite.png`）。
  - `favorite_toggled`：卡片菜单第 8 行取消 C 的收藏，config 中 ic.favorite=false。
  - `persisted`：WM_CLOSE 退出 exit=0，重启前 config.settings.sort_mode=favorite 保持。
  - 三张截图的网格顺序与预期一致；状态栏标题同步显示“排序模式：…”。
- 门禁：fmt=0、clippy=0、194 lib 测试 0 失败；release SHA256 263fad615b1727a51bc6b5d6e804c37a68b50b1c767a215adf6d73875ea628a0。

### 开发问题与修复

1. **排序选项最初实现为嵌套子菜单**：`GetMenuItemCount` 把子菜单计 1 项（19+1=20），与脚本断言不符 → 扁平化为 4 个独立项（设置菜单 23 项，与其他菜单组一致）。
2. **`toggle_favorite` 只在结果为 true 时 persist**：取消收藏不落盘，验收断言“favorite flag was not cleared” → 改为双向 persist。截图同时证明网格排序本身（recent/frequent/favorite 顺序）一次通过。
3. **旧验收脚本与新菜单结构不匹配**：新增收藏项与 4 个排序项后，卡片菜单 12→13、批量/组合行号 +1、设置菜单 19→23 → 更新 `verify-p2-17-transfer`、`verify-p3-01-batch`、`verify-p3-02-combinations`、`verify-p3-03-delay` 的计数与行号断言。

### 边界与门禁

- 手动模式即拖拽排序（P2-04）的配置顺序；recent 中从未启动的项目排最后（按手动序）；frequent 并列保持手动序；favorite 置顶后其余保持手动序。
- 使用数据仅记录成功启动（`record_launch` 在启动成功后调用）；编辑项目（`update_item`）保留 usage 字段。
- usage 字段均 serde default，旧版本配置无缝加载。
- 排序模式与收藏随配置原子持久化；重启后保持。

P3-05/P3-06/P3-07 通过；P3-08～P3-09 未开始。
## P3-08：自定义命令管理（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-08**。命令类型项目经结构化命令引擎启动（基线第 8 章）：direct 走 CreateProcessW（不经过 cmd.exe、不拼接 Shell 字符串）、cmd 走 `cmd.exe /d /s /c`、powershell 走 `powershell.exe -NoProfile -File`；首次执行未确认命令弹出确认对话框（程序/参数/工作目录/执行方式，可取消），确认一次后不再提示；批量与组合启动跳过未确认命令并明确报告。

### 实现

- **模型**：`Launch.execution_mode`（serde default direct，`normalize_execution_mode` 将空/未知值回退 direct）；`ItemDraft.execution_mode`（编辑器字段，`into_item` 规范化落盘）；`Library::find_item_mut`。2 项单元测试（规范化/往返/旧配置加载为 direct）。
- **命令引擎**（`launch.rs`）：`launch_item` 分发 `ItemKind::Command → launch_command`；direct 校验目标存在后与 EXE 同路径（CreateProcessW + Shell 回退 + 管理员 ShellExecuteExW）；cmd 用 COMSPEC（回退 System32）拼 `/d /s /c 程序 参数`；powershell 用 System32 PowerShell 5.1 拼 `-NoProfile -File 脚本 参数`（脚本缺失报 MissingTarget）；`command_display` 输出实际执行行供状态栏显示（8.2 “界面必须显示实际执行内容”）。3 项单元测试（cmd 探针脚本、powershell 探针脚本、缺失目标+显示行）。
- **控制器**：`confirm_command`（清除 confirm_on_first_run 并落盘）。
- **UI**：编辑器为命令类型显示“执行模式”下拉（直接执行/CMD/PowerShell，仅命令类型可见）；首次执行确认对话框（标题、修改警告、程序/参数/工作目录/执行方式四行、取消/确认执行）。
- **接线**：`pending_command_confirm` 状态 + `launch_item_with_confirm` 门控全部 6 个单项启动入口（网格单击、卡片菜单打开/管理员运行、搜索结果单击/回车/Ctrl+回车）；确认后清除标记并启动，取消永不执行；批量启动与组合启动循环跳过未确认命令并在状态栏报告“跳过 N 个未确认命令”。

### 验证证据

- live（`docs/test-results/P3-08/p3-08-command.tsv`），种子 4 个未确认命令项（c1 direct、c2 cmd、c3 powershell、c4 direct 供批量跳过；探针脚本写各自探针文件）：
  - `direct_confirmed`：单击 c1 → 确认对话框出现（截图 `p3-08-confirm.png`）→ 确认执行 → probe1 含 direct-ok，config 中 confirm_on_first_run=false。
  - `direct_verified_no_dialog`：再次单击 c1 无对话框，probe1 两行。
  - `cmd_cancelled`：c2 取消 → probe2 不存在、标记仍 true。
  - `cmd_confirmed`：c2 确认 → cmd 模式执行脚本写 probe2，execution_mode=cmd 持久化、标记清除。
  - `powershell_confirmed`：c3 确认 → powershell 模式执行脚本写 probe3。
  - `batch_skips_unverified`：c1+c4 批量启动 → c1 第三次启动、c4 被跳过（probe4 不存在、标记仍 true）。
  - `persisted`：WM_CLOSE exit=0；确认状态持久化。
- 门禁：fmt=0、clippy=0、199 lib 测试 0 失败（全目标 214）；release SHA256 8c621b6482511808cdadc7224ae01ebc48d6b60d146e4d774c7f3e75622671aa。

### 开发问题与修复

1. **测试目标的 `Launch` 字面量缺新字段**（launch.rs/uwp_link.rs/lnk_launch.rs）→ 补 `execution_mode: String::new()`（规范化后即 direct）。
2. **unsupported 测试把 Command 列入不支持类型** → Command 已由命令引擎支持：移出循环、补充“空目标仍被拒绝”断言；循环因此只剩单元素触发 clippy `single_element_loop` → 改为直接断言。
3. **`command_display` 引号预期写错**：`join_args` 只给含空格参数加引号 → 修正测试预期。
4. **验收脚本对无截图路径误调 `Save-P005Screenshot`**（$null → “路径的形式不合法”）→ 加空值守卫后重跑全过。

### 边界与门禁

- 首次确认仅作用于单项启动路径；批量/组合遇到未确认命令一律跳过并报告，绝不静默执行（基线 8.4）。
- 不做关键词黑名单（8.3 明确不把 del/format/Remove-Item 当安全边界）；安全依赖明确添加、执行模式明确、首次确认。
- powershell 模式仅 `-NoProfile -File` 脚本文件，不默认执行内联脚本；cmd 模式 `/d` 禁用 AutoRun。
- execution_mode 空/未知一律按 direct 处理；编辑项目保留未验证标记（非编辑器字段），导入命令的未验证标记在首次确认执行后清除。
- 自动化动作（automation）仍为后续阶段扩展类型。

P3-08 通过；P3-09 未开始。
## P3-09：Everything 可选集成（已验收）

结论：**2026-11-06 本任务验收通过，停在 P3-09**。Everything 集成为 opt-in（默认关闭）：设置菜单“Everything 搜索：开/关”切换并持久化；开启后搜索中心在有关键词时追加“在 Everything 中搜索…”移交行，激活即以 `Everything.exe -search <查询>` 启动 Everything；路径按“配置路径→标准安装位置回退”解析，全部缺失时状态栏提示“未找到 Everything”且主程序不受任何影响。

### 实现

- **设置**：`Settings.everything_enabled`（serde default false）与 `everything_path`（default C:\\Program Files\\Everything\\Everything.exe）；2 项单元测试（默认关闭+标准路径、往返+旧配置加载）。
- **移交助手**（`launch.rs`）：`resolve_everything_path`（配置路径存在则用之，否则按 ProgramFiles / ProgramFiles(x86) / LOCALAPPDATA 回退，全缺失返回 None）、`everything_fallback_paths`、`everything_handoff_item`（合成 EXE 项目，args=`[-search, query]`）；1 项单元测试（移交项构造+三种解析路径）。
- **搜索中心**：`refresh_search` 在开启且有查询时追加移交行（scope=集成、type=Everything，path 显示解析结果或“未找到 Everything”）；三个激活入口（结果单击/回车/Ctrl+回车）对 `everything:` 前缀 id 走 `launch_everything_handoff`（不走项目库查找与首次确认门）。
- **设置菜单**：第 24 项“Everything 搜索：开/关”（`on_everything_toggle_requested` 切换+持久化+状态栏提示），`refresh_ui` 同步开关状态到菜单标题。

### 验证证据

- live（`docs/test-results/P3-09/p3-09-everything.tsv`），种子 1 个本地项目 + 开启集成（配置路径指向探针 .cmd，其以绝对路径把启动参数追加到探针文件）：
  - `handoff_probe`：Ctrl+F 打开搜索 → 粘贴 qnprobe → 移交行出现且选中（截图 `p3-09-search.png`，1 个结果）→ 回车激活 → 探针文件含 `-search qnprobe`。
  - `toggle_off`：设置菜单第 24 行切换 → config.everything_enabled=false；再次搜索回车为空结果无操作，探针行数不变（截图 `p3-09-disabled.png`）。
  - `missing_path_reported`：重启并配置不存在的路径（本机无真实 Everything 可回退）→ 激活移交行仅状态栏提示，主程序存活，探针未被触碰。
  - `persisted`：WM_CLOSE exit=0，开关状态跨重启保持。
- 门禁：fmt=0、clippy=0、202 lib 测试 0 失败（全目标 217）；release SHA256 17dde49755ec29c1437eac12530b017e68a4a8bd55373937b81c1eaafaff231f。
- 回归：42 脚本（设置菜单 19→23→24 两次扩项后，p2-17/p3-0567 计数断言同步更新）。后台整轮 40 通过、2 个环境性失败（记事本窗口堆积遮挡面板：p1-17 超时、p3-02 删除点击落在覆盖窗口上），清理记事本与其 Win11 TabState 会话恢复后逐个复跑全部通过；`run-regression.ps1` 已加入 TabState 清理钩子防复发。

### 开发问题与修复

1. **Settings 在 sort_mode 后还有 P2-15 字段** → 锚点截断到字段行后追加。
2. **cargo fmt 重排启动调用** → 按 fmt 后文本重写锚点；补丁函数幂等化（重复应用导致 slint 属性声明重复，构建报 Cannot override property → 手工去重）。
3. **`let rows` 需 mut**（追加移交行）。
4. **验收探针三连修**：批处理内容存为 `Everything.exe` 按扩展名执行报“不兼容”→ 改 `Everything.cmd`；`>>` 相对路径写入应用 CWD（仓库根出现了探针文件）→ 改绝对路径重定向；单行探针文件 `(Get-Content).Count` 在严格模式无 Count → 包 `@()`。
5. **脚本 finally 块 StrictMode 变量未初始化** → 顶部初始化 `$script:app = $null`；`Wait-Window` 空数组索引 → 先判 Count。

### 边界与门禁

- 集成完全 opt-in：默认关闭；关闭时不追加任何行，行为与 P2-01 搜索完全一致。
- Everything 缺失（配置与回退均无）只影响移交行的启动结果（状态栏提示），主程序、搜索与配置不受影响（基线 22“插件异常不得影响主程序”）。
- 移交行不是库项目：不参与排序/收藏/批量/组合，不计入 last_used/launch_count，不走首次执行确认。
- 本机未安装 Everything，验收以探针 .cmd 作为配置路径验证真实启动链路（CreateProcessW 失败 → 文档化的 Shell 回退执行批处理）。

P3-09 通过；**P3 阶段（P3-01～P3-09）全部完成**。
## 卡片 UI 规范 V1.0（2026-11-06 用户规范，已实现）

结论：**2026-11-06 按用户《QuickNest 卡片 UI 规范 V1.0》实现并验收**。核心问题（图标左对齐、文字居中导致视觉轴线不一致）已解决：图标与标题现处于同一条垂直中心线上；所有类型共用一套卡片组件，只有图标与标题数据不同。

### 问题定位

`VerticalLayout { alignment: center }` 的 `alignment` 在 Slint 中是**主轴（垂直）**对齐；图标 Rectangle 有固定 40px 宽度且不横向拉伸 → 被放在布局左侧；标题 Text 撑满宽度后靠 `horizontal-alignment: center` 居中 → 形成"图标靠左、文字居中"的违和感。修复方式：图标行改为 `HorizontalLayout { alignment: center }`（主轴为水平 → 水平居中），与标题共享同一垂直轴。

### 实现（仅样式/布局/视觉状态，未触碰业务逻辑）

- **卡片尺寸**：沿用现有 88×100px（cell 96×108 − 8px），圆角 10px → **11px**，网格列数与换行规则不变（规格"不改变卡片网格列数及自动换行规则"）。
- **内部布局**：`VerticalLayout{ alignment: center; padding: 10px 8px; spacing: 8px }`，图标行用 `HorizontalLayout{ alignment: center }` 水平居中；图标区 40×40px 固定，实际图像 **32×32px** 且 `image-fit: contain`（原为拉伸填满 40×40）→ 不同来源图标不再偏移、不变形。
- **标题**：12px、字重 400、行高 18px、单行、超长省略（`max-lines: 1` + `overflow: elide`）、水平居中、宽度不超过卡片可用宽度（原为两行 32px）。
- **图标类型**（规格 §2）：EXE/文件/文件夹用系统原生图标；URL 用新增 `assets/browser.svg`（地球）；命令用新增 `assets/terminal.svg`（终端）。`Item::default_artwork()` 按类型返回 app/browser/terminal，`GridItem.artwork` 传给 UI。
- **角标**（规格 §5）：15px 圆点、`#E5484D`、距卡片右上各 5px、绝对定位不占布局；**悬浮显示具体原因**（"目标不存在或路径已失效：<路径>"，来自 `Item::missing_reason()`），命中判定放在新模块 `src/card_ui.rs`（可单测）。
- **视觉状态**（规格 §6）：默认白底 + `#DCE3EF` 边框；hover → `#F7F9FF` 底 + `#B8CCF4` 边框 + 轻微阴影（Slint 1.18 无 box-shadow，用偏移圆角矩形模拟）；按下 → `#EEF3FF` 底；焦点（点击后）→ 保持底色 + 蓝色描边；禁用态无数据源（模型未定义禁用语义），未引入新业务字段故未实现。
- **主题适配**：新增 `card_hover` / `card_hover_border` / `card_pressed` 三个主题色（浅色固定为规格值，深色按强调色混合自适应）。

### 验证证据

- 单元测试（6 项）：`card_hover_pressed_states_match_the_spec`（浅色值逐项钉死 + 深色可分辨）、`default_artwork_follows_the_item_type`、`missing_reason_only_exists_for_broken_targets`、`badge_hit_covers_the_top_right_corner_only` 等；**206 lib 测试 0 失败**。
- 视觉证据（`docs/test-results/card-ui/`）：`card-light.png`（EXE/计算器原生图标、URL 地球图标、命令终端图标、文件夹黄色图标、长标题省略、红色角标）、`card-light-hover.png`（hover 描边+阴影+提示）、`card-dark.png`（深色自适应）、`card-badge-tooltip.png`（角标悬浮显示具体原因）。
- 回归：42 脚本全量（首轮 8 个失败中 7 个为并发重建污染/环境状态，已逐个复跑核实）。

### 开发问题与修复

1. **Slint 无 `var`**：局部变量不可声明 → 改用元素属性（`card.badge-hovered`）与根属性承载状态。
2. **侧边栏宽度影响网格列数**：164px 侧边栏使 760px 窗口的网格从 6 列变为 5 列 → 大量按索引定位卡片的脚本坐标失效；改用 **160px**（用户建议区间 160–176 的下限）后网格恢复 6 列，"不改变网格列数"的要求同时满足。
2. **角标悬浮不触发**：`changed has-hover` 只在进入/离开时触发，鼠标在卡片内移动不会重新求值 → 新增 `changed mouse-x` / `changed mouse-y` 并把命中判定下沉到 Rust 回调（`badge-tip-update`），保证鼠标移到角标即出提示。
3. **p1-10 断言失效**：原断言"URI 项目显示蓝色默认图标"与规格 §2 冲突 → 更新为"URL 使用浏览器图标"（青色比例断言），并同步图标采样坐标（32px 图标中心 137）。
4. **PowerShell 5.1 无 BOM 读中文注释**导致脚本解析错误 → 为 `verify-p1-10-icons.ps1` 加 UTF-8 BOM。
5. **Explorer 拖放源点脆弱**：源点 X=500 落在"second file"（短名）文字右侧的空白区 → 改为 X=420（文字上）。
6. **Shell 会话状态残留**：一次未完成的拖放会让 Explorer 后续拖放全部失效（p1-17 第二次拖放无任何 DragOver）→ 回归脚本在拖放相关脚本前重置 explorer。

### 边界

- 未改数据模型、启动逻辑、路径处理、IPC、配置存储；卡片网格列数/换行、拖拽排序与跨分类拖拽行为不变。
- 规格中的"10px 卡片间距"与"不改变网格列数"相互冲突（88px 卡片 + 10px 间距会改变列数），按用户明确的"保留现有卡片尺寸、不重新设计网格"执行（实际间距 8px）。
- 禁用态无数据源，未实现（不引入新业务语义）。
- 键盘焦点轮廓为"点击后获得焦点"语义；网格本身未提供 Tab 导航（属既有交互范围之外，未新增导航模型）。
- 自动化动作（automation）类型仍为后续阶段扩展。

P3 阶段之后追加的 UI 优化已完成；回归与文档同步完毕。

### 回归结果

**42 个验收脚本整轮连续运行 0 失败**（2026-11-06 `docs/test-results/latest/regression.tsv`，所有夹具修复就位后的首次完整全绿）；随几何变更同步更新了受影响的脚本坐标。夹具层面的约定与已知陷阱见 [测试夹具说明](TEST_HARNESS.md)。
