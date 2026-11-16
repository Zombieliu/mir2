import type { Locale } from "@/lib/site-copy";

// Status text cannot create or approve a download URL.
const access: Record<Locale, readonly [windows: string, android: string]> = {
  "zh-CN": ["仅在本页提供官方安装包链接时公开下载。没有按钮时，受邀测试玩家请使用已经获得的官方渠道。", "Android ARM64 测试版 APK 可从官网下载页获取。完整手机体验和设备兼容性仍在测试中。"],
  en: ["Public downloads require an official installer link shown here. Without a button, invited testers should use the official channel already provided.", "The Android ARM64 playtest APK is available from the download page. The full mobile experience and device compatibility are still being tested."],
  "zh-TW": ["僅在本頁提供官方安裝包連結時公開下載。沒有按鈕時，受邀測試玩家請使用已提供的官方管道。", "Android ARM64 測試版 APK 可從官網下載頁取得。完整手機體驗與裝置相容性仍在測試中。"],
  ja: ["公開ダウンロードには、このページの公式インストーラーリンクが必要です。ボタンがない場合、招待者は案内済みの公式経路を利用してください。", "Android ARM64 のテスト版 APK は公式ダウンロードページから入手できます。モバイル機能全体と端末の互換性は引き続きテスト中です。"],
  ko: ["공개 다운로드는 이 페이지의 공식 설치 링크가 있어야 합니다. 버튼이 없으면 초대받은 테스터는 이미 제공된 공식 경로를 사용하세요.", "Android ARM64 테스트 APK를 공식 다운로드 페이지에서 받을 수 있습니다. 전체 모바일 경험과 기기 호환성은 계속 테스트 중입니다."],
  es: ["La descarga pública requiere un enlace oficial aquí. Sin botón, los invitados deben usar el canal oficial ya facilitado.", "El APK de prueba para Android ARM64 está disponible en la página de descargas. La experiencia móvil completa y la compatibilidad con dispositivos siguen en pruebas."],
  "pt-BR": ["O download público exige um link oficial nesta página. Sem botão, os convidados devem usar o canal oficial já fornecido.", "O APK de teste para Android ARM64 está disponível na página de downloads. A experiência móvel completa e a compatibilidade com dispositivos continuam em testes."],
  de: ["Öffentliche Downloads erfordern hier einen offiziellen Installer-Link. Ohne Schaltfläche nutzen eingeladene Tester den bereits mitgeteilten offiziellen Weg.", "Die Android-ARM64-Test-APK ist auf der Downloadseite verfügbar. Das vollständige mobile Spielerlebnis und die Gerätekompatibilität werden weiterhin getestet."],
  fr: ["Le téléchargement public exige ici un lien officiel. Sans bouton, les invités utilisent le canal officiel déjà communiqué.", "L’APK de test Android ARM64 est disponible sur la page de téléchargement. L’expérience mobile complète et la compatibilité des appareils sont toujours en cours de test."],
  ru: ["Публичная загрузка требует официальной ссылки здесь. Без кнопки приглашённые тестировщики используют уже предоставленный официальный канал.", "Тестовый APK для Android ARM64 доступен на странице загрузок. Полноценный мобильный игровой процесс и совместимость устройств всё ещё тестируются."],
  pl: ["Publiczne pobieranie wymaga tutaj oficjalnego linku. Bez przycisku zaproszeni testerzy używają już podanego oficjalnego kanału.", "Testowy plik APK dla Androida ARM64 jest dostępny na stronie pobierania. Pełna rozgrywka mobilna i zgodność urządzeń są nadal testowane."],
  tr: ["Genel indirme için burada resmî yükleyici bağlantısı gerekir. Düğme yoksa davetli testçiler önceden verilen resmî kanalı kullanmalıdır.", "Android ARM64 test APK’sı indirme sayfasında mevcuttur. Tam mobil deneyim ve cihaz uyumluluğu test edilmeye devam ediyor."],
  th: ["ดาวน์โหลดสาธารณะได้เมื่อหน้านี้มีลิงก์ติดตั้งทางการ หากไม่มีปุ่ม ผู้ได้รับเชิญให้ใช้ช่องทางทางการที่ได้รับแล้ว", "ดาวน์โหลด APK ทดสอบสำหรับ Android ARM64 ได้จากหน้าดาวน์โหลดทางการ ประสบการณ์มือถือเต็มรูปแบบและความเข้ากันได้ของอุปกรณ์ยังอยู่ระหว่างทดสอบ"],
  vi: ["Chỉ tải công khai khi trang này có liên kết cài đặt chính thức. Không có nút thì người được mời dùng kênh chính thức đã nhận.", "APK thử nghiệm Android ARM64 có trên trang tải xuống chính thức. Trải nghiệm di động đầy đủ và khả năng tương thích thiết bị vẫn đang được thử nghiệm."],
  id: ["Unduhan publik memerlukan tautan pemasang resmi di sini. Tanpa tombol, penguji undangan memakai kanal resmi yang telah diberikan.", "APK uji Android ARM64 tersedia di halaman unduhan resmi. Pengalaman seluler lengkap dan kompatibilitas perangkat masih diuji."],
  ar: ["يتطلب التنزيل العام رابط تثبيت رسميًا ظاهرًا هنا. عند غياب الزر يستخدم المختبرون المدعوون القناة الرسمية المقدمة لهم.", "يتوفر ملف APK لاختبار Android ARM64 في صفحة التنزيل الرسمية. لا تزال تجربة الهاتف الكاملة وتوافق الأجهزة قيد الاختبار."],
};

export function getAccessCopy(locale: Locale) {
  const [windows, android] = access[locale];
  return { windows, android };
}

const previewLabels: Record<Locale, string> = {
  "zh-CN": "演示预览", "zh-TW": "示範預覽", en: "Demo preview", ja: "デモプレビュー", ko: "데모 미리보기",
  es: "Vista de demostración", "pt-BR": "Prévia demonstrativa", de: "Demo-Vorschau", fr: "Aperçu de démonstration",
  ru: "Демонстрационный просмотр", pl: "Podgląd demonstracyjny", tr: "Demo önizlemesi", th: "ตัวอย่างสาธิต", vi: "Xem trước bản minh họa", id: "Pratinjau demo", ar: "معاينة توضيحية",
};

export function getPreviewLabel(locale: Locale) { return previewLabels[locale]; }
