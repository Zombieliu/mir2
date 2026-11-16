// Stable-key copy for shared UI additions. Identical English labels can have
// different contexts (quest "Return" is a delivery location, not navigation).
// Keep every native locale together; this source is used by both generators.
export const sharedKeyLocales = ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"];

export const sharedKeyTranslations = {
  "ui.questProgressHeading": {
    en: "Progress", "zh-TW": "任務進度", "pt-BR": "Progresso",
    ru: "Прогресс", hi: "प्रगति", id: "Progres", vi: "Tiến độ", th: "ความคืบหน้า", ar: "التقدم",
  },
  "ui.questReturnHeading": {
    en: "Return", "zh-TW": "交付地點", "pt-BR": "Local de entrega",
    ru: "Место сдачи", hi: "जमा करने का स्थान", id: "Tempat penyerahan", vi: "Nơi trả nhiệm vụ", th: "สถานที่ส่งภารกิจ", ar: "مكان التسليم",
  },
  "ui.questClientLoading": {
    en: "Preparing quest actions…", "zh-TW": "正在準備任務操作…", "pt-BR": "Preparando as ações de missão…",
    ru: "Подготовка действий задания…", hi: "कार्य की कार्रवाइयाँ तैयार की जा रही हैं…", id: "Menyiapkan tindakan misi…", vi: "Đang chuẩn bị thao tác nhiệm vụ…", th: "กำลังเตรียมการดำเนินการภารกิจ…", ar: "جارٍ تجهيز إجراءات المهمة…",
  },
  "ui.questClientUnavailable": {
    en: "Quest actions are temporarily unavailable. Please retry.", "zh-TW": "任務操作暫時無法使用，請重試。", "pt-BR": "As ações de missão estão temporariamente indisponíveis. Tente novamente.",
    ru: "Действия задания временно недоступны. Попробуйте снова.", hi: "कार्य की कार्रवाइयाँ अभी उपलब्ध नहीं हैं। फिर से प्रयास करें।", id: "Tindakan misi sementara tidak tersedia. Silakan coba lagi.", vi: "Tạm thời không thể thao tác nhiệm vụ. Vui lòng thử lại.", th: "การดำเนินการภารกิจไม่พร้อมใช้งานชั่วคราว โปรดลองอีกครั้ง", ar: "إجراءات المهمة غير متاحة مؤقتًا. يرجى المحاولة مرة أخرى.",
  },
  "ui.questActionPending": {
    en: "Your request is being processed. Please wait.", "zh-TW": "請求處理中，請稍候。", "pt-BR": "Sua solicitação está sendo processada. Aguarde.",
    ru: "Ваш запрос обрабатывается. Пожалуйста, подождите.", hi: "आपके अनुरोध पर काम हो रहा है। कृपया प्रतीक्षा करें।", id: "Permintaan Anda sedang diproses. Harap tunggu.", vi: "Yêu cầu của bạn đang được xử lý. Vui lòng chờ.", th: "กำลังดำเนินการตามคำขอของคุณ โปรดรอสักครู่", ar: "جارٍ معالجة طلبك. يرجى الانتظار.",
  },
  "ui.questActionRewardRequired": {
    en: "Select a reward first.", "zh-TW": "請先選擇獎勵。", "pt-BR": "Selecione uma recompensa primeiro.",
    ru: "Сначала выберите награду.", hi: "पहले एक पुरस्कार चुनें।", id: "Pilih hadiah terlebih dahulu.", vi: "Hãy chọn phần thưởng trước.", th: "โปรดเลือกรางวัลก่อน", ar: "اختر مكافأة أولًا.",
  },
  "ui.questActionTalkToNpc": {
    en: "Talk to the quest NPC to continue.", "zh-TW": "請與任務 NPC 交談後繼續。", "pt-BR": "Fale com o NPC da missão para continuar.",
    ru: "Поговорите с NPC задания, чтобы продолжить.", hi: "आगे बढ़ने के लिए कार्य के NPC से बात करें।", id: "Bicara dengan NPC misi untuk melanjutkan.", vi: "Hãy nói chuyện với NPC nhiệm vụ để tiếp tục.", th: "พูดคุยกับ NPC ภารกิจเพื่อดำเนินการต่อ", ar: "تحدث إلى شخصية المهمة غير اللاعبة للمتابعة.",
  },
  "ui.questActionStateChanged": {
    en: "The quest has changed. Review it and try again.", "zh-TW": "任務狀態已變更，請查看任務後重試。", "pt-BR": "A missão mudou. Confira os detalhes e tente novamente.",
    ru: "Задание изменилось. Проверьте его и попробуйте снова.", hi: "कार्य बदल गया है। उसे देखें और फिर से प्रयास करें।", id: "Misi telah berubah. Tinjau dan coba lagi.", vi: "Nhiệm vụ đã thay đổi. Hãy xem lại rồi thử lại.", th: "ภารกิจมีการเปลี่ยนแปลง โปรดตรวจสอบแล้วลองอีกครั้ง", ar: "تغيرت المهمة. راجعها وحاول مرة أخرى.",
  },
  "ui.retry": {
    en: "Retry", "zh-TW": "重試", "pt-BR": "Tentar novamente",
    ru: "Повторить", hi: "फिर से प्रयास करें", id: "Coba lagi", vi: "Thử lại", th: "ลองอีกครั้ง", ar: "إعادة المحاولة",
  },
};

export function sharedKeyTranslation(key, english) {
  const copy = sharedKeyTranslations[key];
  if (!copy) return undefined;
  if (copy.en !== english) throw new Error(`Shared UI source changed; review translation before regeneration: ${key}`);
  for (const locale of sharedKeyLocales) if (typeof copy[locale] !== "string" || !copy[locale].trim()) throw new Error(`Missing shared UI translation: ${key}/${locale}`);
  return copy;
}
