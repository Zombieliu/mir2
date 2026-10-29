"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PromotionCopy, PromotionLocale } from "./promotion-copy";
import { formatPromotionUsd, promotionRequest, type PromotionBootstrap } from "./promotion-model";
import styles from "./promotion.module.css";
import { prepareCheckoutIntent, recoverCheckoutIntents, type CheckoutIntent, type CheckoutOrder } from "./promotion-checkout-recovery";

// Order: title, recharge, buy, activate, open, credits, review, offline, play.
export const PROMOTION_BILLING_LABELS = {
  en: ["Recharge & membership", "Recharge test USDC", "Buy monthly card", "Activate monthly card", "Open wallet payment", "Credits", "I reviewed the credit price and want to buy this 30-day card.", "Log out of the game before buying or activating here. Refresh an uncertain result before retrying.", "Enter the test game"],
  "zh-Hant": ["充值與月卡", "測試 USDC 充值", "購買月卡", "啟用月卡", "開啟錢包付款", "點券", "我已核對點券價格，確認購買此30天月卡。", "在此購買或啟用前，請先退出遊戲。結果未確認時先重新整理，勿重複操作。", "進入測試遊戲"],
  "pt-BR": ["Recarga e assinatura", "Recarregar USDC de teste", "Comprar cartão mensal", "Ativar cartão mensal", "Abrir pagamento na carteira", "Créditos", "Conferi o preço em créditos e quero comprar este cartão de 30 dias.", "Saia do jogo antes de comprar ou ativar aqui. Atualize um resultado incerto antes de tentar novamente.", "Entrar no jogo de teste"],
  ru: ["Пополнение и подписка", "Пополнить тестовыми USDC", "Купить месячную карту", "Активировать карту", "Открыть оплату в кошельке", "Кредиты", "Я проверил цену в кредитах и покупаю карту на 30 дней.", "Перед покупкой или активацией выйдите из игры. При неизвестном результате сначала обновите статус.", "Войти в тестовую игру"],
  hi: ["रिचार्ज और सदस्यता", "टेस्ट USDC से रिचार्ज", "मासिक कार्ड खरीदें", "मासिक कार्ड सक्रिय करें", "वॉलेट भुगतान खोलें", "क्रेडिट", "मैंने क्रेडिट मूल्य जाँचा है और 30 दिन का कार्ड खरीदना चाहता हूँ।", "यहाँ खरीदने या सक्रिय करने से पहले गेम से लॉग आउट करें। अनिश्चित परिणाम पर दोबारा कोशिश से पहले स्थिति ताज़ा करें।", "टेस्ट गेम खोलें"],
  id: ["Isi ulang dan keanggotaan", "Isi ulang USDC uji", "Beli kartu bulanan", "Aktifkan kartu bulanan", "Buka pembayaran dompet", "Kredit", "Saya sudah memeriksa harga kredit dan ingin membeli kartu 30 hari ini.", "Keluar dari game sebelum membeli atau mengaktifkan di sini. Perbarui hasil yang belum pasti sebelum mencoba lagi.", "Masuk ke game uji"],
  vi: ["Nạp tiền và thẻ tháng", "Nạp USDC thử nghiệm", "Mua thẻ tháng", "Kích hoạt thẻ tháng", "Mở thanh toán ví", "Điểm", "Tôi đã kiểm tra giá điểm và muốn mua thẻ 30 ngày này.", "Thoát trò chơi trước khi mua hoặc kích hoạt tại đây. Làm mới kết quả chưa rõ trước khi thử lại.", "Vào trò chơi thử nghiệm"],
  th: ["เติมเงินและบัตรรายเดือน", "เติม USDC ทดสอบ", "ซื้อบัตรรายเดือน", "เปิดใช้บัตรรายเดือน", "เปิดการชำระเงินในกระเป๋า", "เครดิต", "ฉันตรวจสอบราคาเครดิตแล้วและต้องการซื้อบัตร 30 วันนี้", "ออกจากเกมก่อนซื้อหรือเปิดใช้ที่นี่ หากผลยังไม่ชัดเจนให้รีเฟรชก่อนลองอีกครั้ง", "เข้าเกมทดสอบ"],
  ar: ["الشحن والعضوية", "الشحن بعملة USDC التجريبية", "شراء بطاقة شهرية", "تفعيل البطاقة الشهرية", "فتح الدفع بالمحفظة", "الرصيد", "راجعت السعر بالنقاط وأريد شراء هذه البطاقة لمدة 30 يومًا.", "اخرج من اللعبة قبل الشراء أو التفعيل هنا. حدّث النتيجة غير المؤكدة قبل إعادة المحاولة.", "دخول اللعبة التجريبية"],
} as const;

