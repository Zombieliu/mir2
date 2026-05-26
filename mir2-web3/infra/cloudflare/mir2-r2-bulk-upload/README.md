# Mir2 R2 Bulk Upload Worker

Authenticated helper Worker for publishing generated Mir2 Web asset releases to
R2 without spawning one Wrangler process per object.

Deploy with a one-time secret file outside the repository:

```bash
node -e 'console.log(JSON.stringify({ MIR2_R2_UPLOAD_SECRET: crypto.randomUUID() }))' > /tmp/mir2-r2-upload-secret.json
npx wrangler deploy \
  --config infra/cloudflare/mir2-r2-bulk-upload/wrangler.jsonc \
  --secrets-file /tmp/mir2-r2-upload-secret.json
```

Upload through the Worker-backed driver:

```bash
MIR2_R2_BUCKET=mir2-web3-assets \
MIR2_R2_UPLOAD_DRIVER=worker \
MIR2_R2_UPLOAD_WORKER_URL=https://mir2-r2-bulk-upload.<workers-subdomain>.workers.dev \
MIR2_R2_UPLOAD_SECRET=<secret> \
npm run assets:r2:upload -- --driver worker
```
