# Android native UI coverage — 2026-09-08

Status: shared player UI assembly and offline Android UI baseline, **not whole
Android UI acceptance or a completed online client**. Work is isolated on
`codex/android-player-journey`; the original checkout and Windows backend
are not edited.

## Implemented

- Android enables `client-bevy/native-player-ui`: the exact shared shell,
  HUD/overlays, minimap, chat, notices and quest/NPC plugins used by Windows.
  `native-ui` remains the desktop superset with the audio playback backend.
  Typed UI sound intents are shared separately; this does not implement Android audio.
- All shared root layers use one 1024×768 fit. Android's collapsible panel
  launcher maintains 64×48 OS-logical targets independently of that scale.
- One touch owner feeds existing Crystal pointer/drag handlers. Secondary
  fingers cannot inherit a released drag; focus loss releases the pointer.
  This is not a completed multi-touch movement/combat controller.
- Login, character name, change-password, chat/social drafts, mail text,
  map search, locked storage password and guild/trade amount fields connect
  to the OS IME. Draft edits are focus-guarded; shared validation/reducers
  are reused. Done sends one press/release pair through shared keyboard input.
- Back dismisses editing or local panels before the game menu. No local
  editing operation changes inventory, balances, authentication or world position.
- New-character preview now uses the source offset-aware 16-frame preview
  renderer. Crystal `NewCharacterDialog.cs:95-109` places the anchor at
  dialog +(120,250), with `UseOffSet=true`; the old stretched rectangle
  incorrectly covered the Create button.
- Gradle stages external PNGs from ChrSel, Prguse, Prguse2, Title, Items,
  Help, MMap and StateItem. APKs, generated resources and keys remain ignored.

## Offline verification surface

`MIR2_ANDROID_VARIANT=uiPreview` builds a separate
`com.mir2.web3.uipreview` package with compile-time `ui-preview`.
Normal debug/release variants cannot select fixtures using Activity extras.
The preview Java host refuses to connect, log in or StartGame. Every specimen
has an OFFLINE UI PREVIEW label; no fake Gateway login or bootstrap is emitted.

The 33 named specimens cover (including inventory amount and mail compose):

| Area | Specimens |
| --- | --- |
| Shell | login, empty roster, roster, create, password, SafeKey, delete confirmation, connecting, starting, disconnected |
| Player | HUD, inventory, character, skills, quests, options, platform settings, menu |
| Services/social | cash shop, NPC shop, mail, big map, storage, group, guild, accepted trade, chat settings, NPC |
| Other | death, focused chat/IME, help |

Inventory, storage, mail, skills, quests, NPC goods/dialog and trade include
explicitly offline specimens. An empty cash-shop/map view is **empty-state evidence**, not proof of populated
catalogues, pagination or server actions. Password/SafeKey capture restrictions
remain enabled; a protected screenshot is not visual acceptance.

Run `bash apps/game-client/platform-android/capture-ui-preview.sh OUTPUT_DIR`
after installing the preview APK. It waits for each specimen-ready marker,
captures a screenshot and process log, and fails on panic/FATAL/missing asset errors.

## Still open — do not mark the whole UI done

- Local source `bed749479` adds a guarded notice-body IME touch target.
  API 31 repeated Back/retap retains the draft; Cancel restores the notice.
  Shared576, normal78/preview80 and both APK gates pass. Live publication
  and physical-device acceptance remain open; source is local only.
  [Retap evidence](generated/player-qa/native-android-guild-retap-20260909/README.md).
- Local source `77edc7afb` fixes guild notice IME panning text offscreen.
  API 31 eight-line input and Back/Cancel are verified, normal78/preview80
  and both APK gates pass. Retap is addressed above; live publication remains open.
  [Guild evidence](generated/player-qa/native-android-guild-ime-20260909/README.md).
  Remote remains Draft `99a1cd121`; this source is local only.
- Local source `ac1a37d55` fixes touch rail Close leaving Help open, using the
  shared complete-close method. API 31 before/after replay and host tests
  76/76 normal, 78/78 preview pass; modal/inactive guards remain intact.
  [Close evidence](generated/player-qa/native-android-rail-close-20260908/README.md).
  Read-only remote check remains `99a1cd121`, Draft; no push retry this round.
- Latest local editor source `fafbc716f` adds trade/guild amount IME geometry
  and multiline mail/notice input. Trade amount reopening/cancel and two-line
  mail text have API 31 evidence; guild-specific populated cases remain open.
  Android tests 74/74 normal, 76/76 preview; shared UI 575/575. These later
  commits/evidence are not yet confirmed remote: PR head last verified `99a1cd121`
  after HTTP 408/SSH push failures. [Latest editor evidence](generated/player-qa/native-android-editors-20260908/README.md).
- Source `79d6d7ce2` fixes GameActivity Back bypass, shared modal cancellation,
  same-field IME reopening and inventory amount bounds. Source `99a1cd121`
  bounds mail compose with six-row attachment paging. API 31 amount/recipient
  input, Back priority, Help close and page-two attachment selection have
  targeted evidence. Other nested fields and multiline IME are still open.
  [IME and mail evidence](generated/player-qa/native-android-ime-20260908/README.md).
- The two initial touch gaps have targeted API 31 regression evidence at
  source `62344b0e5`: same-frame tap edges and hit-test ordering are fixed;
  bag/help keep a 24 OS-logical-pixel top gutter. Bag dragging and short-tap
  inspect → Panels → Close now pass. See
  [touch regression evidence](generated/player-qa/native-android-touch-20260908/README.md).
  This does not establish every window's touch behavior or physical-device acceptance.
- The real login socket still lacks the shared gameplay bootstrap/read-model
  projection and complete gameplay/transaction intent dispatch. Real StartGame
  remains on the transition screen, rather than fabricating a playable map.
- Character creation/deletion/password changes and gameplay operations need
  actual request/result wiring and an approved Gateway test environment.
- Populated quests/skills/shops/maps, all nested dialogs and item operations
  need scenario-level interaction evidence, not merely a screenshot of a root panel.
- Dedicated virtual joystick, multi-touch combat/skill controls and touch-only
  revival require a further Android input slice; desktop keyboard affordances
  in the shared UI do not prove phone usability.
- Android audio, exact Crystal visual/feel parity, physical-device touch/IME,
  lifecycle/reconnect, low-end performance, signing and human acceptance remain open.
- The original chat frame PNG is white in this source pack. This work does
  not silently recolour source assets or claim original-client A/B acceptance.
- Full Windows audio-enabled regression is not established here: this local
  crates.io cache lacks Bevy audio 0.19. Shared visual-feature tests are a
  separate denominator.

Evidence: [Android player-UI baseline](generated/player-qa/native-android-player-ui-20260908/README.md).
No production deployment, real account login or save mutation was performed.
