#define AppName "Numeron - Legend of Rebirth"
#define AppVersion "2026.10.01.8"
#define AppExe "mir2-platform-windows.exe"
#define LauncherExe "Mir2Launcher.exe"

[Setup]
AppId={{DC4E7701-25AD-4E48-9970-5C1983CBC77F}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Numeron - Legend of Rebirth
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
OutputBaseFilename=Numeron-Legend-of-Rebirth-20261001-r8-Setup
VersionInfoVersion=2026.10.1.8
VersionInfoDescription=Numeron - Legend of Rebirth multilingual playtest installer
UninstallDisplayName={#AppName}
#ifdef Mir2Bootstrap
UninstallDisplayIcon={app}\{#LauncherExe}
#else
UninstallDisplayIcon={app}\game\{#AppExe}
#endif
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
; Public signing is opt-in and must be configured by the release operator.
; No certificate is selected from a local store by this recipe.
#ifdef Mir2PublicSign
  #ifndef Mir2PublicSignerThumbprint
    #error Mir2PublicSignerThumbprint is required with Mir2PublicSign
  #endif
SignTool=mir2Public
SignedUninstaller=yes
#else
SignedUninstaller=no
#endif
ShowLanguageDialog=yes
LanguageDetectionMethod=uilanguage

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"; InfoBeforeFile: "README.en.txt"
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"; InfoBeforeFile: "README.zh-TW.txt"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"; InfoBeforeFile: "README.pt-BR.txt"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"; InfoBeforeFile: "README.ru.txt"
Name: "hindi"; MessagesFile: "languages\Hindi.isl"; InfoBeforeFile: "README.hi.txt"
Name: "indonesian"; MessagesFile: "languages\Indonesian.isl"; InfoBeforeFile: "README.id.txt"
Name: "vietnamese"; MessagesFile: "languages\Vietnamese.isl"; InfoBeforeFile: "README.vi.txt"
Name: "thai"; MessagesFile: "compiler:Languages\Thai.isl"; InfoBeforeFile: "README.th.txt"
Name: "arabic"; MessagesFile: "compiler:Languages\Arabic.isl"; InfoBeforeFile: "README.ar.txt"

[CustomMessages]
english.DesktopIcon=Create a desktop shortcut
english.ShortcutGroup=Shortcuts:
english.ReadmeTitle=Installation and playtest guide
english.UninstallTitle=Uninstall Numeron - Legend of Rebirth
english.LaunchGame=Launch Numeron - Legend of Rebirth
english.OpenReadme=Read the installation and playtest guide
english.RuntimePermission=Microsoft runtime installation could not start. Allow the administrator prompt for the runtime, then retry. Error:
english.RuntimeFailed=Microsoft runtime installation failed. Complete it before retrying. Exit code:
english.RuntimePending=The Microsoft runtime is not ready. Finish its installation and restart Windows if requested, then run this installer again.
english.LaunchBlocked=Windows application protection prevented the game from starting (error %1). Installation has completed. Keep protection enabled; ask the developer for a trusted signed build or ask your organization administrator to review the policy. Send the error code and installer log to support.
english.LaunchFailed=The game could not start (Windows error %1). Installation has completed. Send this error code and the installer log to support.
english.LaunchLog=Launch diagnostics: %1
english.ReadmeFailed=The guide could not be opened (Windows error %1). Open the installed README manually.
english.ReadmeFile=README.en.txt
chinesetraditional.DesktopIcon=建立桌面捷徑
chinesetraditional.ShortcutGroup=捷徑：
chinesetraditional.ReadmeTitle=安裝與試玩說明
chinesetraditional.UninstallTitle=解除安裝 Numeron - Legend of Rebirth
chinesetraditional.LaunchGame=啟動 Numeron - Legend of Rebirth
chinesetraditional.OpenReadme=閱讀安裝與試玩說明
chinesetraditional.RuntimePermission=無法開始安裝微軟執行階段。請允許該元件的系統管理員提示後重試。錯誤：
chinesetraditional.RuntimeFailed=微軟執行階段安裝失敗。請完成安裝後重試。結束代碼：
chinesetraditional.RuntimePending=微軟執行階段尚未就緒。請完成安裝；如需重新啟動 Windows，請重新啟動後再執行安裝程式。
chinesetraditional.LaunchBlocked=Windows 應用程式保護阻止遊戲啟動（錯誤 %1）。安裝已完成。請保持保護啟用，向開發者索取受信任簽章版本，或請組織管理員檢查原則。回報時請附錯誤碼與安裝記錄。
chinesetraditional.LaunchFailed=遊戲無法啟動（Windows 錯誤 %1）。安裝已完成，請向支援人員提供錯誤碼與安裝記錄。
chinesetraditional.LaunchLog=啟動診斷記錄：%1
chinesetraditional.ReadmeFailed=無法開啟說明（Windows 錯誤 %1）。請手動開啟已安裝的 README。
chinesetraditional.ReadmeFile=README.zh-TW.txt
brazilianportuguese.DesktopIcon=Criar um atalho na área de trabalho
brazilianportuguese.ShortcutGroup=Atalhos:
brazilianportuguese.ReadmeTitle=Guia de instalação e teste
brazilianportuguese.UninstallTitle=Desinstalar Numeron - Legend of Rebirth
brazilianportuguese.LaunchGame=Iniciar Numeron - Legend of Rebirth
brazilianportuguese.OpenReadme=Ler o guia de instalação e teste
brazilianportuguese.RuntimePermission=Não foi possível iniciar a instalação do runtime Microsoft. Autorize a solicitação de administrador desse componente e tente novamente. Erro:
brazilianportuguese.RuntimeFailed=A instalação do runtime Microsoft falhou. Conclua a instalação antes de tentar novamente. Código de saída:
brazilianportuguese.RuntimePending=O runtime Microsoft ainda não está pronto. Conclua a instalação e reinicie o Windows se solicitado. Depois, execute este instalador novamente.
brazilianportuguese.LaunchBlocked=A proteção de aplicativos do Windows impediu o início do jogo (erro %1). A instalação foi concluída. Mantenha a proteção ativada; solicite ao desenvolvedor uma versão com assinatura confiável ou peça ao administrador da organização que revise a política. Envie o código e o registro da instalação ao suporte.
brazilianportuguese.LaunchFailed=Não foi possível iniciar o jogo (erro do Windows %1). A instalação foi concluída. Envie o código e o registro da instalação ao suporte.
brazilianportuguese.LaunchLog=Diagnóstico de inicialização: %1
brazilianportuguese.ReadmeFailed=Não foi possível abrir o guia (erro do Windows %1). Abra o README instalado manualmente.
brazilianportuguese.ReadmeFile=README.pt-BR.txt
russian.DesktopIcon=Создать ярлык на рабочем столе
russian.ShortcutGroup=Ярлыки:
russian.ReadmeTitle=Руководство по установке и тестированию
russian.UninstallTitle=Удалить Numeron - Legend of Rebirth
russian.LaunchGame=Запустить Numeron - Legend of Rebirth
russian.OpenReadme=Прочитать руководство по установке и тестированию
russian.RuntimePermission=Не удалось запустить установку среды Microsoft. Разрешите запрос администратора для этого компонента и повторите попытку. Ошибка:
russian.RuntimeFailed=Установка среды Microsoft завершилась ошибкой. Завершите установку и повторите попытку. Код завершения:
russian.RuntimePending=Среда Microsoft ещё не готова. Завершите установку, при необходимости перезапустите Windows и запустите установщик снова.
russian.LaunchBlocked=Защита приложений Windows не разрешила запуск игры (ошибка %1). Установка завершена. Не отключайте защиту: запросите у разработчика сборку с доверенной подписью или попросите администратора организации проверить политику. Передайте в поддержку код ошибки и журнал установки.
russian.LaunchFailed=Не удалось запустить игру (ошибка Windows %1). Установка завершена. Передайте в поддержку код ошибки и журнал установки.
russian.LaunchLog=Диагностика запуска: %1
russian.ReadmeFailed=Не удалось открыть руководство (ошибка Windows %1). Откройте установленный README вручную.
russian.ReadmeFile=README.ru.txt
hindi.DesktopIcon=डेस्कटॉप पर शॉर्टकट बनाएँ
hindi.ShortcutGroup=शॉर्टकट:
hindi.ReadmeTitle=स्थापना और परीक्षण मार्गदर्शिका
hindi.UninstallTitle=Numeron - Legend of Rebirth हटाएँ
hindi.LaunchGame=Numeron - Legend of Rebirth शुरू करें
hindi.OpenReadme=स्थापना और परीक्षण मार्गदर्शिका पढ़ें
hindi.RuntimePermission=Microsoft रनटाइम की स्थापना शुरू नहीं हो सकी। इस घटक के लिए व्यवस्थापक अनुमति दें और फिर प्रयास करें। त्रुटि:
hindi.RuntimeFailed=Microsoft रनटाइम की स्थापना विफल हुई। स्थापना पूरी करके फिर प्रयास करें। निकास कोड:
hindi.RuntimePending=Microsoft रनटाइम अभी तैयार नहीं है। स्थापना पूरी करें, आवश्यकता होने पर Windows पुनः प्रारंभ करें और यह स्थापना कार्यक्रम फिर चलाएँ।
hindi.LaunchBlocked=Windows की ऐप सुरक्षा ने खेल शुरू होने से रोका (त्रुटि %1)। स्थापना पूरी हो गई है। सुरक्षा चालू रखें; विकासकर्ता से विश्वसनीय हस्ताक्षर वाला संस्करण माँगें या अपने संगठन के व्यवस्थापक से नीति की जाँच कराएँ। सहायता के लिए त्रुटि कोड और स्थापना लॉग भेजें।
hindi.LaunchFailed=खेल शुरू नहीं हो सका (Windows त्रुटि %1)। स्थापना पूरी हो गई है। सहायता के लिए त्रुटि कोड और स्थापना लॉग भेजें।
hindi.LaunchLog=प्रारंभ संबंधी जाँच लॉग: %1
hindi.ReadmeFailed=मार्गदर्शिका नहीं खुल सकी (Windows त्रुटि %1)। स्थापित README को स्वयं खोलें।
hindi.ReadmeFile=README.hi.txt
indonesian.DesktopIcon=Buat pintasan desktop
indonesian.ShortcutGroup=Pintasan:
indonesian.ReadmeTitle=Panduan instalasi dan uji coba
indonesian.UninstallTitle=Hapus Numeron - Legend of Rebirth
indonesian.LaunchGame=Jalankan Numeron - Legend of Rebirth
indonesian.OpenReadme=Baca panduan instalasi dan uji coba
indonesian.RuntimePermission=Instalasi runtime Microsoft tidak dapat dimulai. Izinkan permintaan administrator untuk komponen tersebut, lalu coba lagi. Kesalahan:
indonesian.RuntimeFailed=Instalasi runtime Microsoft gagal. Selesaikan instalasinya sebelum mencoba lagi. Kode keluar:
indonesian.RuntimePending=Runtime Microsoft belum siap. Selesaikan instalasi dan mulai ulang Windows jika diminta, lalu jalankan penginstal ini lagi.
indonesian.LaunchBlocked=Perlindungan aplikasi Windows mencegah game dimulai (kesalahan %1). Instalasi telah selesai. Biarkan perlindungan aktif; minta versi bertanda tangan tepercaya dari pengembang atau minta administrator organisasi meninjau kebijakan. Kirim kode kesalahan dan log instalasi ke dukungan.
indonesian.LaunchFailed=Game tidak dapat dimulai (kesalahan Windows %1). Instalasi telah selesai. Kirim kode kesalahan dan log instalasi ke dukungan.
indonesian.LaunchLog=Diagnostik peluncuran: %1
indonesian.ReadmeFailed=Panduan tidak dapat dibuka (kesalahan Windows %1). Buka README yang terpasang secara manual.
indonesian.ReadmeFile=README.id.txt
vietnamese.DesktopIcon=Tạo lối tắt trên màn hình nền
vietnamese.ShortcutGroup=Lối tắt:
vietnamese.ReadmeTitle=Hướng dẫn cài đặt và chơi thử
vietnamese.UninstallTitle=Gỡ cài đặt Numeron - Legend of Rebirth
vietnamese.LaunchGame=Khởi chạy Numeron - Legend of Rebirth
vietnamese.OpenReadme=Đọc hướng dẫn cài đặt và chơi thử
vietnamese.RuntimePermission=Không thể bắt đầu cài bộ chạy Microsoft. Cho phép yêu cầu quyền quản trị của thành phần này rồi thử lại. Lỗi:
vietnamese.RuntimeFailed=Cài bộ chạy Microsoft thất bại. Hoàn tất cài đặt trước khi thử lại. Mã thoát:
vietnamese.RuntimePending=Bộ chạy Microsoft chưa sẵn sàng. Hoàn tất cài đặt và khởi động lại Windows nếu được yêu cầu, rồi chạy lại trình cài đặt này.
vietnamese.LaunchBlocked=Tính năng bảo vệ ứng dụng của Windows đã chặn khởi chạy trò chơi (lỗi %1). Cài đặt đã hoàn tất. Hãy giữ nguyên bảo vệ; yêu cầu nhà phát triển cung cấp bản có chữ ký đáng tin cậy hoặc nhờ quản trị viên tổ chức kiểm tra chính sách. Gửi mã lỗi và nhật ký cài đặt cho bộ phận hỗ trợ.
vietnamese.LaunchFailed=Không thể khởi chạy trò chơi (lỗi Windows %1). Cài đặt đã hoàn tất. Gửi mã lỗi và nhật ký cài đặt cho bộ phận hỗ trợ.
vietnamese.LaunchLog=Nhật ký chẩn đoán khởi chạy: %1
vietnamese.ReadmeFailed=Không thể mở hướng dẫn (lỗi Windows %1). Hãy tự mở tệp README đã cài đặt.
vietnamese.ReadmeFile=README.vi.txt
thai.DesktopIcon=สร้างทางลัดบนเดสก์ท็อป
thai.ShortcutGroup=ทางลัด:
thai.ReadmeTitle=คู่มือการติดตั้งและทดลองเล่น
thai.UninstallTitle=ถอนการติดตั้ง Numeron - Legend of Rebirth
thai.LaunchGame=เริ่ม Numeron - Legend of Rebirth
thai.OpenReadme=อ่านคู่มือการติดตั้งและทดลองเล่น
thai.RuntimePermission=ไม่สามารถเริ่มติดตั้งรันไทม์ Microsoft ได้ โปรดอนุญาตสิทธิ์ผู้ดูแลระบบสำหรับส่วนประกอบนี้แล้วลองอีกครั้ง ข้อผิดพลาด:
thai.RuntimeFailed=การติดตั้งรันไทม์ Microsoft ล้มเหลว โปรดติดตั้งให้เสร็จก่อนลองอีกครั้ง รหัสออก:
thai.RuntimePending=รันไทม์ Microsoft ยังไม่พร้อม โปรดติดตั้งให้เสร็จและเริ่ม Windows ใหม่หากได้รับแจ้ง จากนั้นเรียกตัวติดตั้งนี้อีกครั้ง
thai.LaunchBlocked=ระบบป้องกันแอปของ Windows ปิดกั้นการเริ่มเกม (ข้อผิดพลาด %1) การติดตั้งเสร็จแล้ว โปรดเปิดการป้องกันไว้ ขอรุ่นที่มีลายเซ็นเชื่อถือได้จากผู้พัฒนา หรือให้ผู้ดูแลระบบขององค์กรตรวจสอบนโยบาย ส่งรหัสข้อผิดพลาดและบันทึกการติดตั้งให้ฝ่ายสนับสนุน
thai.LaunchFailed=ไม่สามารถเริ่มเกมได้ (ข้อผิดพลาด Windows %1) การติดตั้งเสร็จแล้ว โปรดส่งรหัสข้อผิดพลาดและบันทึกการติดตั้งให้ฝ่ายสนับสนุน
thai.LaunchLog=บันทึกการวินิจฉัยการเริ่มเกม: %1
thai.ReadmeFailed=ไม่สามารถเปิดคู่มือได้ (ข้อผิดพลาด Windows %1) โปรดเปิดไฟล์ README ที่ติดตั้งด้วยตนเอง
thai.ReadmeFile=README.th.txt
arabic.DesktopIcon=إنشاء اختصار على سطح المكتب
arabic.ShortcutGroup=الاختصارات:
arabic.ReadmeTitle=دليل التثبيت والتجربة
arabic.UninstallTitle=إزالة Numeron - Legend of Rebirth
arabic.LaunchGame=تشغيل Numeron - Legend of Rebirth
arabic.OpenReadme=قراءة دليل التثبيت والتجربة
arabic.RuntimePermission=تعذر بدء تثبيت مكتبة تشغيل Microsoft. اسمح بطلب صلاحيات المسؤول لهذا المكون ثم أعد المحاولة. الخطأ:
arabic.RuntimeFailed=فشل تثبيت مكتبة تشغيل Microsoft. أكمل تثبيتها قبل إعادة المحاولة. رمز الخروج:
arabic.RuntimePending=مكتبة تشغيل Microsoft غير جاهزة. أكمل التثبيت وأعد تشغيل Windows إذا طُلب منك، ثم شغّل برنامج التثبيت مرة أخرى.
arabic.LaunchBlocked=منعت حماية التطبيقات في Windows تشغيل اللعبة (الخطأ %1). اكتمل التثبيت. أبقِ الحماية مفعّلة؛ اطلب من المطور إصدارًا بتوقيع موثوق أو اطلب من مسؤول مؤسستك مراجعة السياسة. أرسل رمز الخطأ وسجل التثبيت إلى الدعم.
arabic.LaunchFailed=تعذر تشغيل اللعبة (خطأ Windows %1). اكتمل التثبيت. أرسل رمز الخطأ وسجل التثبيت إلى الدعم.
arabic.LaunchLog=سجل تشخيص التشغيل: %1
arabic.ReadmeFailed=تعذر فتح الدليل (خطأ Windows %1). افتح ملف README المثبّت يدويًا.
arabic.ReadmeFile=README.ar.txt

english.GameBusy=Save your character and close the game and updater before installing. The installed game is busy or cannot be replaced.
chinesetraditional.GameBusy=請先儲存角色進度，關閉遊戲與更新程式後再安裝。目前遊戲檔案正在使用或無法替換。
brazilianportuguese.GameBusy=Salve seu personagem e feche o jogo e o atualizador antes de instalar. O jogo está em uso ou não pode ser substituído.
russian.GameBusy=Сохраните персонажа и закройте игру и программу обновления перед установкой. Файлы игры заняты или недоступны для замены.
hindi.GameBusy=स्थापना से पहले पात्र की प्रगति सहेजें और खेल तथा अपडेटर बंद करें। खेल उपयोग में है या बदला नहीं जा सकता।
indonesian.GameBusy=Simpan karakter dan tutup game serta pembaru sebelum memasang. Game sedang digunakan atau tidak dapat diganti.
vietnamese.GameBusy=Hãy lưu nhân vật và đóng trò chơi cùng trình cập nhật trước khi cài đặt. Tệp trò chơi đang được sử dụng hoặc không thể thay thế.
thai.GameBusy=บันทึกตัวละครและปิดเกมกับโปรแกรมอัปเดตก่อนติดตั้ง ไฟล์เกมกำลังใช้งานหรือไม่สามารถแทนที่ได้
arabic.GameBusy=احفظ تقدم الشخصية وأغلق اللعبة وبرنامج التحديث قبل التثبيت. ملفات اللعبة قيد الاستخدام أو لا يمكن استبدالها.
[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:ShortcutGroup}"

[Files]
Source: "tools\vc_redist.x64.exe"; Flags: dontcopy
#include "verified-payload-files.iss"
#include "verified-updater-files.iss"
Source: "README.en.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.zh-TW.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.pt-BR.txt"; DestDir: "{app}"; Flags: ignoreversion

Source: "README.ru.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.hi.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.id.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.vi.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.th.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.ar.txt"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{app}\game\logs"

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#LauncherExe}"; WorkingDir: "{app}"
Name: "{group}\{cm:ReadmeTitle}"; Filename: "{app}\{cm:ReadmeFile}"
Name: "{group}\{cm:UninstallTitle}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#LauncherExe}"; WorkingDir: "{app}"; Tasks: desktopicon

; Finish-page choices are executed in Pascal code so Windows launch errors
; can be reported without security-policy workarounds. Silent installs never launch.

#ifdef Mir2Bootstrap
#include "verified-bootstrap-messages.iss"
#endif

[Code]
var
  RuntimeRestartRequired: Boolean;
  FinishChoices: TNewCheckListBox;
  FinishActionsDone: Boolean;
  InstallLockHandle: NativeUInt;
  InstallLockRoot: String;

function GetFileAttributesW(FileName: String): Cardinal;
  external 'GetFileAttributesW@kernel32.dll stdcall';
function MoveFileExW(ExistingName: String; NewName: String; Flags: Cardinal): Boolean;
  external 'MoveFileExW@kernel32.dll stdcall';
function CreateFileW(FileName: String; Access, ShareMode: Cardinal; Security: NativeUInt;
  Creation, Flags: Cardinal; Template: NativeUInt): NativeUInt;
  external 'CreateFileW@kernel32.dll stdcall';
function CloseHandle(Handle: NativeUInt): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';
function SafeLocalePath(Path: String): Boolean; forward;

#ifdef Mir2Bootstrap
#include "verified-bootstrap-guard.iss"
#endif

function HoldInstallLock: Boolean;
var
  Directory, LockPath: String;
begin
  Result := True;
  Directory := ExpandConstant('{app}');
  if (InstallLockHandle <> 0) and (InstallLockRoot = Directory) then exit;
  if InstallLockHandle <> 0 then
  begin
    CloseHandle(InstallLockHandle);
    InstallLockHandle := 0;
  end;
  Directory := AddBackslash(Directory) + '.update';
  Result := False;
  if not SafeLocalePath(Directory) or not ForceDirectories(Directory) then exit;
  LockPath := AddBackslash(Directory) + 'install.lock';
  if not SafeLocalePath(LockPath) then exit;
  InstallLockHandle := CreateFileW(LockPath, $C0000000, 0, 0, 4, $80, 0);
  if InstallLockHandle = NativeUInt(-1) then
    InstallLockHandle := 0
  else
  begin
    InstallLockRoot := ExpandConstant('{app}');
    Result := True;
  end;
end;

procedure ReleaseInstallLock;
begin
  if InstallLockHandle <> 0 then
  begin
    CloseHandle(InstallLockHandle);
    InstallLockHandle := 0;
    InstallLockRoot := '';
  end;
end;

procedure DeinitializeSetup;
begin
  ReleaseInstallLock;
end;

function CanReplaceInstalledGame: Boolean;
var
  Handle: NativeUInt;
  GamePath: String;
begin
  Result := True;
  GamePath := ExpandConstant('{app}\game\{#AppExe}');
  if not FileExists(GamePath) then exit;
  { Probe exclusive WRITE access: a running old client must save and close.
    Never ask Restart Manager to force-terminate the user's character. }
  Handle := CreateFileW(GamePath, $40000000, 0, 0, 3, $80, 0);
  if Handle = NativeUInt(-1) then
    Result := False
  else
    CloseHandle(Handle);
end;

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
  if not HoldInstallLock or not CanReplaceInstalledGame then
  begin
    Result := CustomMessage('GameBusy');
    exit;
  end;
#ifdef Mir2Bootstrap
  { Seed-only installation must not replace gameplay or updater recovery state. }
  if not BootstrapCanInstall then
  begin
    Result := CustomMessage('BootstrapExisting');
    exit;
  end;
#endif
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

procedure InitializeWizard;
begin
  FinishChoices := TNewCheckListBox.Create(WizardForm);
  FinishChoices.Parent := WizardForm.FinishedPage;
  FinishChoices.SetBounds(WizardForm.RunList.Left, WizardForm.RunList.Top,
    WizardForm.RunList.Width, ScaleY(96));
  FinishChoices.BorderStyle := bsNone;
  FinishChoices.ParentColor := True;
  FinishChoices.Flat := True;
  FinishChoices.MinItemHeight := ScaleY(28);
  FinishChoices.AddCheckBox(CustomMessage('LaunchGame'), '', 0, True, True, False, False, nil);
  FinishChoices.AddCheckBox(CustomMessage('OpenReadme'), '', 0, False, True, False, False, nil);
  FinishChoices.Visible := False;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpFinished then
  begin
    WizardForm.RunList.Visible := False;
    FinishChoices.Visible := not WizardSilent and not RuntimeRestartRequired and
      not WizardForm.YesRadio.Visible;
  end
  else
    FinishChoices.Visible := False;
end;

function SaveLaunchFailure(ResultCode: Integer): String;
var
  Directory, Destination, Temporary, RecordText: String;
begin
  Result := '';
  Directory := ExpandConstant('{localappdata}\mir2-web3');
  Destination := AddBackslash(Directory) + 'installer-last-launch.txt';
  if not SafeLocalePath(Destination) or DirExists(Destination) then exit;
  if not ForceDirectories(Directory) or not SafeLocalePath(Directory) then exit;
  Temporary := GenerateUniqueName(Directory, '.tmp');
  if (Temporary = '') or not SafeLocalePath(Temporary) then exit;
  try
    { Only fixed labels, local wall time and the Windows error number: no command
      line, account, credentials, network URL or game state. Replace one record. }
    RecordText := 'Numeron installer {#AppVersion}' + #13#10 +
      'localTime=' + GetDateTimeString('yyyy-mm-dd hh:nn:ss', '-', ':') + #13#10 +
      'action=launch-game' + #13#10 + 'windowsError=' + IntToStr(ResultCode) + #13#10;
    if not SaveStringToFile(Temporary, RecordText, False) then exit;
    if not SafeLocalePath(Destination) or DirExists(Destination) then exit;
    if MoveFileExW(Temporary, Destination, 9) then Result := Destination;
  finally
    if FileExists(Temporary) then DeleteFile(Temporary);
  end;
end;

procedure RunFinishChoices;
var
  ResultCode: Integer;
  MessageText, DiagnosticPath: String;
begin
  if FinishActionsDone or WizardSilent or RuntimeRestartRequired then exit;
  if not FinishChoices.Visible then exit;
  FinishActionsDone := True;
  if FinishChoices.Checked[0] then
  begin
    { Original-user execution is the same privilege boundary as Inno's normal
      postinstall entry. Do not retry elevated or disable application control. }
    if not ExecAsOriginalUser(ExpandConstant('{app}\{#LauncherExe}'), '',
      ExpandConstant('{app}'), SW_SHOWNORMAL, ewNoWait, ResultCode) then
    begin
      Log('Game launch failed; Windows error ' + IntToStr(ResultCode));
      DiagnosticPath := SaveLaunchFailure(ResultCode);
      if (ResultCode = 4551) or (ResultCode = 1260) or (ResultCode = 577) then
        MessageText := FmtMessage(CustomMessage('LaunchBlocked'), [IntToStr(ResultCode)])
      else
        MessageText := FmtMessage(CustomMessage('LaunchFailed'), [IntToStr(ResultCode)]);
      if DiagnosticPath <> '' then
        MessageText := MessageText + #13#10#13#10 +
          FmtMessage(CustomMessage('LaunchLog'), [DiagnosticPath]);
      MsgBox(MessageText, mbError, MB_OK);
    end
    else
      Log('Game process creation succeeded; this does not assert gameplay readiness.');
  end;
  if FinishChoices.Checked[1] then
    if not ShellExecAsOriginalUser('open', ExpandConstant('{app}\{cm:ReadmeFile}'), '',
      ExpandConstant('{app}'), SW_SHOWNORMAL, ewNoWait, ResultCode) then
    begin
      Log('Guide open failed; Windows error ' + IntToStr(ResultCode));
      MsgBox(FmtMessage(CustomMessage('ReadmeFailed'), [IntToStr(ResultCode)]), mbError, MB_OK);
    end;
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
  if ActiveLanguage = 'russian' then Code := 'ru';
  if ActiveLanguage = 'hindi' then Code := 'hi';
  if ActiveLanguage = 'indonesian' then Code := 'id';
  if ActiveLanguage = 'vietnamese' then Code := 'vi';
  if ActiveLanguage = 'thai' then Code := 'th';
  if ActiveLanguage = 'arabic' then Code := 'ar';
  Temporary := GenerateUniqueName(Directory, '.tmp');
  if (Temporary = '') or not SafeLocalePath(Temporary) then exit;
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

procedure RetirePreviousUpdateTransaction;
var
  Directory, Journal, ArchiveRoot, Archive, Source, Destination: String;
  I: Integer;
begin
  Directory := ExpandConstant('{app}\.update');
  Journal := AddBackslash(Directory) + 'transaction.json';
  if not FileExists(Journal) then exit;
  ArchiveRoot := AddBackslash(Directory) + 'installer-retired';
  if not SafeLocalePath(Journal) or not SafeLocalePath(ArchiveRoot) or
     not ForceDirectories(ArchiveRoot) then
    RaiseException('Cannot safely retire the previous update transaction.');
  Archive := GenerateUniqueName(ArchiveRoot, '.rollback');
  if (Archive = '') or not SafeLocalePath(Archive) or
     FileExists(Archive) or DirExists(Archive) or not ForceDirectories(Archive) then
    RaiseException('Cannot preserve the previous update transaction.');
  { The complete verified installer payload has now replaced the game. Retire
    the old journal first, so it cannot roll back this installation. Preserve
    its backup/staging and all monotonic receipts for recovery/diagnostics. }
  if not MoveFileExW(Journal, AddBackslash(Archive) + 'transaction.json', 8) then
    RaiseException('Cannot retire the previous update journal.');
  for I := 0 to 1 do
  begin
    if I = 0 then Source := AddBackslash(Directory) + 'backup'
      else Source := AddBackslash(Directory) + 'staging';
    if FileExists(Source) or DirExists(Source) then
    begin
      if I = 0 then Destination := AddBackslash(Archive) + 'backup'
        else Destination := AddBackslash(Archive) + 'staging';
      if not SafeLocalePath(Source) or not SafeLocalePath(Destination) or
         not MoveFileExW(Source, Destination, 8) then
        RaiseException('Cannot preserve previous update recovery files.');
    end;
  end;
  Log('Previous update transaction preserved under ' + Archive);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
#ifndef Mir2Bootstrap
    RetirePreviousUpdateTransaction;
#endif
    SeedInitialLocale;
  end;
  if CurStep = ssDone then
  begin
    { All installation writes have finished. The launcher takes the same lock. }
    ReleaseInstallLock;
    RunFinishChoices;
  end;
end;
