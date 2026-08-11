# Platform And Client Strategy

Last updated: 2026-08-11

Status: implementation baseline; real-device and human acceptance remain open.

Purpose: capture the platform coverage strategy for the post-1:1 MMORPG direction. This document complements `docs/TECH-MODERNIZATION-RFC.md`.

## 2026-08-11 Delivery Status

The first cross-platform implementation slice now follows this strategy without
moving gameplay authority out of Simulation/Gateway:

- Web remains the primary maintained client.
- Tauri desktop shells for Windows, macOS, and Linux share the deployed Web
  client and pass hosted compile gates on all three operating systems.
- Capacitor shells for Android and iOS share the same Web client; Android APK
  plus emulator launch and an unsigned iOS simulator build are automated gates.
- `apps/game-client` now contains renderer-neutral client primitives, shared
  Bevy read models/UI, a native desktop host, and an Android native compile host.
- CI keeps Rust/WASM, native Windows assets, Tauri, Android, iOS simulator, and
  shell security/packaging contracts separate so one shell cannot stand in for
  another platform's evidence.

This is Candidate delivery evidence, not final platform acceptance. Physical
Android/iOS lifecycle, touch/virtual-keyboard behavior, thermal/memory soak,
store packaging/signing, and human visual/feel checks remain explicit gates.

## Target Platform Stance

Primary target:

- Web.

Near-term packaged targets:

- Windows desktop.
- macOS desktop.

Secondary targets after validation:

- iOS.
- Android.

Deferred / separate strategy:

- PlayStation.
- Xbox.
- Nintendo Switch.
- Other console platforms.

## Current Recommended Stack

Use the current Bevy + NextJS + Rust-server direction, with explicit platform boundaries:

- `NextJS`: login, account flows, character selection, page shell, React overlay UI, account/payment/activity pages, and possible admin frontend.
- `Bevy`: game rendering, scene presentation, input, camera, animation, and effects.
- `Rust server`: authoritative gameplay, persistence commands, world/zone simulation, protocol, and operations API.
- `Postgres`: authoritative long-lived state.
- `Redis`: non-authoritative session, routing, cache, queues, and online state.

The client stack should remain replaceable. Server protocol and gameplay authority should not assume a specific frontend runtime.

## Platform Matrix

| Platform | Recommendation | Notes |
| --- | --- | --- |
| Web | First-class target | Use NextJS + Bevy WASM/canvas + React overlay. |
| Windows | Near-term target | Start with Tauri desktop shell; keep path open for Bevy native desktop. |
| macOS | Near-term target | Start with Tauri desktop shell; keep path open for Bevy native desktop. |
| iOS | Validate before committing | Web/PWA/WebView can work, but memory, WebGPU/WebGL support, app lifecycle, input, and store constraints need testing. |
| Android | Validate before committing | Similar to iOS, usually less restrictive; still needs performance and lifecycle testing. |
| Consoles | Defer | Treat as separate platform engineering and business track. Current Bevy/Web route is not a mature console path. |

## Tauri Desktop Shell

Tauri is a good near-term packaging option for Windows and macOS, but it does not make the game native.

If the desktop app runs NextJS + Bevy WASM/canvas inside a Tauri WebView:

- rendering still happens through WebView;
- Bevy still runs as WASM, not native Bevy;
- WebGPU/WebGL support depends on the platform WebView;
- React overlay and DOM/canvas compositing add overhead;
- Windows and macOS WebView behavior may differ.

Expected performance order:

```text
Bevy native desktop > Browser WebGPU/WASM > Tauri WebView/WASM > mobile WebView
```

The likely bottleneck is WebView + WASM + graphics backend behavior, not Tauri itself.

## When Tauri Is Enough

Tauri should be acceptable for early desktop distribution if the product remains close to a Mir2-style 2D/2.5D MMORPG:

- moderate entity counts;
- moderate effects;
- server-authoritative gameplay;
- React-heavy UI panels;
- web-first account and login flows;
- fast iteration over native-only performance.

This should be validated with real performance tests before being treated as final.

## When To Move To Bevy Native Desktop

Move from Tauri-contained WASM to native Bevy desktop if any of these become product requirements:

- high frame-rate combat feel;
- large same-screen entity counts;
- heavy particles, lighting, shaders, or post-processing;
- lower input latency;
- native controller integration;
- local resource streaming;
- stronger anti-tamper or launcher integration;
- significant WebView/WebGPU inconsistency across platforms.

One possible long-term split:

- Tauri as launcher, account shell, updater, and web admin surface.
- Bevy native as the actual desktop game client.

## Mobile Strategy

Do not assume the web desktop client automatically becomes a good mobile game.

Mobile validation must cover:

- touch input model;
- small-screen panel layout;
- virtual keyboard behavior;
- memory limits;
- background/foreground lifecycle;
- asset loading;
- network reconnect;
- battery and thermal behavior;
- App Store and Play Store packaging constraints.

Recommended path:

1. Keep Web responsive enough for smoke and QA.
2. Prototype iOS/Android WebView packaging only after desktop shell stabilizes.
3. Decide whether mobile remains WebView/PWA or needs native Bevy mobile based on measured performance and UX.

## Console Strategy

Do not optimize the first production architecture around consoles.

Reasons:

- console SDK access requires business/platform approval;
- console integration involves NDA-bound APIs, certification, platform services, controller requirements, and store processes;
- Bevy/Rust open-source ecosystem is not as mature for consoles as Unity/Unreal;
- forcing console support now would slow the core MMORPG architecture.

If consoles become a commercial requirement later:

- preserve Rust server authority;
- keep protocol client-agnostic;
- consider a dedicated Unity or Unreal console client if Bevy console support is not viable;
- treat console as a separate platform project with its own budget and acceptance plan.

## Architecture Requirements To Keep Options Open

To avoid locking into one client runtime:

- keep gameplay authority on the server;
- keep protocol and event schemas client-neutral;
- keep UI commands separate from simulation commands;
- abstract input commands rather than browser events;
- keep asset manifest and loading paths portable;
- avoid browser-only assumptions in shared game state;
- keep Stage 5 smoke and packet traces as regression references;
- test Web, Tauri, and native paths separately if native clients are introduced.

## Current Recommendation

Use this rollout order:

1. Web remains the primary target.
2. Windows/macOS use Tauri shell for near-term distribution.
3. Keep Bevy native desktop as an escape hatch if measured performance requires it.
4. Validate iOS/Android after desktop shell and responsive UI are stable.
5. Defer consoles until product-market and commercial strategy justify the extra platform engineering.

Do not rewrite the project around Unity or Unreal now. If native console support becomes mandatory later, add a separate console client while preserving the Rust server and protocol boundary.
