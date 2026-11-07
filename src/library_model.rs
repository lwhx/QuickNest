//! Quick library data model (baseline chapters 2/3/14).
//!
//! Three layers: group (一级分组) → category (二级分类) → item (项目).
//! Baseline 14.1/14.2 fixes the persisted shape: every entity is a UUID v4,
//! the active group lives in `current_group_id`, groups carry `color`, and
//! items store `item_type` plus a nested `launch` block. The model is pure
//! data plus the operations the UI drives; persistence goes through the
//! atomic save chain (P0-13 `config_save`).

use serde::{Deserialize, Serialize};

/// Item kinds from baseline 3.1 (serialized as `item_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Exe,
    Lnk,
    File,
    Folder,
    Uri,
    Command,
    Automation,
}

impl ItemKind {
    /// Stable order for the editor's type dropdown.
    pub const ALL: [ItemKind; 7] = [
        Self::Exe,
        Self::Lnk,
        Self::File,
        Self::Folder,
        Self::Uri,
        Self::Command,
        Self::Automation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Exe => "Windows 程序",
            Self::Lnk => "Windows 快捷方式",
            Self::File => "普通文件",
            Self::Folder => "文件夹",
            Self::Uri => "网址/协议",
            Self::Command => "自定义命令",
            Self::Automation => "自动化动作",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|kind| *kind == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Exe)
    }

    /// Infers the kind from a target path (baseline 3.1/3.2): a URI when it
    /// has a scheme, scripts become commands, existing directories become
    /// folders, `.lnk`/`.exe` map to their kinds, everything else is a file.
    /// P2-10: `::{CLSID}` Shell namespace targets and UNC shares are folder-
    /// like; they must be classified without probing the network/filesystem.
    pub fn from_path(path: &str) -> Self {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Self::File;
        }
        if is_uri_target(trimmed) {
            return Self::Uri;
        }
        if trimmed.starts_with("::") || is_unc_path(trimmed) {
            return Self::Folder;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.ends_with(".lnk") {
            return Self::Lnk;
        }
        if lower.ends_with(".exe") {
            return Self::Exe;
        }
        if lower.ends_with(".bat") || lower.ends_with(".cmd") || lower.ends_with(".ps1") {
            return Self::Command;
        }
        if std::path::Path::new(trimmed).is_dir() {
            return Self::Folder;
        }
        Self::File
    }
}

/// Whether the value is a URI with an RFC 3986 scheme (same rule the launcher
/// uses). Single-letter schemes are rejected so `C:\x` is not a URI.
pub fn is_uri_target(value: &str) -> bool {
    let trimmed = value.trim();
    let Some(colon) = trimmed.find(':') else {
        return false;
    };
    let scheme = &trimmed[..colon];
    if scheme.len() < 2 {
        return false;
    }
    let mut characters = scheme.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// P2-10 (baseline 4.4): Shell namespace targets. `::{CLSID}` parsing names,
/// `shell:` monikers and `ms-settings:` URIs are resolved and launched by the
/// Windows Shell; they have no local filesystem path to check.
pub fn is_shell_namespace_target(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with("::") {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("shell:") || lower.starts_with("ms-settings:")
}

/// P2-10 (baseline 3.1/4.4): UNC shares (`\\server\share`) are valid folder
/// targets but must never be probed with a local exists/is_dir check (that can
/// block or prompt while the share is offline).
pub fn is_unc_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("\\\\") && trimmed.len() > 2
}

/// Window state shown when the item is launched (baseline 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
}

/// The nested launch block of an item (baseline 14.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Launch {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub workdir: String,
    /// P3-08 (baseline 8.1/8.2): how the command engine runs a command item —
    /// direct (structured CreateProcessW, no cmd.exe), cmd (`cmd.exe /d /s /c`)
    /// or powershell (`powershell.exe -NoProfile -File`). Missing or unknown
    /// values load as direct.
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    #[serde(default)]
    pub run_as_admin: bool,
    /// P2-07: the AppUserModel ID read from a UWP shortcut's target Shell
    /// item; empty for everything else. Saved (baseline 4.2.3) so P2-08 can
    /// activate directly instead of guessing from display names.
    #[serde(default)]
    pub aumid: String,
    /// P2-09: the shortcut is an MSI advertised shortcut (Darwin ID). The
    /// original LNK is still the launch target; this only drives the
    /// "repair install or re-add" guidance when the shortcut breaks
    /// (baseline 4.3).
    #[serde(default)]
    pub msi_advertised: bool,
    /// P2-17 (baseline 8.1/8.4): a command/automation item that has not been
    /// confirmed by the user. Items imported from a file are marked
    /// unverified; the command engine (P3-08) prompts once before running.
    #[serde(default)]
    pub confirm_on_first_run: bool,
    /// P3-03 (baseline 22 延时启动): seconds to wait before this item starts
    /// during a batch or combination launch. Zero means immediate. A single
    /// item click ignores the delay.
    #[serde(default)]
    pub delay_seconds: u32,
}

/// A shortcut item (baseline 3.4 fields in the 14.2 shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub item_type: ItemKind,
    pub launch: Launch,
    /// Custom icon path; empty means the system default.
    #[serde(default)]
    pub icon: String,
    pub window_state: WindowState,
    /// Free-form note.
    #[serde(default)]
    pub note: String,
    /// P3-05 (baseline 22 最近使用): unix milliseconds of the last successful
    /// launch; 0 = never launched.
    #[serde(default)]
    pub last_used: i64,
    /// P3-06 (baseline 22 高频排序): how many times the item was launched.
    #[serde(default)]
    pub launch_count: u64,
    /// P3-07 (baseline 22 收藏置顶): favorites sort to the top of the grid.
    #[serde(default)]
    pub favorite: bool,
}

impl Item {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ItemKind,
        target: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            item_type: kind,
            launch: Launch {
                path: target.into(),
                ..Launch::default()
            },
            icon: String::new(),
            window_state: WindowState::default(),
            note: String::new(),
            last_used: 0,
            launch_count: 0,
            favorite: false,
        }
    }

    /// Whether the launch target is missing on disk (baseline 2.4: the grid
    /// shows a "path invalid" badge). Only file-like kinds are checked:
    /// URI/command/automation targets are not file paths, and an empty path is
    /// not treated as broken (the item may be a future automation action).
    pub fn path_missing(&self) -> bool {
        if matches!(
            self.item_type,
            ItemKind::Uri | ItemKind::Command | ItemKind::Automation
        ) {
            return false;
        }
        let path = self.launch.path.trim();
        // P2-10 (baseline 4.4): Shell namespace entries and UNC shares have no
        // local file to test; probing a network share can block or prompt.
        if is_shell_namespace_target(path) || is_unc_path(path) {
            return false;
        }
        // 便携版：相对路径按便携根解析后再探测存在性。
        !path.is_empty() && !crate::portable::resolve_path(path).exists()
    }

    /// 便携版（基线 23 章）：返回相对目标按便携根解析后的副本（绝对/空
    /// 目标原样）。启动、位置打开、缺失校验共用这一个入口。
    ///
    /// 只解析文件系统路径：URI（https://…）、Shell namespace（::{CLSID}、
    /// shell:…）和 UNC 共享不是便携根下的相对文件，解析它们会把合法目标
    /// 拼坏（2026-11-07 审核修复）。
    pub fn with_resolved_target(&self) -> Item {
        let mut resolved = self.clone();
        let path = self.launch.path.trim();
        let filesystem_target = !path.is_empty()
            && !is_uri_target(path)
            && !is_shell_namespace_target(path)
            && !is_unc_path(path);
        if filesystem_target {
            resolved.launch.path = crate::portable::resolve_path(&self.launch.path)
                .to_string_lossy()
                .into_owned();
        }
        let workdir = self.launch.workdir.trim();
        if !workdir.is_empty()
            && !is_uri_target(workdir)
            && !is_shell_namespace_target(workdir)
            && !is_unc_path(workdir)
        {
            resolved.launch.workdir = crate::portable::resolve_path(&self.launch.workdir)
                .to_string_lossy()
                .into_owned();
        }
        resolved
    }

    /// 卡片规范 V1.0 §2: the default artwork used when the item has no
    /// native icon — URLs get the browser artwork, commands the terminal
    /// artwork, everything else the application artwork.
    pub fn default_artwork(&self) -> &'static str {
        match self.item_type {
            ItemKind::Uri => "browser",
            ItemKind::Command => "terminal",
            _ => "app",
        }
    }

    /// 卡片规范 V1.0 §5: the reason shown when the user hovers the red
    /// "!" badge — the concrete cause, not just the flag.
    pub fn missing_reason(&self) -> String {
        if !self.path_missing() {
            return String::new();
        }
        let path = self.launch.path.trim();
        format!("目标不存在或路径已失效：{path}")
    }

    /// The default working directory when adding (baseline 3.3): EXE and
    /// plain files record their parent directory; shortcuts keep their own
    /// working directory (empty stays empty so the LNK keeps its semantics);
    /// other kinds start empty.
    pub fn default_workdir(kind: ItemKind, path: &str) -> String {
        match kind {
            ItemKind::Exe | ItemKind::File => std::path::Path::new(path.trim())
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// The file whose icon represents this item (P1-10). A custom icon wins;
    /// its `path,index` metadata keeps only the path (the index is applied by
    /// later icon work). URI/command/automation items have no file icon.
    pub fn icon_source(&self) -> Option<String> {
        let custom = self.icon.trim();
        if !custom.is_empty() {
            let path = custom.split(',').next().unwrap_or(custom).trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
        if matches!(
            self.item_type,
            ItemKind::Uri | ItemKind::Command | ItemKind::Automation
        ) {
            return None;
        }
        let target = self.launch.path.trim();
        // P2-10: Shell namespace entries and UNC shares have no local file
        // whose icon could be extracted; the grid keeps the default icon.
        if is_shell_namespace_target(target) || is_unc_path(target) {
            return None;
        }
        (!target.is_empty()).then(|| target.to_string())
    }
}

/// Quotes a Windows CRT argument (not argv[0], CMD source or PowerShell source).
/// A run of n backslashes before a quote becomes 2n+1 backslashes; a trailing
/// run becomes 2n before the closing quote. Unicode is copied unchanged.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '"')) {
        return arg.to_string();
    }
    quote_arg_always(arg)
}

/// Forced quoting is also used for native arguments inside CMD's command tail:
/// shell metacharacters must stay inside quotes even when there are no spaces.
pub(crate) fn quote_arg_always(arg: &str) -> String {
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for ch in arg.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            quoted.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
        } else {
            quoted.extend(std::iter::repeat_n('\\', slashes));
        }
        slashes = 0;
        quoted.push(ch);
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    quoted
}

