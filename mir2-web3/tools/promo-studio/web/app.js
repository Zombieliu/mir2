'use strict';
const $ = (selector) => document.querySelector(selector);
const state = { status: null, campaignId: null, script: null, selected: new Set(), busy: false, uploading: false, jobsSignature: '', assetsSignature: '', initial: true, activePreview: null };
const kinds = { ai_creative: 'AI 情境', gameplay: '遊戲實錄', game_screenshot: '遊戲截圖' };
const tagNames = { bichon: '比奇', mining: '挖礦', classes: '三職業', warrior: '戰士', wizard: '法師', taoist: '道士' };

function node(tag, className, text) { const item = document.createElement(tag); if (className) item.className = className; if (text !== undefined) item.textContent = text; return item; }
function notice(text, error = false) { const item = $('#notice'); item.textContent = text; item.classList.toggle('error', error); item.hidden = !text; }
async function api(url, body) { const response = await fetch(url, body === undefined ? { cache: 'no-store' } : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }); const data = await response.json(); if (!response.ok) throw new Error(data.error || '請求失敗'); return data; }
function campaign() { return state.status?.campaigns.find((item) => item.id === state.campaignId); }
function localized(item) { return { ...item, ...(item.locales?.[$('#language').value] || {}) }; }
function template(item) { const c = localized(item); const script = {}; for (const key of ['title', 'theme', 'description', 'claims', 'shots', 'cta', 'hashtags', 'creativeTokens']) script[key] = c[key] || (['claims', 'shots', 'hashtags', 'creativeTokens'].includes(key) ? [] : ''); return structuredClone(script); }
function duration(script) { return (script?.shots || []).reduce((total, shot) => total + Number(shot.seconds || 0), 0); }
function setBusy(value) { state.busy = value; $('#brief-button').disabled = value; $('#apply-script').disabled = value; updateReady(); }

