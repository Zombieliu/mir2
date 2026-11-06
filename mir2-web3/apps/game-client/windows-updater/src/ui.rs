//! Thread-owned, DPI-aware native launcher with explicit download and play actions.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

mod launcher_copy;
mod transfer;
mod version_info;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LauncherAction {
    Proceed,
    PlayInstalled,
    Retry,
    Exit,
}

fn locale_index(locale: &str) -> usize {
    match crate::canonical_os_locale(locale) {
        "zh-TW" => 1,
        "pt-BR" => 2,
        "ru" => 3,
        "hi" => 4,
        "id" => 5,
        "vi" => 6,
        "th" => 7,
        "ar" => 8,
        "zh-CN" => 9,
        _ => 0,
    }
}

/// These small native strings are independent of the game catalog version.
pub fn stage(locale: &str, key: &str) -> String {
    if let Some(strings) = launcher_copy::strings(key) {
        return strings[locale_index(locale)].into();
    }
    if let Some((phase, counts)) = key.split_once(':') {
        if let Some((done, total)) = counts.split_once('/') {
            if let (Ok(done), Ok(total)) = (done.parse::<usize>(), total.parse::<usize>()) {
                if done <= total
                    && total <= crate::model::MAX_TRANSACTION_FILES
                    && matches!(
                        phase,
                        "extracting"
                            | "staging"
                            | "preparing"
                            | "installing"
                            | "checking-installed"
                            | "recovering"
                    )
                {
                    return format!("{}  ({} / {})", stage(locale, phase), done, total);
                }
            }
        }
    }
    let strings: [&str; crate::SUPPORTED_LOCALES.len()] = match key {
        "extracting" => [
            "Unpacking and verifying files…",
            "正在解壓並驗證檔案…",
            "Descompactando e verificando arquivos…",
            "Распаковка и проверка файлов…",
            "फ़ाइलें खोलकर जाँची जा रही हैं…",
            "Mengekstrak dan memverifikasi file…",
            "Đang giải nén và xác minh tệp…",
            "กำลังแตกไฟล์และตรวจสอบไฟล์…",
            "جارٍ فك الملفات والتحقق منها…",
            "正在解压并验证文件…",
        ],
        "staging" => [
            "Preparing verified files locally…",
            "正在本機暫存已驗證的檔案…",
            "Preparando arquivos verificados no computador…",
            "Подготовка проверенных файлов на компьютере…",
            "जाँची गई फ़ाइलें इस कंप्यूटर पर तैयार हो रही हैं…",
            "Menyiapkan file terverifikasi di komputer…",
            "Đang chuẩn bị tệp đã xác minh trên máy…",
            "กำลังเตรียมไฟล์ที่ตรวจสอบแล้วในเครื่อง…",
            "جارٍ تجهيز الملفات المتحقق منها على الجهاز…",
            "正在本地暂存已验证的文件…",
        ],
        "preparing" => [
            "Preparing rollback and checking staged files…",
            "正在準備還原備份並檢查暫存檔案…",
            "Preparando restauração e verificando arquivos locais…",
            "Подготовка резервных копий и проверка файлов…",
            "बहाली की प्रतियाँ और स्थानीय फ़ाइलें तैयार हो रही हैं…",
            "Menyiapkan pemulihan dan memeriksa file lokal…",
            "Đang chuẩn bị khôi phục và kiểm tra tệp cục bộ…",
            "กำลังเตรียมการย้อนกลับและตรวจสอบไฟล์ในเครื่อง…",
            "جارٍ تجهيز الاستعادة وفحص الملفات المحلية…",
            "正在准备回滚备份并检查暂存文件…",
        ],
        "installing" => [
            "Installing verified files locally…",
            "正在本機安裝已驗證的檔案…",
            "Instalando arquivos verificados no computador…",
            "Установка проверенных файлов на компьютере…",
            "जाँची गई फ़ाइलें इस कंप्यूटर पर इंस्टॉल हो रही हैं…",
            "Menginstal file terverifikasi di komputer…",
            "Đang cài đặt tệp đã xác minh trên máy…",
            "กำลังติดตั้งไฟล์ที่ตรวจสอบแล้วในเครื่อง…",
            "جارٍ تثبيت الملفات المتحقق منها على الجهاز…",
            "正在本地安装已验证的文件…",
        ],
        "checking-installed" => [
            "Checking installed files…",
            "正在檢查已安裝的檔案…",
            "Verificando arquivos instalados…",
            "Проверка установленных файлов…",
            "इंस्टॉल की गई फ़ाइलें जाँची जा रही हैं…",
            "Memeriksa file yang terinstal…",
            "Đang kiểm tra các tệp đã cài đặt…",
            "กำลังตรวจสอบไฟล์ที่ติดตั้งแล้ว…",
            "جارٍ فحص الملفات المثبتة…",
            "正在检查已安装的文件…",
        ],
        "checking" => [
            "Checking for updates…",
            "正在檢查更新…",
            "Verificando atualizações…",
            "Проверка обновлений…",
            "अपडेट की जाँच हो रही है…",
            "Memeriksa pembaruan…",
            "Đang kiểm tra cập nhật…",
            "กำลังตรวจสอบการอัปเดต…",
            "جارٍ التحقق من التحديثات…",
            "正在检查更新…",
        ],
        "downloading" => [
            "Downloading changed files…",
            "正在下載變更的檔案…",
            "Baixando arquivos alterados…",
            "Загрузка изменённых файлов…",
            "बदली गई फ़ाइलें डाउनलोड हो रही हैं…",
            "Mengunduh file yang berubah…",
            "Đang tải các tệp đã thay đổi…",
            "กำลังดาวน์โหลดไฟล์ที่เปลี่ยนแปลง…",
            "جارٍ تنزيل الملفات المعدّلة…",
            "正在下载有变更的文件…",
        ],
        "verifying" => [
            "Verifying downloaded files…",
            "正在驗證下載的檔案…",
            "Verificando os arquivos baixados…",
            "Проверка загруженных файлов…",
            "डाउनलोड की गई फ़ाइलों की जाँच हो रही है…",
            "Memverifikasi file unduhan…",
            "Đang xác minh các tệp đã tải…",
            "กำลังตรวจสอบไฟล์ที่ดาวน์โหลด…",
            "جارٍ التحقق من الملفات المنزّلة…",
            "正在验证下载的文件…",
        ],
        "applying" => [
            "Installing the update…",
            "正在套用更新…",
            "Instalando a atualização…",
            "Установка обновления…",
            "अपडेट इंस्टॉल हो रहा है…",
            "Memasang pembaruan…",
            "Đang cài đặt bản cập nhật…",
            "กำลังติดตั้งการอัปเดต…",
            "جارٍ تثبيت التحديث…",
            "正在安装更新…",
        ],
        "recovering" => [
            "Recovering the previous version…",
            "正在還原先前的版本…",
            "Restaurando a versão anterior…",
            "Восстановление предыдущей версии…",
            "पिछला संस्करण बहाल हो रहा है…",
            "Memulihkan versi sebelumnya…",
            "Đang khôi phục phiên bản trước…",
            "กำลังกู้คืนเวอร์ชันก่อนหน้า…",
            "جارٍ استعادة الإصدار السابق…",
            "正在恢复上一版本…",
        ],
        "recovered" => [
            "The previous version has been restored.",
            "已還原先前的版本。",
            "A versão anterior foi restaurada.",
            "Предыдущая версия восстановлена.",
            "पिछला संस्करण बहाल कर दिया गया है।",
            "Versi sebelumnya telah dipulihkan.",
            "Đã khôi phục phiên bản trước.",
            "กู้คืนเวอร์ชันก่อนหน้าแล้ว",
            "تمت استعادة الإصدار السابق.",
            "已恢复上一版本。",
        ],
        "launching" => [
            "Starting the game…",
            "正在啟動遊戲…",
            "Iniciando o jogo…",
            "Запуск игры…",
            "गेम शुरू हो रहा है…",
            "Memulai permainan…",
            "Đang khởi động trò chơi…",
            "กำลังเริ่มเกม…",
            "جارٍ تشغيل اللعبة…",
            "正在启动游戏…",
        ],
        "offline" => [
            "The update server is unavailable. Using the installed version.",
            "無法連線至更新伺服器，將使用已安裝的版本。",
            "Servidor de atualização indisponível. Usando a versão instalada.",
            "Сервер обновлений недоступен. Используется установленная версия.",
            "अपडेट सर्वर उपलब्ध नहीं है। इंस्टॉल किया गया संस्करण इस्तेमाल होगा।",
            "Server pembaruan tidak tersedia. Menggunakan versi terpasang.",
            "Máy chủ cập nhật không khả dụng. Sử dụng phiên bản đã cài đặt.",
            "เซิร์ฟเวอร์อัปเดตไม่พร้อมใช้งาน จะใช้เวอร์ชันที่ติดตั้งไว้",
            "خادم التحديث غير متاح. سيُستخدم الإصدار المثبّت.",
            "无法连接更新服务器，将使用已安装的版本。",
        ],
        "busy" => [
            "Please close the game before updating.",
            "請先關閉遊戲再更新。",
            "Feche o jogo antes de atualizar.",
            "Закройте игру перед обновлением.",
            "अपडेट से पहले गेम बंद करें।",
            "Tutup permainan sebelum memperbarui.",
            "Vui lòng đóng trò chơi trước khi cập nhật.",
            "กรุณาปิดเกมก่อนอัปเดต",
            "يرجى إغلاق اللعبة قبل التحديث.",
            "请先关闭游戏再更新。",
        ],
        "failed" => [
            "Update failed",
            "更新失敗",
            "Falha na atualização",
            "Ошибка обновления",
            "अपडेट विफल हुआ",
            "Pembaruan gagal",
            "Cập nhật thất bại",
            "อัปเดตไม่สำเร็จ",
            "فشل التحديث",
            "更新失败",
        ],
        "cancel" => [
            "Cancel",
            "取消",
            "Cancelar",
            "Отмена",
            "रद्द करें",
            "Batal",
            "Hủy",
            "ยกเลิก",
            "إلغاء",
            "取消",
        ],
        "title" => [
            "Game update",
            "遊戲更新",
            "Atualização do jogo",
            "Обновление игры",
            "गेम अपडेट",
            "Pembaruan permainan",
            "Cập nhật trò chơi",
            "อัปเดตเกม",
            "تحديث اللعبة",
            "游戏更新",
        ],
        _ => return key.to_string(),
    };
    strings[locale_index(locale)].into()
}

