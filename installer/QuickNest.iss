; QuickNest 安装包脚本（基线 23 章 P4-01 / 29 章 P4-01）。
;
; 标准 Inno Setup 向导：开始菜单快捷方式、可选桌面快捷方式、可选开机启动、
; 卸载程序（自动写入"应用和功能"/ARP）、安装后普通权限启动。
;
; PrivilegesRequired=lowest → 逐用户安装（默认 %LOCALAPPDATA%\Programs\QuickNest，
; 全程不需要管理员权限，UAC 不弹窗）；卸载信息也写在 HKCU（基线 23 章卸载程序 +
; "安装后普通权限启动"）。版本与冻结输入由离线发布包装器传入。

#ifndef MyAppVersion
  #error "Use scripts/build-release.ps1: MyAppVersion is required from locked offline Cargo metadata"
#endif
#ifndef ReleaseStageDir
  #error "Use scripts/build-release.ps1: ReleaseStageDir is required"
#endif
#ifndef ReleaseOutputDir
  #error "Use scripts/build-release.ps1: ReleaseOutputDir is required"
#endif
#define MyAppName "QuickNest"
#define MyAppPublisher "QuickNest"
#define MyAppExeName "quicknest.exe"

[Setup]
AppId={{8F6A2E52-3B4C-4E9A-9F1D-8C2E51B04A7D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#ReleaseOutputDir}
OutputBaseFilename=QuickNest-{#MyAppVersion}-setup
SetupIconFile={#ReleaseStageDir}\QuickNest.ico
UninstallDisplayName={#MyAppName} {#MyAppVersion}
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
ChangesEnvironment=no

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
; 可选桌面快捷方式（基线 23 章"可选桌面快捷方式"）。
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; \
    GroupDescription: "{cm:AdditionalIcons}"
; 可选开机启动（基线 23 章"开机启动"）——默认不勾选，用户显式选择。
Name: "autostart"; Description: "开机自动启动 QuickNest"; \
    GroupDescription: "其他选项："; Flags: unchecked

[Files]
Source: "{#ReleaseStageDir}\quicknest.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ReleaseStageDir}\QuickNest.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ReleaseStageDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; \
    IconFilename: "{app}\QuickNest.ico"
Name: "{group}\卸载 {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; \
    IconFilename: "{app}\QuickNest.ico"; Tasks: desktopicon

[Registry]
; 开机启动（基线 23 章）：HKCU 当前用户 Run 键；卸载时删除该值。
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "QuickNest"; \
    ValueData: """{app}\{#MyAppExeName}"""; \
    Flags: uninsdeletevalue; Tasks: autostart

[Run]
; 安装完成后可选立即启动：runasoriginaluser 使用 Inno 保存的原始用户上下文。
; 手动以管理员身份启动安装器时不保证存在 Medium 上下文；该场景仍需实测。
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; \
    Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
; 图标文件由安装器复制，卸载时一并清理。
Type: files; Name: "{app}\QuickNest.ico"
