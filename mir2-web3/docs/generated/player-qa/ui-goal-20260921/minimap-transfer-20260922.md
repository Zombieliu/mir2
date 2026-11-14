# Native minimap transfer and cave self-visibility repair

Source: 7532ac047. The reported D401 minimap is a client defect, not an absent
source image. MMap/8.png exists (300x199); D401 is 200x200. The prior native
minimap only accepted Bichon titles. MapModel now carries authoritative minimap
index and parsed dimensions, while the renderer uses the loaded image size for
cropping and marker placement. Zero/unknown metadata stays collapsed. Small
images use matching marker scaling.

The ordinary worldSnapshot schema omits the minimap index. A connection-owned,
map-bound cursor therefore retains MapInformation.miniMapIndex and
MapChanged.miniMap through subsequent same-map snapshots. Zero clears it;
scene/session resets clear it; a delayed different-map snapshot cannot inherit
or erase the latest packet binding. Tests exercise real envelope ingestion,
not only hand-built map models.

The black cave ceilings are genuine opaque source pixels. Crystal GameScene
10949 redraws its own body/head/wings after terrain. Native now reproduces the
self body/head redraw at fixed 0.4 opacity, even when highlighting is disabled
or the ordinary body uses hidden opacity. Direction order is preserved, weapon,
mount, shadow and effects are excluded. Wings preserve DrawBlend semantics if
present, but native does not yet generate the wing base layer; this is not a
claim of complete wing rendering. No source alpha or collision was changed.

Evidence under C:/mir2-ui-repair-20260921:

- minimap-client-full.log: 1051/1051 native UI tests pass.
- self-redraw-windows-full.log: 695/695 Windows tests pass.
- self-redraw-hidden-targeted.log: final fixed-opacity correction 2/2 pass.
- minimap-map-cursor-tests.log: MapChanged/MapInformation, periodic snapshots,
  zero-map, stale source snapshot and logout lifecycle regression passes.
- d401-19-156-live-before-minimap.png: old 19c23c262 client defect evidence.

The old client check included one real death at D401 (18,155), ordinary V town
revival to Bichon (288,616), and confirmed Exit with Success. No saves or items
were edited. This is separate from the completed newcomer functional cohort.
The attempted path check was interrupted by death and is not a pass.

Build/deployment and same-version visual evidence must be recorded separately.
Passing tests do not close the whole-UI goal or establish Crystal screenshot
parity, map-route acceptance, clean human timing, or complete map coverage.
