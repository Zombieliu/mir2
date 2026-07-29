# Responsive UI and unified input

## Outcome

Mir2 keeps one Web client and one canonical play URL while adapting its
presentation and controls to desktop, touch devices, and TV/Xbox browsers.
Game state, commands, panels, and content stay shared.

## Profiles

Layout and input are independent:

| Profile | Values | Responsibility |
| --- | --- | --- |
| Layout | `desktop`, `touch`, `tv` | Placement, sizing, safe areas, panel presentation |
| Input | `keyboardMouse`, `touch`, `gamepad` | Movement, actions, focus navigation, prompts |

Automatic defaults:

- Xbox user agent: `tv + gamepad`
- Primary coarse pointer: `touch + touch`
- Other browsers: `desktop + keyboardMouse`
- A connected gamepad switches the input profile but does not force a desktop
  browser into TV layout.

URL overrides are supported for QA and accessibility:

```text
/?layout=desktop&input=keyboardMouse
/?layout=touch&input=touch
/?layout=tv&input=gamepad
```

The legacy `?mobile=1` and `?mobileControls=1` switches remain aliases for
`touch + touch`.

## UI architecture

```text
OriginalClientShell
├── Game scene and renderer (shared)
├── Game UI content (shared)
│   ├── HUD
│   ├── inventory
│   ├── character
│   ├── NPC dialog
│   └── system panels
├── layout presentation
│   ├── desktop: original 1024x768 composition
│   ├── touch: landscape controls and centered enlarged panels
│   └── tv: safe-area composition and controller focus treatment
└── input adapters
    ├── keyboard and mouse
    ├── nipplejs touch controls
    └── native Gamepad API
```

The existing 1024x768 render surface remains authoritative. Layout profiles
change surrounding controls, panel placement, visibility, and focus treatment;
they do not fork the renderer or network command path.

## Controller contract

Baseline standard-gamepad mapping:

| Control | Gameplay | UI |
| --- | --- | --- |
| Left stick / D-pad | Move | Move focus |
| A | Primary action | Activate |
| B | Cancel/close | Back |
| X | Pick nearest drop | Secondary action |
| Y | Approach selected target | Secondary action |
| View | Character | Previous surface |
| Menu | Inventory | Next surface |
| LB / RB | Belt items 1 / 2 | Reserved |
| LT / RT | Skill slots 1 / 2 | Page controls |

Gameplay input pauses while a modal game UI surface is open. This prevents a
D-pad press from moving both the focused menu item and the player.

## Layout acceptance

### Desktop

- Existing pixel-art composition and keyboard/mouse behavior do not regress.
- Connecting a gamepad changes prompts/input without changing layout.

### Touch

- Landscape play controls appear for coarse-pointer devices of any tablet or
  phone height.
- Safe-area insets keep controls clear of notches and browser gestures.
- Inventory, character, NPC, shop, map, and system panels remain usable at
  touch target sizes.
- Portrait gameplay shows a rotate affordance; observer and account surfaces
  may receive a portrait layout later.

### TV/Xbox

- No touch controls are rendered.
- Every supported flow has visible focus and works without hover.
- The UI stays inside a TV-safe inset and remains readable at viewing distance.
- Login, character select, gameplay, inventory, and logout are completable
  with a standard Xbox controller.

## Delivery estimate

| Phase | Scope | Estimate |
| --- | --- | --- |
| Foundation | Profiles, URL overrides, data attributes, tests | 0.5–1 day |
| Gamepad gameplay | Polling, movement, actions, debug state | 1–1.5 days |
| Focus navigation | Login/select and game panels, back handling | 1–1.5 days |
| Touch/TV presentation | Responsive panel shells, focus and safe-area CSS | 1–2 days |
| Browser QA | Desktop and mobile emulation, regression fixes | 0.5–1 day |
| Hardware QA | Xbox Edge and representative phones/controllers | 1–2 days |

The mergeable browser-tested version is expected in roughly 3–5 engineering
days. A hardware-validated release candidate is roughly 5–8 engineering days,
depending on Xbox Edge behavior and the number of devices in the matrix.

## Verification matrix

- Chromium desktop: keyboard/mouse and Xbox controller
- WebKit mobile emulation: iPhone landscape and safe areas
- Chromium mobile emulation: Android phone and tablet landscape
- Xbox Series S/X Microsoft Edge hardware
- Optional: Steam Deck browser and Bluetooth controller on iOS/Android
