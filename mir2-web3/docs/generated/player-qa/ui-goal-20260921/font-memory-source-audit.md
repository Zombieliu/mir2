# Image/font memory source audit — 2026-09-21

Read-only live check: old auth-lifetime process49272 remains responsive at29,253,332,992 private bytes; latest old soak log reports13,425 image assets and13,949,273,628 CPU image bytes. This is not a pass, nor evidence from any newer staged executable. No desktop input/restart was performed.

Bevy0.19 FontAtlas creates minimum512x512 RGBA8 atlas images (1,048,576 bytes each), retained in main/render worlds. FontAtlasSet uses persistent keys containing font blob ID, face, size bits, variation/hinting/smoothing and has no runtime eviction. The observed ~1MiB/image ratio is consistent with font atlas retention but does not identify these live images.

Source FontSource::Family(Arial) alone is not proof of changing IDs. Fontique0.9 SourceCache reuses cloned blobs with stable IDs while accessed. Its two-prune inactivity policy can reload a later font source with a fresh blob ID, leaving older atlas keys retained. Generic native overlay helpers also despawn/recreate visible content each Update; that churn is concrete, but continuous recreation should normally reuse font IDs. Do not remove fallback fonts or label this the proven live cause.

Next discriminating evidence is already implemented in the staged runtime: fontAtlasKeys/pages/bytes, distinct font IDs/size bits, asset-path buckets and untracked images. It needs the ordinary save/logout/new-package handoff. Atlas growth alongside image growth supports the font path; stable atlas totals with growing images rejects it. A separate headless continuous-recreation versus three-frame-absence probe can test font source reactivation independently, without asserting it caused the player's memory usage. No additional telemetry or speculative fix was added this round.

Source references: local registry bevy_text-0.19.0/src/{font_atlas.rs,font_atlas_set.rs,pipeline.rs}; fontique-0.9.0/src/source_cache.rs; linebender_resource_handle-0.1.1/src/blob.rs. Project typography: client-bevy/src/crystal_ui/typography.rs. Existing diagnostics: runtime/src/lib.rs resource snapshot. Latest exact package hashes remain in README.md.
## Headless mechanism reproduction

`cargo +1.95.0 run --offline --example font_atlas_probe --features native-ui` passed, using the actual Bevy text pipeline with no App, window or renderer. Log: C:/mir2-ui-repair-20260921/font-atlas-probe.log. The100 continuous Arial recreations retained1 atlas/1 font ID/1,048,576bytes. After3 source-cache prune passes without Arial followed by recreation, the same text retained2 atlases/2 font IDs/2,097,152bytes. Thus source-cache expiry can produce a second retained font atlas; continuous recreation alone did not do so in this probe. This is a reproducible engine mechanism, not proof that it accounts for the old live client's29GB or a production repair. Live attribution and stable-font integration remain open.

## 2026-09-21 installed font lifetime candidate

Windows now retains strong Bevy Font assets for installed Arial regular/bold/italic/bold-italic and Microsoft YaHei regular/bold/light before native UI plugins start. Existing Family declarations and system discovery/fallback remain enabled. All faces are included because a registered memory family takes precedence over the system family and HUD uses bold Arial; pinning regular alone would change that appearance. Unavailable files are logged rather than silently claiming successful pinning. Fonts are read from WINDIR/Fonts; no font binaries are redistributed.

The headless probe now exercises regular and bold Arial: continuous recreation uses2 atlas pages/2MiB; system-source expiry and recreation adds a third page. With installed memory-backed faces,100 expiry/recreate cycles keep2 pages/2MiB, with exact sample raster pixels and glyph count matching the original path. Native integration2/2 passes using the actual Bevy font-asset registration system: installed TTF/TTC faces resolve by the original family names to retained blob IDs and survive source pruning. Logs: C:/mir2-ui-repair-20260921/font-atlas-pinned-probe.log and native-fonts-tests.log.

This closes the reproduced font-source identity mechanism for the explicitly pinned faces. It does not attribute all old-client29GB growth, cover arbitrary fallback fonts or sizes, or prove whole-UI typography and live soak acceptance. The old client is not replaced or killed; same-build native screenshots, font telemetry and long-run memory checks remain open.

## Chinese glyph comparison

The headless probe now additionally shapes a real Chinese quest-guidance sample with installed Microsoft YaHei regular and bold, compares both raster atlases byte-for-byte against the system-font path, and repeats100 three-prune/recreate cycles. Both faces remain at2 pages/2MiB and glyph counts/pixels match. Receipt: C:/mir2-ui-repair-20260921/font-atlas-cjk-probe.log. The run also emits ICU4X missing Japanese segmentation-model warnings; identical raster pixels do not establish correct wrapping. That dependency/fallback behavior needs separate investigation. This example change does not alter the staged production executable or establish live UI acceptance.

Read-only desktop check still found the old ui-auth-lifetime executable, with a1 in game. No mouse/key input or deployment occurred; a fresh explicit handoff question is pending after earlier user interference.

## ICU diagnostic clarification

Read-only dependency audit separates word boundaries from line wrapping: Parley0.9 constructs ICU's non-complex-script WordSegmenter, whose Han/ChineseOrJapanese bucket reports missing `ja` dictionary and falls back to the end-of-run word boundary. Parley separately uses LineSegmenter, with UAX14 CJK line boundaries; quest/mail WordOrCharacter maps to OverflowWrap::Anywhere. The resolved no-logging ICU provider prints this diagnostic only in debug. No dependency change is justified by this warning. Source references: registry parley-0.9.0/src/analysis/mod.rs; icu_segmenter-2.2.0/src/complex/{language.rs,mod.rs}, src/line.rs; icu_provider/src/lib.rs. A constrained-width layout probe is being added to check actual wrap output separately; the raster comparison alone is not that proof.

Constrained CJK check now passes in font-atlas-cjk-wrap-probe.log:29glyphs,5lines,size84x85 within96px,max glyph-center x77.88965. This verifies the sample's line wrapping without removing the diagnostic or changing dependencies. It is not whole-page visual proof.

## Old live client OOM — not a new-package result

Read-only process/log verification found the old auth-lifetime client exited with code1 at 2026-09-21 13:24:40 UTC. stderr first reports Out of Memory at13:24:38.629, then invalid textures/UI material errors and intentional renderer shutdown. Latest pre-fault sample:15308 images /15794462044 image CPU bytes; private bytes peaked33084MB, working set18595MB. Image growth accelerated after the final StartGame; map tiles1424, entity atlases7 and active effects52 remained stable. Queue stayed empty. The old trace has no FontAtlasSet/path ownership telemetry, so it cannot establish the allocation owner or prove the newer pinned-font fix.

Logs: C:/mir2-ui-repair-20260921/auth-lifetime-live/20260921-080429-331.stderr.log and matching .process.jsonl. The new settings candidate must repeat ordinary login/StartGame and reconnect, with at least three stationary minutes after each, capturing image-path/font-atlas attribution and process memory. No same-build stability pass exists. Do not classify expected cleanup warnings after the OOM as the initial cause.
