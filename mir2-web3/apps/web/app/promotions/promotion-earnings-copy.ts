import type { PromotionLocale } from "./promotion-copy";
import type { PromotionCampaign } from "./promotion-model";

export type PromotionEarningsCopy = {
  accruedCommission: string;
  finalizedCommission: string;
  autoUpdate: string;
  updatedAt: string;
  seasonEnd: string;
  settlementTiming: string;
  finalizedNote: string;
  revenueBasis: string;
  monthlyPurchaseFull: string;
  monthlyServiceProrated: string;
  revenueAdjustments: string;
};

/** Older campaigns keep their original basis until the server explicitly changes it. */
export function promotionRevenueBasis(campaign: PromotionCampaign | null | undefined, locale: PromotionLocale): string {
  const copy = PROMOTION_EARNINGS_COPY[locale];
  return campaign?.rules?.revenueRecognition === "monthly_purchase_full" ? copy.monthlyPurchaseFull : copy.monthlyServiceProrated;
}

export const PROMOTION_EARNINGS_COPY: Record<PromotionLocale, PromotionEarningsCopy> = {
  "zh-Hans": {
    accruedCommission: "本赛季累计佣金（待结算）",
    finalizedCommission: "本赛季已封账佣金",
    autoUpdate: "每 10 秒更新",
    updatedAt: "最近更新",
    seasonEnd: "赛季结束",
    settlementTiming: "合格月卡收入核实后更新累计预估佣金；赛季结束核账后结算并发放至已验证钱包。",
    finalizedNote: "本赛季已封账，结算和付款记录如下。",
    revenueBasis: "计佣口径",
    monthlyPurchaseFull: "赛季内购买并激活的合格月卡，按已核实的实付金额全额计佣：月卡消费金额 × 对应分段费率，不按赛季剩余时长折算。同一张月卡只计佣一次。",
    monthlyServiceProrated: "合格月卡按服务有效期与本赛季重叠的时长折算计佣，超出本赛季的部分不计入本赛季。",
    revenueAdjustments: "充值点券本身不计佣；退款会扣回对应佣金。收益需满足活动资格并核实付款来源，赛季结束核账后统一结算。",
  },
  "zh-Hant": {
    accruedCommission: "本賽季累計佣金（待結算）",
    finalizedCommission: "本賽季已封帳佣金",
    autoUpdate: "每 10 秒更新",
    updatedAt: "最近更新",
    seasonEnd: "賽季結束",
    settlementTiming: "合格月卡收入核實後更新累計預估佣金；賽季結束核帳後結算並發放至已驗證錢包。",
    finalizedNote: "本賽季已封帳，結算及付款紀錄如下。",
    revenueBasis: "計佣口徑",
    monthlyPurchaseFull: "賽季內購買並啟用的合格月卡，按已核實的實付金額全額計佣：月卡消費金額 × 對應分段費率，不按賽季剩餘時長折算。同一張月卡只計佣一次。",
    monthlyServiceProrated: "合格月卡按服務有效期與本賽季重疊的時長折算計佣，超出本賽季的部分不計入本賽季。",
    revenueAdjustments: "儲值點券本身不計佣；退款會扣回對應佣金。收益須符合活動資格並核實付款來源，賽季結束核帳後統一結算。",
  },
  en: {
    accruedCommission: "Accrued commission (pending settlement)",
    finalizedCommission: "Finalized commission for this season",
    autoUpdate: "Updates every 10 seconds",
    updatedAt: "Last updated",
    seasonEnd: "Season ends",
    settlementTiming: "Estimated accrued commission updates after eligible monthly-card revenue is verified. After the season ends, accounts are reconciled and settled before payment to your verified wallet.",
    finalizedNote: "This season is finalized. Settlement and payment records appear below.",
    revenueBasis: "Commission basis",
    monthlyPurchaseFull: "An eligible monthly card purchased and activated during the season earns commission on its full verified paid amount: monthly-card spending × the applicable marginal rate. The remaining season length does not reduce it. Each card is counted once.",
    monthlyServiceProrated: "Eligible monthly-card revenue is prorated by the overlap between its service period and this season. Service outside the season is excluded from this season.",
    revenueAdjustments: "Credit top-ups alone earn no commission. Refunds reverse the corresponding commission. Activity eligibility and payment sources must be verified before reconciliation and settlement at the end of the season.",
  },
  "pt-BR": {
    accruedCommission: "Comissão acumulada (aguardando liquidação)",
    finalizedCommission: "Comissão consolidada da temporada",
    autoUpdate: "Atualiza a cada 10 segundos",
    updatedAt: "Última atualização",
    seasonEnd: "Fim da temporada",
    settlementTiming: "A comissão acumulada estimada é atualizada após a verificação da receita elegível de cartões mensais. Após o fim da temporada, as contas são conferidas e liquidadas antes do pagamento à sua carteira verificada.",
    finalizedNote: "As contas desta temporada foram encerradas. Os registros de liquidação e pagamento aparecem abaixo.",
    revenueBasis: "Base da comissão",
    monthlyPurchaseFull: "Um cartão mensal elegível comprado e ativado durante a temporada gera comissão sobre todo o valor pago e verificado: gasto com o cartão × taxa marginal aplicável. O tempo restante da temporada não reduz esse valor. Cada cartão é contado uma vez.",
    monthlyServiceProrated: "A receita elegível do cartão mensal é proporcional ao período de serviço que coincide com esta temporada. O serviço fora da temporada não é incluído nela.",
    revenueAdjustments: "Recargas de créditos, por si só, não geram comissão. Reembolsos revertem a comissão correspondente. A elegibilidade e as fontes de pagamento devem ser verificadas antes da conferência e liquidação no fim da temporada.",
  },
  ru: {
    accruedCommission: "Начисленная комиссия (ожидает расчёта)",
    finalizedCommission: "Итоговая комиссия сезона",
    autoUpdate: "Обновляется каждые 10 секунд",
    updatedAt: "Последнее обновление",
    seasonEnd: "Конец сезона",
    settlementTiming: "Предварительная сумма начисленной комиссии обновляется после проверки подходящих доходов от месячных карт. После окончания сезона проводится сверка и окончательный расчёт, затем выплата поступает на подтверждённый кошелёк.",
    finalizedNote: "Расчёты за этот сезон закрыты. Записи о расчётах и выплатах приведены ниже.",
    revenueBasis: "База комиссии",
    monthlyPurchaseFull: "Подходящая месячная карта, купленная и активированная в течение сезона, учитывается на полную проверенную сумму оплаты: расходы на карту × применимая ступенчатая ставка. Оставшееся время сезона не уменьшает сумму. Каждая карта учитывается один раз.",
    monthlyServiceProrated: "Подходящий доход от месячной карты рассчитывается пропорционально пересечению срока её действия с этим сезоном. Время за пределами сезона в него не включается.",
    revenueAdjustments: "Само пополнение кредитов не даёт комиссии. При возврате оплаты соответствующая комиссия отменяется. Право на участие и источники оплаты проверяются до сверки и расчёта по окончании сезона.",
  },
  hi: {
    accruedCommission: "इस सीज़न का संचित कमीशन (निपटान लंबित)",
    finalizedCommission: "इस सीज़न का अंतिम कमीशन",
    autoUpdate: "हर 10 सेकंड में अपडेट",
    updatedAt: "अंतिम अपडेट",
    seasonEnd: "सीज़न समाप्ति",
    settlementTiming: "पात्र मासिक कार्ड आय सत्यापित होने पर संचित अनुमानित कमीशन अपडेट होता है। सीज़न समाप्त होने पर हिसाब जाँच और निपटान के बाद भुगतान आपके सत्यापित वॉलेट में भेजा जाएगा।",
    finalizedNote: "इस सीज़न के खाते बंद हो चुके हैं। निपटान और भुगतान के रिकॉर्ड नीचे हैं।",
    revenueBasis: "कमीशन का आधार",
    monthlyPurchaseFull: "सीज़न के दौरान खरीदे और सक्रिय किए गए पात्र मासिक कार्ड की पूरी सत्यापित भुगतान राशि पर कमीशन बनता है: मासिक कार्ड पर खर्च × लागू सीमांत दर। सीज़न का बचा समय इसे कम नहीं करता। हर कार्ड केवल एक बार गिना जाता है।",
    monthlyServiceProrated: "पात्र मासिक कार्ड आय उसकी सेवा अवधि और इस सीज़न की साझा अवधि के अनुपात में गिनी जाती है। सीज़न से बाहर की सेवा इस सीज़न में शामिल नहीं है।",
    revenueAdjustments: "केवल क्रेडिट रिचार्ज पर कमीशन नहीं बनता। रिफंड होने पर संबंधित कमीशन वापस घटाया जाता है। सीज़न के अंत में हिसाब जाँच और निपटान से पहले पात्रता और भुगतान स्रोत सत्यापित होना ज़रूरी है।",
  },
  id: {
    accruedCommission: "Akumulasi komisi musim ini (menunggu penyelesaian)",
    finalizedCommission: "Komisi final musim ini",
    autoUpdate: "Diperbarui setiap 10 detik",
    updatedAt: "Terakhir diperbarui",
    seasonEnd: "Musim berakhir",
    settlementTiming: "Perkiraan akumulasi komisi diperbarui setelah pendapatan kartu bulanan yang memenuhi syarat diverifikasi. Setelah musim berakhir, akun direkonsiliasi dan diselesaikan sebelum pembayaran ke dompet terverifikasi Anda.",
    finalizedNote: "Pembukuan musim ini telah ditutup. Catatan penyelesaian dan pembayaran tersedia di bawah.",
    revenueBasis: "Dasar komisi",
    monthlyPurchaseFull: "Kartu bulanan yang memenuhi syarat, dibeli dan diaktifkan selama musim, menghasilkan komisi atas seluruh pembayaran terverifikasi: belanja kartu bulanan × tarif marginal yang berlaku. Sisa waktu musim tidak menguranginya. Setiap kartu dihitung sekali.",
    monthlyServiceProrated: "Pendapatan kartu bulanan yang memenuhi syarat dihitung proporsional dengan masa layanan yang bertumpang tindih dengan musim ini. Layanan di luar musim tidak termasuk dalam musim ini.",
    revenueAdjustments: "Isi ulang kredit saja tidak menghasilkan komisi. Pengembalian dana membatalkan komisi terkait. Kelayakan dan sumber pembayaran harus diverifikasi sebelum rekonsiliasi dan penyelesaian pada akhir musim.",
  },
  vi: {
    accruedCommission: "Hoa hồng tích lũy mùa này (chờ quyết toán)",
    finalizedCommission: "Hoa hồng đã chốt mùa này",
    autoUpdate: "Cập nhật mỗi 10 giây",
    updatedAt: "Cập nhật gần nhất",
    seasonEnd: "Kết thúc mùa",
    settlementTiming: "Ước tính hoa hồng tích lũy được cập nhật sau khi doanh thu thẻ tháng đủ điều kiện được xác minh. Sau khi mùa kết thúc, số liệu được đối soát, quyết toán rồi thanh toán vào ví đã xác minh.",
    finalizedNote: "Mùa này đã chốt sổ. Chi tiết quyết toán và thanh toán ở bên dưới.",
    revenueBasis: "Cơ sở tính hoa hồng",
    monthlyPurchaseFull: "Thẻ tháng đủ điều kiện được mua và kích hoạt trong mùa được tính hoa hồng trên toàn bộ số tiền thanh toán đã xác minh: chi tiêu thẻ tháng × tỷ lệ lũy tiến tương ứng. Thời gian còn lại của mùa không làm giảm số tiền này. Mỗi thẻ chỉ được tính một lần.",
    monthlyServiceProrated: "Doanh thu thẻ tháng đủ điều kiện được tính theo tỷ lệ thời gian dịch vụ trùng với mùa này. Thời gian ngoài mùa không được tính vào mùa này.",
    revenueAdjustments: "Chỉ nạp điểm không tạo hoa hồng. Khoản hoàn tiền sẽ khấu trừ hoa hồng tương ứng. Điều kiện tham gia và nguồn thanh toán phải được xác minh trước khi đối soát, quyết toán vào cuối mùa.",
  },
  th: {
    accruedCommission: "ค่าคอมมิชชันสะสมฤดูกาลนี้ (รอชำระบัญชี)",
    finalizedCommission: "ค่าคอมมิชชันที่สรุปแล้วของฤดูกาลนี้",
    autoUpdate: "อัปเดตทุก 10 วินาที",
    updatedAt: "อัปเดตล่าสุด",
    seasonEnd: "สิ้นสุดฤดูกาล",
    settlementTiming: "ยอดประมาณการค่าคอมมิชชันสะสมจะอัปเดตหลังตรวจสอบรายได้จากบัตรรายเดือนที่เข้าเงื่อนไข เมื่อจบฤดูกาลจะตรวจสอบยอดและสรุปบัญชี ก่อนจ่ายเข้ากระเป๋าเงินที่ยืนยันแล้ว",
    finalizedNote: "ฤดูกาลนี้ปิดบัญชีแล้ว รายการสรุปบัญชีและการจ่ายเงินแสดงอยู่ด้านล่าง",
    revenueBasis: "หลักการคำนวณค่าคอมมิชชัน",
    monthlyPurchaseFull: "บัตรรายเดือนที่เข้าเงื่อนไข ซื้อและเปิดใช้งานในฤดูกาล จะคิดค่าคอมมิชชันจากยอดชำระที่ตรวจสอบแล้วเต็มจำนวน: ยอดใช้จ่ายบัตรรายเดือน × อัตราขั้นที่เกี่ยวข้อง ระยะเวลาที่เหลือของฤดูกาลไม่ลดจำนวนนี้ แต่ละบัตรนับเพียงครั้งเดียว",
    monthlyServiceProrated: "รายได้บัตรรายเดือนที่เข้าเงื่อนไขคิดตามสัดส่วนช่วงบริการที่ทับซ้อนกับฤดูกาลนี้ ช่วงบริการนอกฤดูกาลไม่นับรวมในฤดูกาลนี้",
    revenueAdjustments: "การเติมเครดิตเพียงอย่างเดียวไม่ก่อให้เกิดค่าคอมมิชชัน การคืนเงินจะหักค่าคอมมิชชันส่วนที่เกี่ยวข้อง ต้องตรวจสอบคุณสมบัติและแหล่งชำระเงินก่อนตรวจยอดและสรุปบัญชีเมื่อสิ้นสุดฤดูกาล",
  },
  ar: {
    accruedCommission: "عمولة الموسم المتراكمة (بانتظار التسوية)",
    finalizedCommission: "عمولة الموسم النهائية",
    autoUpdate: "تُحدَّث كل 10 ثوانٍ",
    updatedAt: "آخر تحديث",
    seasonEnd: "نهاية الموسم",
    settlementTiming: "تُحدَّث العمولة التقديرية المتراكمة بعد التحقق من إيرادات البطاقات الشهرية المؤهلة. بعد انتهاء الموسم، تُراجع الحسابات وتُسوّى، ثم يُرسل الدفع إلى محفظتك الموثّقة.",
    finalizedNote: "أُغلقت حسابات هذا الموسم. تظهر سجلات التسوية والدفع أدناه.",
    revenueBasis: "أساس احتساب العمولة",
    monthlyPurchaseFull: "تُحتسب عمولة البطاقة الشهرية المؤهلة المشتراة والمفعّلة خلال الموسم على كامل المبلغ المدفوع المتحقق منه: الإنفاق على البطاقة × نسبة الشريحة المعنية. لا تقل العمولة بسبب الوقت المتبقي من الموسم. تُحتسب كل بطاقة مرة واحدة.",
    monthlyServiceProrated: "تُحتسب إيرادات البطاقة الشهرية المؤهلة بالتناسب مع مدة تداخل خدمتها مع هذا الموسم. ولا تُدرج الخدمة خارج الموسم ضمنه.",
    revenueAdjustments: "شحن الرصيد وحده لا يولّد عمولة. تُخصم العمولة المقابلة عند رد الأموال. يجب التحقق من الأهلية ومصادر الدفع قبل مراجعة الحسابات وتسويتها في نهاية الموسم.",
  },
};
