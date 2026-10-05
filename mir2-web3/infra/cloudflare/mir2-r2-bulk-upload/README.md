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

For a one-off Worker mounted behind a path-specific route, set
`MIR2_R2_UPLOAD_WORKER_PATH=/unique-upload-path`. The default remains
`/upload`; the explicit override lets a temporary publisher coexist with the
production read route without changing its credentials or bindings.

## Fixed native R17 delivery

The reviewed `/upload/native-r17` extension uses the existing
`MIR2_R2_UPLOAD_SECRET` and `MIR2_ASSETS` binding. Preserve that secret when
deploying an existing Worker; the one-time Web example above is not a native
secret rotation step. Native clients never receive an upload credential.

Only the 36 frozen R17 objects in `src/native-r17-publication-plan.json` may be
imported. The Worker streams each object from the fixed origin and asks R2 to
validate SHA256 with a create-only condition. The legacy Web PUT refuses
`mir2/native/windows-invited/` so it cannot bypass immutable native publishing.
Other Web paths keep their existing behavior.

Import and private inspection do not change the channel pointer. Promotion is
a separate authenticated operation after full public byte verification; it
requires the exact retained stage envelope, rechecks the source signed pair
and creates the absent pointer conditionally. Public alias verification is
still mandatory after private readback. See the endpoint contract and remaining
live gates in [the delivery record](../../../docs/NATIVE-DOWNLOAD-ACCELERATION-20261005.md).
