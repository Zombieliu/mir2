# Portable standalone Crystal full-pack trace contract

Status: root-reviewed bounded implementation contract; source/build/actual-client acceptance is still pending. This batch belongs to the active cross-platform reuse goal.

## Problem and chosen boundary

The current 32 generated NFT metadata records contain 61,164 files rows. Only server/app/api/asset-manifest/route.js.nft.json contains two full-pack rows: the local directory link and a cross-volume encoded index. Next 16.2.11 merges pre-existing route NFT rows without reapplying its shared ignore. Per-route excludes join against the Web project directory and do not repair the Windows joined-pattern mismatch.

Choose the public awaited compiler.runAfterProductionCompile hook to transform one owned build NFT before explicitly serial collection, plus one narrow next-server shared ignore to prevent new collection from adding the namespace again. No private TraceEntryPointsPlugin mutation or custom webpack callback is adopted. The supported build is the normal production next build --webpack with MIR2_NEXT_STANDALONE=1; root enforces a fresh hook receipt and new dist directory. Split compile/generate and Turbopack are not accepted by this contract.

The shared rule is exactly **/generated/crystal-packs/full/**. Proof02 verifies this single string against installed picomatch contains:true/dot:true, target/neighbor/traversal cases, and an owned tiny junction NFT. The bare terminal **/generated/crystal-packs/full rule is rejected because it matches neighboring names. These are Windows-host fixture results, not an actual Next or Linux build. NFT can still inspect link/index metadata; no zero-access claim follows.

## Sole product writer and three paths

Only Sol high /root/shared_weight_image_impl may edit:

- apps/web/next.config.ts (existing; initial 11,616 bytes, SHA-256 baaf8cd80f1ca3092f8a0f88566c78a9f10add98e87ad18462001984806b7eaa).
- apps/web/lib/standalone-full-pack-tracing.ts (new; absence explicitly observed before grant).
- apps/web/scripts/test-standalone-full-pack-tracing.mjs (new; absence explicitly observed before grant).

The new 379-row source-before manifest preserves all 377 freshly archived existing inputs and adds two explicit absent rows. Exactly one existing source may change and two new sources may appear; all other 376 existing rows and frozen references must match. Root owns contract/docs/lease/integration; explorers remain read-only. The writer may create evidence and tiny retained fixtures only in its implementation01 QA directory. TypeScript build-info is an explicitly supplemental generated output; record its before/after separately rather than treating it as a frozen product change. No full Next/package/service/GUI execution is granted to this source worker.

## Config extension

Install the hook, experimental.parallelServerBuildTraces:false and next-server shared ignore only for production standalone (MIR2_NEXT_STANDALONE=1 and NODE_ENV not development). Preserve ordinary nonstandalone/dev config behavior, existing compiler/experimental values, all existing route rules, output selection, pinned asset env/capabilities, public URL/header/rewrite/cache choices and webpack worker settings. Correct the adjacent inaccurate tracing-root comment within this file. Do not add a custom webpack callback, dependencies or model/environment toggles.

Use the hook metadata projectDir and distDir; never infer the bundler from undocumented metadata. Reject the installed Turbopack marker at distDir/turbopack before changing any NFT. Marker absence alone is not complete bundler proof; the fresh root build command gate is mandatory. Hook failures must reject the build.

## One owned metadata transform

Target only distDir/server/app/api/asset-manifest/route.js.nft.json. Validate project/output containment with path-relative checks, accept an intentional checkout alias only at the project boundary, and reject links/redirection in owned output components below it. Check all target parents and require a regular nonsymlink NFT. Missing/malformed/unsupported version, outside output, linked NFT/parent, unsupported marker or budget breach must fail before writing. Use NFT version 1, a 1 MiB raw-byte maximum and 4,096 files-row maximum; file rows must be nonempty strings.

Rows are opaque strings: never stat/read/realpath/open their targets. Normalize each single backslash to slash and lexical dot segments with path.posix.normalize only for classification. Remove only a complete generated/crystal-packs/full segment namespace, including its terminal directory node and descendants. Preserve full-other, full.json, fullish, unrelated segment names and full/../ordinary.json after lexical normalization.

Retain all other row strings byte-for-byte as strings, in original order and with duplicates. Preserve version and every other JSON field; do not add runtime fields or normalize/sort retained rows. If no rows match, leave the entire NFT byte-identical. A positive change performs one awaited same-directory atomic replacement, using an exclusively created owned temporary file; never delete the original to make rename succeed, and clean up only the exact owned temporary file on failure after checking containment. No recursive deletion or relocation is granted.

Return/emit a bounded audit with relative target, before/after SHA-256, row counts and removed directory/descendant counts. Avoid target path contents, absolute drive/checkout values, index contents or invented success based only on warning absence.

## Source validation and freeze

Tests must load the actual production helper and actual next.config extension, not a copied classifier/config. Cover native-backslash/forward/encoded-drive and synthetic POSIX strings, neighbors, dot segments, retained fields/order/duplicates, zero-match byte identity, positive atomic transform, missing/malformed/schema/version/byte/row bounds, linked parent/file and outside-output rejection, the actual Turbopack marker, and standalone/nonstandalone/dev config gates. Test the shared single glob using installed bundled picomatch including negative controls. Tiny fixtures are QA-owned; retain them and reports. No actual full-pack index/payload access is needed.

Use absolute Node --no-global-search-paths and clear parent Node overrides before startup. Preserve raw stdout/stderr and actual command exit; bind all read inputs and named current supplemental configs before/after. Run the new focused suite, TypeScript noEmit and a bounded diff check. Freeze all 379 current sources and archives, references and actual results, then release write ownership. No extra unique cases are claimed from repeated executions. If source or reference drift or an out-of-scope change is observed, stop that batch and report; never restore unrelated work.

## Later root acceptance gates

Root and independent review must accept source/freeze before a new normal Webpack standalone build. Require fresh before/after guards and hook audit, no retained full-pack members in any final root/server NFT, and no unexpected non-full-pack loss. Compare the known route remaining rows with the saved controlled baseline; if path hashes change, justify the mapping rather than silently accepting loss. A second affected route requires a new explicit scope decision.

Then verify the clean thin package under the unchanged 360 MiB cap, fresh isolated copy, native Sharp dependency closure, bounded local public asset-manifest metadata behavior and existing declared HTTP checks, owned service cleanup and post-HTTP integrity. Optional index/capability behavior stays unchanged; removing these trace rows does not certify public origin, full media coverage, gameplay, actual IME, Windows UI, phones, persistence or final acceptance. Reuse unchanged Rust/native/runtime artifacts explicitly; do not claim new Rust/native execution.

## Provenance

Read-only original exploration and its two corrected conclusions, correction01, proof01 and proof02, failed root read-only metadata/search/template-comparison attempts, architecture review and fresh baseline remain immutable. Root acceptance of proof02 closes only the narrow shared-ignore prerequisite; actual collector/Next integration remains open.

Primary public references: https://nextjs.org/docs/architecture/nextjs-compiler and https://nextjs.org/docs/app/api-reference/config/next-config-js/output. Installed 16.2.11 source and guarded fixtures control this version-bound implementation.
