#define AppName "Mir2 Playtest"
#define AppVersion "2026.09.29.4"
#define AppExe "mir2-platform-windows.exe"

[Setup]
AppId={{DC4E7701-25AD-4E48-9970-5C1983CBC77F}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Mir2 Playtest
DefaultDirName={localappdata}\Programs\Mir2Invite
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..
OutputBaseFilename=Mir2-Invite-20260929-r4-Setup
VersionInfoVersion=2026.9.29.4
VersionInfoDescription=Mir2 multilingual playtest installer
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\game\{#AppExe}
UninstallFilesDir={app}\uninstall
WizardStyle=modern
WizardSizePercent=110
Compression=lzma2/normal
SolidCompression=yes
LZMANumBlockThreads=2
ExtraDiskSpaceRequired=67108864
CloseApplications=no
RestartApplications=no
SetupLogging=yes
ShowLanguageDialog=yes
LanguageDetectionMethod=uilanguage

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"; InfoBeforeFile: "README.en.txt"
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"; InfoBeforeFile: "README.zh-TW.txt"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"; InfoBeforeFile: "README.pt-BR.txt"

[CustomMessages]
english.DesktopIcon=Create a desktop shortcut
english.ShortcutGroup=Shortcuts:
english.ReadmeTitle=Installation and playtest guide
english.ReadmeFile=README.en.txt
english.UninstallTitle=Uninstall Mir2 Playtest
english.LaunchGame=Launch Mir2 Playtest
english.OpenReadme=Read the installation and playtest guide
english.RuntimePermission=Microsoft runtime installation could not start. Allow the administrator prompt for the runtime, then retry. Error:
english.RuntimeFailed=Microsoft runtime installation failed. Complete it before retrying. Exit code:
english.RuntimePending=The Microsoft runtime is not ready. Finish its installation and restart Windows if requested, then run this installer again.
chinesetraditional.DesktopIcon=建立桌面捷徑
chinesetraditional.ShortcutGroup=捷徑：
chinesetraditional.ReadmeTitle=安裝與試玩說明
chinesetraditional.ReadmeFile=README.zh-TW.txt
chinesetraditional.UninstallTitle=解除安裝 Mir2 Playtest
chinesetraditional.LaunchGame=啟動 Mir2 Playtest
chinesetraditional.OpenReadme=閱讀安裝與試玩說明
chinesetraditional.RuntimePermission=無法開始安裝微軟執行階段。請允許執行階段安裝所需的系統管理員權限，然後重試。錯誤：
chinesetraditional.RuntimeFailed=微軟執行階段安裝失敗。請完成安裝後重試。結束代碼：
chinesetraditional.RuntimePending=微軟執行階段尚未就緒。請完成安裝；若要求重新啟動 Windows，請重新啟動後再執行此安裝程式。
brazilianportuguese.DesktopIcon=Criar um atalho na área de trabalho
brazilianportuguese.ShortcutGroup=Atalhos:
brazilianportuguese.ReadmeTitle=Guia de instalação e teste
brazilianportuguese.ReadmeFile=README.pt-BR.txt
brazilianportuguese.UninstallTitle=Desinstalar Mir2 Playtest
brazilianportuguese.LaunchGame=Iniciar Mir2 Playtest
brazilianportuguese.OpenReadme=Ler o guia de instalação e teste
brazilianportuguese.RuntimePermission=Não foi possível iniciar a instalação do runtime Microsoft. Autorize a solicitação de administrador para o runtime e tente novamente. Erro:
brazilianportuguese.RuntimeFailed=A instalação do runtime Microsoft falhou. Conclua a instalação antes de tentar novamente. Código de saída:
brazilianportuguese.RuntimePending=O runtime Microsoft ainda não está pronto. Conclua a instalação e reinicie o Windows se solicitado. Depois, execute este instalador novamente.

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:ShortcutGroup}"

[Files]
Source: "tools\vc_redist.x64.exe"; Flags: dontcopy
#include "verified-payload-files.iss"
Source: "README.en.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.zh-TW.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.pt-BR.txt"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{app}\game\logs"

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\game\{#AppExe}"; WorkingDir: "{app}\game"
Name: "{group}\{cm:ReadmeTitle}"; Filename: "{app}\{cm:ReadmeFile}"
Name: "{group}\{cm:UninstallTitle}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\game\{#AppExe}"; WorkingDir: "{app}\game"; Tasks: desktopicon

[Run]
Filename: "{app}\game\{#AppExe}"; WorkingDir: "{app}\game"; Description: "{cm:LaunchGame}"; Flags: nowait postinstall skipifsilent; Check: CanLaunchGame
Filename: "{app}\{cm:ReadmeFile}"; Description: "{cm:OpenReadme}"; Flags: shellexec postinstall skipifsilent unchecked

[Code]
var
  RuntimeRestartRequired: Boolean;

function GetFileAttributesW(FileName: String): Cardinal;
  external 'GetFileAttributesW@kernel32.dll stdcall';
function MoveFileExW(ExistingName: String; NewName: String; Flags: Cardinal): Boolean;
  external 'MoveFileExW@kernel32.dll stdcall';

function VCRuntimeReady: Boolean;
var
  VersionMS, VersionLS: Cardinal;
begin
  Result := False;
  if GetVersionNumbers(ExpandConstant('{sys}\vcruntime140.dll'), VersionMS, VersionLS) then
    Result := (VersionMS > ((14 shl 16) + 44)) or
      ((VersionMS = ((14 shl 16) + 44)) and (VersionLS >= (35211 shl 16)));
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  if VCRuntimeReady then
  begin
    Log('Microsoft Visual C++ x64 runtime meets 14.44.35211.0 minimum; no runtime changes.');
    exit;
  end;
  ExtractTemporaryFile('vc_redist.x64.exe');
  if not ShellExec('runas', ExpandConstant('{tmp}\vc_redist.x64.exe'),
    '/install /passive /norestart', '', SW_SHOWNORMAL, ewWaitUntilTerminated, ResultCode) then
  begin
    Result := CustomMessage('RuntimePermission') + ' ' + SysErrorMessage(ResultCode);
    exit;
  end;
  Log('Microsoft Visual C++ runtime installer exit code: ' + IntToStr(ResultCode));
  if (ResultCode <> 0) and (ResultCode <> 3010) then
  begin
    Result := CustomMessage('RuntimeFailed') + ' ' + IntToStr(ResultCode);
    exit;
  end;
  RuntimeRestartRequired := ResultCode = 3010;
  if not VCRuntimeReady then
  begin
    NeedsRestart := RuntimeRestartRequired;
    Result := CustomMessage('RuntimePending');
  end;
end;

function NeedRestart: Boolean;
begin
  Result := RuntimeRestartRequired;
end;

function CanLaunchGame: Boolean;
begin
  Result := not RuntimeRestartRequired;
end;

function SafeLocalePath(Path: String): Boolean;
var
  Current, Parent: String;
  Attributes: Cardinal;
begin
  Result := False;
  Current := Path;
  while Current <> '' do
  begin
    Attributes := GetFileAttributesW(Current);
    if (Attributes <> $FFFFFFFF) and ((Attributes and $400) <> 0) then exit;
    Parent := ExtractFileDir(Current);
    if Parent = Current then break;
    Current := Parent;
  end;
  Result := True;
end;

procedure SeedInitialLocale;
var
  Directory, Preference, Seed, Temporary, Code: String;
begin
  Directory := ExpandConstant('{userappdata}\mir2-web3');
  Preference := AddBackslash(Directory) + 'locale.json';
  Seed := AddBackslash(Directory) + 'locale-seed.json';
  if FileExists(Preference) or FileExists(Seed) then exit;
  if not SafeLocalePath(Preference) or not SafeLocalePath(Seed) then
  begin
    Log('Locale seed skipped: unsafe path. The client can use its OS language.');
    exit;
  end;
  if not ForceDirectories(Directory) then
  begin
    Log('Locale seed skipped: preference directory unavailable.');
    exit;
  end;
  if not SafeLocalePath(Directory) then exit;
  Code := 'en';
  if ActiveLanguage = 'chinesetraditional' then Code := 'zh-TW';
  if ActiveLanguage = 'brazilianportuguese' then Code := 'pt-BR';
  Temporary := GetTempFileName(Directory);
  if Temporary = '' then exit;
  try
    if not SaveStringToFile(Temporary, '{"schema":1,"locale":"' + Code + '"}', False) then exit;
    if FileExists(Preference) or FileExists(Seed) then exit;
    if not SafeLocalePath(Seed) then exit;
    { WRITE_THROUGH, deliberately no REPLACE_EXISTING: never overwrite a preference/seed. }
    if not MoveFileExW(Temporary, Seed, 8) then
      Log('Locale seed was not saved; an existing preference is preserved.');
  finally
    if FileExists(Temporary) then DeleteFile(Temporary);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then SeedInitialLocale;
end;
