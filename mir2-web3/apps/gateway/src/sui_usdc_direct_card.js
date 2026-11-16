const USDC_TYPE = '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC';
const ADDRESS = /^0x[0-9a-f]{64}$/;
const ORDER_ID = /^usdc_[0-9a-f]{64}$/;
const NOT_READY = '直转自动到账接入中，请勿付款';
const READY = '请使用下方已验证的钱包，在 Sui Testnet 支付指定测试 USDC；点券到账以服务器核验为准。';
const isAddress = (value) => typeof value === 'string' && ADDRESS.test(value) && value !== `0x${'0'.repeat(64)}`;

function formatMicros(value) {
  if (typeof value !== 'string' || !/^[1-9][0-9]{0,19}$/.test(value)) return null;
  const micros = BigInt(value);
  if (micros > 18446744073709551615n) return null;
  return `${micros / 1000000n}.${(micros % 1000000n).toString().padStart(6, '0')}`;
}

// The native wallet URL is a payment request, not a transaction receipt. This
// allowlist also prevents the QR renderer from becoming an arbitrary URL sink.
export function validateDirectPaymentRequestUrl(value) {
  if (typeof value !== 'string' || value.length > 4096 || /[\s#]/.test(value) ||
      !value.startsWith('https://wallet.dubhe.obelisk.build/send?')) return null;
  let url;
  try { url = new URL(value); } catch { return null; }
  if (url.origin !== 'https://wallet.dubhe.obelisk.build' || url.pathname !== '/send' ||
      url.username || url.password || url.hash) return null;
  const expectedKeys = ['recipient', 'coinType', 'amount', 'requestId', 'request'];
  const keys = [...url.searchParams.keys()];
  if (keys.length !== expectedKeys.length || expectedKeys.some((key) => url.searchParams.getAll(key).length !== 1)) return null;
  const fields = Object.fromEntries(url.searchParams);
  if (!isAddress(fields.recipient) || fields.coinType !== USDC_TYPE || !ORDER_ID.test(fields.requestId || '') ||
      fields.request !== '1' || !/^(?:0|[1-9][0-9]{0,13})\.[0-9]{6}$/.test(fields.amount || '')) return null;
  const micros = `${fields.amount.split('.')[0]}${fields.amount.split('.')[1]}`.replace(/^0+/, '');
  if (formatMicros(micros) !== fields.amount) return null;
  return fields;
}

function approvedState(state, clock) {
  const order = state?.order, direct = state?.directTransfer;
  if (state?.paymentMode !== 'direct_transfer' || direct?.available !== true || direct.payerVerified !== true ||
      state.chain !== 'sui:testnet' || order?.network !== 'testnet' || state.coinType !== USDC_TYPE ||
      state.decimals !== 6 || state.testPayment !== true || state.gasPayer !== 'wallet' ||
      state.invoiceId != null || order.transactionDigest != null ||
      state.chainConfirmed !== false || state.gameDelivered !== false || state.payExpired !== false ||
      !['prepared', 'awaitingPayment'].includes(order.state) || !ORDER_ID.test(order.id || '') ||
      !isAddress(state.payer) || !isAddress(state.merchant) || state.payer === state.merchant ||
      !Number.isSafeInteger(clock) || !Number.isSafeInteger(order.expiresAtMs) || order.expiresAtMs <= clock) return null;
  const amount = formatMicros(order.amountMicros);
  if (!amount || direct.payer !== state.payer || direct.merchant !== state.merchant || direct.chain !== state.chain ||
      direct.coinType !== state.coinType || direct.amountMicros !== order.amountMicros || direct.expiresAtMs !== order.expiresAtMs) return null;
  const request = validateDirectPaymentRequestUrl(direct.walletRequestUrl);
  if (!request || request.recipient !== state.merchant || request.amount !== amount || request.requestId !== order.id) return null;
  return Object.freeze({ payer: state.payer, merchant: state.merchant, amount, expiresAtMs: order.expiresAtMs,
    id: order.id, walletRequestUrl: direct.walletRequestUrl });
}

export function createDirectTransferCard({ document, clipboard, renderQr, now = Date.now }) {
  const element = (id) => document.getElementById(id);
  const panel = element('direct-transfer-panel'), qr = element('direct-transfer-qr');
  const placeholder = element('direct-transfer-placeholder'), address = element('direct-transfer-address');
  const copy = element('copy-direct-address'), amount = element('direct-transfer-amount');
  const payer = element('direct-transfer-payer'), copyStatus = element('direct-transfer-copy-status');
  const status = element('direct-transfer-status');
  let current, lastState, fingerprint, generation = 0, disposed = false, expiryTimer;
  address.readOnly = true;
  panel.hidden = false;

  function retire(message = NOT_READY) {
    generation++; clearTimeout(expiryTimer); expiryTimer = undefined;
    current = undefined; fingerprint = undefined;
    qr.hidden = true; qr.removeAttribute('src'); placeholder.hidden = false;
    address.value = ''; copy.disabled = true; amount.textContent = '—'; payer.textContent = '尚未绑定';
    copyStatus.textContent = ''; status.textContent = message;
  }
  function stillCurrent(snapshot, revision) {
    const approved = !disposed && approvedState(lastState, now());
    return Boolean(approved && current === snapshot && revision === generation &&
      JSON.stringify(approved) === fingerprint);
  }
  function scheduleExpiry(snapshot, revision) {
    clearTimeout(expiryTimer);
    expiryTimer = setTimeout(() => {
      if (current !== snapshot || generation !== revision || disposed) return;
      if (!stillCurrent(snapshot, revision)) retire('付款期限已过或订单状态已改变，请回游戏查看订单；勿再次付款。');
      else scheduleExpiry(snapshot, revision);
    }, Math.min(2147483647, Math.max(1, snapshot.expiresAtMs - now())));
    expiryTimer.unref?.();
  }
  async function copyAddress(event) {
    event?.preventDefault();
    const snapshot = current, revision = generation;
    if (!snapshot || !stillCurrent(snapshot, revision)) {
      if (!disposed) retire();
      return;
    }
    try {
      if (typeof clipboard?.writeText !== 'function') throw Error('Clipboard unavailable');
      // Never read the address back from the editable DOM or copy the request URL.
      await clipboard.writeText(snapshot.merchant);
      if (stillCurrent(snapshot, revision)) copyStatus.textContent = '收款地址已复制；请同时核对测试网、USDC 币种和金额。';
      else if (!disposed && current === snapshot) retire();
    } catch {
      if (!stillCurrent(snapshot, revision)) {
        if (!disposed && current === snapshot) retire();
        return;
      }
      address.value = snapshot.merchant; address.focus(); address.select();
      copyStatus.textContent = '未能自动复制，已选中收款地址，请手动复制。';
    }
  }
  copy.addEventListener('click', copyAddress);
  retire();
  return Object.freeze({
    show(state) {
      if (disposed) return;
      lastState = state;
      const approved = approvedState(state, now());
      if (!approved) {
        const ended = state?.payExpired || state?.chainConfirmed || state?.gameDelivered || state?.order?.state === 'paymentSubmitted';
        retire(ended ? '此订单已停止收款，请回游戏查看到账状态；勿再次付款。' : NOT_READY);
        return;
      }
      const nextFingerprint = JSON.stringify(approved);
      if (fingerprint === nextFingerprint) return;
      retire();
      current = approved; fingerprint = nextFingerprint;
      const revision = generation;
      address.value = approved.merchant; amount.textContent = `${approved.amount} 测试 USDC`;
      payer.textContent = approved.payer; copy.disabled = false; status.textContent = READY;
      scheduleExpiry(approved, revision);
      Promise.resolve().then(() => {
        if (stillCurrent(approved, revision)) return renderQr(approved.walletRequestUrl);
      }).then((src) => {
        if (!stillCurrent(approved, revision)) {
          if (!disposed && current === approved && generation === revision) retire();
          return;
        }
        if (typeof src !== 'string' || src.length > 2097152 || !/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(src)) throw Error('Invalid QR image');
        qr.src = src; qr.hidden = false; placeholder.hidden = true;
      }).catch(() => {
        if (stillCurrent(approved, revision)) status.textContent = '二维码生成失败；可复制收款地址，并核对测试网、币种、金额和已验证的钱包。';
        else if (!disposed && current === approved && generation === revision) retire();
      });
    },
    dispose() {
      if (disposed) return;
      disposed = true; lastState = undefined; retire(); copy.removeEventListener('click', copyAddress);
    },
  });
}
