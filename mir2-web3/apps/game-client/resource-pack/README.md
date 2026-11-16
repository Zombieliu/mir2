# Native resource packs

`PackStore::open` verifies the entire authenticated catalog, every pack and index,
and retains the existing Windows read leases. Resources remain immutable and each
read verifies its content hash. Callers supply the catalog digest from their
authenticated release flow; this library does not authenticate a release itself.

## Dependency observations

`PackStore::open_profiled` performs the same full validation and enables bounded,
in-memory read counters. `read_profile_snapshot` returns successful verified
logical reads, failed-read count and overflow/poison flags, bound to the catalog
digest. Ordinary stores return `None` and collect no profile.

The native Game enables this only with its existing valid
`MIR2_NATIVE_CAPTURE_DIR` diagnostic configuration. Screenshot requests freeze
the cumulative snapshot; a separate `.resource-reads.json` binds it to the final
PNG hash, request frame and generation. This is an observed lower bound, not a
signed bootstrap closure or proof that unvisited maps and skills are covered.

## Offline full-inventory repacking

The `repack` example reorganizes verified source packs without extracting a
quarter-million loose files. It preserves every logical path, size and content
digest. Neither repacking nor its receipt authorizes partial mounting, signs a
generation, downloads anything or changes the selected release.

```text
cargo +1.95.0 run --locked --offline --release --manifest-path <crate>/Cargo.toml --example repack -- <old-catalog.json> <trusted-old-catalog-sha256> <old-objects-directory> <plan.json> <fresh-output-directory>
```

Use absolute paths. The output parent must exist and the output itself must not
exist, including an empty directory. It must lie outside the source object tree.
Errors retain the partial output; choose a new output directory after inspecting
the failure. The tool never deletes or replaces an existing file.

```json
{
  "schema": "mir2.native.resource-repack-plan.v1",
  "sourceCatalogSha256": "<trusted old catalog SHA256>",
  "targetPackBytes": 33554432,
  "groups": [
    {"name": "observed-startup", "paths": ["original-ui/Title/320.png"]}
  ]
}
```

Explicit groups are prioritized in array order; all unspecified paths survive
in the final `default` group. A content alias shared across groups is stored
once in the earliest group. Names and paths are validated, unknown/duplicate
paths fail, and source SHA plus object/count/byte limits are checked before
creating output. A single larger resource may exceed the target, within the
existing hard pack bound.

Output includes `objects/packs`, `catalog.json` and `REPACK-RECEIPT.json`. The
receipt is written only after full logical equivalence and the new PackStore's
full hash/index validation; `bootstrapAuthorized` remains false.

## Controlled scoped availability

`ScopedPackStore` is a separate library API. `PackStore::open` and its profiling
variant continue to require every pack. The caller must authenticate **both**
the full catalog SHA and the independent required-scope SHA in its release flow
(intended for a future signed V2 manifest); this library does not verify CMS,
certify dependency closure, select a release, schedule threads or download.

```json
{
  "schema": "mir2.native.required-resource-scope.v1",
  "catalogSha256": "<authenticated full catalog SHA256>",
  "id": "login-and-first-map",
  "requiredPaths": ["original-ui/Title/320.png"]
}
```

Scope bytes are limited to 4MiB; `id` is opaque printable ASCII of 1..64 bytes.
Paths must be nonempty, strictly lexically sorted, unique, safe logical paths
present in the full catalog. Unknown or duplicate fields are rejected. Opening
the scoped store fully verifies every required path's whole pack and index,
including the extents of **all** full-catalog resources in those packs, and
retains the same directory/file leases as the full store.

An updater can select required descriptors before any cache directory exists:

```rust,ignore
use mir2_resource_pack::{Catalog, scoped::RequiredResourceScope};
let catalog = Catalog::parse(catalog_bytes, authenticated_catalog_sha)?;
let scope = RequiredResourceScope::parse(
    scope_bytes, authenticated_scope_sha, authenticated_catalog_sha, &catalog,
)?;
let required_packs = scope.required_objects(&catalog)?;
```

Use that same authenticated catalog model for the helper; it rechecks scope
structure/membership but cannot authenticate a different model without its bytes.
`RequiredResourceScope` is public through `mir2_resource_pack::scoped`, without
changing the existing crate-root exports. The offline
`windows-updater/scripts/prepare-progressive-resource-scope.py` emits deterministic
scope bytes, a complete-inventory repack plan and an unsigned preparation receipt
from a pinned full catalog and a pinned explicit JSON array of logical paths.
Its list is an explicit prospective lower bound, never proof of first-play closure.

```text
python -I -S -B <scripts>/prepare-progressive-resource-scope.py --catalog <full-catalog.json> --catalog-sha256 <trusted-sha> --paths-json <explicit-array.json> --paths-sha256 <array-sha> --id <stable-scope-id> --output <fresh-directory>
```

The tool checks the catalog bytes and model but does not read pack bodies or
replace the caller's full resource verification. It emits `required-scope.json`,
`REPACK-PLAN.json` and `PREPARED-SCOPE.json`, all create-only. The repack plan keeps
every unlisted path in the default group. After actually repacking, rerun scope
preparation against the **new** catalog bytes/SHA with the same explicit list;
an old scope cannot authorize a different catalog. Sign/manifest integration and
proof that the list covers real first-play remain separate caller work.

`read(path) -> Result<ScopedRead>` returns `Ready(bytes)` from an admitted handle,
or `Pending(PackObject)` for an existing logical resource whose pack has not been
admitted. A missing logical path or damaged admitted resource is an error.
`exists` always means logical existence. Even a valid deferred pack already on
disk stays Pending until explicitly admitted; `read` never opens or whole-hashes
a new pack on the game thread.

A caller-owned background thread can call `admit_cached_pack(sha) -> Result<bool>`.
It accepts only full-catalog objects, verifies whole SHA/index/all corresponding
extents outside the file-map lock, then atomically publishes an `Arc` handle.
It returns true only for a new admission; repeated successful admissions return
false. Failed validation publishes nothing. Reads clone an admitted handle under
a short read lock and perform positional I/O and per-resource SHA verification
after releasing it. On Windows, required and newly admitted handles deny writes
and deletes until store/read handles are dropped; Unix still requires an
immutable object-directory policy.

`required_objects()` returns the required object slice. `pending_objects()` and
`is_fully_available()` return `Result` snapshots of admission state and fail
closed if the map lock is poisoned. `catalog()`, `catalog_sha256()`, `scope()` and
`scope_sha256()` expose immutable authenticated identities. Pending is an explicit
availability state, never permission to substitute unsigned loose/network data.

```text
cargo +1.95.0 test --locked --offline --manifest-path <crate>/Cargo.toml --all-targets
```

The [five-minute goal and current limits](../../../docs/WINDOWS-FIVE-MINUTE-GOAL-20261101.zh-CN.md)
track runtime integration and real regional acceptance separately.