const RECOVERY_LABELS = {
  en: ["Continue original order", "A recharge is unresolved. Continue or check its original order before starting another.", "Order result pending confirmation."],
  "zh-Hant": ["繼續原訂單", "有充值訂單尚未確認，請繼續或查詢原訂單，再開始新的充值。", "訂單結果待確認。"],
  "pt-BR": ["Continuar pedido original", "Há uma recarga pendente. Continue ou consulte o pedido original antes de iniciar outra.", "Resultado do pedido aguardando confirmação."],
  ru: ["Продолжить исходный заказ", "Результат пополнения ещё не подтверждён. Проверьте исходный заказ перед новым.", "Результат заказа ожидает подтверждения."],
  hi: ["मूल ऑर्डर जारी रखें", "एक रिचार्ज की पुष्टि बाकी है। नया रिचार्ज शुरू करने से पहले मूल ऑर्डर जारी रखें या उसकी स्थिति जाँचें।", "ऑर्डर के परिणाम की पुष्टि बाकी है।"],
  id: ["Lanjutkan pesanan semula", "Ada isi ulang yang belum pasti. Lanjutkan atau periksa pesanan semula sebelum membuat yang baru.", "Hasil pesanan menunggu konfirmasi."],
  vi: ["Tiếp tục đơn ban đầu", "Một lần nạp chưa được xác nhận. Tiếp tục hoặc kiểm tra đơn ban đầu trước khi tạo đơn mới.", "Kết quả đơn đang chờ xác nhận."],
  th: ["ดำเนินการคำสั่งเดิมต่อ", "มีรายการเติมเงินที่ยังไม่ยืนยัน โปรดดำเนินการหรือตรวจสอบคำสั่งเดิมก่อนเริ่มรายการใหม่", "กำลังรอยืนยันผลคำสั่ง"],
  ar: ["متابعة الطلب الأصلي", "هناك عملية شحن لم تُحسم. تابع الطلب الأصلي أو تحقّق منه قبل بدء طلب جديد.", "نتيجة الطلب بانتظار التأكيد."],
} as const;

type BillingStatus = { credit: number; pendingCredit: number; monthlyCardCreditPrice: number | null; suiUsdcEnabled: boolean;
  suiUsdcOffers: { id: string; label: string; credits: number; amountMicros: string }[];
  ownedMonthlyCards: { uniqueId: string; label: string }[];
  suiUsdcOrders: CheckoutOrder[] };

