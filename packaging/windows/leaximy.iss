; Leaximy's Windows installer, built with Inno Setup 6 from a release binary.
; The GitHub workflow .github/workflows/leaximy-windows.yml runs it:
;
;   iscc /DVersion=0.11.2 /DArch=x86_64 /DBinary=...\leaximy.exe ^
;        /DOutputDir=dist packaging\windows\leaximy.iss
;
; It needs no administrator rights: the program goes to the user's own
; Programs folder with a Start menu entry, and a running copy is closed
; before an update replaces it. The wizard speaks Turkish or English.

#ifndef Version
  #error Version must be defined on the ISCC command line
#endif
#ifndef Arch
  #define Arch "x86_64"
#endif
#ifndef Binary
  #error Binary must be defined on the ISCC command line
#endif
#ifndef OutputDir
  #define OutputDir "dist"
#endif
#if Arch == "aarch64"
  #define InnoArch "arm64"
#else
  #define InnoArch "x64compatible"
#endif

#define AppName "Leaximy"
#define AppExeName "leaximy.exe"
#define AppIdentity "Leaximy"

[Setup]
; Never change: this is how Windows tells an update from a new program.
; It differs from Spotifast's, so both can be installed side by side.
AppId={{1DC4306B-B3F2-49BC-97DC-E05212E3DC5F}
AppName={#AppName}
AppVersion={#Version}
AppVerName={#AppName} {#Version}
AppPublisher=Leaximy
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed={#InnoArch}
ArchitecturesInstallIn64BitMode={#InnoArch}
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=Leaximy-{#Version}-Setup
SetupIconFile=spotifast.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
ShowLanguageDialog=auto
; The file version has to be numbers: a release candidate's -rc1 comes off.
#define Dash Pos("-", Version)
#if Dash > 0
  #define NumericVersion Copy(Version, 1, Dash - 1)
#else
  #define NumericVersion Version
#endif
VersionInfoVersion={#NumericVersion}.0
VersionInfoProductName={#AppName}

[Languages]
Name: "tr"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
tr.DesktopIcon=Masaüstüne kısayol oluştur
tr.Shortcuts=Ek kısayollar:
tr.SpotifyLinks=Spotify bağlantılarını (spotify:...) Leaximy ile aç
tr.Integration=Bağlantılar:
tr.Launch=Leaximy'yi başlat
en.DesktopIcon=Create a desktop shortcut
en.Shortcuts=Additional shortcuts:
en.SpotifyLinks=Open Spotify links (spotify:...) in Leaximy
en.Integration=Links:
en.Launch=Launch Leaximy

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:Shortcuts}"
; Off by default: it would take the links from the official Spotify app.
Name: "spotifylinks"; Description: "{cm:SpotifyLinks}"; GroupDescription: "{cm:Integration}"; Flags: unchecked

[Files]
Source: "{#Binary}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LEAXIMY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; Tells the in-app updater this copy came from the setup program, so an
; update runs the new setup silently over it.
Source: "leaximy-installer.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\spotify"; ValueType: string; ValueName: ""; ValueData: "URL:Spotify link"; Tasks: spotifylinks
Root: HKCU; Subkey: "Software\Classes\spotify"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Tasks: spotifylinks
Root: HKCU; Subkey: "Software\Classes\spotify\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"",0"; Tasks: spotifylinks
Root: HKCU; Subkey: "Software\Classes\spotify\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Tasks: spotifylinks
; Listed under Settings > Apps > Default apps either way, so links can be
; handed to Leaximy later without reinstalling.
Root: HKCU; Subkey: "Software\Classes\Leaximy.spotify"; ValueType: string; ValueName: ""; ValueData: "URL:Spotify link"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Leaximy.spotify"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Leaximy.spotify\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"",0"
Root: HKCU; Subkey: "Software\Classes\Leaximy.spotify\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Spotify client"
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities\URLAssociations"; ValueType: string; ValueName: "spotify"; ValueData: "Leaximy.spotify"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppIdentity}"; ValueData: "Software\{#AppIdentity}\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:Launch}"; Flags: nowait postinstall skipifsilent

[Code]
// The spotify: scheme key is shared with whatever else opens the links, so
// uninstalling takes it away only while it still names this program.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
  Exe: String;
begin
  if CurUninstallStep <> usUninstall then
    Exit;
  if not RegQueryStringValue(HKCU, 'Software\Classes\spotify\shell\open\command', '', Command) then
    Exit;
  Exe := Lowercase(ExpandConstant('{app}\{#AppExeName}'));
  if Pos(Exe, Lowercase(Command)) > 0 then
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\spotify');
end;
