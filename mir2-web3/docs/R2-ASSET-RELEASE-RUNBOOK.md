# R2 Asset Release Runbook

This runbook publishes the complete converted Crystal asset set to an
immutable Cloudflare R2 prefix. It then switches the Worker and Vercel
production build before deleting objects outside the new release.

The full source pack is local-only and ignored by Git. Run these commands on
the authorized Windows asset workstation.

## Safety Invariants

- Never overwrite an existing `mir2/v/<version>` prefix.
- Build and upload from `latest-remote-asset-release.json`.
- Full-pack JSON is stored with deterministic gzip and
  `Content-Encoding: gzip`; PNG/audio files remain unchanged.
- Keep the old production prefix until the new prefix passes R2, Worker, and
  browser smoke checks.
- Bucket cleanup is dry-run by default.
- Cleanup apply requires the exact bucket, keep prefix, plan SHA-256, and a
  production manifest that already points at the keep prefix.
- Cleanup deletes objects, not the R2 bucket.
- Do not commit R2 credentials or generated full-pack files.

## Current Candidate

The locally generated candidate at the time this runbook was added is:

```text
version:       20260730-fullcrystal-f71b89aa-gzip1
object prefix: mir2/v/20260730-fullcrystal-f71b89aa-gzip1
asset base:    https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1
source hash:   f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e
objects:       46,003 assets plus remote-asset-release.json
raw bytes:     10,293,455,313
stored bytes:  7,922,261,854 before the release manifest
missing:       0
```

This is a candidate, not proof that production has switched. The live
manifest and R2 object checks below are authoritative.

## Credentials

Create an R2 S3 token scoped to the asset bucket with Object Read & Write.
Keep it only in the current shell or a secret store:

```powershell
$env:CLOUDFLARE_ACCOUNT_ID = "<account-id>"
$env:MIR2_R2_BUCKET = "<asset-bucket>"
$env:MIR2_R2_ACCESS_KEY_ID = "<r2-access-key-id>"
$env:MIR2_R2_SECRET_ACCESS_KEY = "<r2-secret-access-key>"
```

Alternatively set the endpoint directly:

```powershell
$env:MIR2_R2_S3_ENDPOINT = "https://<account-id>.r2.cloudflarestorage.com"
```

## 1. Build The Immutable Release

```powershell
cd E:\mir2\mir2-web3\apps\web

npm run assets:full-pack:verify

$env:MIR2_ASSET_VERSION = "20260730-fullcrystal-f71b89aa-gzip1"
$env:MIR2_ASSET_OBJECT_PREFIX = "mir2/v/$env:MIR2_ASSET_VERSION"
$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL = `
  "https://assets.mir2.obelisk.build/mir2/v/$env:MIR2_ASSET_VERSION"

npm run assets:full-release:build
npm run assets:r2:dry-run
```

The dry-run must report:

- `missingCount` from the release build is zero.
- `encodedUploadCount` is 1,441.
- `totalBytes` is below 10,000,000,000.
- `objectPrefix` and `assetBaseUrl` contain the exact immutable version.

## 2. Upload Without Switching Production

```powershell
$env:MIR2_R2_UPLOAD_DRIVER = "r2-s3"
npm run assets:r2:upload
```

The uploader sends all assets first and publishes
`remote-asset-release.json` last.

## 3. Verify The R2 Closure

```powershell
npm run release:doctor -- `
  --manifest ..\..\..\docs\generated\remote-assets\latest-remote-asset-release.json `
  --checkManifest true `
  --checkR2 true `
  --checkWorker false `
  --checkBevyRuntime false `
  --requireFullCrystalPack true `
  --assetBaseUrl $env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL
```

This checks the required UI/map/entity assets, all 5,887 full-pack objects,
all map-atlas pages, and the gzip response headers.

## 4. Switch Worker And Vercel

Commit and push the release pipeline changes before dispatching the workflow.
Then run:

```powershell
cd E:\mir2
gh workflow run web-assets-r2-release.yml `
  --ref main `
  -f run_id="$env:MIR2_ASSET_VERSION" `
  -f asset_base_url="$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL" `
  -f object_prefix="$env:MIR2_ASSET_OBJECT_PREFIX" `
  -f publish_r2=false `
  -f use_existing_release=true `
  -f remote_release_manifest_url="$env:NEXT_PUBLIC_MIR2_ASSET_BASE_URL/remote-asset-release.json" `
  -f require_full_crystal_pack=true `
  -f deploy_worker=true `
  -f deploy_vercel=true
```

The workflow validates the existing release before changing either production
surface. It then verifies the full-pack closure through the same-origin Worker.

## 5. Production Smoke And Rollback Window

Before deleting the old prefix, verify:

```powershell
Invoke-RestMethod "https://mir2.obelisk.build/api/asset-manifest" |
  Select-Object version, remoteAssets

npm --prefix E:\mir2\mir2-web3\apps\web run release:doctor -- `
  --manifest E:\mir2\mir2-web3\docs\generated\remote-assets\latest-remote-asset-release.json `
  --checkManifest true `
  --checkR2 false `
  --checkWorker true `
  --checkBevyRuntime true `
  --requireFullCrystalPack true `
  --webBaseUrl "https://mir2.obelisk.build"
```

Also log in and enter the game once with a cold browser cache. If any check
fails, switch Worker/Vercel back to the previous prefix while it still exists.

## 6. Plan Bucket Cleanup

Cleanup keeps exact prefix subtrees. For example, keeping `mir2/v/new` does
not accidentally keep `mir2/v/new-old`.

```powershell
cd E:\mir2\mir2-web3\apps\web
$PlanPath = Join-Path $env:TEMP "mir2-r2-cleanup-plan.json"

npm run assets:r2:cleanup -- `
  --bucket $env:MIR2_R2_BUCKET `
  --keepPrefix $env:MIR2_ASSET_OBJECT_PREFIX `
  > $PlanPath

$Plan = Get-Content $PlanPath -Raw | ConvertFrom-Json
$Plan.planSha256
$Plan.plan.listed
$Plan.plan.kept
$Plan.plan.delete | Select-Object objectCount,totalBytes
```

Inspect the counts and bytes. This command does not delete anything.

## 7. Apply The Exact Plan

Apply re-lists the bucket. Any object-set change produces a different plan
hash and aborts before deletion.

```powershell
npm run assets:r2:cleanup -- `
  --bucket $env:MIR2_R2_BUCKET `
  --keepPrefix $env:MIR2_ASSET_OBJECT_PREFIX `
  --apply true `
  --confirmBucket $env:MIR2_R2_BUCKET `
  --confirmKeepPrefix $env:MIR2_ASSET_OBJECT_PREFIX `
  --planSha256 $Plan.planSha256 `
  --productionManifestUrl "https://mir2.obelisk.build/api/asset-manifest"
```

Deletion uses batches of at most 1,000 objects and fails on any per-object
R2 error.

## 8. Verify The Final Bucket State

Run the dry-run again:

```powershell
npm run assets:r2:cleanup -- `
  --bucket $env:MIR2_R2_BUCKET `
  --keepPrefix $env:MIR2_ASSET_OBJECT_PREFIX
```

Completion requires:

- `delete.objectCount` is zero.
- Every listed object belongs to the new immutable prefix.
- R2 release doctor passes.
- Same-origin Worker release doctor passes.
- The production app manifest reports the new version and object prefix.
- A cold-cache browser can log in and render the game.