/// Serializes an argument array for the editor and native Windows launches.
/// `parse_args(&join_args(args)) == args`, including empty arguments.
pub fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses argument text using Windows CRT rules. Only space and tab separate
/// arguments; other Unicode whitespace is literal. Backslashes only escape
/// quotes, paired quotes inside a quoted token produce a literal quote, and
/// an unmatched opening quote consumes the remainder (the CRT's behavior).
/// This parses arguments, not the special argv[0] field or shell source.
pub fn parse_args(text: &str) -> Vec<String> {
    let mut chars = text.chars().peekable();
    let mut args = Vec::new();
    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| matches!(c, ' ' | '\t')) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let mut current = String::new();
        let mut in_quotes = false;
        loop {
            let mut slashes = 0;
            while chars.peek() == Some(&'\\') {
                chars.next();
                slashes += 1;
            }
            if chars.peek() == Some(&'"') {
                chars.next();
                current.extend(std::iter::repeat_n('\\', slashes / 2));
                if slashes % 2 == 1 {
                    current.push('"');
                } else if in_quotes && chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
            } else {
                current.extend(std::iter::repeat_n('\\', slashes));
                match chars.peek() {
                    None => break,
                    Some(' ' | '\t') if !in_quotes => break,
                    _ => current.push(chars.next().expect("peeked character")),
                }
            }
        }
        args.push(current);
    }
    args
}

/// The editable fields of an item (baseline 3.4). The add and edit dialogs
/// share this shape; `args` is the raw argument line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDraft {
    pub name: String,
    pub item_type: ItemKind,
    pub path: String,
    pub args: String,
    pub workdir: String,
    pub icon: String,
    pub window_state: WindowState,
    pub run_as_admin: bool,
    pub note: String,
    /// P2-07: AUMID read from a UWP shortcut; set programmatically at import.
    pub aumid: String,
    /// P2-09: set programmatically when the picked LNK is an MSI advertised
    /// shortcut (baseline 4.3). Not shown in the editor.
    pub msi_advertised: bool,
    /// P2-17 (baseline 8.1/8.3): command items start unverified; the command
    /// engine asks once before running them. Not shown in the editor.
    pub confirm_on_first_run: bool,
    /// P3-03 (baseline 22 延时启动): launch delay in seconds (batch and
    /// combination launches only). Not shown in the editor.
    pub delay_seconds: u32,
    /// P3-08 (baseline 8.1/8.2): the command execution mode (editor field for
    /// command items).
    pub execution_mode: String,
    /// P3-05/P3-06/P3-07: usage state; preserved on edit, not editor fields.
    pub last_used: i64,
    pub launch_count: u64,
    pub favorite: bool,
}

impl ItemDraft {
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            item_type: ItemKind::Exe,
            path: String::new(),
            args: String::new(),
            workdir: String::new(),
            icon: String::new(),
            window_state: WindowState::Normal,
            run_as_admin: false,
            note: String::new(),
            aumid: String::new(),
            msi_advertised: false,
            confirm_on_first_run: true,
            delay_seconds: 0,
            execution_mode: default_execution_mode(),
            last_used: 0,
            launch_count: 0,
            favorite: false,
        }
    }

    pub fn from_item(item: &Item) -> Self {
        Self {
            name: item.name.clone(),
            item_type: item.item_type,
            path: item.launch.path.clone(),
            // Shared Windows argument grammar preserves every argument on edit.
            args: join_args(&item.launch.args),
            workdir: item.launch.workdir.clone(),
            icon: item.icon.clone(),
            window_state: item.window_state,
            run_as_admin: item.launch.run_as_admin,
            note: item.note.clone(),
            aumid: item.launch.aumid.clone(),
            msi_advertised: item.launch.msi_advertised,
            confirm_on_first_run: item.launch.confirm_on_first_run,
            delay_seconds: item.launch.delay_seconds,
            execution_mode: item.launch.execution_mode.clone(),
            last_used: item.last_used,
            launch_count: item.launch_count,
            favorite: item.favorite,
        }
    }

    /// Builds a draft for a path dropped from Explorer (baseline 3.2): the
    /// name uses the file stem, the type is inferred from the path, and EXE/
    /// file targets get their parent folder as the default working directory.
    /// The source is only referenced, never copied or moved.
    pub fn from_path(path: &std::path::Path) -> Self {
        let target = path.to_string_lossy().into_owned();
        let item_type = ItemKind::from_path(&target);
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or_else(|| target.clone());
        Self {
            name,
            item_type,
            path: target.clone(),
            workdir: Item::default_workdir(item_type, &target),
            ..Self::empty()
        }
    }

    /// Validates the user input; returns a user-facing reason.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty() {
            return Err("名称不能为空");
        }
        if self.path.trim().is_empty() && self.item_type != ItemKind::Automation {
            return Err("目标不能为空");
        }
        Ok(())
    }

    /// Builds the stored item, trimming fields and parsing the argument line.
    pub fn into_item(self, id: impl Into<String>) -> Item {
        Item {
            id: id.into(),
            name: self.name.trim().to_string(),
            item_type: self.item_type,
            launch: Launch {
                path: self.path.trim().to_string(),
                args: parse_args(&self.args),
                workdir: self.workdir.trim().to_string(),
                run_as_admin: self.run_as_admin,
                aumid: self.aumid.trim().to_string(),
                msi_advertised: self.msi_advertised,
                confirm_on_first_run: self.confirm_on_first_run,
                delay_seconds: self.delay_seconds,
                execution_mode: normalize_execution_mode(&self.execution_mode).to_string(),
            },
            icon: self.icon.trim().to_string(),
            window_state: self.window_state,
            note: self.note.trim().to_string(),
            last_used: self.last_used,
            launch_count: self.launch_count,
            favorite: self.favorite,
        }
    }
}

/// A second-level category (baseline 2.3); belongs to exactly one group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub items: Vec<Item>,
}

impl Category {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: String::new(),
            items: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// A first-level group (baseline 2.2). Owns its categories and remembers the
/// category it last showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    /// Optional accent color, `#rrggbb` (baseline 14.2 `color`).
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub categories: Vec<Category>,
    /// The category shown when this group is selected (baseline 2.2).
    pub last_category_id: Option<String>,
}

impl Group {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: String::new(),
            color: String::new(),
            categories: Vec::new(),
            last_category_id: None,
        }
    }

    /// Whether the group holds any content. Baseline 2.2 requires a
    /// confirmation before deleting a non-empty group; "content" covers the
    /// categories themselves (deleting the group would drop their names),
    /// not only the items inside them.
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    pub fn item_count(&self) -> usize {
        self.categories
            .iter()
            .map(|category| category.items.len())
            .sum()
    }

    pub fn category(&self, category_id: &str) -> Option<&Category> {
        self.categories
            .iter()
            .find(|category| category.id == category_id)
    }

    pub fn category_mut(&mut self, category_id: &str) -> Option<&mut Category> {
        self.categories
            .iter_mut()
            .find(|category| category.id == category_id)
    }
}

/// P3-02 (baseline 22 启动组合): a named, persisted set of item ids that can
/// be launched together. Items that no longer exist are skipped at launch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaunchCombination {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub item_ids: Vec<String>,
}

impl LaunchCombination {
    pub fn new(name: &str, item_ids: Vec<String>) -> Self {
        Self {
            id: new_uuid(),
            name: name.trim().to_string(),
            item_ids,
        }
    }
}

/// Window bounds persisted with the settings (baseline 14.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub monitor_id: String,
}

impl Default for WindowBounds {
    fn default() -> Self {
        Self {
            x: 420,
            y: 200,
            width: 860,
            height: 580,
            monitor_id: "DISPLAY1".to_string(),
        }
    }
}

/// Top-level settings (baseline 14.2). P1-02 carries the defaults; the
/// settings UI binds them in later tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub accent_color: String,
    pub opacity: f64,
    pub hotkey: String,
    pub auto_hide_on_blur: bool,
    pub auto_hide_on_launch: bool,
    pub pinned: bool,
    pub icon_size: String,
    /// P2-13 (baseline 16.2/外观): window corners use the DWM rounded shape.
    /// Off means square; platforms without the attribute degrade to square.
    #[serde(default = "default_true")]
    pub rounded_corners: bool,
    /// P2-14 (baseline 16.3): DWM system backdrop material — `none`, `mica`
    /// or `acrylic`; anything else loads as `none`.
    #[serde(default = "default_backdrop")]
    pub backdrop: String,
    /// P3-04 (baseline 22 鼠标热区): whether hovering a screen edge toggles
    /// the panel. Off by default so the behavior stays opt-in.
    #[serde(default)]
    pub hot_zone_enabled: bool,
    /// P3-04: which screen edge is the hot zone — left/right/top/bottom;
    /// anything else loads as left.
    #[serde(default = "default_hot_zone_edge")]
    pub hot_zone_edge: String,
    /// P4-04（基线 23 章）：用户可关闭更新检查——默认开启。
    #[serde(default = "default_true")]
    pub update_check_enabled: bool,
    /// P4-04：更新源清单地址；生产部署时指向真实 HTTPS 端点。
    #[serde(default = "default_update_url")]
    pub update_url: String,
    /// P3-05/P3-06/P3-07: grid sort mode — manual (default), recent,
    /// frequent, favorite; anything else loads as manual.
    #[serde(default = "default_sort_mode")]
    pub sort_mode: String,
    /// P3-09 (baseline 22 Everything 可选集成): opt-in; when enabled the
    /// search center offers to hand the query to Everything.
    #[serde(default)]
    pub everything_enabled: bool,
    /// P3-09: Everything.exe used for the handoff; resolved against the
    /// fallback install locations at use time. Missing Everything never
    /// affects the main program.
    #[serde(default = "default_everything_path")]
    pub everything_path: String,
    /// P2-15 (baseline 16.4): the window position the user left the panel at.
    /// `None` on a fresh configuration (and in configurations written before
    /// the position was tracked); nothing is restored then and the window
    /// keeps the position the window manager gives it.
    #[serde(default)]
    pub window_bounds: Option<WindowBounds>,
}

fn default_true() -> bool {
    true
}

fn default_backdrop() -> String {
    "none".to_string()
}

/// P2-13 (baseline 16.2): opacity is 50%–100%. Out-of-range or NaN values
/// clamp into the range instead of failing the load.
pub fn normalize_opacity(value: f64) -> f64 {
    if value.is_nan() {
        return 0.95;
    }
    value.clamp(0.5, 1.0)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            accent_color: "#2563EB".to_string(),
            opacity: 0.95,
            hotkey: "Ctrl+Alt+Space".to_string(),
            auto_hide_on_blur: true,
            auto_hide_on_launch: true,
            pinned: false,
            icon_size: "medium".to_string(),
            rounded_corners: true,
            backdrop: "none".to_string(),
            window_bounds: None,
            hot_zone_enabled: false,
            hot_zone_edge: default_hot_zone_edge(),
            sort_mode: default_sort_mode(),
            update_check_enabled: default_true(),
            update_url: default_update_url(),
            everything_enabled: false,
            everything_path: default_everything_path(),
        }
    }
}

/// P3-04: the default hot-zone edge.
fn default_hot_zone_edge() -> String {
    "left".to_string()
}

/// P4-04：布尔设置默认 true。
/// An update source must be configured explicitly before any network request.
fn default_update_url() -> String {
    String::new()
}

