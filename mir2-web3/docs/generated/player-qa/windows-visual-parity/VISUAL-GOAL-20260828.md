# Windows visual parity execution goal

Date: 2026-08-28

## Purpose

Drive the Windows native client toward a source-auditable Crystal/Mir2 1:1
visual and interaction Candidate without fabricating a whole-game percentage.
Every claimed leaf must stay bound to Crystal source, exact assets, automated
tests and the final same-EXE/DPI/human gates.

## What the current native window already proves

The current native client can already render a playable Bichon baseline:
terrain, actors, labels, minimap, orb HUD, chat strip, quick bar and the
right-side control cluster are visibly present. That is not visual 100%.

The current audited denominator is still materially open:

- Player pixel libraries: 477 libraries / 541,010 frames; only 7 roots /
  7,360 frames are currently closed in the native audit base.
- Monster pixel libraries: 546 libraries / 219,607 frames; only 8 Monster
  libraries / 1,742 frames are currently closed in the native audit base.
- Non-None spells: 129; the first bounded effect/audio leaves exist, but the
  full skill/effect visual chain is still open.
- Fixed/template UI scope: 410 leaves; some shell, button and dialog leaves
  are source-bound, but the full HUD/panel/button denominator is not closed.

## Execution waves

1. HUD and button UI wave
   Bind the visible fixed controls first: exact images, hover/pressed/disabled
   state, click sound, geometry, z-order and authoritative enable gates.
   Priority families are Main HUD, skill bar, minimap buttons, inventory tabs
   and character tabs.

2. Player-character wave
   Expand exact player body, hair, weapon, mount and corpse/name overlays from
   the current starter subset toward the full class/gender/equipment matrix.
   Do not claim closed player parity until body/action/effect registries are
   enumerated against Crystal source.

3. Skill/effect wave
   Continue the typed native/Web effect lane from the existing Lightning,
   FireBall, SoulFireBall, FireWall, FlamingSword and Healing leaves into the
   rest of the first observable combat slice: cast, projectile, impact,
   persistence, struck, die, dead and revive.

4. Monster wave
   Continue the source-derived monster chain from Scarecrow Attack1/Struck/Die
   into remaining `Monster/005` actions, then expand to additional families.
   Every family must bind real action/frame/audio semantics instead of generic
   fallback behavior.

## First bounded write target

The next bounded write target should be the visible HUD/button path rather than
another backend-only leaf. This is the smallest user-facing surface that
directly answers the current gap report about buttons and panel fidelity.

Target:

- `VIS-03` main HUD button matrix expansion

Bounded scope:

- exact source images and pressed/hover/disabled behavior for the visible
  right-side main HUD control cluster and its authoritative enable gates;
- local ButtonA emission only on valid pressed transitions;
- no fabricated disabled art when Crystal reuses the normal frame;
- native tests first, then package/verify allowlist updates if new source
  assets are required.

Out of scope for the first write:

- same-EXE screenshots
- live WSS/UI acceptance
- real 125%/150% DPI
- human visual/feel judgment
- any whole-game percentage

## Non-negotiable open gates

Even if the next leaves are green, the project must still keep these gates
open until real evidence exists:

- same-EXE UI and authenticated live WSS
- real 100% / 125% / 150% OS DPI
- native 30-minute soak
- human visual, animation, audio and gameplay feel acceptance
- clean Crystal source binding
- complete semantic denominator closure for the claimed aggregate
- legal asset review and formal publisher signing
