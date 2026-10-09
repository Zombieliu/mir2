# Native R29 memory growth: Bevy 0.19 text-atlas review

Read-only source review, 2026-09-16. No process/UI/store/source mutation was performed.

## Observed run evidence

`native-r29-isolated.stderr.log` reports approximately `imageAssetCount=324`, `imageDataBytes=235,189,020` near 20,014 ms, then after the Mail click window at 160,780.3 ms / 160,915.1 ms: 1,139 / 1,082,006,636 at 170,099 ms; 1,964 / 1,947,081,836 at 180,105 ms; and 14,985 / 15,601,560,716 at 340,220 ms. The increment is about 1,048,576 bytes per image asset, exactly 512*512*4. The log has no reliable user-close or normal-exit event; do not claim one.

## Source-backed interpretation

Bevy 0.19's `bevy_text/src/font_atlas.rs` is a direct match for the increment. `FontAtlas::new` creates an `Image::new_fill` with `TextureFormat::Rgba8UnormSrgb` and `Rgba8` data, and `add_glyph_to_atlas` chooses `max(512, next_power_of_two(glyph_max_size))`; see lines 42-63 and 132-166. A new 512-square font atlas therefore contributes exactly 1,048,576 CPU image bytes and one `Assets<Image>` entry. `FontAtlasSet` is a persistent resource keyed by font data id/index, font size, variations, hinting, and smoothing (`font_atlas_set.rs:12-29`); the layout path gets an existing atlas for that key and only calls `add_glyph_to_atlas` when a glyph is absent (`pipeline.rs:331-370`). Thus Mail text can explain the *shape* of the growth, and a missing/fallback glyph can select an additional font-face key, but a single stable Arial + `▶` fallback should normally create a finite atlas set and then reuse it. Rebuilding/despawning text entities alone should not recreate 14,000 atlases while `FontAtlasSet` remains alive.

The project explicitly uses `FontSource::Family("Arial")` at `apps/game-client/client-bevy/src/crystal_ui/typography.rs:15-22`; Bevy documents family fallback through Parley (`bevy_text/src/text.rs:270-280`). This makes the selected triangle a plausible *trigger to inspect*, but source review does not prove it is the repeated allocator. The current Mail renderer uses dynamic text spans and `fill_panel` despawns/recreates panel children (`apps/game-client/client-bevy/src/crystal_ui/overlays.rs:8001-8018`, Mail call near 7710-7724), so that rebuild is a credible trigger for the test. However, fixed-path `AssetServer::load` calls in static image helpers are path handles, not proven direct allocators here; same-path deduplication/reload behavior must be measured rather than assumed.

## Conclusion and next discriminator

The strongest current conclusion is: the 512-square RGBA pattern is highly consistent with Bevy font atlas allocation, but the observed unbounded rate is not explained by one stable Arial/fallback glyph under the inspected Bevy cache design. Persisting the Mail tree/revision-gating its render is a valid low-risk discriminator. If growth stops, the rebuild path is causal. If growth continues, inspect `FontAtlasSet` keys/counts and identify changing font data ids, sizes, variation hashes, or smoothing; also enumerate image asset dimensions/paths. Do not label the incident a confirmed font-fallback bug or a confirmed `AssetServer::load` leak until that discriminator is observed.