/// P3-05/P3-06/P3-07: the default grid sort mode.
fn default_sort_mode() -> String {
    "manual".to_string()
}

/// P3-09 (baseline 22): the default Everything.exe location.
fn default_everything_path() -> String {
    "C:\\Program Files\\Everything\\Everything.exe".to_string()
}

/// P3-08 (baseline 8.2): the default command execution mode.
fn default_execution_mode() -> String {
    "direct".to_string()
}

/// P3-08 (baseline 8.2): normalizes the command execution mode; empty or
/// unknown values fall back to direct.
pub fn normalize_execution_mode(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "cmd" => "cmd",
        "powershell" => "powershell",
        _ => "direct",
    }
}

/// P3-05/P3-06/P3-07: normalizes the grid sort mode; unknown values fall
/// back to manual.
pub fn normalize_sort_mode(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "recent" => "recent",
        "frequent" => "frequent",
        "favorite" => "favorite",
        _ => "manual",
    }
}

/// P3-04: normalizes the hot-zone edge; unknown values fall back to left.
pub fn normalize_hot_zone_edge(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "right" => "right",
        "top" => "top",
        "bottom" => "bottom",
        _ => "left",
    }
}

/// Icon size presets for the project grid (baseline 2.4: 大/中/小, default
/// 中). Values map to `settings.icon_size`; invalid settings fall back to
/// medium instead of failing the load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl IconSize {
    pub fn from_settings(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }

    /// The persisted settings value.
    pub fn key(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    /// The grid cell size in logical pixels (width, height).
    pub fn cell(self) -> (f32, f32) {
        match self {
            Self::Small => (76.0, 88.0),
            Self::Medium => (96.0, 108.0),
            Self::Large => (128.0, 140.0),
        }
    }

    /// The icon edge in logical pixels.
    pub fn icon_px(self) -> f32 {
        match self {
            Self::Small => 28.0,
            Self::Medium => 40.0,
            Self::Large => 56.0,
        }
    }
}

/// The whole library, persisted as one JSON document (baseline 14.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub schema_version: u32,
    #[serde(default)]
    pub settings: Settings,
    pub current_group_id: Option<String>,
    pub groups: Vec<Group>,
    /// P3-02 (baseline 22 启动组合): named item sets launched together.
    #[serde(default)]
    pub combinations: Vec<LaunchCombination>,
}

/// P3-05/P3-06/P3-07 (baseline 22): a usage snapshot for sorting.
pub struct UsageSnapshot {
    pub id: String,
    pub name: String,
    pub last_used: i64,
    pub launch_count: u64,
    pub favorite: bool,
}

/// P3-05/P3-06/P3-07: the sort rank of one item within the grid.
type UsageRank = (usize, String, i64, u64, bool);

/// P3-05/P3-06/P3-07 (baseline 22): sorts item ids by the active grid order.
/// Favorites come first (ties keep the manual order), then by most recent
/// use, then by highest launch count; everything else keeps the manual order.
/// Returns the ids in display order.
pub fn sort_item_ids_by_mode(ids: &[String], items: &[UsageSnapshot], mode: &str) -> Vec<String> {
    let rank = |id: &str| -> UsageRank {
        items
            .iter()
            .enumerate()
            .find(|(_, snapshot)| snapshot.id == id)
            .map(|(index, snapshot)| {
                (
                    index,
                    snapshot.name.clone(),
                    snapshot.last_used,
                    snapshot.launch_count,
                    snapshot.favorite,
                )
            })
            .unwrap_or((usize::MAX, String::new(), 0, 0, false))
    };
    let mut sorted: Vec<(String, UsageRank)> =
        ids.iter().map(|id| (id.clone(), rank(id))).collect();
    match mode {
        // Favorites to the top (by name), then the manual order.
        "favorite" => {
            sorted.sort_by(|left, right| {
                right
                    .1
                    .4
                    .cmp(&left.1.4)
                    .then_with(|| left.1.0.cmp(&right.1.0))
            });
        }
        // Most recent first (missing last_used sorts last, by manual order).
        "recent" => {
            sorted.sort_by(|left, right| {
                right
                    .1
                    .2
                    .cmp(&left.1.2)
                    .then_with(|| left.1.0.cmp(&right.1.0))
            });
        }
        // Highest launch count first (ties keep the manual order).
        "frequent" => {
            sorted.sort_by(|left, right| {
                right
                    .1
                    .3
                    .cmp(&left.1.3)
                    .then_with(|| left.1.0.cmp(&right.1.0))
            });
        }
        _ => {}
    }
    sorted.into_iter().map(|(id, _)| id).collect()
}

impl Default for Library {
    fn default() -> Self {
        Self {
            schema_version: 2,
            settings: Settings::default(),
            groups: Vec::new(),
            current_group_id: None,
            combinations: Vec::new(),
        }
    }
}

/// Why a group deletion needs the user's decision (baseline 2.2: deleting a
/// non-empty group must confirm migration or deletion of its content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// The group was empty and removed.
    Removed,
    /// The group still holds content; the caller must ask the user whether to
    /// migrate the content or delete it.
    NeedsConfirmation {
        content_summary: String,
        item_count: usize,
        category_count: usize,
    },
    /// The group id was unknown.
    NotFound,
}

/// Why a category deletion needs the user's decision (baseline 2.3:
/// deleting a non-empty category must offer migration or explicit deletion,
/// never silent data loss).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoryDeleteOutcome {
    Removed,
    NeedsConfirmation { item_count: usize },
    NotFound,
}

