"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PromotionCopy, PromotionLocale } from "./promotion-copy";
import { formatPromotionUsd, promotionRequest, type PromotionBootstrap } from "./promotion-model";
import styles from "./promotion.module.css";

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

type BillingStatus = { credit: number; pendingCredit: number; monthlyCardCreditPrice: number | null; suiUsdcEnabled: boolean;
  suiUsdcOffers: { id: string; label: string; credits: number; amountMicros: string }[];
  ownedMonthlyCards: { uniqueId: string; label: string }[];
  suiUsdcOrders: { id: string; state: string; transactionDigest?: string | null }[] };

export function PromotionBilling({ bootstrap, copy, locale }: { bootstrap: PromotionBootstrap; copy: PromotionCopy; locale: PromotionLocale }) {
  const labels = PROMOTION_BILLING_LABELS[locale];
  const [character, setCharacter] = useState(bootstrap.characters[0]?.index ?? -1);
  const [status, setStatus] = useState<BillingStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const [checkout, setCheckout] = useState<string | null>(null);
  const keys = useRef(new Map<string, string>());
  const generation = useRef(0);
  const refresh = useCallback(async (signal?: AbortSignal) => {
    const current = ++generation.current;
    try {
      const value = await promotionRequest<BillingStatus>(`billing/status?characterIndex=${character}&requestId=${Date.now()}`, { signal });
      if (!signal?.aborted && current === generation.current) { setStatus(value); setError(false); }
    } catch { if (!signal?.aborted && current === generation.current) setError(true); }
  }, [character]);
  useEffect(() => {
    setStatus(null); setReviewed(false); setCheckout(null);
    if (character < 0) return;
    const controller = new AbortController(); void refresh(controller.signal);
    return () => { controller.abort(); ++generation.current; };
  }, [character, refresh]);

  async function mutate(operation: "refresh" | "checkout" | "buy" | "activate", fields: Record<string, unknown> = {}) {
    if (busy || !bootstrap.csrfToken) return;
    setBusy(true); setError(false);
    const scope = `${operation}:${character}:${JSON.stringify(fields)}`;
    const key = keys.current.get(scope) ?? crypto.randomUUID(); keys.current.set(scope, key);
    try {
      const result = await promotionRequest<{ checkoutUrl?: string }>(`billing/${operation}`, { csrfToken: bootstrap.csrfToken, idempotencyKey: key,
        body: { characterIndex: character, requestId: Date.now(), ...fields, ...(["checkout", "buy"].includes(operation) ? { orderRequestId: key } : {}) } });
      keys.current.delete(scope);
      if (result.checkoutUrl) setCheckout(result.checkoutUrl);
      if (operation === "buy") setReviewed(false);
      await refresh();
    } catch { setError(true); } finally { setBusy(false); }
  }
  return <section className={styles.section}><h2>{labels[0]}</h2><p className={styles.notice}>{copy.testnetBody}</p>
    <a className={styles.secondary} href="/promotions/play">{labels[8]} ↗</a>
    {!bootstrap.characters.length ? <p className={styles.empty}>{copy.noCharacters}</p> : <>
      <label className={styles.field}>{copy.selectCharacter}<select value={character} disabled={busy} onChange={event => setCharacter(Number(event.target.value))}>
        {bootstrap.characters.map(item => <option key={item.index} value={item.index}>{item.name} · {item.level}</option>)}
      </select></label><p>{labels[7]}</p>
      <button className={styles.secondary} disabled={busy} onClick={() => void mutate("refresh")}>{copy.refresh}</button>
      {status ? <><p>{labels[5]}: {status.credit} · {copy.pending}: {status.pendingCredit}</p>
        <div className={styles.actions}>{status.suiUsdcEnabled ? status.suiUsdcOffers.map(offer => <button key={offer.id} className={styles.primary} disabled={busy} onClick={() => void mutate("checkout", { offerId: offer.id })}>
          {labels[1]} · {formatPromotionUsd(offer.amountMicros, locale)} · {offer.credits} {labels[5]}</button>) : null}</div>
        {checkout ? <p><a className={styles.primary} href={checkout} target="_blank" rel="noopener noreferrer">{labels[4]} ↗</a></p> : null}
        {status.monthlyCardCreditPrice !== null ? <><label className={styles.checkbox}><input type="checkbox" checked={reviewed} disabled={busy} onChange={event => setReviewed(event.target.checked)} />{labels[6]} ({status.monthlyCardCreditPrice} {labels[5]})</label>
          <button className={styles.secondary} disabled={busy || !reviewed || status.credit < status.monthlyCardCreditPrice} onClick={() => void mutate("buy")}>{labels[2]}</button></> : null}
        {status.ownedMonthlyCards.map(card => <p key={card.uniqueId}>{card.label} <button className={styles.secondary} disabled={busy} onClick={() => void mutate("activate", { itemUniqueId: card.uniqueId })}>{labels[3]}</button></p>)}
        {status.suiUsdcOrders.map(order => <p key={order.id} className={styles.code}>{order.id} · {order.state}{order.transactionDigest ? ` · ${order.transactionDigest}` : ""}</p>)}
      </> : null}
    </>}{error ? <p role="alert">{copy.unavailable}</p> : null}
  </section>;
}