export function PromotionBilling({ bootstrap, copy, locale }: { bootstrap: PromotionBootstrap; copy: PromotionCopy; locale: PromotionLocale }) {
  const labels = PROMOTION_BILLING_LABELS[locale];
  const recoveryLabels = RECOVERY_LABELS[locale];
  const credentialId = bootstrap.binding?.credentialId ?? "";
  const [character, setCharacter] = useState(bootstrap.characters[0]?.index ?? -1);
  const [status, setStatus] = useState<BillingStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const [checkout, setCheckout] = useState<string | null>(null);
  const [recoveries, setRecoveries] = useState<CheckoutIntent[]>([]);
  const [recoveryReady, setRecoveryReady] = useState(false);
  const keys = useRef(new Map<string, string>());
  const operationBusy = useRef(false);
  const context = `${credentialId}:${character}`;
  const activeContext = useRef(context); activeContext.current = context;
  const generation = useRef(0);
  const refresh = useCallback(async (signal?: AbortSignal) => {
    const current = ++generation.current;
    try {
      const value = await promotionRequest<BillingStatus>(`billing/status?characterIndex=${character}&requestId=${Date.now()}`, { signal });
      if (!signal?.aborted && current === generation.current) {
        setStatus(value);
        try {
          setRecoveries(recoverCheckoutIntents(window.sessionStorage, { credentialId, characterIndex: character }, value.suiUsdcOrders));
          setRecoveryReady(true); setError(false);
        } catch { setRecoveryReady(false); setError(true); }
      }
    } catch { if (!signal?.aborted && current === generation.current) setError(true); }
  }, [character, credentialId]);
  useEffect(() => {
    setStatus(null); setReviewed(false); setCheckout(null); setRecoveries([]); setRecoveryReady(false);
    keys.current.clear(); operationBusy.current = false; setBusy(false);
    if (character < 0) return;
    const controller = new AbortController(); void refresh(controller.signal);
    return () => { controller.abort(); ++generation.current; };
  }, [character, credentialId, refresh]);
  useEffect(() => {
    if (!bootstrap.characters.some(value => value.index === character)) setCharacter(bootstrap.characters[0]?.index ?? -1);
  }, [bootstrap.characters, character]);

  async function mutate(operation: "refresh" | "checkout" | "buy" | "activate", fields: Record<string, unknown> = {}, originalRequestId?: string) {
    if (operationBusy.current || !bootstrap.csrfToken) return;
    operationBusy.current = true; setBusy(true); setError(false);
    const requestContext = context;
    const scope = `${operation}:${character}:${JSON.stringify(fields)}`;
    let key = keys.current.get(scope) ?? crypto.randomUUID();
    try {
      if (operation === "checkout") {
        if (!status || !recoveryReady || typeof fields.offerId !== "string") throw new Error("checkout_recovery_required");
        const intent = prepareCheckoutIntent(window.sessionStorage, { credentialId, characterIndex: character }, status.suiUsdcOrders, fields.offerId, originalRequestId);
        key = intent.orderRequestId;
        setRecoveries(recoverCheckoutIntents(window.sessionStorage, { credentialId, characterIndex: character }, status.suiUsdcOrders));
      } else { keys.current.set(scope, key); }
      const result = await promotionRequest<{ checkoutUrl?: string }>(`billing/${operation}`, { csrfToken: bootstrap.csrfToken, idempotencyKey: key,
        body: { characterIndex: character, requestId: Date.now(), ...fields, ...(["checkout", "buy"].includes(operation) ? { orderRequestId: key } : {}) } });
      if (activeContext.current !== requestContext) return;
      if (operation !== "checkout") keys.current.delete(scope);
      if (result.checkoutUrl) setCheckout(result.checkoutUrl);
      if (operation === "buy") setReviewed(false);
      await refresh();
    } catch { if (activeContext.current === requestContext) setError(true); }
    finally { if (activeContext.current === requestContext) { operationBusy.current = false; setBusy(false); } }
  }
  return <section className={styles.section}><h2>{labels[0]}</h2><p className={styles.notice}>{copy.testnetBody}</p>
    <a className={styles.secondary} href={`/promotions/play${bootstrap.campaign?.campaignId ? `?campaignId=${encodeURIComponent(bootstrap.campaign.campaignId)}` : ""}`}>{labels[8]} ↗</a>
    {!bootstrap.characters.length ? <p className={styles.empty}>{copy.noCharacters}</p> : <>
      <label className={styles.field}>{copy.selectCharacter}<select value={character} disabled={busy} onChange={event => setCharacter(Number(event.target.value))}>
        {bootstrap.characters.map(item => <option key={item.index} value={item.index}>{item.name} · {item.level}</option>)}
      </select></label><p>{labels[7]}</p>
      <button className={styles.secondary} disabled={busy} onClick={() => void mutate("refresh")}>{copy.refresh}</button>
      {status ? <><p>{labels[5]}: {status.credit} · {copy.pending}: {status.pendingCredit}</p>
        <div className={styles.actions}>{status.suiUsdcEnabled ? status.suiUsdcOffers.map(offer => <button key={offer.id} className={styles.primary} disabled={busy || !recoveryReady || recoveries.length > 0} onClick={() => void mutate("checkout", { offerId: offer.id })}>
          {labels[1]} · {formatPromotionUsd(offer.amountMicros, locale)} · {offer.credits} {labels[5]}</button>) : null}</div>
        {recoveries.length > 0 ? <div role="status"><p>{recoveryLabels[1]}</p>
          {recoveries.map(intent => <p key={`${intent.offerId}:${intent.orderRequestId}`}>
            <span className={styles.code}>{intent.orderId || intent.orderRequestId} · {intent.state || recoveryLabels[2]}</span>{" "}
            <button className={styles.secondary} disabled={busy || !recoveryReady || intent.state === "review"} onClick={() => void mutate("checkout", { offerId: intent.offerId }, intent.orderRequestId)}>{recoveryLabels[0]}</button>
          </p>)}
          <button className={styles.secondary} disabled={busy} onClick={() => void refresh()}>{copy.refresh}</button>
        </div> : null}
        {checkout ? <p><a className={styles.primary} href={checkout} target="_blank" rel="noopener noreferrer">{labels[4]} ↗</a></p> : null}
        {status.monthlyCardCreditPrice !== null ? <><label className={styles.check}><input type="checkbox" checked={reviewed} disabled={busy} onChange={event => setReviewed(event.target.checked)} />{labels[6]} ({status.monthlyCardCreditPrice} {labels[5]})</label>
          <button className={styles.secondary} disabled={busy || !reviewed || status.credit < status.monthlyCardCreditPrice} onClick={() => void mutate("buy")}>{labels[2]}</button></> : null}
        {status.ownedMonthlyCards.map(card => <p key={card.uniqueId}>{card.label} <button className={styles.secondary} disabled={busy} onClick={() => void mutate("activate", { itemUniqueId: card.uniqueId })}>{labels[3]}</button></p>)}
        {status.suiUsdcOrders.map(order => <p key={order.id} className={styles.code}>{order.id} · {order.state}{order.transactionDigest ? ` · ${order.transactionDigest}` : ""}</p>)}
      </> : null}
    </>}{error ? <p role="alert">{copy.unavailable}</p> : null}
  </section>;
}