impl Library {
    pub fn group(&self, group_id: &str) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == group_id)
    }

    pub fn group_mut(&mut self, group_id: &str) -> Option<&mut Group> {
        self.groups.iter_mut().find(|group| group.id == group_id)
    }

    pub fn group_index(&self, group_id: &str) -> Option<usize> {
        self.groups.iter().position(|group| group.id == group_id)
    }

    /// The group currently shown; falls back to the first group so the UI
    /// always has something to render.
    pub fn active_group(&self) -> Option<&Group> {
        match &self.current_group_id {
            Some(id) => self.group(id),
            None => self.groups.first(),
        }
    }

    pub fn active_group_id(&self) -> Option<&str> {
        self.active_group().map(|group| group.id.as_str())
    }

    /// The category shown inside the active group (baseline 2.2's
    /// `last_category_id`), falling back to the first category.
    pub fn active_category(&self) -> Option<&Category> {
        let group = self.active_group()?;
        match &group.last_category_id {
            Some(id) => group.category(id).or_else(|| group.categories.first()),
            None => group.categories.first(),
        }
    }

    /// The category a group currently shows (its `last_category_id` or the
    /// first one); P2-06 spring-load drops land here.
    pub fn active_category_of_group(&self, group_id: &str) -> Option<&Category> {
        let group = self.group(group_id)?;
        match &group.last_category_id {
            Some(id) => group.category(id).or_else(|| group.categories.first()),
            None => group.categories.first(),
        }
    }

    /// Selects a group (baseline 2.2: clicking a group switches its
    /// categories). Unknown ids are ignored.
    pub fn select_group(&mut self, group_id: &str) -> bool {
        if self.group(group_id).is_none() {
            return false;
        }
        self.current_group_id = Some(group_id.to_string());
        true
    }

    /// Selects a category inside the active group and remembers it in the
    /// group's `last_category_id`.
    pub fn select_category(&mut self, category_id: &str) -> bool {
        let Some(group_id) = self.active_group_id().map(str::to_string) else {
            return false;
        };
        let Some(group) = self.group_mut(&group_id) else {
            return false;
        };
        if !group
            .categories
            .iter()
            .any(|category| category.id == category_id)
        {
            return false;
        }
        group.last_category_id = Some(category_id.to_string());
        true
    }

    /// Renames a group; empty names are rejected.
    pub fn rename_group(&mut self, group_id: &str, name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(group) = self.group_mut(group_id) else {
            return false;
        };
        group.name = trimmed.to_string();
        true
    }

    /// Sets the optional accent color (baseline 2.2); an empty string clears
    /// it, a non-empty value must look like `#rrggbb`.
    pub fn set_group_color(&mut self, group_id: &str, color: &str) -> bool {
        let trimmed = color.trim();
        if !trimmed.is_empty() && !is_hex_color(trimmed) {
            return false;
        }
        let Some(group) = self.group_mut(group_id) else {
            return false;
        };
        group.color = trimmed.to_string();
        true
    }

    /// Creates a group and makes it active (baseline 2.2: "新分组" is a real
    /// user group, not a fixed button). Returns the new group id.
    pub fn add_group(&mut self, name: &str, id: impl Into<String>) -> Option<String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        let id = id.into();
        if id.is_empty() || self.groups.iter().any(|group| group.id == id) {
            return None;
        }
        self.groups.push(Group::new(id.clone(), trimmed));
        self.current_group_id = Some(id.clone());
        Some(id)
    }

    /// Moves a group to a new position (baseline 2.2: drag to reorder).
    pub fn move_group(&mut self, group_id: &str, new_index: usize) -> bool {
        let Some(from) = self.group_index(group_id) else {
            return false;
        };
        let to = new_index.min(self.groups.len().saturating_sub(1));
        let group = self.groups.remove(from);
        self.groups.insert(to, group);
        true
    }

    /// Deletes a group. A group with content is NOT removed silently: the
    /// caller must confirm migration or deletion first (baseline 2.2).
    pub fn delete_group(&mut self, group_id: &str) -> DeleteOutcome {
        let Some(index) = self.group_index(group_id) else {
            return DeleteOutcome::NotFound;
        };
        let group = &self.groups[index];
        if !group.is_empty() {
            let category_count = group.categories.len();
            let item_count = group.item_count();
            let content_summary = group
                .categories
                .iter()
                .map(|category| category.name.as_str())
                .collect::<Vec<_>>()
                .join("、");
            return DeleteOutcome::NeedsConfirmation {
                content_summary,
                item_count,
                category_count,
            };
        }
        self.groups.remove(index);
        if self.current_group_id.as_deref() == Some(group_id) {
            self.current_group_id = self.groups.first().map(|group| group.id.clone());
        }
        DeleteOutcome::Removed
    }

    /// Deletes a group after the user confirmed the content is dropped.
    pub fn delete_group_confirmed(&mut self, group_id: &str) -> bool {
        let Some(index) = self.group_index(group_id) else {
            return false;
        };
        self.groups.remove(index);
        if self.current_group_id.as_deref() == Some(group_id) {
            self.current_group_id = self.groups.first().map(|group| group.id.clone());
        }
        true
    }

    /// Moves a group (with all its content) onto another group — the
    /// "migrate" choice of the delete confirmation (baseline 2.2).
    pub fn migrate_group_into(&mut self, group_id: &str, target_group_id: &str) -> bool {
        if group_id == target_group_id {
            return false;
        }
        let Some(index) = self.group_index(group_id) else {
            return false;
        };
        if self.group(target_group_id).is_none() {
            return false;
        }
        let source = self.groups.remove(index);
        let Some(target) = self.group_mut(target_group_id) else {
            return false;
        };
        for mut category in source.categories {
            // Category ids are global UUIDs; a collision (imported twice)
            // re-homes the category under a fresh id instead of overwriting.
            if target
                .categories
                .iter()
                .any(|existing| existing.id == category.id)
            {
                category.id = new_uuid();
            }
            target.categories.push(category);
        }
        true
    }

    /// Creates a category inside a group (baseline 2.3).
    pub fn add_category(
        &mut self,
        group_id: &str,
        name: &str,
        id: impl Into<String>,
    ) -> Option<String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        let id = id.into();
        if id.is_empty() {
            return None;
        }
        let group = self.group_mut(group_id)?;
        if group.categories.iter().any(|category| category.id == id) {
            return None;
        }
        group.categories.push(Category::new(id.clone(), trimmed));
        if group.last_category_id.is_none() {
            group.last_category_id = Some(id.clone());
        }
        Some(id)
    }

    /// The category the user is currently working with inside the active
    /// group (selected row).
    pub fn active_category_mut(&mut self) -> Option<&mut Category> {
        let group = self.active_group_mut()?;
        let preferred = group.last_category_id.clone();
        let position = preferred
            .as_deref()
            .and_then(|id| group.categories.iter().position(|c| c.id == id))
            .unwrap_or(0);
        group.categories.get_mut(position)
    }

    /// The item with `item_id` inside the active category (P1-05).
    pub fn active_category_item(&self, item_id: &str) -> Option<&Item> {
        self.active_category()?
            .items
            .iter()
            .find(|item| item.id == item_id)
    }

    /// The item with `item_id` in any group/category (P2-01 all-group search).
    /// P3-02: adds a named combination and returns it. The name is trimmed;
    /// an empty name falls back to "组合 N".
    pub fn add_combination(&mut self, name: &str, item_ids: Vec<String>) -> LaunchCombination {
        let trimmed = name.trim();
        let name = if trimmed.is_empty() {
            format!("组合 {}", self.combinations.len() + 1)
        } else {
            trimmed.to_string()
        };
        let combination = LaunchCombination::new(&name, item_ids);
        self.combinations.push(combination.clone());
        combination
    }

    /// P3-02: removes a combination; true when it existed.
    pub fn remove_combination(&mut self, id: &str) -> bool {
        let before = self.combinations.len();
        self.combinations.retain(|combination| combination.id != id);
        self.combinations.len() != before
    }

    /// P3-02: the combination with the given id.
    pub fn combination(&self, id: &str) -> Option<&LaunchCombination> {
        self.combinations
            .iter()
            .find(|combination| combination.id == id)
    }

    /// P3-05/P3-06: records a successful launch (last_used + launch_count);
    /// returns false when the item does not exist.
    pub fn record_launch(&mut self, item_id: &str, now_ms: i64) -> bool {
        let Some(item) = self
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.categories)
            .flat_map(|category| &mut category.items)
            .find(|item| item.id == item_id)
        else {
            return false;
        };
        item.last_used = now_ms;
        item.launch_count = item.launch_count.saturating_add(1);
        true
    }

    /// P3-07: toggles the favorite flag; returns the new value.
    pub fn toggle_favorite(&mut self, item_id: &str) -> bool {
        let Some(item) = self
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.categories)
            .flat_map(|category| &mut category.items)
            .find(|item| item.id == item_id)
        else {
            return false;
        };
        item.favorite = !item.favorite;
        item.favorite
    }

    pub fn find_item(&self, item_id: &str) -> Option<&Item> {
        self.groups
            .iter()
            .flat_map(|group| group.categories.iter())
            .flat_map(|category| category.items.iter())
            .find(|item| item.id == item_id)
    }

    /// P3-08: mutable lookup for the confirmation flag flip.
    pub fn find_item_mut(&mut self, item_id: &str) -> Option<&mut Item> {
        self.groups
            .iter_mut()
            .flat_map(|group| group.categories.iter_mut())
            .flat_map(|category| category.items.iter_mut())
            .find(|item| item.id == item_id)
    }

    /// Adds an item to the active category; duplicate ids are refused.
    pub fn add_item(&mut self, item: Item) -> bool {
        let Some(category) = self.active_category_mut() else {
            return false;
        };
        if category.items.iter().any(|existing| existing.id == item.id) {
            return false;
        }
        category.items.push(item);
        true
    }

    /// Adds an item to a specific category, searching every group (P1-17
    /// external drops onto a category or a non-active group tab).
    pub fn add_item_to_category(&mut self, category_id: &str, item: Item) -> bool {
        let Some(category) = self
            .groups
            .iter_mut()
            .flat_map(|group| group.categories.iter_mut())
            .find(|category| category.id == category_id)
        else {
            return false;
        };
        if category.items.iter().any(|existing| existing.id == item.id) {
            return false;
        }
        category.items.push(item);
        true
    }

    /// Replaces an item in the active category in place, keeping its index.
    pub fn update_item(&mut self, item: Item) -> bool {
        let Some(category) = self.active_category_mut() else {
            return false;
        };
        let Some(slot) = category
            .items
            .iter_mut()
            .find(|existing| existing.id == item.id)
        else {
            return false;
        };
        *slot = item;
        true
    }

    /// Removes an item from the active category.
    pub fn remove_item(&mut self, item_id: &str) -> bool {
        let Some(category) = self.active_category_mut() else {
            return false;
        };
        let before = category.items.len();
        category.items.retain(|item| item.id != item_id);
        category.items.len() != before
    }

    /// Reorders an item inside its active category (P2-04 grid drag sorting).
    /// The target index is clamped to the last slot; no-op moves return false.
    pub fn move_item_to_index(&mut self, item_id: &str, new_index: usize) -> bool {
        let Some(category) = self.active_category_mut() else {
            return false;
        };
        let Some(current) = category.items.iter().position(|item| item.id == item_id) else {
            return false;
        };
        if category.items.is_empty() {
            return false;
        }
        let target = new_index.min(category.items.len() - 1);
        if current == target {
            return false;
        }
        let item = category.items.remove(current);
        category.items.insert(target, item);
        true
    }

    /// Moves an item from its current category to another category of the
    /// active group (baseline 3.5 "移动到其他分类"). Moving to its own
    /// category is a no-op and returns false.
    pub fn move_item_to_category(&mut self, item_id: &str, target_category_id: &str) -> bool {
        let Some(group) = self.active_group_mut() else {
            return false;
        };
        if group.category(target_category_id).is_none() {
            return false;
        }
        let Some(source_position) = group
            .categories
            .iter()
            .position(|category| category.items.iter().any(|item| item.id == item_id))
        else {
            return false;
        };
        if group.categories[source_position].id == target_category_id {
            return false;
        }
        let Some(item_position) = group.categories[source_position]
            .items
            .iter()
            .position(|item| item.id == item_id)
        else {
            return false;
        };
        let item = group.categories[source_position]
            .items
            .remove(item_position);
        let Some(target) = group.category_mut(target_category_id) else {
            return false;
        };
        target.items.push(item);
        true
    }

    /// The id of the category that holds `item_id`, searching every group
    /// (P2-06 cross-group drops need to know where the item lives).
    pub fn category_id_of_item(&self, item_id: &str) -> Option<String> {
        self.groups
            .iter()
            .flat_map(|group| group.categories.iter())
            .find(|category| category.items.iter().any(|item| item.id == item_id))
            .map(|category| category.id.clone())
    }

    /// Moves an item into a target category in any group (P2-06 cross-group
    /// drag). Moving into the item's current category is a no-op; the item is
    /// appended to the target category. Unknown items or targets are refused
    /// without touching the library.
    pub fn move_item_across_groups(&mut self, item_id: &str, target_category_id: &str) -> bool {
        if self.category_id_of_item(item_id).as_deref() == Some(target_category_id) {
            return false;
        }
        let target_exists = self
            .groups
            .iter()
            .any(|group| group.categories.iter().any(|c| c.id == target_category_id));
        if !target_exists {
            return false;
        }
        let mut source: Option<(usize, usize, usize)> = None;
        for (group_index, group) in self.groups.iter().enumerate() {
            for (category_index, category) in group.categories.iter().enumerate() {
                if let Some(item_index) = category.items.iter().position(|item| item.id == item_id)
                {
                    source = Some((group_index, category_index, item_index));
                    break;
                }
            }
            if source.is_some() {
                break;
            }
        }
        let Some((source_group, source_category, source_item)) = source else {
            return false;
        };
        let item = self.groups[source_group].categories[source_category]
            .items
            .remove(source_item);
        for group in &mut self.groups {
            for category in &mut group.categories {
                if category.id == target_category_id {
                    category.items.push(item);
                    return true;
                }
            }
        }
        false
    }

    fn active_group_mut(&mut self) -> Option<&mut Group> {
        match self.current_group_id.clone() {
            Some(id) => self.group_mut(&id),
            None => self.groups.first_mut(),
        }
    }

    /// Renames a category of the active group; empty names are rejected.
    pub fn rename_category(&mut self, category_id: &str, name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(group) = self.active_group_mut() else {
            return false;
        };
        let Some(category) = group.categories.iter_mut().find(|c| c.id == category_id) else {
            return false;
        };
        category.name = trimmed.to_string();
        true
    }

    /// Deletes a category of the active group. A category with items is NOT
    /// removed silently (baseline 2.3).
    pub fn delete_category(&mut self, category_id: &str) -> CategoryDeleteOutcome {
        let Some(group) = self.active_group_mut() else {
            return CategoryDeleteOutcome::NotFound;
        };
        let Some(position) = group.categories.iter().position(|c| c.id == category_id) else {
            return CategoryDeleteOutcome::NotFound;
        };
        let item_count = group.categories[position].items.len();
        if item_count > 0 {
            return CategoryDeleteOutcome::NeedsConfirmation { item_count };
        }
        group.categories.remove(position);
        if group.last_category_id.as_deref() == Some(category_id) {
            group.last_category_id = group.categories.first().map(|c| c.id.clone());
        }
        CategoryDeleteOutcome::Removed
    }

    /// Deletes a category after the user confirmed dropping its items.
    pub fn delete_category_confirmed(&mut self, category_id: &str) -> bool {
        let Some(group) = self.active_group_mut() else {
            return false;
        };
        let Some(position) = group.categories.iter().position(|c| c.id == category_id) else {
            return false;
        };
        group.categories.remove(position);
        if group.last_category_id.as_deref() == Some(category_id) {
            group.last_category_id = group.categories.first().map(|c| c.id.clone());
        }
        true
    }

    /// Moves the items of one category into another (the "migrate" choice of
    /// the delete confirmation, baseline 2.3). The source category is then
    /// removed.
    pub fn migrate_category_items_into(&mut self, category_id: &str, target_id: &str) -> bool {
        if category_id == target_id {
            return false;
        }
        let Some(group) = self.active_group_mut() else {
            return false;
        };
        let Some(source_position) = group.categories.iter().position(|c| c.id == category_id)
        else {
            return false;
        };
        if !group.categories.iter().any(|c| c.id == target_id) {
            return false;
        }
        let source = group.categories.remove(source_position);
        let Some(target) = group.categories.iter_mut().find(|c| c.id == target_id) else {
            return false;
        };
        target.items.extend(source.items);
        if group.last_category_id.as_deref() == Some(category_id) {
            group.last_category_id = Some(target_id.to_string());
        }
        true
    }

    /// Moves a category to a new position inside the active group (baseline
    /// 2.3: drag to reorder).
    pub fn move_category(&mut self, category_id: &str, new_index: usize) -> bool {
        let Some(group) = self.active_group_mut() else {
            return false;
        };
        let Some(from) = group
            .categories
            .iter()
            .position(|category| category.id == category_id)
        else {
            return false;
        };
        let to = new_index.min(group.categories.len().saturating_sub(1));
        let category = group.categories.remove(from);
        group.categories.insert(to, category);
        true
    }
}

