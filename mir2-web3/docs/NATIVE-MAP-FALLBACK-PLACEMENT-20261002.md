# Native full-pack fallback placement, 2026-10-02

This source change starts from `9cf50711ad3810fbb7c3fd6cdafa69c2cd44e8d6`.
It repairs ordinary full-pack fallback metadata in the native keyed-map
generator. Frozen private R13 installers, generated resources and the
`ab970e0ef0656bcb30c719b97f752fa0de422b23` game executable remain intact.
No package rebuild, installation, game launch, publication or player switch
is part of this checkpoint. Paused goals retain their status.

## Cause and original drawing rules

`FullCrystalMapFrameSource.render` previously declared `source-offset` for
every full-pack frame with integer `.Lib` X/Y. An ordinary `Objects2` frame
with X/Y `(7,-44)` therefore moved right 7 px and up 44 px from the native
bottom-left position. The old generated-manifest fixture explicitly expected
that wrong placement. The coordinator's R13 audit reported 1,322 affected
ordinary fallback entries; this checkpoint does not regenerate that artifact.

The original client sources inspected are:

| Source | Relevant original rules | SHA-256 |
| --- | --- | --- |
| `Crystal/Client/MirScenes/GameScene.cs` | DrawFloor 10705–10770; DrawObjects 10813–10935 | `935D1CA35DDF4800BCCDAC4CD1675FCB59A269560A57BF98C2B952A459FFB504` |
| `Crystal/Client/MirGraphics/MLibrary.cs` | Draw 640–670; DrawBlend 692–707; DrawUp/DrawUpBlend 800–835 | `AFDA6EBB7A163D745D6398EA5C92CE80524DAA6FB85C6167A09514867CF6F8C8` |

Ordinary middle `DrawUp`/`DrawUpBlend` and ordinary front `Draw`/`DrawBlend`
subtract the image height without applying library X/Y. Library offsets are
explicit front-layer exceptions: normal library 28 (`Objects27`) with a
nonzero offset; additive libraries 14, 27 and 100–198; or other additive
front frames 2723–2732 inclusive. Library 14/27 correspond to `Objects13`
and `Objects26`; the supported 100–198 libraries use `ShandaMir2` keys.

## Bounded change

The full-fallback generator now emits X/Y metadata only for those explicit
front-layer offset paths. All ordinary fallbacks omit placement fields,
allowing the existing native parser's `BottomLeft` default. Their rendered
left position remains at the cell's left edge, including wide images.

Source atlas extraction, RGBA, map blend classification, local-export
placement, animation closure, no-draw handling, full-pack content/library/page
hash verification and the missing-source budget are unchanged.

## Verification

Run from the repository root:

```text
node --check mir2-web3/apps/web/scripts/build-native-keyed-map-pack.mjs
node --test mir2-web3/apps/web/scripts/test-native-keyed-map-pack.mjs
node --test mir2-web3/apps/web/scripts/test-map-render-routing.mjs
git diff --check
```

The updated existing Type1 full-fallback fixture fails on the original code
with actual `source-offset` versus expected absent placement metadata.
After the fix, the focused script passes 25 tests: the fixture matrix and
24 placement rows. The adjacent map-routing script passes its single Node
test; syntax and whitespace checks pass.

The generated-manifest matrix covers ordinary front/middle/48x32 floor
images, ordinary additive front, all eight `DrawUpBlend` phases, Objects27
normal/zero/middle/additive contexts, library 14/27/Shanda front exceptions,
2723/2732 inclusive boundaries, 2722/2733 exclusions and a normal frame in
the special index range. It checks key and animation/blend counts, zero
missing/no-draw entries, dimensions and extracted RGBA. Ordinary coordinates
are compared with the original drawing equations at drawX 470/drawY 384;
this is a generated-metadata contract test, not a rendered native capture.
Existing direct-export tests also retain their PNG byte-identity assertions.

Special offset fields are preserved by these tests. The existing native
`SourceOffset` mode still adds Y to a bottom-left baseline; Crystal's distinct
special-library baselines and other floor geometry are separate renderer
parity questions. This change makes no new acceptance claim for them.
Fresh package resource verification and human visual acceptance remain open;
`visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
