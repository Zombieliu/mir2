import type { PromotionLocale } from "./promotion-copy";

export type PromotionEarningsCopy = {
  accruedCommission: string;
  finalizedCommission: string;
  autoUpdate: string;
  updatedAt: string;
  seasonEnd: string;
  settlementTiming: string;
  finalizedNote: string;
};

export const PROMOTION_EARNINGS_COPY: Record<PromotionLocale, PromotionEarningsCopy> = {
  "zh-Hans": {
    accruedCommission: "本赛季累计佣金（待结算）",
    finalizedCommission: "本赛季已封账佣金",
    autoUpdate: "每 10 秒更新",
    updatedAt: "最近更新",
    seasonEnd: "赛季结束",
    settlementTiming: "合格月卡收入核实后更新累计预估佣金；赛季结束核账后结算并发放至已验证钱包。",
    finalizedNote: "本赛季已封账，结算和付款记录如下。",
  },
  "zh-Hant": {
    accruedCommission: "本賽季累計佣金（待結算）",
    finalizedCommission: "本賽季已封帳佣金",
    autoUpdate: "每 10 秒更新",
    updatedAt: "最近更新",
    seasonEnd: "賽季結束",
    settlementTiming: "合格月卡收入核實後更新累計預估佣金；賽季結束核帳後結算並發放至已驗證錢包。",
    finalizedNote: "本賽季已封帳，結算及付款紀錄如下。",
  },
  en: {
    accruedCommission: "Accrued commission (pending settlement)",
    finalizedCommission: "Finalized commission for this season",
    autoUpdate: "Updates every 10 seconds",
    updatedAt: "Last updated",
    seasonEnd: "Season ends",
    settlementTiming: "Estimated accrued commission updates after eligible monthly-card revenue is verified. After the season ends, accounts are reconciled and settled before payment to your verified wallet.",
    finalizedNote: "This season is finalized. Settlement and payment records appear below.",
  },
  "pt-BR": {
    accruedCommission: "Comissão acumulada (aguardando liquidação)",
    finalizedCommission: "Comissão consolidada da temporada",
    autoUpdate: "Atualiza a cada 10 segundos",
    updatedAt: "Última atualização",
    seasonEnd: "Fim da temporada",
    settlementTiming: "A comissão acumulada estimada é atualizada após a verificação da receita elegível de cartões mensais. Após o fim da temporada, as contas são conferidas e liquidadas antes do pagamento à sua carteira verificada.",
    finalizedNote: "As contas desta temporada foram encerradas. Os registros de liquidação e pagamento aparecem abaixo.",
  },
  ru: {
    accruedCommission: "Начисленная комиссия (ожидает расчёта)",
    finalizedCommission: "Итоговая комиссия сезона",
    autoUpdate: "Обновляется каждые 10 секунд",
    updatedAt: "Последнее обновление",
    seasonEnd: "Конец сезона",
    settlementTiming: "Предварительная сумма начисленной комиссии обновляется после проверки подходящих доходов от месячных карт. После окончания сезона проводится сверка и окончательный расчёт, затем выплата поступает на подтверждённый кошелёк.",
    finalizedNote: "Расчёты за этот сезон закрыты. Записи о расчётах и выплатах приведены ниже.",
  },
  hi: {
    accruedCommission: "इस सीज़न का संचित कमीशन (निपटान लंबित)",
    finalizedCommission: "इस सीज़न का अंतिम कमीशन",
    autoUpdate: "हर 10 सेकंड में अपडेट",
    updatedAt: "अंतिम अपडेट",
    seasonEnd: "सीज़न समाप्ति",
    settlementTiming: "पात्र मासिक कार्ड आय सत्यापित होने पर संचित अनुमानित कमीशन अपडेट होता है। सीज़न समाप्त होने पर हिसाब जाँच और निपटान के बाद भुगतान आपके सत्यापित वॉलेट में भेजा जाएगा।",
    finalizedNote: "इस सीज़न के खाते बंद हो चुके हैं। निपटान और भुगतान के रिकॉर्ड नीचे हैं।",
  },
  id: {
    accruedCommission: "Akumulasi komisi musim ini (menunggu penyelesaian)",
    finalizedCommission: "Komisi final musim ini",
    autoUpdate: "Diperbarui setiap 10 detik",
    updatedAt: "Terakhir diperbarui",
    seasonEnd: "Musim berakhir",
    settlementTiming: "Perkiraan akumulasi komisi diperbarui setelah pendapatan kartu bulanan yang memenuhi syarat diverifikasi. Setelah musim berakhir, akun direkonsiliasi dan diselesaikan sebelum pembayaran ke dompet terverifikasi Anda.",
    finalizedNote: "Pembukuan musim ini telah ditutup. Catatan penyelesaian dan pembayaran tersedia di bawah.",
  },
  vi: {
    accruedCommission: "Hoa hồng tích lũy mùa này (chờ quyết toán)",
    finalizedCommission: "Hoa hồng đã chốt mùa này",
    autoUpdate: "Cập nhật mỗi 10 giây",
    updatedAt: "Cập nhật gần nhất",
    seasonEnd: "Kết thúc mùa",
    settlementTiming: "Ước tính hoa hồng tích lũy được cập nhật sau khi doanh thu thẻ tháng đủ điều kiện được xác minh. Sau khi mùa kết thúc, số liệu được đối soát, quyết toán rồi thanh toán vào ví đã xác minh.",
    finalizedNote: "Mùa này đã chốt sổ. Chi tiết quyết toán và thanh toán ở bên dưới.",
  },
  th: {
    accruedCommission: "ค่าคอมมิชชันสะสมฤดูกาลนี้ (รอชำระบัญชี)",
    finalizedCommission: "ค่าคอมมิชชันที่สรุปแล้วของฤดูกาลนี้",
    autoUpdate: "อัปเดตทุก 10 วินาที",
    updatedAt: "อัปเดตล่าสุด",
    seasonEnd: "สิ้นสุดฤดูกาล",
    settlementTiming: "ยอดประมาณการค่าคอมมิชชันสะสมจะอัปเดตหลังตรวจสอบรายได้จากบัตรรายเดือนที่เข้าเงื่อนไข เมื่อจบฤดูกาลจะตรวจสอบยอดและสรุปบัญชี ก่อนจ่ายเข้ากระเป๋าเงินที่ยืนยันแล้ว",
    finalizedNote: "ฤดูกาลนี้ปิดบัญชีแล้ว รายการสรุปบัญชีและการจ่ายเงินแสดงอยู่ด้านล่าง",
  },
  ar: {
    accruedCommission: "عمولة الموسم المتراكمة (بانتظار التسوية)",
    finalizedCommission: "عمولة الموسم النهائية",
    autoUpdate: "تُحدَّث كل 10 ثوانٍ",
    updatedAt: "آخر تحديث",
    seasonEnd: "نهاية الموسم",
    settlementTiming: "تُحدَّث العمولة التقديرية المتراكمة بعد التحقق من إيرادات البطاقات الشهرية المؤهلة. بعد انتهاء الموسم، تُراجع الحسابات وتُسوّى، ثم يُرسل الدفع إلى محفظتك الموثّقة.",
    finalizedNote: "أُغلقت حسابات هذا الموسم. تظهر سجلات التسوية والدفع أدناه.",
  },
};
