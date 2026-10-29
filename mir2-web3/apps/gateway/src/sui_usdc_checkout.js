'use strict';
// Wallet Standard registration protocol; no signing key or game session enters
// this page. The fragment carries a short-lived, one-order capability only.
(() => {
  const byId = (id) => document.getElementById(id);
  const result = (s) => { byId('result').textContent = s; };
  const capability = location.hash.slice(1);
  history.replaceState(null, '', location.pathname);
  const validCapability = /^usdc_[0-9a-f]{64}\.[A-Za-z0-9_-]{43}$/.test(capability);
  let current, wallet, account, attempted = false, busy = false, validState = false;
  let walletGeneration = 0, offWalletChange;
  const USDC_TYPE = '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC';
  function changedWallet(message) {
    walletGeneration++; account = undefined;
    byId('pay').disabled = true; byId('accounts').disabled = true;
    result(message);
  }
  const wallets = new Set();
  const registered = (...items) => {
    for (const item of items) if (item && typeof item.name === 'string' && item.features?.['standard:connect'] && item.features?.['sui:signPersonalMessage'] && item.features?.['sui:signAndExecuteTransaction']) wallets.add(item);
    redrawWallets();
    return () => { for (const item of items) { wallets.delete(item); if (item === wallet) { changedWallet('钱包已断开，请重新连接并确认。'); wallet = undefined; } } redrawWallets(); };
  };
  const api = Object.freeze({ register: registered });
  function redrawWallets() {
    const select = byId('wallets'); const selected = select.value;
    select.replaceChildren();
    [...wallets].forEach((w, index) => { const o = document.createElement('option'); o.value = String(index); o.textContent = w.name; select.append(o); });
    if ([...select.options].some((o) => o.value === selected)) select.value = selected;
    if (!wallets.size) { const o = document.createElement('option'); o.textContent = '未发现支持的 Sui 钱包'; o.value = ''; select.append(o); }
  }
  window.addEventListener('wallet-standard:register-wallet', ({ detail }) => { if (typeof detail === 'function') detail(api); });
  window.dispatchEvent(new CustomEvent('wallet-standard:app-ready', { detail: api }));
  redrawWallets();
  async function request(operation, fields = {}) {
    if (!validCapability) throw Error('充值链接无效，请回游戏重新打开本订单。');
    const response = await fetch('/v1/billing/sui-usdc/session', { method: 'POST', credentials: 'omit', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${capability}` }, body: JSON.stringify({ operation, ...fields }) });
    const data = await response.json();
    if (!response.ok || data.error) throw Error(data.error === 'usdcCheckoutExpired' ? '充值链接已过期，请回游戏查看订单。已付款仍会继续到账。' : (data.error || '充值状态暂时未知，请刷新查询。'));
    return data;
  }
  function show(s) {
    const o = s?.order;
    if (!o || s.chain !== 'sui:testnet' || o.network !== 'testnet' || s.coinType !== USDC_TYPE || !/^0x[0-9a-f]{64}$/.test(s.merchant || '') || s.decimals !== 6 || s.testPayment !== true || s.gasPayer !== 'wallet' || !/^[1-9][0-9]*$/.test(o.amountMicros)) {
      validState = false; current = undefined; account = undefined;
      byId('connect').disabled = true; byId('pay').disabled = true;
      throw Error('充值网络或付款规则不匹配，已停止。');
    }
    current = s; validState = true;
    const n = BigInt(o.amountMicros);
    byId('details').textContent = `订单：${o.id}\n网络：Sui Testnet\n测试 USDC：${n / 1000000n}.${(n % 1000000n).toString().padStart(6, '0')}\n到账点券：${o.credits}\n收款方：${s.merchant}\n原生 USDC 类型：${s.coinType}\n付款钱包：${s.payer || '尚未确认'}\n付款截止：${new Date(o.expiresAtMs).toLocaleString()}\n订单状态：${o.state}${o.transactionDigest ? `\n交易：${o.transactionDigest}` : ''}`;
    byId('pay').disabled = busy || attempted || !s.invoiceId || s.payExpired || s.chainConfirmed || !account || account.address !== s.payer;
    byId('connect').disabled = busy || s.chainConfirmed || s.payExpired;
    if (s.gameDelivered) result('点券已到账。请回游戏查看余额；无需再次付款。');
    else if (s.chainConfirmed) result('链上付款已确认，游戏点券正在到账。请回游戏刷新或重新登录。');
    else if (attempted || o.state === 'paymentSubmitted') result('付款结果待核实。请刷新查询；不要再次付款。');
    else if (s.payExpired) result('付款期限已过。已提交的交易仍由服务器核对，不要再付款。');
    else if (s.invoiceId) result('链上账单已就绪，请核对金额、收款方和钱包后确认付款。');
    else if (s.payer) result('钱包已确认，服务器正在创建链上账单。');
  }
  async function refresh() { show(await request('status')); }
  byId('refresh').onclick = () => refresh().catch((e) => result(e.message));
  byId('accounts').onchange = () => changedWallet('账户已改变，请重新确认充值钱包。');
  byId('wallets').onchange = () => { offWalletChange?.(); offWalletChange = undefined; wallet = undefined; changedWallet('钱包已改变，请重新确认充值钱包。'); };
  byId('connect').onclick = async () => {
    if (busy || !validState) return; busy = true;
    let failure;
    try {
      wallet = [...wallets][Number(byId('wallets').value)];
      if (!wallet) throw Error('请先安装并选择支持 Sui 的钱包。');
      const connected = await wallet.features['standard:connect'].connect();
      offWalletChange?.();
      if (!wallet.features['standard:events']?.on) throw Error('钱包不支持账户状态通知，无法安全付款。');
      offWalletChange = wallet.features['standard:events'].on('change', (change) => {
        if (change.accounts) changedWallet('钱包账户已改变或断开，请重新确认充值钱包。');
      });
      const accounts = connected.accounts.filter((a) => a.chains?.includes('sui:testnet') && a.features?.includes('sui:signPersonalMessage') && a.features?.includes('sui:signAndExecuteTransaction'));
      if (!accounts.length) throw Error('钱包没有提供 Sui Testnet 账户，请切换到测试网。');
      const select = byId('accounts'); const selected = select.value; select.replaceChildren();
      accounts.forEach((a, i) => { const o = document.createElement('option'); o.value = String(i); o.textContent = a.address; select.append(o); });
      select.disabled = false;
      if (Number(selected) < accounts.length && selected !== '') select.value = selected;
      account = accounts[Number(select.value)];
      const selectedAccount = account, selectedWallet = wallet, generation = walletGeneration;
      if (current?.payer && current.payer !== account.address) throw Error('当前钱包与本订单已确认付款钱包不同。请切回原钱包。');
      const challenge = await request('challenge', { payer: account.address });
      if (generation !== walletGeneration || account !== selectedAccount || wallet !== selectedWallet) throw Error('钱包已改变，请重新确认。');
      const signed = await wallet.features['sui:signPersonalMessage'].signPersonalMessage({ account, chain: 'sui:testnet', message: new TextEncoder().encode(challenge.message) });
      if (generation !== walletGeneration || account !== selectedAccount || wallet !== selectedWallet) throw Error('钱包已改变，请重新确认。');
      show(await request('bindPayer', { payer: account.address, signature: signed.signature }));
    } catch (e) { failure = e.message || '钱包确认未完成。'; }
    finally { busy = false; if (current) show(current); if (failure) result(failure); }
  };
  byId('pay').onclick = async () => {
    if (busy || attempted || !validState || !wallet || !account || current?.payer !== account.address || !account.chains.includes('sui:testnet')) return;
    busy = true; byId('pay').disabled = true;
    let failure;
    try {
      const selectedAccount = account, selectedWallet = wallet, generation = walletGeneration;
      const built = await request('buildPay');
      show(built.state);
      if (!validState || generation !== walletGeneration || account !== selectedAccount || wallet !== selectedWallet || current.payer !== selectedAccount.address) throw Error('钱包或付款规则已改变，请重新确认。');
      const transaction = { toJSON: async () => built.transactionJson };
      attempted = true;
      result('请在钱包中核对并确认这笔 USDC 付款。');
      const submitted = await wallet.features['sui:signAndExecuteTransaction'].signAndExecuteTransaction({ transaction, account, chain: 'sui:testnet' });
      if (typeof submitted.digest !== 'string') throw Error('钱包未返回交易编号，结果未知。');
      show(await request('submitted', { digest: submitted.digest }));
    } catch (e) { failure = attempted ? '付款结果待核实，请刷新查询，不要重复付款。' : (e.message || '未能准备付款，请刷新查询。'); }
    finally { busy = false; if (current) show(current); if (failure) result(failure); }
  };
  if (!validCapability) { result('充值链接无效，请从游戏充值界面打开。'); byId('connect').disabled = true; }
  else { refresh().catch((e) => result(e.message)); setInterval(() => { if (!busy) refresh().catch((e) => result(e.message)); }, 5000); }
})();
