; Inno Setup script for Ember IDE.
; Build:  iscc /DAppVersion=0.1.0 packaging\windows\installer.iss   (run from the repo root, after `cargo build --release`)

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{B7E1C6A2-5D3F-4E8A-9C21-3E4B5D6F7A81}
AppName=Ember IDE
AppVersion={#AppVersion}
AppPublisher=Ember IDE
DefaultDirName={autopf}\Ember IDE
DefaultGroupName=Ember IDE
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\ember.exe
SetupIconFile=..\..\assets\icon.ico
OutputDir=..\..\dist
OutputBaseFilename=Ember-IDE-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"
Name: "contextmenu"; Description: "Add ""Open with Ember IDE"" to the folder context menu"; GroupDescription: "Integration:"

[Files]
Source: "..\..\target\release\ember.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\plugins\*"; DestDir: "{app}\plugins"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Ember IDE"; Filename: "{app}\ember.exe"
Name: "{autodesktop}\Ember IDE"; Filename: "{app}\ember.exe"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\Directory\shell\EmberIDE"; ValueType: string; ValueName: ""; ValueData: "Open with Ember IDE"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\EmberIDE"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\ember.exe"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\EmberIDE\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ember.exe"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\EmberIDE"; ValueType: string; ValueName: ""; ValueData: "Open with Ember IDE"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\EmberIDE\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ember.exe"" ""%V"""; Tasks: contextmenu

[Run]
Filename: "{app}\ember.exe"; Description: "Launch Ember IDE"; Flags: nowait postinstall skipifsilent
