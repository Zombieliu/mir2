import https from 'node:https';
const base = new URL(process.env.MIR2_R2_UPLOAD_WORKER_URL.trim());
if (base.protocol !== 'https:' || base.username || base.password) throw new Error('Invalid existing upload service origin');
const url = new URL('/upload', base);
url.searchParams.set('key', 'invalid-probe-key-no-write');
const authorization = `Bearer ${process.env.MIR2_R2_UPLOAD_SECRET}`;
const response = await fetch(url, { method: 'PUT', redirect: 'error', headers: { authorization, 'content-type': 'application/octet-stream' }, body: new Uint8Array() });
const body = await response.text();
let json;
try { json = JSON.parse(body); } catch {}
const title = body.match(/<title>(.*?)<\/title>/s)?.[1]?.slice(0, 160);
console.log(JSON.stringify({ probe: 'existing_node_uploader_auth', status: response.status, error: json?.error, title, mitigated: response.headers.get('cf-mitigated') }));
if (response.status !== 400 || json?.error !== 'invalid_key') process.exit(1);
await new Promise((resolve) => {
  const probe = https.request(url, { method: 'PUT', headers: { authorization, 'content-type': 'application/octet-stream', 'content-length': '272465834', expect: '100-continue' } });
  probe.setTimeout(15000, () => probe.destroy(new Error('header probe timeout')));
  probe.on('response', (r) => { console.log(JSON.stringify({ probe: 'gzip_size_header_without_body_or_write', status: r.statusCode })); r.resume(); resolve(); probe.destroy(); });
  probe.on('error', () => { console.log(JSON.stringify({ probe: 'gzip_size_header_without_body_or_write', status: 'no_early_response' })); resolve(); });
  probe.flushHeaders();
});
