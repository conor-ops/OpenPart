[Setup]
AppName=OpenPart
AppVersion={#AppVersion}
AppPublisher=OpenPart
AppPublisherURL=https://github.com/mahmadabid/OpenPart
AppSupportURL=https://github.com/mahmadabid/OpenPart
AppUpdatesURL=https://github.com/mahmadabid/OpenPart
DefaultDirName={pf}\OpenPart
DefaultGroupName=OpenPart
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\
OutputBaseFilename=OpenPartSetup
SetupIconFile=..\src-tauri\icons\icon.ico
Compression=lzma
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
PrivilegesRequired=admin

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\OpenPart-v{#AppVersion}-windows\OpenPart.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\OpenPart-v{#AppVersion}-windows\OpenPartCLI.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\OpenPart-v{#AppVersion}-windows\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\OpenPart-v{#AppVersion}-windows\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\OpenPart"; Filename: "{app}\OpenPart.exe"
Name: "{group}\OpenPart CLI"; Filename: "{app}\OpenPartCLI.exe"
Name: "{autodesktop}\OpenPart"; Filename: "{app}\OpenPart.exe"; Tasks: desktopicon
