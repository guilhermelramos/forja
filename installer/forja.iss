; Instalador da Forja para Windows (Inno Setup 6).
; Gere com `cargo xtask installer`, que monta dist/ e chama o ISCC.exe.
; Instala por usuário (sem pedir administrador) em %LOCALAPPDATA%\Programs\Forja.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef StageDir
  #define StageDir "..\dist\forja-" + AppVersion + "-win-x64"
#endif

[Setup]
AppId={{85D7026D-8AD3-4EFA-BC4F-491D8BEBB624}
AppName=Forja
AppVersion={#AppVersion}
AppVerName=Forja {#AppVersion}
AppPublisher=Forja
DefaultDirName={autopf}\Forja
DefaultGroupName=Forja
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Forja-Setup-{#AppVersion}
SetupIconFile=..\design\assets\logo.ico
UninstallDisplayIcon={app}\forja-app.exe
LicenseFile={#StageDir}\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Forja"; Filename: "{app}\forja-app.exe"
Name: "{autodesktop}\Forja"; Filename: "{app}\forja-app.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\forja-app.exe"; Description: "{cm:LaunchProgram,Forja}"; Flags: nowait postinstall skipifsilent