function renderCampaigns() {
  const list = $('#campaigns'); list.replaceChildren();
  for (const [index, item] of state.status.campaigns.entries()) {
    const button = node('button', 'campaign' + (item.id === state.campaignId ? ' selected' : '')); button.type = 'button'; button.dataset.campaign = item.id; button.setAttribute('aria-pressed', String(item.id === state.campaignId));
    button.append(node('span', 'icon', ['⌂', '◇', '✦'][index % 3]));
    const copy = node('span', 'copy'); copy.append(node('strong', '', item.title), node('small', '', item.theme)); button.append(copy, node('span', 'length', `${duration(item)}s`));
    button.addEventListener('click', () => selectCampaign(item.id)); list.append(button);
  }
}
function selectCampaign(id) {
  state.campaignId = id; const item = campaign(); if (!item) return;
  const language = $('#language'); const previous = language.value; language.replaceChildren();
  for (const code of item.languages || ['zh-TW']) { const option = node('option', '', code === 'en' ? 'English' : '繁體中文'); option.value = code; language.append(option); }
  language.value = (item.languages || ['zh-TW']).includes(previous) ? previous : 'zh-TW';
  state.script = template(item); renderCampaigns(); renderScript(); showStoryboard(); updateReady();
}
function renderScript() {
  $('#script-json').value = JSON.stringify(state.script, null, 2);
  const list = $('#shots'); list.replaceChildren();
  for (const [index, shot] of state.script.shots.entries()) {
    const row = node('div', 'shot'); row.append(node('span', 'shot-number', String(index + 1).padStart(2, '0')));
    const copy = node('div'); const heading = node('div', 'shot-label'); heading.append(node('strong', '', shot.headline), node('small', '', `${shot.assetRole === 'creative' ? '情境' : '實錄'} · ${shot.seconds}s`));
    copy.append(heading, node('p', '', shot.caption)); if (shot.narration) copy.append(node('p', 'narration', `旁白｜${shot.narration}`)); row.append(copy); list.append(row);
  }
  $('#claims').replaceChildren(...state.script.claims.map((text) => node('li', '', text))); $('#fact-count').textContent = `${state.script.claims.length} 項`;
  $('#poster-title').textContent = state.script.title; $('#poster-caption').textContent = state.script.theme;
}
function providerLabel() {
  const provider = state.status?.provider || {};
  $('#provider-label').textContent = provider.configured ? 'AI 腳本介面已設定 · 生成會使用已核驗的選題內容。' : '目前使用 AI 編寫的預設分鏡。連接模型介面後可生成新組合。';
  $('#brief-button').textContent = provider.configured ? 'AI 生成分鏡' : '載入預設分鏡';
  const available = (provider.systemVoices || []).some((voice) => voice.culture.toLowerCase().startsWith($('#language').value === 'en' ? 'en' : 'zh'));
  $('#voice').options[1].disabled = !available; if (!available && $('#voice').value === 'system') $('#voice').value = 'none';
}
function updateReady() {
  const item = campaign(); const selected = state.status?.assets.filter((a) => state.selected.has(a.id)) || [];
  const tags = new Set(selected.flatMap((a) => a.tags || [])); const missing = (item?.requiredTags || []).filter((tag) => !tags.has(tag));
  const roles = new Set(selected.map((a) => a.provenance.kind === 'ai_creative' ? 'creative' : 'gameplay')); const absentRoles = [...new Set(state.script?.shots.map((s) => s.assetRole) || [])].filter((role) => !roles.has(role));
  const messages = [];
  if (state.status?.health?.storage === 'error') messages.push('狀態儲存異常，請修復磁碟或資料目錄後重新啟動工作室');
  if (missing.length) messages.push(`需要 ${missing.map((tag) => tagNames[tag] || tag).join('、')} 素材`);
  if (absentRoles.length) messages.push(`請加入${absentRoles.map((role) => role === 'creative' ? ' AI 情境圖' : '遊戲實錄或截圖').join('與')}`);
  if (selected.length > 12) messages.push('一次最多選取 12 個素材');
  if (!$('#vertical').checked && !$('#landscape').checked) messages.push('請選擇至少一種畫幅');
  $('#material-check').textContent = messages.length ? messages.join('；') : `已選 ${selected.length} 個素材 · ${duration(state.script)} 秒 · 可以製作`;
  $('#create-button').disabled = !item || !!messages.length || state.busy || state.uploading;
}
function showStoryboard() {
  state.activePreview = null; const video = $('#video-preview'); video.pause(); video.removeAttribute('src'); video.hidden = true;
  $('#preview-frame').classList.remove('playing'); $('#poster-copy').hidden = false; $('#preview-downloads').hidden = true; $('#preview-label').textContent = '分鏡預覽';
  const asset = state.status?.assets.find((a) => state.selected.has(a.id) && a.provenance.kind === 'ai_creative') || state.status?.assets.find((a) => a.provenance.kind === 'ai_creative');
  $('#poster').hidden = !asset; $('#preview-empty').hidden = !!asset;
  if (asset) $('#poster').src = asset.mediaUrl;
}
function renderAssets() {
  const list = $('#assets'); list.replaceChildren(); $('#asset-count').textContent = `${state.status.assets.length} 個素材`;
  if (!state.status.assets.length) list.append(node('p', 'muted small', '加入原創情境圖與真實遊戲錄影。'));
  for (const asset of state.status.assets) {
    const label = node('label', 'asset' + (state.selected.has(asset.id) ? ' selected' : '')); label.dataset.asset = asset.id;
    const image = node('img'); image.src = asset.thumbnailUrl; image.alt = asset.label; image.loading = 'lazy';
    const copy = node('span', 'asset-copy'); const seconds = asset.durationSeconds ? ` · ${asset.durationSeconds.toFixed(1)}s` : '';
    copy.append(node('strong', '', asset.label), node('small', '', `${kinds[asset.provenance.kind]} · ${asset.width}×${asset.height}${seconds}`), node('small', '', (asset.tags || []).map((tag) => tagNames[tag] || tag).join(' / ')));
    const checkbox = node('input'); checkbox.type = 'checkbox'; checkbox.checked = state.selected.has(asset.id); checkbox.setAttribute('aria-label', `選取 ${asset.label}`);
    checkbox.addEventListener('change', () => { if (checkbox.checked) state.selected.add(asset.id); else state.selected.delete(asset.id); label.classList.toggle('selected', checkbox.checked); updateReady(); if (!state.activePreview) showStoryboard(); });
    label.append(image, copy, checkbox); list.append(label);
  }
}
function downloadLink(url, label, filename) { const link = node('a', '', label); link.href = url; link.download = filename || ''; return link; }
function preview(job, output) {
  state.activePreview = `${job.id}/${output.format}`; $('#preview-frame').classList.add('playing'); $('#poster').hidden = true; $('#preview-empty').hidden = true; $('#poster-copy').hidden = true;
  const video = $('#video-preview'); video.hidden = false; video.poster = output.coverUrl; video.src = output.videoUrl; video.load();
  $('#preview-label').textContent = `${output.format === 'vertical' ? '9:16' : '16:9'} · ${output.durationSeconds.toFixed(1)}s`;
  const links = $('#preview-downloads'); links.hidden = false; links.replaceChildren(downloadLink(output.videoUrl, '下載 MP4'), downloadLink(output.coverUrl, '封面'), downloadLink(output.subtitleUrl, '字幕 SRT'), downloadLink(output.metadataUrl, '投稿素材包'));
  $('#preview-frame').scrollIntoView({ behavior: 'smooth', block: 'center' });
}
function renderJobs() {
  const list = $('#jobs'); list.replaceChildren(); const jobs = [...state.status.jobs].sort((a, b) => b.createdAt - a.createdAt);
  if (!jobs.length) list.append(node('p', 'muted', '還沒有成片。完成第一個故事吧。'));
  for (const job of jobs) {
    const card = node('article', 'job'); card.dataset.job = job.id; const thumb = node('img', 'job-thumbnail'); const output = job.outputs?.[0];
    if (output) { thumb.src = output.coverUrl; thumb.alt = job.script.title; } else { thumb.alt = ''; const a = state.status.assets.find((asset) => job.assetIds.includes(asset.id)); if (a) thumb.src = a.thumbnailUrl; }
    const body = node('div'); body.append(node('div', 'job-title', job.script.title)); body.append(node('p', 'job-subtitle', new Date(job.createdAt).toLocaleString('zh-TW') + ' · ' + (job.language === 'en' ? 'English' : '繁體中文')));
    const meta = node('div', 'job-meta'); for (const format of job.formats) meta.append(node('span', '', format === 'vertical' ? '9:16 直式' : '16:9 橫式')); meta.append(node('span', '', job.voiceMode === 'system' ? '系統旁白' : '字幕版')); body.append(meta);
    const actions = node('div', 'job-actions');
    if (job.status === 'completed') for (const out of job.outputs) {
      const play = node('button', 'button subtle', `預覽 ${out.format === 'vertical' ? '直式' : '橫式'}`); play.type = 'button'; play.dataset.preview = `${job.id}/${out.format}`; play.addEventListener('click', () => preview(job, out)); actions.append(play, downloadLink(out.videoUrl, `MP4 ${out.format === 'vertical' ? '9:16' : '16:9'}`), downloadLink(out.subtitleUrl, '字幕'), downloadLink(out.coverUrl, '封面'), downloadLink(out.metadataUrl, '投稿文案'));
    }
    if (job.status === 'error') { body.append(node('p', 'job-error', job.error || '製作失敗，請檢查素材。')); const retry = node('button', 'button subtle', '重新製作'); retry.type = 'button'; retry.addEventListener('click', () => enqueue({campaignId:job.campaignId,assetIds:job.assetIds,formats:job.formats,language:job.language,voiceMode:job.voiceMode,script:job.script})); actions.append(retry); }
    if (job.status === 'queued' || job.status === 'running') { const progress = node('progress'); progress.max = 100; progress.value = job.progress; progress.setAttribute('aria-label', '影片編碼進度'); body.append(progress); }
    body.append(actions); const status = node('span', 'job-status' + (job.status === 'error' ? ' error' : ''), { completed: '已生成 · 待核對', error: '製作失敗', queued: '排隊中', running: `製作中 · ${Math.round(job.progress)}%` }[job.status] || job.status); card.append(thumb, body, status); list.append(card);
  }
}
async function refresh() {
  try {
    const status = await api('/api/status'); const previousIds = new Set(state.status?.assets.map((a) => a.id) || []); state.status = status;
    $('#connection-dot').classList.add('online'); $('#connection-label').textContent = '本機工作室已連線';
    for (const asset of status.assets) if (!previousIds.has(asset.id)) state.selected.add(asset.id);
    if (!state.campaignId && status.campaigns.length) selectCampaign(status.campaigns[0].id);
    providerLabel();
    const assetsSignature = JSON.stringify(status.assets); if (assetsSignature !== state.assetsSignature) { state.assetsSignature = assetsSignature; renderAssets(); if (!state.activePreview) showStoryboard(); }
    const jobsSignature = JSON.stringify(status.jobs); if (jobsSignature !== state.jobsSignature) { state.jobsSignature = jobsSignature; renderJobs(); }
    updateReady(); state.initial = false;
    if (status.health?.storage === 'error') notice('工作室暫停製作：' + (status.health.error || '無法儲存佇列。請檢查磁碟空間後重新啟動。'), true);
    if (!status.tooling?.ffmpegAvailable || !status.tooling?.ffprobeAvailable) notice('缺少 FFmpeg 或 ffprobe。請先依 README 安裝影片工具。', true);
  } catch (error) { $('#connection-dot').classList.remove('online'); $('#connection-label').textContent = '連線中斷'; if (state.initial) notice('工作室還未就緒：' + error.message, true); }
}
async function enqueue(body) { setBusy(true); try { const job = await api('/api/jobs', body); notice('短片已加入背景製作佇列。你可以繼續編輯下一支。'); await refresh(); document.querySelector(`[data-job="${job.id}"]`)?.scrollIntoView({behavior:'smooth',block:'center'}); } catch (error) { notice(error.message, true); } finally { setBusy(false); } }
$('#create-button').addEventListener('click', () => enqueue({ campaignId: state.campaignId, assetIds: [...state.selected], formats: [$('#vertical').checked && 'vertical', $('#landscape').checked && 'landscape'].filter(Boolean), language: $('#language').value, voiceMode: $('#voice').value, script: state.script }));
$('#brief-button').addEventListener('click', async () => { setBusy(true); try { const result = await api('/api/brief', { campaignId: state.campaignId, language: $('#language').value }); state.script = result.script; renderScript(); showStoryboard(); notice(result.mode === 'provider' ? '已生成 AI 分鏡，內容仍限制在核驗過的玩法與情境。' : '已載入預設分鏡。即時模型介面尚未設定。'); } catch (error) { notice(error.message, true); } finally { setBusy(false); } });
$('#apply-script').addEventListener('click', () => { try { const script = JSON.parse($('#script-json').value); if (!script || !Array.isArray(script.shots) || !Array.isArray(script.claims)) throw new Error('需要完整的 shots 和 claims 欄位'); state.script = script; renderScript(); showStoryboard(); updateReady(); notice('已套用分鏡。製作時會再驗證內容和秒數。'); } catch (error) { notice('分鏡格式有誤：' + error.message, true); } });
$('#language').addEventListener('change', () => { state.script = template(campaign()); providerLabel(); renderScript(); showStoryboard(); updateReady(); });
$('#vertical').addEventListener('change', updateReady); $('#landscape').addEventListener('change', updateReady);
function uploadParameters(filename) { return new URLSearchParams({ filename, label: filename.replace(/\.[^.]+$/, ''), kind: $('#provenance').value, source: $('#asset-source').value, tags: $('#asset-tag').value }); }
async function upload(files) {
  if (state.uploading) return; state.uploading = true; updateReady(); const progress = $('#upload-progress'); progress.hidden = false;
  try { for (const [index, file] of [...files].entries()) {
    if (file.size > 2 * 1024 ** 3) throw new Error('單個素材不能超過 2 GB'); notice(`正在匯入 ${file.name}（${index + 1}/${files.length}）`); progress.value = 0;
    await new Promise((resolve, reject) => { const xhr = new XMLHttpRequest(); xhr.open('POST', '/api/upload?' + uploadParameters(file.name)); xhr.setRequestHeader('Content-Type', 'application/octet-stream'); xhr.timeout = 300000; xhr.upload.addEventListener('progress', (event) => { if (event.lengthComputable) progress.value = event.loaded / event.total * 100; }); xhr.onload = () => { let result; try { result = JSON.parse(xhr.responseText); } catch { reject(new Error('無法讀取匯入結果')); return; } if (xhr.status >= 200 && xhr.status < 300) resolve(result); else reject(new Error(result.error || '素材匯入失敗')); }; xhr.onerror = () => reject(new Error('素材匯入連線中斷')); xhr.ontimeout = () => reject(new Error('素材匯入逾時')); xhr.send(file); });
    await refresh();
  } notice('素材已加入。請核對來源與玩法標籤，再選取製作。'); } catch (error) { notice(error.message, true); } finally { state.uploading = false; progress.hidden = true; $('#file-input').value = ''; updateReady(); }
}
$('#file-input').addEventListener('change', (event) => upload(event.target.files)); const drop = $('#drop-zone'); for (const type of ['dragenter', 'dragover']) drop.addEventListener(type, (event) => { event.preventDefault(); drop.classList.add('dragging'); }); for (const type of ['dragleave', 'drop']) drop.addEventListener(type, (event) => { event.preventDefault(); drop.classList.remove('dragging'); }); drop.addEventListener('drop', (event) => upload(event.dataTransfer.files));
$('#import-button').addEventListener('click', async () => { const button = $('#import-button'); button.disabled = true; try { const path = $('#import-path').value; await api('/api/assets', { path, label: path.split(/[\\/]/).pop().replace(/\.[^.]+$/, ''), tags: $('#asset-tag').value.split(','), provenance: { kind: $('#provenance').value, source: $('#asset-source').value } }); await refresh(); notice('本機素材已匯入。原檔保留。'); $('#import-path').value = ''; } catch (error) { notice(error.message, true); } finally { button.disabled = false; } });
refresh(); setInterval(refresh, 2500);
