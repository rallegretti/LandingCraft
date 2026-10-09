; LandingCraft's Windows installer, made by packaging/windows/package.sh with Inno Setup 6
; (https://jrsoftware.org/isinfo.php):
;
;   ISCC /DVersion=x.y.z /DArch=x64 /DSourceDir=<folder with landingcraft.exe> /DOutputDir=<dir> landingcraft.iss
;
; It installs for the current user only, into %LOCALAPPDATA%\Programs\LandingCraft, so Windows doesn't
; ask for administrator approval. It adds a Start menu shortcut (and a desktop one if chosen) and an
; entry in Settings > Apps. A newer installer updates in place, closing a running launcher first.
; Uninstalling removes only the launcher: its settings and the Crafting Apps it installed stay.

#ifndef Version
  #error Pass the version with /DVersion=x.y.z
#endif

[Setup]
; Never change the AppId: it's how a newer installer finds the installed launcher.
AppId={{488B1218-157F-445D-AA05-901B4DFE1A02}
AppName=LandingCraft
AppVersion={#Version}
AppVerName=LandingCraft {#Version}
AppPublisherURL=https://github.com/rallegretti/LandingCraft
AppSupportURL=https://github.com/rallegretti/LandingCraft/issues
AppUpdatesURL=https://github.com/rallegretti/LandingCraft/releases
VersionInfoVersion={#Version}
VersionInfoDescription=LandingCraft Setup
DefaultDirName={autopf}\LandingCraft
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=landingcraft-{#Version}-windows-{#Arch}-setup
SetupIconFile={#SourcePath}\landingcraft.ico
UninstallDisplayIcon={app}\landingcraft.exe
UninstallDisplayName=LandingCraft
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: desktopicon; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\landingcraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\LandingCraft"; Filename: "{app}\landingcraft.exe"
Name: "{autodesktop}\LandingCraft"; Filename: "{app}\landingcraft.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\landingcraft.exe"; Description: "{cm:LaunchProgram,LandingCraft}"; Flags: nowait postinstall skipifsilent