/// Generates a UUID v4 via UuidCreate (baseline 14.1); falls back to a
/// nanosecond-unique string when the RPC call fails.
pub fn new_uuid() -> String {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }
    #[link(name = "rpcrt4")]
    unsafe extern "system" {
        fn UuidCreate(uuid: *mut Guid) -> u32;
    }
    // SAFETY: UuidCreate only writes through the provided pointer.
    let mut guid = Guid::default();
    let status = unsafe { UuidCreate(&mut guid) };
    if status != 0 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        return format!("fallback-{nanos:x}-{:x}", std::process::id());
    }
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

/// `#rrggbb` check for accent colors.
fn is_hex_color(value: &str) -> bool {
    let Some(rest) = value.strip_prefix('#') else {
        return false;
    };
    rest.len() == 6 && rest.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_library() -> Library {
        let mut library = Library::default();
        let group_a = new_uuid();
        let group_b = new_uuid();
        library.add_group("常用工具", group_a.clone());
        library.add_group("应用", group_b.clone());
        library.add_category(&group_a, "必备软件", new_uuid());
        library.add_category(&group_b, "浏览器", new_uuid());
        library
    }

    #[test]
    fn adding_a_group_makes_it_active_and_rejects_duplicates() {
        let mut library = Library::default();
        let id = new_uuid();
        assert_eq!(library.add_group("常用工具", id.clone()), Some(id.clone()));
        assert_eq!(library.active_group_id(), Some(id.as_str()));
        let second = new_uuid();
        assert_eq!(
            library.add_group("应用", second.clone()),
            Some(second.clone())
        );
        assert_eq!(library.active_group_id(), Some(second.as_str()));
        assert_eq!(
            library.add_group("重复", second),
            None,
            "duplicate id rejected"
        );
        assert_eq!(library.add_group("   ", new_uuid()), None, "blank rejected");
        assert_eq!(library.groups.len(), 2);
    }

    #[test]
    fn selecting_a_group_switches_its_categories() {
        let mut library = sample_library();
        let first_id = library.groups[0].id.clone();
        let second_id = library.groups[1].id.clone();
        assert!(library.select_group(&first_id));
        assert_eq!(library.active_group_id(), Some(first_id.as_str()));
        assert_eq!(
            library
                .active_category()
                .map(|category| category.name.as_str()),
            Some("必备软件")
        );
        assert!(library.select_group(&second_id));
        assert_eq!(
            library
                .active_category()
                .map(|category| category.name.as_str()),
            Some("浏览器")
        );
        assert!(!library.select_group("no-such-group"));
    }

    #[test]
    fn selecting_a_category_is_remembered_per_group() {
        let mut library = sample_library();
        let second_category = new_uuid();
        let second_group_id = library.groups[1].id.clone();
        let first_group_id = library.groups[0].id.clone();
        library.add_category(&second_group_id, "办公软件", second_category.clone());
        library.select_group(&second_group_id);
        assert!(library.select_category(&second_category));
        // Switching away and back keeps the group's own memory.
        library.select_group(&first_group_id);
        library.select_group(&second_group_id);
        assert_eq!(
            library
                .active_category()
                .map(|category| category.id.clone()),
            Some(second_category),
            "last_category_id survives group switches"
        );
    }

    #[test]
    fn rename_rejects_blank_and_unknown_groups() {
        let mut library = sample_library();
        let first_id = library.groups[0].id.clone();
        assert!(library.rename_group(&first_id, " 常用工具 "));
        assert_eq!(library.group(&first_id).expect("group").name, "常用工具");
        assert!(!library.rename_group(&first_id, "   "));
        assert!(!library.rename_group("no-such-group", "x"));
    }

    #[test]
    fn accent_color_validates_hex_form() {
        let mut library = sample_library();
        let first_id = library.groups[0].id.clone();
        assert!(library.set_group_color(&first_id, "#ff8800"));
        assert_eq!(library.group(&first_id).expect("group").color, "#ff8800");
        assert!(library.set_group_color(&first_id, ""));
        assert_eq!(library.group(&first_id).expect("group").color, "");
        assert!(
            !library.set_group_color(&first_id, "ff8800"),
            "missing # rejected"
        );
        assert!(
            !library.set_group_color(&first_id, "#ff88"),
            "short hex rejected"
        );
        assert!(
            !library.set_group_color(&first_id, "#gggggg"),
            "non-hex rejected"
        );
    }

    #[test]
    fn moving_a_group_reorders_the_list() {
        let mut library = sample_library();
        let third = new_uuid();
        library.add_group("自动化", third.clone());
        assert!(library.move_group(&third, 0));
        assert_eq!(
            library
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            vec!["自动化", "常用工具", "应用"]
        );
        assert!(library.move_group(&third, 99), "out of range clamps");
        assert_eq!(
            library.groups.last().map(|group| group.name.as_str()),
            Some("自动化")
        );
    }

    #[test]
    fn deleting_an_empty_group_removes_it() {
        let mut library = sample_library();
        let empty = new_uuid();
        library.add_group("空组", empty.clone());
        let before = library.groups.len();
        let outcome = library.delete_group(&empty);
        assert_eq!(outcome, DeleteOutcome::Removed);
        assert_eq!(library.groups.len(), before - 1);
        assert_eq!(
            library.delete_group("no-such-group"),
            DeleteOutcome::NotFound
        );
    }

    #[test]
    fn deleting_a_non_empty_group_requires_confirmation() {
        let mut library = sample_library();
        let first_id = library.groups[0].id.clone();
        let outcome = library.delete_group(&first_id);
        match outcome {
            DeleteOutcome::NeedsConfirmation {
                item_count,
                category_count,
                content_summary,
            } => {
                assert_eq!(item_count, 0);
                assert_eq!(category_count, 1);
                assert_eq!(content_summary, "必备软件");
            }
            other => panic!("expected NeedsConfirmation, got {other:?}"),
        }
        assert_eq!(
            library.groups.len(),
            2,
            "the group must survive until the user confirms"
        );
        assert!(library.delete_group_confirmed(&first_id));
        assert_eq!(library.groups.len(), 1);
    }

    #[test]
    fn migrating_moves_categories_and_keeps_content() {
        let mut library = sample_library();
        let first_id = library.groups[0].id.clone();
        let second_id = library.groups[1].id.clone();
        library.groups[0].categories[0].items.push(Item::new(
            new_uuid(),
            "记事本",
            ItemKind::Exe,
            "notepad.exe",
        ));
        assert!(library.migrate_group_into(&first_id, &second_id));
        assert_eq!(library.groups.len(), 1);
        let target = library.group(&second_id).expect("target group");
        assert_eq!(target.categories.len(), 2);
        assert_eq!(target.item_count(), 1, "items travelled with the category");
        assert!(
            !library.migrate_group_into(&second_id, &second_id),
            "no self-migration"
        );
    }

    #[test]
    fn migrating_onto_a_conflicting_category_id_rehomes_it() {
        let mut library = sample_library();
        let duplicated = library.groups[0].categories[0].clone();
        let second_id = library.groups[1].id.clone();
        library
            .group_mut(&second_id)
            .expect("group")
            .categories
            .push(duplicated);
        let before = library.group(&second_id).expect("group").categories.len();
        assert!(library.migrate_group_into(&library.groups[0].id.clone(), &second_id));
        let target = library.group(&second_id).expect("target group");
        assert_eq!(target.categories.len(), before + 1);
        let ids: Vec<&str> = target
            .categories
            .iter()
            .map(|category| category.id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "category ids stay unique: {ids:?}");
    }

    #[test]
    fn serialization_round_trips_the_whole_library() {
        let mut library = sample_library();
        let second_id = library.groups[1].id.clone();
        let second_category = library.group(&second_id).expect("group").categories[0]
            .id
            .clone();
        library.select_group(&second_id);
        library.select_category(&second_category);
        library.set_group_color(&second_id, "#112233");
        let json = serde_json::to_string_pretty(&library).expect("serialize");
        let restored: Library = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, library);
        assert_eq!(restored.active_group_id(), Some(second_id.as_str()));
        assert_eq!(
            restored
                .active_category()
                .map(|category| category.id.clone()),
            Some(second_category),
            "last_category_id survives the round trip"
        );
    }

    #[test]
    fn persisted_shape_matches_baseline_14_2() {
        let mut library = sample_library();
        library.groups[0].categories[0].items.push(Item::new(
            new_uuid(),
            "Everything",
            ItemKind::Exe,
            "C:\\Tools\\Everything.exe",
        ));
        let json = serde_json::to_string(&library).expect("serialize");
        for needle in [
            "\"schema_version\":2",
            "\"settings\"",
            "\"current_group_id\"",
            "\"color\"",
            "\"last_category_id\"",
            "\"item_type\"",
            "\"launch\"",
            "\"workdir\"",
            "\"run_as_admin\"",
        ] {
            assert!(json.contains(needle), "persisted JSON missing {needle}");
        }
    }

    #[test]
    fn item_kinds_and_window_states_serialize_as_snake_case() {
        let item = Item {
            window_state: WindowState::Maximized,
            ..Item::new(new_uuid(), "应用", ItemKind::Exe, "app.exe")
        };
        let json = serde_json::to_string(&item).expect("serialize");
        assert!(json.contains("\"item_type\":\"exe\""), "{json}");
        assert!(json.contains("\"window_state\":\"maximized\""), "{json}");
        let restored: Item = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, item);
    }

    #[test]
    fn deleting_the_active_group_moves_the_selection() {
        let mut library = sample_library();
        let second_id = library.groups[1].id.clone();
        library.select_group(&second_id);
        assert!(library.delete_group_confirmed(&second_id));
        assert_eq!(
            library.active_group_id(),
            Some(library.groups[0].id.as_str()),
            "the selection falls back to a remaining group"
        );
    }

    #[test]
    fn settings_carry_the_baseline_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.accent_color, "#2563EB");
        assert!((settings.opacity - 0.95).abs() < f64::EPSILON);
        assert_eq!(settings.hotkey, "Ctrl+Alt+Space");
        assert!(settings.auto_hide_on_blur);
        assert!(settings.auto_hide_on_launch);
        assert!(!settings.pinned);
        assert_eq!(settings.icon_size, "medium");
        assert!(
            settings.rounded_corners,
            "P2-13: corners default to rounded"
        );
        assert_eq!(
            settings.backdrop, "none",
            "P2-14: backdrop defaults to plain color"
        );
        assert!(
            settings.window_bounds.is_none(),
            "P2-15: no remembered position on a fresh config"
        );
    }

    #[test]
    fn opacity_clamps_into_the_baseline_16_2_range() {
        assert!((normalize_opacity(0.95) - 0.95).abs() < f64::EPSILON);
        assert_eq!(normalize_opacity(1.5), 1.0);
        assert_eq!(normalize_opacity(0.2), 0.5);
        assert_eq!(normalize_opacity(f64::NAN), 0.95);
        assert_eq!(normalize_opacity(-1.0), 0.5);
    }

    #[test]
    fn rounded_corners_defaults_on_and_round_trips() {
        // A config written before P2-13 has no rounded_corners key; the serde
        // default must turn the DWM rounded shape on.
        let legacy = r##"{"theme":"system","accent_color":"#2563EB","opacity":0.95,"hotkey":"Ctrl+Alt+Space","auto_hide_on_blur":true,"auto_hide_on_launch":true,"pinned":false,"icon_size":"medium","window_bounds":{"x":0,"y":0,"width":860,"height":580,"monitor_id":"DISPLAY1"}}"##;
        let settings: Settings = serde_json::from_str(legacy).expect("legacy settings");
        assert!(settings.rounded_corners);
        let settings = Settings {
            rounded_corners: false,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        assert!(json.contains("\"rounded_corners\":false"), "{json}");
        let restored: Settings = serde_json::from_str(&json).expect("deserialize");
        assert!(!restored.rounded_corners);
    }

    #[test]
    fn backdrop_defaults_none_and_round_trips() {
        // A config written before P2-14 has no backdrop key; the serde default
        // must be the plain solid color.
        let legacy = r##"{"theme":"system","accent_color":"#2563EB","opacity":0.95,"hotkey":"Ctrl+Alt+Space","auto_hide_on_blur":true,"auto_hide_on_launch":true,"pinned":false,"icon_size":"medium","window_bounds":{"x":0,"y":0,"width":860,"height":580,"monitor_id":"DISPLAY1"}}"##;
        let settings: Settings = serde_json::from_str(legacy).expect("legacy settings");
        assert_eq!(settings.backdrop, "none");
        let settings = Settings {
            backdrop: "mica".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        assert!(json.contains("\"backdrop\":\"mica\""), "{json}");
        let restored: Settings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.backdrop, "mica");
    }

    #[test]
    fn uuids_are_unique_and_shaped() {
        let first = new_uuid();
        let second = new_uuid();
        assert_ne!(first, second);
        assert_eq!(first.len(), 36, "UUID v4 text form: {first}");
        assert_eq!(first.matches('-').count(), 4);
    }

    #[test]
    fn item_kind_inference_from_paths() {
        assert_eq!(
            ItemKind::from_path("C:\\Windows\\System32\\notepad.exe"),
            ItemKind::Exe
        );
        assert_eq!(ItemKind::from_path("C:\\Tools\\Demo.LNK"), ItemKind::Lnk);
        assert_eq!(
            ItemKind::from_path("C:\\Scripts\\run.bat"),
            ItemKind::Command
        );
        assert_eq!(
            ItemKind::from_path("C:\\Scripts\\run.ps1"),
            ItemKind::Command
        );
        assert_eq!(ItemKind::from_path("https://example.com/a"), ItemKind::Uri);
        assert_eq!(ItemKind::from_path("steam://run/123"), ItemKind::Uri);
        assert_eq!(ItemKind::from_path("mailto:a@b.c"), ItemKind::Uri);
        // P2-10: Shell namespace/URI and UNC targets.
        assert_eq!(ItemKind::from_path("shell:MyComputerFolder"), ItemKind::Uri);
        assert_eq!(ItemKind::from_path("ms-settings:display"), ItemKind::Uri);
        assert_eq!(
            ItemKind::from_path("shell:::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
            ItemKind::Uri
        );
        assert_eq!(
            ItemKind::from_path("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
            ItemKind::Folder
        );
        assert_eq!(ItemKind::from_path("\\\\localhost\\C$"), ItemKind::Folder);
        assert_eq!(ItemKind::from_path("C:\\docs\\readme.txt"), ItemKind::File);
        assert_eq!(ItemKind::from_path("  "), ItemKind::File);
        let directory = std::env::temp_dir();
        assert_eq!(
            ItemKind::from_path(&directory.to_string_lossy()),
            ItemKind::Folder
        );
        assert_eq!(
            ItemKind::from_index(ItemKind::Command.index()),
            ItemKind::Command
        );
        assert_eq!(ItemKind::from_index(99), ItemKind::Exe);
    }

    #[test]
    fn shell_namespace_and_unc_targets_are_recognized() {
        for uri in [
            "shell:MyComputerFolder",
            "shell:AppsFolder",
            "ms-settings:display",
        ] {
            assert!(is_uri_target(uri), "{uri} is a URI");
            assert!(is_shell_namespace_target(uri), "{uri} is a Shell target");
        }
        assert!(is_shell_namespace_target(
            "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"
        ));
        assert!(!is_shell_namespace_target("C:\\Windows"));
        assert!(is_unc_path("\\\\localhost\\C$"));
        assert!(!is_unc_path("C:\\Windows"));
        for not_uri in [
            "C:\\Windows\\notepad.exe",
            "\\\\localhost\\C$",
            "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
            "not a uri",
        ] {
            assert!(!is_uri_target(not_uri), "{not_uri} is not a URI");
        }
    }

    #[test]
    fn shell_and_unc_targets_are_not_missing_and_have_no_file_icon() {
        for (kind, path) in [
            (ItemKind::Folder, "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
            (ItemKind::Folder, "\\\\localhost\\C$"),
            (ItemKind::Uri, "shell:MyComputerFolder"),
        ] {
            let item = Item::new("p210", "Shell", kind, path);
            assert!(!item.path_missing(), "{path}");
            assert_eq!(item.icon_source(), None, "{path}");
        }
    }

    #[test]
    fn default_workdir_follows_baseline_3_3() {
        assert_eq!(
            Item::default_workdir(ItemKind::Exe, "C:\\Apps\\demo.exe"),
            "C:\\Apps"
        );
        assert_eq!(
            Item::default_workdir(ItemKind::File, "C:\\Docs\\a.txt"),
            "C:\\Docs"
        );
        assert_eq!(
            Item::default_workdir(ItemKind::Lnk, "C:\\Apps\\demo.lnk"),
            ""
        );
        assert_eq!(Item::default_workdir(ItemKind::Folder, "C:\\Apps"), "");
        assert_eq!(
            Item::default_workdir(ItemKind::Uri, "https://example.com"),
            ""
        );
    }

    #[test]
    fn argument_windows_roundtrip_preserves_editor_values() {
        let mut values: Vec<String> = [
            "",
            "plain",
            "a b",
            "a\tb",
            "\"",
            "a\"b",
            "C:\\space dir\\",
            "中文🙂",
            "x\u{2003}y",
            "a\nb",
            "&|<>^()%!",
            "  padded  ",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        for count in 0..12 {
            let slashes = "\\".repeat(count);
            values.extend([format!("a b{slashes}"), format!("x{slashes}\"y"), slashes]);
        }
        assert_eq!(parse_args(&join_args(&values)), values);
        let mut item = Item::new("id", "item", ItemKind::Command, "tool.exe");
        item.launch.args = values;
        item.launch.execution_mode = "cmd".into();
        item.launch.run_as_admin = true;
        item.launch.confirm_on_first_run = false;
        for _ in 0..3 {
            let edited = ItemDraft::from_item(&item).into_item(item.id.clone());
            assert_eq!(
                edited, item,
                "opening/saving an editor must preserve arguments"
            );
            item = edited;
        }
    }

    #[test]
    fn argument_windows_crt_examples_and_exhaustive_roundtrip() {
        assert_eq!(
            parse_args(r#"a\\\b d\"e f"g h"i"#),
            [r"a\\\b", "d\"e", "fg hi"]
        );
        assert_eq!(parse_args(r#"a\\\"b c d"#), ["a\\\"b", "c", "d"]);
        assert_eq!(
            parse_args(r#""a""b" "unterminated space"#),
            ["a\"b", "unterminated space"]
        );
        let alphabet = ['a', ' ', '\t', '\\', '"', '中', '🙂'];
        let mut words = vec![String::new()];
        for _ in 0..5 {
            let previous = words.clone();
            words = previous
                .iter()
                .flat_map(|word| {
                    alphabet.iter().map(move |ch| {
                        let mut next = word.clone();
                        next.push(*ch);
                        next
                    })
                })
                .collect();
            for word in &words {
                assert_eq!(
                    parse_args(&quote_arg(word)).as_slice(),
                    std::slice::from_ref(word),
                    "{word:?}"
                );
            }
        }
    }

    #[test]
    fn argument_line_splits_with_quotes() {
        assert_eq!(parse_args(""), Vec::<String>::new());
        assert_eq!(parse_args("  --flag   value "), vec!["--flag", "value"]);
        assert_eq!(parse_args("--name \"a b\" c"), vec!["--name", "a b", "c"]);
        assert_eq!(parse_args("\"one two\""), vec!["one two"]);
        assert_eq!(
            parse_args("--path \"C:\\Program Files\\App\""),
            vec!["--path", "C:\\Program Files\\App"]
        );
    }

    #[test]
    fn draft_validates_and_round_trips() {
        let mut draft = ItemDraft::empty();
        assert_eq!(draft.validate(), Err("名称不能为空"));
        draft.name = "  记事本 ".to_string();
        assert_eq!(draft.validate(), Err("目标不能为空"));
        draft.path = " C:\\Windows\\notepad.exe ".to_string();
        draft.args = "-a \"b c\"".to_string();
        draft.workdir = " C:\\Windows ".to_string();
        draft.note = " 备注 ".to_string();
        draft.run_as_admin = true;
        draft.window_state = WindowState::Maximized;
        assert!(draft.validate().is_ok());
        let item = draft.clone().into_item("id-1");
        assert_eq!(item.name, "记事本");
        assert_eq!(item.launch.path, "C:\\Windows\\notepad.exe");
        assert_eq!(item.launch.args, vec!["-a", "b c"]);
        assert_eq!(item.launch.workdir, "C:\\Windows");
        assert!(item.launch.run_as_admin);
        assert_eq!(item.window_state, WindowState::Maximized);
        assert_eq!(item.note, "备注");
        let back = ItemDraft::from_item(&item);
        assert_eq!(back.name, "记事本");
        assert_eq!(back.args, "-a \"b c\""); // P4 审核修复：往返无损
        assert_eq!(back.item_type, ItemKind::Exe);
    }

    #[test]
    fn automation_draft_may_omit_the_target() {
        let mut draft = ItemDraft::empty();
        draft.name = "自动备份".to_string();
        draft.item_type = ItemKind::Automation;
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn icon_source_prefers_custom_and_skips_uris() {
        let mut item = Item::new("i", "记事本", ItemKind::Exe, "C:\\Windows\\notepad.exe");
        assert_eq!(
            item.icon_source().as_deref(),
            Some("C:\\Windows\\notepad.exe")
        );
        item.icon = "C:\\Icons\\custom.ico,3".to_string();
        assert_eq!(item.icon_source().as_deref(), Some("C:\\Icons\\custom.ico"));
        item.icon = "  ".to_string();
        item.item_type = ItemKind::Uri;
        item.launch.path = "https://example.com".to_string();
        assert_eq!(item.icon_source(), None);
        item.item_type = ItemKind::File;
        item.launch.path = "  ".to_string();
        assert_eq!(item.icon_source(), None);
    }

    #[test]
    fn draft_from_path_infers_name_type_and_workdir() {
        let exe = ItemDraft::from_path(std::path::Path::new("C:\\Tools\\Demo App.exe"));
        assert_eq!(exe.name, "Demo App");
        assert_eq!(exe.item_type, ItemKind::Exe);
        assert_eq!(exe.path, "C:\\Tools\\Demo App.exe");
        assert_eq!(exe.workdir, "C:\\Tools");

        let file = ItemDraft::from_path(std::path::Path::new("C:\\Docs\\readme.txt"));
        assert_eq!(file.item_type, ItemKind::File);
        assert_eq!(file.workdir, "C:\\Docs");

        let link = ItemDraft::from_path(std::path::Path::new("C:\\Links\\Editor.lnk"));
        assert_eq!(link.name, "Editor");
        assert_eq!(link.item_type, ItemKind::Lnk);
        assert_eq!(link.workdir, "", "LNK keeps its own shell semantics");
        assert!(!link.msi_advertised, "detection happens at import");
        assert!(link.validate().is_ok());
    }

    #[test]
    fn msi_advertised_flag_defaults_and_round_trips() {
        // Older configs without the field stay readable (default false).
        let legacy = r#"{"path":"C:\\Links\\msi.lnk","args":[],"workdir":"","run_as_admin":false,"aumid":""}"#;
        let launch: Launch = serde_json::from_str(legacy).expect("legacy launch");
        assert!(!launch.msi_advertised);

        let mut item = Item::new("msi", "MSI", ItemKind::Lnk, "C:\\Links\\msi.lnk");
        assert!(!item.launch.msi_advertised);
        item.launch.msi_advertised = true;
        let json = serde_json::to_string(&item).expect("serialize");
        assert!(json.contains("\"msi_advertised\":true"), "{json}");
        let restored: Item = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.launch.msi_advertised);

        let draft = ItemDraft::from_item(&restored);
        assert!(draft.msi_advertised);
        assert!(draft.into_item("msi").launch.msi_advertised);
    }

    #[test]
    fn add_item_to_category_targets_other_groups() {
        let mut library = sample_library();
        let target = library.groups[1].categories[0].id.clone();
        let item = Item::new("x", "x", ItemKind::File, "C:\\x.txt");
        assert!(library.add_item_to_category(&target, item));
        assert_eq!(library.groups[1].categories[0].items.len(), 1);
        assert_eq!(library.groups[0].categories[0].items.len(), 0);
        assert!(
            !library
                .add_item_to_category("missing", Item::new("y", "y", ItemKind::File, "C:\\y.txt"))
        );
    }

    #[test]
    fn moving_an_item_to_an_index_reorders_and_clamps() {
        let mut library = Library::default();
        let group = library.add_group("Sort", new_uuid()).expect("group");
        library.add_category(&group, "Cat", new_uuid());
        for (id, name) in [("a", "A"), ("b", "B"), ("c", "C")] {
            assert!(library.add_item(Item::new(id, name, ItemKind::File, "C:\\x.txt")));
        }
        assert!(library.move_item_to_index("c", 0));
        assert_eq!(
            library
                .active_category()
                .expect("category")
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["C", "A", "B"]
        );
        assert!(
            library.move_item_to_index("c", 99),
            "clamps to the last slot"
        );
        assert_eq!(
            library
                .active_category()
                .expect("category")
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B", "C"]
        );
        assert!(!library.move_item_to_index("missing", 0));
        assert!(!library.move_item_to_index("b", 1), "same slot is a no-op");
    }

    #[test]
    fn moving_an_item_across_groups_appends_to_the_target() {
        let mut library = Library::default();
        let group_a = library.add_group("A", new_uuid()).expect("group a");
        let group_b = library.add_group("B", new_uuid()).expect("group b");
        library.add_category(&group_a, "CatA", "cat-a");
        library.add_category(&group_b, "CatB1", "cat-b1");
        library.add_category(&group_b, "CatB2", "cat-b2");
        assert!(library.select_group(&group_a), "items go into group A");
        assert!(library.add_item(Item::new("m", "Move", ItemKind::File, "C:\\m.txt")));
        assert!(library.add_item(Item::new("k", "Keep", ItemKind::File, "C:\\k.txt")));

        assert_eq!(
            library.category_id_of_item("m").as_deref(),
            Some("cat-a"),
            "the item's category is found across groups"
        );
        assert!(library.move_item_across_groups("m", "cat-b1"));
        assert_eq!(
            library.groups[0].categories[0]
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["k"],
            "the source category loses the item"
        );
        assert_eq!(
            library.groups[1].categories[0]
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["m"],
            "the target category receives it"
        );
        assert!(
            !library.move_item_across_groups("m", "cat-b1"),
            "moving into the current category is a no-op"
        );
        assert!(
            library.move_item_across_groups("m", "cat-b2"),
            "a second target works"
        );
        assert_eq!(library.groups[1].categories[1].items.len(), 1);
        assert!(!library.move_item_across_groups("missing", "cat-a"));
        assert!(!library.move_item_across_groups("k", "no-such-cat"));
        assert_eq!(
            library.groups[0].categories[0].items.len(),
            1,
            "failed moves keep the library untouched"
        );
    }
}

#[cfg(test)]
mod combination_tests {
    use super::*;

    fn seeded() -> Library {
        let mut library = Library::default();
        let mut group = Group::new("g1", "Alpha");
        let mut category = Category::new("c1", "Main");
        category.items.push(Item::new(
            "i1",
            "Notepad",
            ItemKind::Exe,
            "C:/Windows/System32/notepad.exe",
        ));
        category.items.push(Item::new(
            "i2",
            "Calc",
            ItemKind::Exe,
            "C:/Windows/System32/calc.exe",
        ));
        group.categories.push(category);
        library.groups.push(group);
        library.current_group_id = Some("g1".to_string());
        library
    }

    // P3-02: an added combination persists with a fresh id, a trimmed name
    // and the given items; an empty name falls back to "组合 N".
    #[test]
    fn add_combination_trims_and_defaults_the_name() {
        let mut library = seeded();
        let first = library.add_combination("  工作  ", vec!["i1".to_string()]);
        assert_eq!(first.name, "工作");
        assert_eq!(first.item_ids, vec!["i1".to_string()]);
        assert!(library.combination(&first.id).is_some());

        let second = library.add_combination("   ", vec!["i2".to_string()]);
        // The fallback is numbered by the existing count, which stays unique.
        assert_eq!(second.name, "组合 2");
    }

    // P3-02: a removed combination is gone; removing an unknown id is a no-op
    // that reports false.
    #[test]
    fn remove_combination_reports_whether_it_existed() {
        let mut library = seeded();
        let combination = library.add_combination("工作", vec!["i1".to_string()]);
        assert!(library.remove_combination(&combination.id));
        assert!(library.combination(&combination.id).is_none());
        assert!(!library.remove_combination(&combination.id));
    }

    // P3-02: combinations round-trip through the configuration format, so a
    // saved combination survives a restart.
    #[test]
    fn combinations_round_trip_through_serialization() {
        let mut library = seeded();
        library.add_combination("工作", vec!["i1".to_string(), "i2".to_string()]);
        let json = serde_json::to_string(&library).expect("serialize");
        let parsed: Library = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.combinations.len(), 1);
        assert_eq!(parsed.combinations[0].name, "工作");
        assert_eq!(
            parsed.combinations[0].item_ids,
            vec!["i1".to_string(), "i2".to_string()]
        );
    }

    // P3-02: a configuration written before combinations existed loads with
    // an empty list (serde default), keeping old configs compatible.
    #[test]
    fn legacy_configs_load_without_combinations() {
        let legacy = r##"{"schema_version":2,"groups":[]}"##;
        let parsed: Library = serde_json::from_str(legacy).expect("deserialize");
        assert!(parsed.combinations.is_empty());
    }
}

#[cfg(test)]
mod delay_tests {
    use super::*;

    // P3-03: the delay defaults to zero (immediate) and round-trips through
    // the configuration format.
    #[test]
    fn delay_defaults_to_zero_and_round_trips() {
        let mut item = Item::new(
            "i1",
            "Delayed",
            ItemKind::Exe,
            "C:/Windows/System32/notepad.exe",
        );
        assert_eq!(item.launch.delay_seconds, 0, "no delay by default");
        item.launch.delay_seconds = 3;
        let json = serde_json::to_string(&item).expect("serialize");
        let parsed: Item = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.launch.delay_seconds, 3);
    }

    // P3-03: a configuration written before the delay existed loads with
    // zero delay (serde default).
    #[test]
    fn legacy_items_load_without_delay() {
        let legacy = r##"{"id":"i1","name":"N","item_type":"exe","window_state":"normal","launch":{"path":"C:/a.exe"}}"##;
        let parsed: Item = serde_json::from_str(legacy).expect("deserialize");
        assert_eq!(parsed.launch.delay_seconds, 0);
    }
}

#[cfg(test)]
mod hot_zone_tests {
    use super::*;

    // P3-04: the hot zone is opt-in (disabled by default) and the edge
    // normalizes to one of the four supported values.
    #[test]
    fn hot_zone_defaults_to_disabled_left() {
        let settings = Settings::default();
        assert!(!settings.hot_zone_enabled, "hot zone is opt-in");
        assert_eq!(settings.hot_zone_edge, "left");
    }

    #[test]
    fn hot_zone_edge_normalizes_unknown_values() {
        assert_eq!(normalize_hot_zone_edge("right"), "right");
        assert_eq!(normalize_hot_zone_edge("TOP"), "top");
        assert_eq!(normalize_hot_zone_edge(" bottom "), "bottom");
        assert_eq!(normalize_hot_zone_edge("diagonal"), "left");
    }

    // P3-04: the settings round-trip through the configuration format, so the
    // opt-in state survives a restart.
    #[test]
    fn hot_zone_settings_round_trip() {
        let mut library = Library::default();
        library.settings.hot_zone_enabled = true;
        library.settings.hot_zone_edge = "right".to_string();
        let json = serde_json::to_string(&library).expect("serialize");
        let parsed: Library = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.settings.hot_zone_enabled);
        assert_eq!(parsed.settings.hot_zone_edge, "right");
    }
}

#[cfg(test)]
mod sort_mode_tests {
    use super::*;

    fn ids() -> Vec<String> {
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    }

    fn items() -> Vec<UsageSnapshot> {
        vec![
            UsageSnapshot {
                id: "a".to_string(),
                name: "Alpha".to_string(),
                last_used: 100,
                launch_count: 1,
                favorite: false,
            },
            UsageSnapshot {
                id: "b".to_string(),
                name: "Beta".to_string(),
                last_used: 300,
                launch_count: 3,
                favorite: false,
            },
            UsageSnapshot {
                id: "c".to_string(),
                name: "Gamma".to_string(),
                last_used: 200,
                launch_count: 2,
                favorite: true,
            },
        ]
    }

    // P3-05: recent sorts by last_used, newest first.
    #[test]
    fn recent_sorts_by_last_used_descending() {
        let sorted = sort_item_ids_by_mode(&ids(), &items(), "recent");
        assert_eq!(
            sorted,
            vec!["b".to_string(), "c".to_string(), "a".to_string()]
        );
    }

    // P3-06: frequent sorts by launch count, highest first.
    #[test]
    fn frequent_sorts_by_launch_count_descending() {
        let sorted = sort_item_ids_by_mode(&ids(), &items(), "frequent");
        assert_eq!(
            sorted,
            vec!["b".to_string(), "c".to_string(), "a".to_string()]
        );
    }

    // P3-07: favorite sorts to the top, ties keep the manual order.
    #[test]
    fn favorite_sorts_to_the_top() {
        let sorted = sort_item_ids_by_mode(&ids(), &items(), "favorite");
        assert_eq!(
            sorted,
            vec!["c".to_string(), "a".to_string(), "b".to_string()]
        );
    }

    // Manual keeps the input order; unknown modes fall back to manual.
    #[test]
    fn manual_and_unknown_keep_the_input_order() {
        assert_eq!(sort_item_ids_by_mode(&ids(), &items(), "manual"), ids());
        assert_eq!(sort_item_ids_by_mode(&ids(), &items(), "nonsense"), ids());
    }

    // P3-05/P3-06/P3-07: the sort mode normalizes and round-trips.
    #[test]
    fn sort_mode_normalizes_and_round_trips() {
        assert_eq!(normalize_sort_mode("recent"), "recent");
        assert_eq!(normalize_sort_mode("FREQUENT"), "frequent");
        assert_eq!(normalize_sort_mode("favorite"), "favorite");
        assert_eq!(normalize_sort_mode("other"), "manual");
        let mut library = Library::default();
        library.settings.sort_mode = "recent".to_string();
        let json = serde_json::to_string(&library).expect("serialize");
        let parsed: Library = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.settings.sort_mode, "recent");
    }

    // P3-05/P3-06/P3-07: usage fields default to zero/false on legacy items.
    #[test]
    fn legacy_items_load_without_usage_state() {
        let legacy = r##"{"id":"i1","name":"N","item_type":"exe","window_state":"normal","launch":{"path":"C:/a.exe"}}"##;
        let parsed: Item = serde_json::from_str(legacy).expect("deserialize");
        assert_eq!(parsed.last_used, 0);
        assert_eq!(parsed.launch_count, 0);
        assert!(!parsed.favorite);
    }
}

#[cfg(test)]
mod execution_mode_tests {
    use super::*;

    // P3-08: the execution mode normalizes; unknown values fall back to
    // direct (baseline 8.2 Direct is the default).
    #[test]
    fn execution_mode_normalizes_unknown_to_direct() {
        assert_eq!(normalize_execution_mode(""), "direct");
        assert_eq!(normalize_execution_mode("direct"), "direct");
        assert_eq!(normalize_execution_mode("CMD"), "cmd");
        assert_eq!(normalize_execution_mode(" powershell "), "powershell");
        assert_eq!(normalize_execution_mode("nonsense"), "direct");
    }

    // P3-08: the mode round-trips through the persisted shape and legacy
    // items without the field load as direct.
    #[test]
    fn execution_mode_round_trips_and_legacy_loads_as_direct() {
        let json = r#"{"path":"C:/a.cmd","args":[],"execution_mode":"cmd"}"#;
        let parsed: Launch = serde_json::from_str(json).expect("deserialize");
        assert_eq!(normalize_execution_mode(&parsed.execution_mode), "cmd");
        let legacy: Launch = serde_json::from_str(r#"{"path":"C:/a.exe"}"#).expect("deserialize");
        assert_eq!(normalize_execution_mode(&legacy.execution_mode), "direct");
        let mut draft = ItemDraft::empty();
        draft.item_type = ItemKind::Command;
        draft.path = "C:/a.cmd".to_string();
        draft.execution_mode = "powershell".to_string();
        let item = draft.into_item("id1");
        assert_eq!(
            normalize_execution_mode(&item.launch.execution_mode),
            "powershell"
        );
    }
}

#[cfg(test)]
mod everything_tests {
    use super::*;

    // P3-09: the integration is opt-in (disabled by default) with the
    // standard install location as the default path.
    #[test]
    fn everything_defaults_to_disabled_standard_path() {
        let library = Library::default();
        assert!(!library.settings.everything_enabled);
        assert_eq!(
            library.settings.everything_path,
            "C:\\Program Files\\Everything\\Everything.exe"
        );
    }

    // P3-09: the settings round-trip and legacy configs without the fields
    // load disabled.
    #[test]
    fn everything_settings_round_trip_and_legacy_load() {
        let mut library = Library::default();
        library.settings.everything_enabled = true;
        library.settings.everything_path = "D:\\Tools\\Everything.exe".to_string();
        let json = serde_json::to_string(&library).expect("serialize");
        let parsed: Library = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.settings.everything_enabled);
        assert_eq!(parsed.settings.everything_path, "D:\\Tools\\Everything.exe");
        let legacy: Library =
            serde_json::from_str(r#"{"schema_version":2,"groups":[]}"#).expect("deserialize");
        assert!(!legacy.settings.everything_enabled);
        assert_eq!(
            legacy.settings.everything_path,
            "C:\\Program Files\\Everything\\Everything.exe"
        );
    }
}

#[cfg(test)]
mod card_ui_tests {
    use super::*;

    // 卡片规范 V1.0 §2: the default artwork follows the item type.
    #[test]
    fn default_artwork_follows_the_item_type() {
        let exe = Item::new("i1", "记事本", ItemKind::Exe, "C:/Windows/notepad.exe");
        assert_eq!(exe.default_artwork(), "app");
        let uri = Item::new("i2", "站点", ItemKind::Uri, "https://example.com");
        assert_eq!(uri.default_artwork(), "browser");
        let command = Item::new(
            "i3",
            "命令",
            ItemKind::Command,
            "C:/Windows/System32/cmd.exe",
        );
        assert_eq!(command.default_artwork(), "terminal");
        let folder = Item::new("i4", "目录", ItemKind::Folder, "C:/Windows");
        assert_eq!(folder.default_artwork(), "app");
    }

    // 卡片规范 V1.0 §5: the badge reason names the concrete cause; healthy
    // items have no reason at all.
    #[test]
    fn missing_reason_only_exists_for_broken_targets() {
        let broken = Item::new("i1", "失效", ItemKind::File, "C:/definitely/missing.txt");
        assert!(broken.path_missing());
        assert!(broken.missing_reason().contains("missing.txt"));
        let healthy = Item::new("i2", "正常", ItemKind::Uri, "https://example.com");
        assert!(!healthy.path_missing());
        assert_eq!(healthy.missing_reason(), "");
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    // P4-10（基线 29 章 数据迁移测试）：v1.1 时代的 config（schema_version=2，
    // 无 P3/P4 新增字段）加载后用户数据完整，新字段以默认值补齐。
    #[test]
    fn v1_1_config_migrates_with_user_data_intact() {
        // v1.1 形状：launch 无 delay_seconds/execution_mode/confirm_on_first_run，
        // item 无 last_used/launch_count/favorite，settings 无 sort_mode/
        // hot_zone/everything。
        let legacy = r##"{
            "schema_version": 2,
            "current_group_id": "g1",
            "settings": {
                "theme": "dark",
                "accent_color": "#16a34a",
                "pinned": true
            },
            "groups": [
                {
                    "id": "g1",
                    "name": "迁移前分组",
                    "color": "#3B82F6",
                    "last_category_id": "c1",
                    "categories": [
                        {
                            "id": "c1",
                            "name": "迁移前分类",
                            "items": [
                                {
                                    "id": "i1",
                                    "name": "记事本",
                                    "item_type": "exe",
                                    "launch": {
                                        "path": "C:\\Windows\\System32\\notepad.exe"
                                    },
                                    "window_state": "normal"
                                }
                            ]
                        }
                    ]
                }
            ]
        }"##;
        let library: Library = serde_json::from_str(legacy).expect("deserialize legacy");
        // 用户数据零丢失。
        assert_eq!(library.groups.len(), 1);
        assert_eq!(library.groups[0].name, "迁移前分组");
        assert_eq!(library.groups[0].categories[0].items[0].name, "记事本");
        assert_eq!(
            library.groups[0].categories[0].items[0].launch.path,
            "C:\\Windows\\System32\\notepad.exe"
        );
        assert_eq!(library.settings.theme, "dark");
        assert_eq!(library.settings.accent_color, "#16a34a");
        assert!(library.settings.pinned);
        // 新字段默认值补齐（P3/P4 逐项）。
        assert_eq!(library.schema_version, 2);
        assert_eq!(library.settings.sort_mode, "manual");
        assert!(!library.settings.hot_zone_enabled);
        assert!(!library.settings.everything_enabled);
        let item = &library.groups[0].categories[0].items[0];
        assert_eq!(item.launch.delay_seconds, 0);
        assert_eq!(item.launch.execution_mode, "direct");
        assert!(!item.launch.run_as_admin);
        assert_eq!(item.last_used, 0);
        assert_eq!(item.launch_count, 0);
        assert!(!item.favorite);
        // 迁移往返：再序列化仍可读，数据不丢。
        let json = serde_json::to_string(&library).expect("serialize");
        let reparsed: Library = serde_json::from_str(&json).expect("reparse");
        assert_eq!(reparsed.groups[0].categories[0].items[0].name, "记事本");
    }

    // P4-10：迁移后编辑保存不会破坏既有用户数据（load→mutate→serialize）。
    #[test]
    fn migrated_config_survives_a_mutate_serialize_round_trip() {
        let legacy = r##"{"schema_version":2,"current_group_id":"g1","groups":[{"id":"g1","name":"G","last_category_id":"c1","categories":[{"id":"c1","name":"C","items":[{"id":"i1","name":"N","item_type":"uri","launch":{"path":"https://example.com"},"window_state":"normal"}]}]}]}"##;
        let mut library: Library = serde_json::from_str(legacy).expect("deserialize");
        // 模拟更新链路里的一次使用：项目计数 +1。
        library.groups[0].categories[0].items[0].launch_count = 15;
        library.groups[0].categories[0].items[0].favorite = true;
        let json = serde_json::to_string(&library).expect("serialize");
        let reparsed: Library = serde_json::from_str(&json).expect("reparse");
        let item = &reparsed.groups[0].categories[0].items[0];
        assert_eq!(item.launch_count, 15);
        assert!(item.favorite);
        assert_eq!(item.launch.path, "https://example.com");
    }
}
