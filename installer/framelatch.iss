; FrameLatch installer (Inno Setup 7). Build with scripts\build-installer.ps1,
; which compiles the release executable and passes the version from Cargo.toml.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Keep AppId fixed: Windows uses it to recognise upgrades and the uninstaller.
AppId={{45F9B04F-4D94-4800-B75E-6CA2BA5E6B8D}
AppName=FrameLatch
AppVersion={#AppVersion}
AppVerName=FrameLatch {#AppVersion}
AppPublisher=FrameLatch
VersionInfoVersion={#AppVersion}
; Per-user install by default (no administrator prompt); the user may choose
; an all-users install in the wizard.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\FrameLatch
DefaultGroupName=FrameLatch
DisableProgramGroupPage=yes
; Windows Graphics Capture features used by the recorder need Windows 10 2004.
MinVersion=10.0.19041
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Some antivirus products block the icon resource update; the build script
; then retries with /DNoSetupIcon (the installed app keeps its own icon).
#ifndef NoSetupIcon
SetupIconFile=..\assets\framelatch.ico
#endif
UninstallDisplayIcon={app}\framelatch.exe
UninstallDisplayName=FrameLatch
WizardStyle=modern dynamic
OutputDir=..\dist
OutputBaseFilename=FrameLatch-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
; Close a running FrameLatch before replacing it during upgrades.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\framelatch.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\FrameLatch"; Filename: "{app}\framelatch.exe"
Name: "{autodesktop}\FrameLatch"; Filename: "{app}\framelatch.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\framelatch.exe"; Description: "{cm:LaunchProgram,FrameLatch}"; Flags: nowait postinstall skipifsilent

; Settings (%LOCALAPPDATA%\FrameLatch) and recordings are left in place on
; uninstall so a reinstall keeps the user's preferences and videos.