fn render(locale: &str, key: &str, values: &[(&str, String)]) -> String {
    let mut result = stage(locale, key);
    for (name, value) in values {
        result = result.replace(&format!("{{{name}}}"), value);
    }
    result
}

#[cfg(windows)]
mod skin;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{show_error, ProgressWindow};

#[cfg(not(windows))]
#[derive(Clone)]
pub struct ProgressWindow {
    cancelled: Arc<AtomicBool>,
}
#[cfg(not(windows))]
impl ProgressWindow {
    pub fn spawn(_: &str) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn update(&self, _: &str, _: u32) {}
    pub fn home(&self, _: &str, _: &str, _: bool) -> LauncherAction {
        LauncherAction::Exit
    }
    pub fn probe_result(&self, _: &str, _: Option<&str>, _: bool) {}
    pub fn installed_available(&self, _: bool) {}
    pub fn confirm_update_choice(&self, _: &str, _: &str, _: u64, _: bool) -> LauncherAction {
        LauncherAction::Exit
    }
    pub fn set_stage(&self, _: &str, _: u32) {}
    pub fn transfer(&self, _: u64, _: u64) {}
    pub fn confirm_update(&self, _: &str, _: &str, _: u64) -> bool {
        false
    }
    pub fn ready(&self, _: &str, _: &str, _: bool) -> LauncherAction {
        LauncherAction::Exit
    }
    pub fn retry(&self, _: &str, _: &str) -> bool {
        false
    }
    pub fn close(&self) {}
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn is_open(&self) -> bool {
        false
    }
    pub fn begin_launch(&self) -> bool {
        false
    }
}
#[cfg(not(windows))]
pub fn show_error(_: &str, _: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_supported_locale_has_every_native_update_stage() {
        for (index, locale) in crate::SUPPORTED_LOCALES.into_iter().enumerate() {
            assert_eq!(locale_index(locale), index);
            for key in [
                "extracting",
                "staging",
                "preparing",
                "installing",
                "checking-installed",
                "hero-title",
                "hero-caption",
                "status-label",
                "current-version",
                "installed-build",
                "release-notes",
                "release-notes-open-failed",
                "home-ready",
                "home-repair",
                "checking-optional",
                "optional-found",
                "probe-failed",
                "up-to-date",
                "play-without-update",
                "repair-hint",
                "review-update",
                "play-current",
                "ready-label",
                "minimize",
                "checking",
                "downloading",
                "verifying",
                "applying",
                "recovering",
                "recovered",
                "launching",
                "offline",
                "busy",
                "failed",
                "cancel",
                "title",
                "update-available",
                "update",
                "play",
                "retry",
                "exit",
                "working",
                "ready",
                "offline-ready",
                "checking-hint",
                "ready-hint",
                "retry-hint",
                "target-version",
                "download-size",
                "elapsed",
                "current-download",
                "estimating",
                "download-speed",
                "download-waiting",
            ] {
                let translated = stage(locale, key);
                assert!(!translated.is_empty());
                assert_ne!(translated, key);
                assert!(!translated.contains('\0'));
            }
        }
    }
    #[test]
    fn unknown_locale_uses_english_and_region_variants_are_normalized() {
        assert_eq!(stage("xx-ZZ", "checking"), stage("en", "checking"));
        assert_eq!(stage("pt_BR", "checking"), stage("pt-BR", "checking"));
        assert_eq!(stage("ru-RU", "checking"), stage("ru", "checking"));
        assert_eq!(stage("zh-CN", "checking"), "正在检查更新…");
        assert_eq!(stage("zh-SG", "title"), "游戏更新");
        for locale in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant"] {
            assert_eq!(stage(locale, "checking"), "正在檢查更新…");
        }
        assert_eq!(stage("zh-XX", "checking"), stage("en", "checking"));
        assert_eq!(stage("zh-Hansfake", "checking"), stage("en", "checking"));
    }

    #[test]
    fn launcher_placeholders_render_in_all_ten_locales() {
        for locale in crate::SUPPORTED_LOCALES {
            for (key, values) in [
                ("optional-found", vec![("version", "r42".into())]),
                ("target-version", vec![("version", "r42".into())]),
                ("download-size", vec![("size", "80 MiB".into())]),
                ("elapsed", vec![("time", "1:23".into())]),
                (
                    "current-download",
                    vec![("received", "40 MiB".into()), ("total", "80 MiB".into())],
                ),
                (
                    "download-speed",
                    vec![("speed", "10 MiB".into()), ("time", "0:04".into())],
                ),
            ] {
                let rendered = render(locale, key, &values);
                assert!(
                    !rendered.contains('{') && !rendered.contains('}'),
                    "{locale}/{key}: {rendered}"
                );
                for (_, value) in values {
                    assert!(rendered.contains(&value), "{locale}/{key}: {rendered}");
                }
            }
        }
        assert_eq!(stage("zh-CN", "home-ready"), "准备就绪");
        assert_eq!(stage("zh-TW", "home-ready"), "準備就緒");
    }

    #[test]
    fn simplified_progress_counters_are_translated_and_stay_bounded() {
        for phase in [
            "extracting",
            "staging",
            "preparing",
            "installing",
            "checking-installed",
            "recovering",
        ] {
            assert!(stage("zh-CN", &format!("{phase}:3/7")).ends_with("(3 / 7)"));
        }
        for key in [
            "extracting:8/7",
            "extracting:-1/7",
            "extracting:3/no",
            "arbitrary:3/7",
        ] {
            assert_eq!(stage("zh-CN", key), key);
        }
        let key = format!("installing:1/{}", crate::model::MAX_TRANSACTION_FILES + 1);
        assert_eq!(stage("zh-CN", &key), key);
    }
}
