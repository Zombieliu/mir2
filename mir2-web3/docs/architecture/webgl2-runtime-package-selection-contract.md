# WebGL2 package selection and startup agreement

2026-10-01. The sealed controlled pair measures a default GL2 WASM smaller by
10,006,769 raw/593,642 gzip bytes. Its canonical95e4 publisher still serves the
UI-capable GL2 package. This next contract makes the lean package selectable
without mismatching immutable canvas placement and Rust startup rendering.
The weight-pair independent audit is complete and root-reviewed. This slice has
separate before copies and evidence under `webgl2-package-selection`.

## Assigned wire shapes

New manifests use `schemaVersion: 2`, `version`, `packages`, and the existing
hashed `files` array. Package records have `id` (`webgpu`, `webgl2`, or
`webgl2-shared`), `backend`, `packageDir` (`pkg-` plus id), `questUiAbiVersion`,
`bagUiAbiVersion` (both 0 or both 1), and `primarySharedUiCompiled`. GPU and
default GL2 records are required; shared GL2 is optional in validation, requires
both ABI1 and compiled-primary true, and is published in UI-enabled builds.
Default GL2/GPU primary capability is false. No undeclared or incomplete package
pair is valid. Legacy manifests have neither schema nor packages and exactly
the original four valid files; their UI capabilities remain unknown.

The additive WASM export is `getMir2RuntimeUiCapabilities(): string`. Its JSON
has `schemaVersion: 1`, `backend`, the two ABI versions, compiled-primary flag,
and `primarySharedUiStartup`. The last value uses the existing Rust startup
parser; compiled-primary is true only for the real WASM GL2 shared build. The
getter is available with UI disabled, performs no App/resource mutation, and
native reports backend `native` with primary false. ABI presence reports compiled
host availability, including the lean GL2 unsupported-status ABI.

An invalid manifest selects DOM compatibility without full-runtime boot. A valid
schema2 manifest without shared GL2 may select its explicitly declared lean GL2
with normal canvas layout, even for an exact shared request; its getter must
confirm compiled-primary false. Exact shared requests with legacy metadata skip
the unknown full runtime. Selection remains fixed before first canvas mount.

## Package and page invariants

Use separately named GPU, default GL2 and shared GL2 packages. In UI-enabled
Candidate builds, default GL2 has `webgl2,web-quest-ui` and shared GL2 adds
`webgl2-shared-ui`; each must have a separate Cargo target directory. UI-disabled
builds must advertise the absence of shared capability and must not publish a
phantom shared package. Version metadata must describe the compiled capabilities
and a complete hashed JS/WASM pair per package.

Resolve exact raw-first startup mode together with complete package metadata
before the first canvas mount/module import, then keep the choice fixed for the
document. Metadata presence is not proof of a successful HTTP load. Missing or
malformed shared metadata can select the declared lean package with compatible
DOM/React layout; shared HTTP/import/initialization/boot failure must fail closed
to a usable DOM/React route or explicit reload, never implicitly boot a second
runtime in the same page.

A legacy two-package manifest needs special care: its GL2 file may itself be
shared-capable and interpret the raw flag inside Rust. New JS must not choose
a normal/nested canvas layout and boot that unknown legacy package with an
exact shared opt-in query. Skip the full renderer or require explicit reload
in that case. Normal legacy routes may remain compatible where the old raw
query cannot activate primary shared rendering.

Add an additive, read-only runtime capability/startup getter available before
App boot. It reports a schema version, compiled backend/UI capability and the
effective primary-shared startup mode from the existing exact parser. The Web
loader checks that it agrees with the selected package and fixed canvas layout
immediately before `bootMir2Runtime`, with no asynchronous gap. A disagreement
does not boot an App. Do not mutate location, inject a startup override, create
another device/WASM instance or change the intent ledger to make it agree.

## Version and release closure

Root leads the manifest schema, complete package enumeration, version hash,
atomic publisher/rollback and immutable routing. Build metadata is generated
from the assigned Cargo feature commands, not guessed from filenames. Local
versioned rewrites must reject stale/unknown versions rather than serving current
bytes at an old immutable URL. Remote/prebuilt validation, bundle release,
thin-client inclusion, doctor/smoke/budget checks and path/file-count allowlists
must agree on complete two- or three-package layouts according to capabilities.
Preserve legacy manifest validation where needed without letting its unknown
shared capability bypass the startup agreement.

Only one worker may edit each coupled module. Assign bounded sets after final
weight audit: Sol/high for the pure runtime getter and for isolated Web leaf work,
separate Sol/high read-only review; root owns the version/publication integration,
serial builds and actual clients; Luna/medium audits sealed results. Complete
current files are backed up before mutation; preserve unrelated staged work.

## Acceptance evidence

Meaningful unit/native/WASM checks cover exact/encoded/duplicate raw flags,
compiled capabilities, complete/missing/duplicate/malformed package metadata,
old manifests, version mismatch, URL selection, capability handshake, and
failure without a second boot. Preserve the UI-only cfg edge compilation and
existing native-only portable typography limitation.

After source freeze, root serially builds and verifies all advertised artifacts
and unchanged byte budgets. Actual ordinary default/shared/GPU routes must show
the selected immutable response URLs and UI ownership, fresh/cached resource
timing, ordinary local input and normal save/relogin. Exercise missing/import/
WASM/boot failure recovery on isolated QA paths; record intentional faults
separately from unexpected runtime errors. No runtime state/packet/admin injection
or acceptance-by-fixture is allowed.

CPU/memory/first-usable-frame performance requires its own calibrated measurement;
local transfer timing alone is not mobile performance. Full world composition,
locale/HUD/inventory, physical Android/iOS, DPR2/IME/safe area and CP-02–04 remain
open. No production deployment or whole-goal completion follows from this slice.

## Concurrent local publication repair

Independent integration review found that swapping the flat current package tree
before its manifest could alias old version URLs to new bytes. Prepare verified
immutable copies or hardlinks under `public/bevy-runtime-releases/<version>/`
before replacing current files/metadata. The proxy rewrites only declared current
URLs into that version directory; a compiled old manifest during HMR may still
serve its original retained bytes, never the new flat tree. Seed an existing
complete pinned legacy bundle before switching. Prebuilt mode retains its exact
manifest and prepares that same immutable directory. Thin packaging includes only
the declared current version tree and its metadata, avoiding all old versions or
a second flat binary copy. The schema2 digest uses explicit code-unit sorting,
independent of host locale. Historical release directories are preserved; no
old-version cleanup is authorized in this slice.

The same Next config.env snapshot supplies Page and proxy runtime metadata.
Thin packaging checks required-server-files.json from both the source Next
build and copied standalone output against the pinned runtime, before copying
public media. Old unstamped or mismatched builds, including --skipBuild, require
a fresh build. A runtime publication does not update an already started dev
server's captured build configuration; restart that isolated server before
verifying a new release. Sidecar file hashes alone do not establish that the
compiled Page requests the packaged version.
