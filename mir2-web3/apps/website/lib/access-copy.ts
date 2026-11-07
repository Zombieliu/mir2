import type { Locale } from "@/lib/site-copy";

// Status text cannot create or approve a download URL.
const access: Record<Locale, readonly [windows: string, android: string]> = {
  "zh-CN": ["仅在本页提供官方安装包链接时公开下载。没有按钮时，受邀测试玩家请使用已经获得的官方渠道。", "Android 发行与完整手机体验尚待验收；浏览器能打开或 PWA 工程能力不代表已发布 Android 客户端。"],
  en: ["Public downloads require an official installer link shown here. Without a button, invited testers should use the official channel already provided.", "Android release and the complete mobile experience await acceptance. Browser access or engineering PWA support does not establish a released Android client."],
  "zh-TW": ["僅在本頁提供官方安裝包連結時公開下載。沒有按鈕時，受邀測試玩家請使用已提供的官方管道。", "Android 發行與完整手機體驗仍待驗收；瀏覽器可開啟或 PWA 工程能力不代表已發布 Android 客戶端。"],
  ja: ["公開ダウンロードには、このページの公式インストーラーリンクが必要です。ボタンがない場合、招待者は案内済みの公式経路を利用してください。", "Android 公開と完全なモバイル体験は未承認です。ブラウザー起動や PWA の技術対応は Android クライアントの公開を意味しません。"],
  ko: ["공개 다운로드는 이 페이지의 공식 설치 링크가 있어야 합니다. 버튼이 없으면 초대받은 테스터는 이미 제공된 공식 경로를 사용하세요.", "Android 출시와 전체 모바일 경험은 검수 전입니다. 브라우저 실행이나 PWA 기술 지원만으로 Android 클라이언트가 출시된 것은 아닙니다."],
  es: ["La descarga pública requiere un enlace oficial aquí. Sin botón, los invitados deben usar el canal oficial ya facilitado.", "El lanzamiento Android y la experiencia móvil completa están pendientes de validación. El navegador o el soporte técnico PWA no equivalen a un cliente Android publicado."],
  "pt-BR": ["O download público exige um link oficial nesta página. Sem botão, os convidados devem usar o canal oficial já fornecido.", "O lançamento Android e a experiência móvel completa aguardam aceitação. Navegador ou suporte técnico PWA não significam um cliente Android lançado."],
  de: ["Öffentliche Downloads erfordern hier einen offiziellen Installer-Link. Ohne Schaltfläche nutzen eingeladene Tester den bereits mitgeteilten offiziellen Weg.", "Android-Veröffentlichung und vollständiges mobiles Erlebnis sind noch nicht abgenommen. Browserzugang oder technische PWA-Unterstützung bedeuten keinen veröffentlichten Android-Client."],
  fr: ["Le téléchargement public exige ici un lien officiel. Sans bouton, les invités utilisent le canal officiel déjà communiqué.", "La sortie Android et l'expérience mobile complète attendent validation. Le navigateur ou une capacité technique PWA ne constituent pas un client Android publié."],
  ru: ["Публичная загрузка требует официальной ссылки здесь. Без кнопки приглашённые тестировщики используют уже предоставленный официальный канал.", "Выпуск Android и полноценная мобильная игра ещё не приняты. Работа в браузере или техническая поддержка PWA не означают выпуск Android-клиента."],
  pl: ["Publiczne pobieranie wymaga tutaj oficjalnego linku. Bez przycisku zaproszeni testerzy używają już podanego oficjalnego kanału.", "Wydanie Android i pełna gra mobilna wymagają odbioru. Przeglądarka lub techniczna obsługa PWA nie oznaczają wydanego klienta Android."],
  tr: ["Genel indirme için burada resmî yükleyici bağlantısı gerekir. Düğme yoksa davetli testçiler önceden verilen resmî kanalı kullanmalıdır.", "Android yayını ve tam mobil deneyim kabul bekliyor. Tarayıcı erişimi veya teknik PWA desteği yayımlanmış Android istemcisi anlamına gelmez."],
  th: ["ดาวน์โหลดสาธารณะได้เมื่อหน้านี้มีลิงก์ติดตั้งทางการ หากไม่มีปุ่ม ผู้ได้รับเชิญให้ใช้ช่องทางทางการที่ได้รับแล้ว", "การเผยแพร่ Android และประสบการณ์มือถือครบถ้วนยังรอตรวจรับ เบราว์เซอร์หรือความสามารถ PWA ทางเทคนิคไม่เท่ากับเผยแพร่ไคลเอนต์ Android"],
  vi: ["Chỉ tải công khai khi trang này có liên kết cài đặt chính thức. Không có nút thì người được mời dùng kênh chính thức đã nhận.", "Phát hành Android và trải nghiệm di động đầy đủ chưa được nghiệm thu. Trình duyệt hoặc khả năng PWA kỹ thuật không đồng nghĩa đã phát hành ứng dụng Android."],
  id: ["Unduhan publik memerlukan tautan pemasang resmi di sini. Tanpa tombol, penguji undangan memakai kanal resmi yang telah diberikan.", "Rilis Android dan pengalaman seluler lengkap menunggu penerimaan. Browser atau dukungan teknis PWA bukan bukti klien Android telah dirilis."],
  ar: ["يتطلب التنزيل العام رابط تثبيت رسميًا ظاهرًا هنا. عند غياب الزر يستخدم المختبرون المدعوون القناة الرسمية المقدمة لهم.", "إصدار Android وتجربة الهاتف الكاملة بانتظار الاعتماد. المتصفح أو دعم PWA الهندسي لا يعني إصدار عميل Android."],
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
