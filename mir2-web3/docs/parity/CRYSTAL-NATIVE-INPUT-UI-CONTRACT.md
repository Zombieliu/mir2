# Crystal Native Client UI/Input Contract

Status: read-only audit artifact. This document is not an acceptance claim and
does not declare the denominator complete.

## 1. Scope and authority

The authoritative behaviour order is:

1. `Crystal/Client` control, scene, focus, mouse, keyboard, window and render
   code.
2. `Crystal/Shared/ClientPackets.cs`, `Crystal/Shared/ServerPackets.cs` and
   the packet IDs in `Crystal/Shared/Enums.cs`.
3. `mir2-web3/apps/game-client/ui-core`, `client-bevy`,
   `platform-windows`, `platform-android`, the runtime bridge, and the Web
   client are implementation evidence and regression targets.

Web code is a comparison/compatibility surface. Reusing the Gateway and
Simulation packet semantics does not make a native control equivalent: the
native host still has to reproduce Crystal's focus, hit-testing, pressed and
disabled sprites, modal ordering, world-click capture, window focus and
resize behaviour.

This audit is bound to the following dirty Crystal snapshot:

```text
Crystal HEAD: 484983404e3d6afa584e93801f8006ae3429bea9
sourceRootClean: false
sourceFileInventoryComplete: false
semanticLeafInventoryComplete: false
inventoryComplete: false
source inventory: 403 .cs files
inventory aggregate: aad6086d4e0833827571d222b7ca978256210e6dbcbf1300c0decfc6a01cc25e
```

`sourceRootClean=false` is material. A later clean checkout or a changed
working tree must regenerate the inventory and re-review every affected row.
No percentage or “100%” statement is valid while any completion flag above is
false.

## 2. Machine-counted registry and counting rules

The values below are reproducible from the repository root with the commands
shown immediately after the table. They are audit counts, not proof of
semantic parity. All paths in the commands are repository-relative; no drive
letter or machine-specific checkout path is required.

| Registry | Machine-counted result | Exact counting rule |
|---|---:|---|
| `Crystal/Client/MirScenes/**/*.cs` | 39 files | Recursive files whose extension is `.cs` under exactly `Crystal/Client/MirScenes` |
| Public `class` declarations in that tree | 118 | Regex `^\s*public\s+(?:sealed\s+)?class\s+` over the concatenated file contents |
| Crystal control-type token occurrences in that tree | 1,764 | Only the closed type set listed below; every matching token occurrence is counted, including declarations, arrays, construction and use |
| `.Click +=` handlers in `Crystal/Client/MirScenes` | 473 | One `rg` output line per matching `.Click +=` occurrence |
| `.Click +=` handlers in `Crystal/Client/MirControls` | 39 | One `rg` output line per matching `.Click +=` occurrence |
| Default KeyBind assignments | 96 | Only lines containing the exact assignment prefix `InputKey = new KeyBind {` |
| `Crystal/Shared/ClientPackets.cs` packet subclasses | 153 | Only public classes matching `: Packet`; this file has no other public class outside that packet rule |
| `Crystal/Shared/ServerPackets.cs` packet subclasses | 277 | Only public classes matching `: Packet`; all public classes total 279, so two public classes are deliberately excluded |
| Native `ui-core::registry::all_controls()` | 173 | The checked-in unit-test assertion, not a macro-occurrence heuristic |
| Native registry test `controls.len()` | 173 | `registry.rs:1388-1391` asserts the same value |

### 2.1 Exact PowerShell/rg reproduction commands

Run these commands from the repository root. The output shown is the output
for the bound audit snapshot; a changed/dirty source tree is expected to
change the corresponding count.

```powershell
# 39 source files
$sceneFiles = @(Get-ChildItem Crystal/Client/MirScenes -Recurse -File -Filter *.cs)
$sceneFiles.Count
# 39

# 118 public class declarations; helpers/rows are intentionally included
$sceneText = ($sceneFiles | ForEach-Object { Get-Content -Raw $_.FullName }) -join "`n"
([regex]::Matches($sceneText, '(?m)^\s*public\s+(?:sealed\s+)?class\s+')).Count
# 118

# 1,764 strict Crystal control-type token occurrences.
# This is NOT the broad token regex \\bMir[A-Za-z0-9_]+\\b.
$controlTypePattern = '\bMir(?:Button|TextBox|InputBox|CheckBox|DropDownBox|AmountBox|ItemCell|GoodsCell|GameShopCell|ImageControl|Label)\b'
([regex]::Matches($sceneText, $controlTypePattern)).Count
# 1764

# The broad comparison mentioned below, over the same 39 MirScenes files
$broadMirPattern = '\bMir[A-Za-z0-9_]+\b'
([regex]::Matches($sceneText, $broadMirPattern)).Count
# 2606

# 473 scene click handlers and 39 base/control click handlers
$sceneClicks = @(rg -n '\.Click\s*\+=' Crystal/Client/MirScenes)
$controlClicks = @(rg -n '\.Click\s*\+=' Crystal/Client/MirControls)
$sceneClicks.Count
# 473
$controlClicks.Count
# 39

# 96 exact KeyBind assignments (not enum members, not descriptions)
$keyBindAssignments = @(rg -n 'InputKey = new KeyBind \{' Crystal/Client/KeyBindSettings.cs)
$keyBindAssignments.Count
# 96

# 153 client packet subclasses; 277 server packet subclasses
$packetClassPattern = '(?m)^\s*public\s+(?:sealed\s+)?class\s+\w+\s*:\s*Packet'
$clientPacketText = Get-Content -Raw Crystal/Shared/ClientPackets.cs
$serverPacketText = Get-Content -Raw Crystal/Shared/ServerPackets.cs
([regex]::Matches($clientPacketText, $packetClassPattern)).Count
# 153
([regex]::Matches($serverPacketText, $packetClassPattern)).Count
# 277

# Cross-check that ServerPackets has 279 public classes, of which 277 are : Packet
$publicClassPattern = '(?m)^\s*public\s+(?:sealed\s+)?class\s+'
([regex]::Matches($serverPacketText, $publicClassPattern)).Count
# 279
```

The strict control-type metric is therefore a **token-occurrence metric**,
not a count of unique controls, instances, classes, or semantic leaves. Its
closed type set is exactly:

```text
MirButton, MirTextBox, MirInputBox, MirCheckBox, MirDropDownBox,
MirAmountBox, MirItemCell, MirGoodsCell, MirGameShopCell,
MirImageControl, MirLabel
```

It intentionally excludes `MirControl`, `MirScene`, `MirAnimatedControl`,
`MirAnimatedButton`, `MirScrollingLabel`, `MirMessageBox` and every other
`Mir*` symbol. The broad regex `\bMir[A-Za-z0-9_]+\b` answers a different
question and produced 2,606 tokens in the review; that number must not be
compared with 1,764. Neither value is the player-control denominator.

For the native count, do not count `c!(` occurrences: the macro definition,
tests and multiline invocations make that heuristic non-equivalent to the
registry. The authoritative source assertion is:

```text
mir2-web3/apps/game-client/ui-core/src/registry.rs:1388-1391
controls.len() == 173
```

The reproducible test command is:

```powershell
cargo test --manifest-path mir2-web3/apps/game-client/ui-core/Cargo.toml all_controls_are_unique_and_action_mapped -- --exact --nocapture
# expected: test registry::tests::all_controls_are_unique_and_action_mapped ... ok
# expected assertion: controls.len() == 173
```

The native value is the existing Candidate registry size only. It is not a
claim that the Crystal denominator is 173 or complete.

The Crystal registry is represented below as control families with explicit
instance cardinality rules. A family row is not a hand-picked example: it
means every named field/array/row type in the cited source range. Where an
array is runtime-sized, the row records the source bound rather than inventing
a fixed count. The denominator stays **open** until a generator emits one
record per concrete control instance, including dynamically created rows,
message pages, quest cells, item cells and NPC/script-created dialog choices.

Each eventual generated record must have these fields:

```text
scene, panel, controlId, sourcePath, sourceLine, sourceMember,
visibleLabelOrLocalizationKey, imageLibrary, baseIndex, hoverIndex,
pressedIndex, disabledIndex, rectOrLayoutRule, instanceCount,
hotkeyAndModifierRule, mouseGesture, visibleWhen, enabledWhen,
localStateTransition, uiActionOrNativeIntent, gatewayCommand,
clientPacket, serverReceipt, authoritativeSideEffect,
closePath, worldClickBlocking, status, evidenceRefs
```

`baseIndex/hoverIndex/pressedIndex/disabledIndex` must be `null` only when the
source control has no sprite index (for example a text box, a pure label, or a
procedurally rendered Bevy widget). A localization key is not a visible label
unless the resolved text is also captured.

## 3. Status vocabulary

- `VERIFIED_SOURCE_FACT`: directly visible in the cited Crystal/shared source.
- `INFERENCE`: a mapping inferred from naming or an adapter, not proven by a
  Crystal packet/control path.
- `IMPLEMENTATION_GAP`: Crystal has the operation, but the native/Web path is
  absent, narrower, or has different state/packet semantics.
- `BLOCKED_EXTERNAL`: cannot be proved from the repository snapshot; requires
  a clean Crystal source, live packet capture, real DPI/window run, or human
  visual/feel evidence.

P0 means a wrong authoritative action, unsafe credential/delete flow,
unauthenticated mutation, lost save/receipt, world input leaking through a
modal, or a control that can report success without a server receipt. P1 means
an observable control, focus, layout, sprite, hotkey, disabled state, or
close-path mismatch that does not directly corrupt authoritative state.

## 4. Scene and panel registry

### 4.1 Startup, login, account and character lifecycle

| Family / source | Label or image evidence | Input and guards | State/packet/effect contract | Close/blocking/status |
|---|---|---|---|---|
| `LoginScene` and `LoginDialog` — `Crystal/Client/MirScenes/LoginScene.cs:12-20,323-505` | Dialog base `Libraries.Prguse` index `1084`; title `Libraries.Title:30`; account/pass labels `Title:31/32`; OK `Title:320/321/322`; account `Title:323/324/325`; password `Title:326/327/328`; safe-key `Title:332/333/334`; close `Title:329/330/331`. Visible labels are localization-driven, not hard-coded English. | Account/password text boxes at `:413-458`; OK disabled until both validity flags; click, Enter and Tab focus are separate paths. | Login click sends `ClientPackets.Login` (`Crystal/Shared/ClientPackets.cs:170-189`); success is `ServerPackets.LoginSuccess` (`Crystal/Shared/ServerPackets.cs:282-353`) and transitions to select. No local success. | Login owns focus; cancel closes the form. P0 if password is logged/persisted or a local success is inferred. `VERIFIED_SOURCE_FACT`; native adapter evidence in `mir2-web3/apps/game-client/client-bevy/src/native_shell.rs:964-1165` and `native_shell_ui.rs:406-617`. |
| New-account modal — `LoginScene.cs:746-1132` | Base `Prguse:63`; OK `Title:200/201/202`; Cancel `Title:203/204/205`; seven text fields: account, password x2, user name, birth date, question, answer, e-mail at `:750-913`; validation notices at `:1060-1113`. | All required validity flags must be true; text fields are bounded and password fields masked. Mouse click, Tab, Enter and Escape must not bypass validation. | Sends `ClientPackets.NewAccount` (`ClientPackets.cs:53-87`); result is `ServerPackets.NewAccount` (`ServerPackets.cs:88-117`). Server result controls notice and whether login remains available. | Modal blocks login/world. `VERIFIED_SOURCE_FACT`; native command exists in `ui-core::GatewayCommand::RegisterAccount` (`mir2-web3/apps/game-client/ui-core/src/effect.rs:118-143`), but all source fields/receipt details are not proven 1:1: P1/`IMPLEMENTATION_GAP`. |
| Change password — `LoginScene.cs:1162-1344` | OK/Cancel and four fields are constructed at `:1183-1263`; button indices are in the Title library; labels are localized. | Account, old password, new password and confirmation validity are required; Enter submits only from the submit focus; Escape cancels. | Crystal `ClientPackets.ChangePassword` (`ClientPackets.cs:89-111`) and receipts `ServerPackets.ChangePassword`/`ChangePasswordBanned` (`ServerPackets.cs:119-167`). Native uses a secret-bearing in-memory `SecurityRequest` and waits for `ChangePasswordResult` (`ui-core/src/effect.rs:41-77`, `native_shell_ui.rs:764-829`). | Must never stage credentials in `UiState`, logs or JSON. Cancel hides only this modal. P0 security gate if a receipt is accepted without matching account/request; otherwise `VERIFIED_SOURCE_FACT` plus native security path. |
| Safe Key — `LoginScene.cs:553-744` | Numeric/alphabetic key buttons are generated from the source lists; Esc/Delete/Random/Enter are at `:571-698`; image indices are generated per button, not a single fixed sprite. | Account/password focus, key press, delete, randomize and Enter; only masked password input is edited. | Local input aid; no Crystal server packet is sent by the Safe Key dialog. Native explicitly maps `SafeKey` to local state (`ui-core/src/action.rs:10-20`, `native_shell_ui.rs:831-850`). | P0 if treated as password reset or server success. Close returns to login. `VERIFIED_SOURCE_FACT`. |
| Character select — `SelectScene.cs:10-18,60-193,566-635` | Start `Title:340/341/342`; New `343/344/345`; Delete `346/347/348`; Credits `349/350/351`; Exit `352/353/354`; character rows are four generated row controls at `:134-193`, with character sprite `ChrSel:220` plus `+560` blend at `:245-258`. | Four row hit regions; empty slots are visible but must not select; Start/Delete require a selected character; New requires a free slot. | Start sends `ClientPackets.StartGame` (`ClientPackets.cs:226-239`), delete sends `DeleteCharacter` (`:211-224`) only after confirmation; receipts are `ServerPackets.StartGame*` (`ServerPackets.cs:398-467`) and `DeleteCharacter*` (`:355-396`). | Credits is an intentional source no-op; Exit closes the client. `VERIFIED_SOURCE_FACT`; native registry records Credits as no-op, but native screenshot/human evidence remains `BLOCKED_EXTERNAL`. |
| Character create — `Crystal/Client/MirScenes/Dialogs/NewCharacterDialog.cs:8-27` and native shell `native_shell_ui.rs:1086-1163` | Class/gender controls use `CrystalButtonSpec` and Title assets; exact current native constructors are at `native_shell_ui.rs:1086-1163`. | Name input; class/gender selection; Create disabled until valid; Enter/Create and Escape/Cancel. | Sends `ClientPackets.NewCharacter` (`ClientPackets.cs:191-209`); roster refresh/receipt is authoritative. | Modal blocks select. `VERIFIED_SOURCE_FACT` for Crystal class; native mapping is `INFERENCE` until field-by-field visual/packet capture. |
| Delete confirm — Crystal prompt call sites include `SelectScene.cs:370-395`; native implementation `native_shell.rs:895-922`, `native_shell_ui.rs:852-880,1460-1493` | Confirm/Cancel button sprites are from the native Crystal spec; source dialog uses localized message/input. | Selected character and exact name/confirmation required; Enter follows focused Confirm/Cancel; Escape cancels; second confirm must be effect-free. | Only Confirm sends `GatewayCommand::DeleteCharacter` -> `ClientPackets.DeleteCharacter`; server receipt controls roster update. | P0: no direct delete on first click, no double send, no wrong index. Modal blocks all underlying select controls. Native reducer tests explicitly cover this (`ui-core/src/reducer.rs:1475-1498`). `VERIFIED_SOURCE_FACT` for safety; visual `BLOCKED_EXTERNAL`. |
| Starting/disconnect — native shell `native_shell.rs:964-1165`, UI `native_shell_ui.rs:1539-1553`; Crystal disconnect base `MirScene.cs` packet path | Connecting/Starting/Connection Lost are shell states, not world panels. | Retry only on ConnectionLost; no world movement while not InGame; window focus must gate input. | `LoginSuccess`, `StartGame`, `Disconnect`, `ReturnToLogin` are receipts; retry sends a new connection request, not a replay of stale gameplay mutations. | `UiScreen::ConnectionLost` and `blocks_world_click` must be fail-closed. `VERIFIED_SOURCE_FACT` for native state machine; live reconnect timing `BLOCKED_EXTERNAL`. |

### 4.2 In-game HUD and world input

| Family / source | Label/image and instances | Input contract | State/packet/side effect | Status |
|---|---|---|---|---|
| Main HUD — `Crystal/Client/MirScenes/GameScene.cs:291-360`; `MainDialogs.cs:13-28,64-250` | Main base chooses `Prguse` index by resolution; buttons use `Prguse` indices around `1900-1914` (Character/Inventory/Skill/Quest/Option/GameShop/Menu); labels are HP/MP/name/level/gold/weight/modes; one instance each. | Click buttons, hover/pressed sprites; HUD buttons toggle or switch panels. Hotkeys are the corresponding KeyBind records. | Mostly local panel state; panel actions become `UiAction`/Gateway commands only where Crystal has a packet. | HUD should not block world outside its button rects; `BLOCKED_EXTERNAL` for exact pixel/scale parity. |
| Minimap / mail / big map — `MainDialogs.cs:1764-2110`; `BigMapDialog.cs:12-88,101-252,368-445` | Minimap has Toggle/BigMap/Mail buttons, map name/location labels, light/new-mail indicators; BigMap has Close/scroll/world/my-location/teleport/search and NPC rows. BigMap rows are dynamic, source bound is 18 visible rows in the native registry. | V key toggles minimap, B toggles BigMap, wheel/drag scrolls map/NPC list, world click selects map coordinates, search requires enabled text box, teleport is disabled unless selected NPC/map eligibility is true. | `SearchMap`, `TeleportToNPC`, `RequestMapInfo`; server `NPCGoods`, map/NPC markers and location receipts. Local cursor/selection never teleports optimistically. | Map canvas and list modal block world clicks; exact coordinate transform and zoom/resize are P1 `BLOCKED_EXTERNAL`. |
| World movement — `GameScene.cs:504-575`, `10275-10420`; `MirControls/MirScene.cs` mouse dispatch | MapControl is the world hit region. Crystal movement uses mouse buttons, drag/target rules and optional `Settings.NewMove`; default key records include movement-related actions. | Click/hold/drag world, right-click/target, keyboard movement, Alt/Shift modifiers; window deactivation clears buttons/modifiers (`Crystal/Client/Forms/CMain.cs:42-57,112-130`). | `ClientPackets.Walk`, `Run`, `Turn` (`ClientPackets.cs:252-293`) are intents; server returns `UserLocation`, `ObjectWalk`, `ObjectRun`, `ObjectTurn`. | World input must be suppressed when an overlay, text field, scrollbar, window drag, button press or unfocused window captures the pointer. Native gate is `platform-windows/src/input.rs:64-121,173-232`; P0 if a modal leaks a move/attack. |
| Combat/harvest/pickup — `GameScene.cs:728-832,1100-1120,11359+`; packet classes `ClientPackets.cs:701-859` | Target cursor/labels are map objects, not ordinary HUD buttons. Attack/harvest/pickup labels and feedback are dynamic. | Left/right click target rules, pickup cooldown, spell target lock, Alt harvest, dead-player revive. | `Attack`, `RangeAttack`, `Harvest`, `PickUp`, `TownRevive`; receipts include object attack/health/death/drop/remove packets (`ServerPackets.cs:2128-2164,2372-2592`). | P0 if native fabricates damage or pickup; P1 if cursor/feedback differs. Native shortcut coverage is `platform-windows/src/input.rs:337-498`; real visual/hand-feel is `BLOCKED_EXTERNAL`. |
| Chat — `MainDialogs.cs:563-748,1255-1512`; `ChatOptionDialog.cs:7-291` | Chat input, Home/Up/Down/End/position controls, channel buttons Normal/Shout/Whisper/Lover/Mentor/Group/Guild/Trade, report/settings; option dialog has All/General/Whisper/Shout/System/Lover/Mentor/Group/Guild and transparency controls. | Focus captures keyboard; Enter sends; Escape cancels; scrolling and resize are local. No world click while text input is focused. | `ClientPackets.Chat` (`ClientPackets.cs:295-320`), `ReportIssue`; server `Chat`, `ObjectChat`, `ChatItemStats`; filters/transparency are local settings. | Native shared reducer has `FocusChat/SendChat/CancelChatDraft`; Web and Android must preserve channel and draft separation. `VERIFIED_SOURCE_FACT` for Crystal; native implementation `IMPLEMENTATION_GAP` where only keyboard rather than mouse geometry is available. |
| Skill bar / spell assignment — `MainDialogs.cs:1516-1762,3270-3903`; KeyBind defaults `KeyBindSettings.cs:243+` | Two 8-slot bars (`Cells`, `KeyNameLabels`, cooldowns) plus F1-F8/secondary modifier and assignment panel F-key array. Crystal indices are dynamic per spell/skill sprite, not one global index. | F1-F8 with SkillMode modifier; click slot, assign/clear binding; cooldown/level/MP/target guards. | `MagicKey`, `Magic`, `SpellToggle` (`ClientPackets.cs:1091-1141,1488-1511`) and object magic/effect/projectile receipts (`ServerPackets.cs:3469-3560,4062+`). | Must not send duplicate cast on key repeat or cast while not InGame. Native key mapping `platform-windows/src/input.rs:363-522`; exact spell image/animation parity is P1/`BLOCKED_EXTERNAL`. |

### 4.3 Character, inventory, belt and item operations

| Family / source | Label/image and count | Input/guard | State/packet/result | Status |
|---|---|---|---|---|
| Inventory and quest inventory — `InventoryDialog.cs:10-18,430-440` | `Grid` and `QuestGrid` are arrays; ten `LockBar` entries; Use/Item/Quest/Add/Delete/Close buttons; item sprite and label are data-driven by `UserItem` and `Items` library. | Click item cell selects/inspects; double-click/use; drag source→destination; quest tab must not route to ordinary inventory. | `UseItem`, `MoveItem`, `MergeItem`, `SplitItem`, `DropItem`, `RemoveItem`, `RemoveSlotItem`; server receipts `UserSlotsRefresh`, `MoveItem`, `UseItem`, `DropItem`, `DeleteItem`. | P0 if Quest routes to Inventory or a local item mutation is treated as authoritative. Native reducer explicitly guards Quest/Option route (`ui-core/src/reducer.rs:293-328` and tests). |
| Belt — `InventoryDialog.cs:600-727` | Six `Grid` slots and six key labels; Rotate/Close. | Z toggles/rotates; Digit1-Digit6 use belt slots; item cell click/use. | `UseItem` with belt grid and authoritative inventory refresh. | Native `platform-windows/src/input.rs:363-396` covers Digit1-6. Source/UI exact belt sprite placement remains P1. |
| Character/equipment/status/skill pages — `CharacterDialog.cs:8-20,146-342,599-699` | Character/Status/State/Skill page buttons; 14 equipment cells initialized at `CharacterDialog.cs:227-342`; stats labels; Next/Back. Item sprite comes from `Items`/equipment libraries and is not a single fixed UI index. | Page clicks and cell inspect; equipment cell use/equip/unequip rules; page visibility is mutually exclusive. | `EquipItem`, `RemoveItem`, `RemoveSlotItem`, `UseItem`; authoritative `UserInformation`, slot refresh and item receipts. | P0 for wrong grid/slot; P1 for page/sprite/label mismatch. Native `OpenCharacter` and overlay path exist but hero equipment remains a separate gap. |
| Item inspect/action family — source calls in `GameScene.cs:1208-1280,4324-4344` and native overlay `client-bevy/src/crystal_ui/overlays.rs:6650-6860` | Use/Equip/Unequip action buttons; label derives from selected item. | Visible only with a valid selected cell; disabled for wrong grid, dead/unauthenticated, or no item. | Sends typed `UseItem`/`EquipItem`/`UnequipItem`; no optimistic inventory removal. | `VERIFIED_SOURCE_FACT` for action distinction; UID and exact metadata round-trip is P0/`IMPLEMENTATION_GAP` where native drop/pickup differs from Crystal. |
| Hero inventory/belt/equipment — `HeroDialogs.cs:9-22,248-375,385-866`; GameScene construction `GameScene.cs:6168-6169` | Hero inventory/equipment/belt arrays, auto-potion buttons/locks and hero menu buttons. | Only visible when Hero is summoned/available; Ctrl-modified keybinds use hero context. | Hero item packets include `TakeBackHeroItem`, `TransferHeroItem`, `SetAutoPotItem/Value`, `ChangeHero`; current native support must not fall back into player inventory. | `IMPLEMENTATION_GAP` / P0 for silent fallback; source is verified, native/Web coverage must be proven separately. |

### 4.4 Quest, NPC, options and system menu

| Family / source | Visible controls and source | Transition / packet | Blocking and parity status |
|---|---|---|---|
| Quest diary/list/detail/tracking — `QuestDialogs.cs:15-1006,1396-1908`; `GameScene.cs:356` | Quest rows/cells, requirement/name/icon/selected images, scroll/position controls, reward rows and tracking. Crystal Q opens QuestDiary via `KeybindOptions.Quests` (`GameScene.cs:660-663`). | Local selected/track state; Accept/Finish/Abandon/Share/Reward paths map to `ClientPackets.AcceptQuest`, `FinishQuest`, `AbandonQuest`, `ShareQuest` (`ClientPackets.cs:1900-1970`) and authoritative quest/receipt packets. | Quest panel blocks world, and closing only hides the quest layer. P0 if an Option click leaves Quest visible or routes to Quest; native regression covers this in `client-bevy/src/crystal_ui/overlays.rs:8984-9077`. Exact imported script/result coverage remains `BLOCKED_EXTERNAL`. |
| NPC dialog and service — `NPCDialogs.cs:16-34,1051-1065,1433-1440,1867-1874,2256-2803`; GameScene `NPCDialog` construction `:302-310` | NPC text rows/scroll/buttons/Quest/Help; goods cells; drop/craft/refine/storage/awakening item cells and buttons. Image indices are library/data-driven; labels are script/localization output. | `CallNPC`, `NPCConfirmInput`, `BuyItem`, `SellItem`, `RepairItem`, `SRepairItem`, `CraftItem`, `CheckRefine`, `RefineItem`, `UnlockStorage` etc. (`ClientPackets.cs:859-980,2456+`); receipts include `NPCResponse`, `NPCGoods`, `NPCStorage` and service packets (`ServerPackets.cs:2844-3212`). | NPC service context, range, enabled state and modal nesting are authoritative. `IMPLEMENTATION_GAP` for complete script/edge semantics; no UI should fabricate a successful trade/craft. |
| Crystal Options — `MainDialogs.cs:2527-2800` | Close `Prguse2:360/361/362`; seven on/off pairs with pressed indices around `451,454,457,460,463,466`; sound/music bars are image controls; Observe on/off is separate server request. | SkillMode, SkillBar, Effect, DropView, NameView, HPView, NewMove are immediate local settings; Close hides and does not Apply/Cancel/Defaults. Observe sends `ClientPackets.Observe` (`ClientPackets.cs:737-754`) and waits for `ServerPackets.AllowObserve` (`ServerPackets.cs:3916-3933`). | P0 if native uses a staged Apply/Cancel contract for Crystal Options or changes observe locally before receipt. Native reducer test is `ui-core/src/reducer.rs:1503-1599,1634-1662`; status `VERIFIED_SOURCE_FACT` for semantics. |
| System menu — `MainDialogs.cs:3007-3270`; GameScene `Closeall` `:668-712` | Exit, Logout, Group, Guild and source-only Help/Keyboard/Ranking/Creature/Ride/Fishing/Friend/Mentor/Relationship families. Some controls are visible source sprites but not necessarily wired in the current native slice. | Exit is local process action; Logout sends `ClientPackets.LogOut` (`ClientPackets.cs:241-250`); Group/Guild open panels; unsupported source controls must remain disabled/no-op rather than redirect. | CloseAll hides every overlay and clears item-label state. Native registry marks a disabled source family; any newly interactive native control needs a typed action owner. P1 for visual/disabled mismatch; P0 for logout/save failure. |

### 4.5 Mail, shop, storage, trade, group and guild

| Family / source | Control family / source line | Data flow and authority | Close, blocking, status |
|---|---|---|---|
| Mail list/read/compose — `MailDialogs.cs:9-426,596-1170`; Main HUD `MainDialogs.cs:1764-1772` | Dynamic mail rows with sender/message/icon/unread/parcel/locked/selected images; letter and parcel compose/read controls; parcel has exactly five `MirItemCell` slots at `:689-696,1114`. | `SendMail`, `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail` (`ClientPackets.cs:2122-2275`); receipts `ReceiveMail`, `MailSent`, `MailCost`, `MailLockedItem` (`ServerPackets.cs:5886-5996`). Recipient/message/gold/attachment drafts are local until send; claim/delete/read require matching server receipt. | Mail modal captures world clicks; compose cancel discards local draft only. Native intent queue and Web mail model are evidence, not Crystal proof: `client-bevy/src/crystal_ui/overlays.rs:659-705,3718-3760`, `apps/web/app/page.tsx:7410-7615`. P0 for local claim/delete or lost attachment; P1 for row/page visual mismatch. |
| NPC shop — `NPCDialogs.cs:1051-1432`; service is opened by `NPCGoods` | Goods cell family, Buy, scroll/position; Sell/Repair/SRepair are separate service modes. | `BuyItem`, `SellItem`, `RepairItem`, `SRepairItem`; authoritative `NPCGoods`, `NPCSell`, `NPCRepair`, `NPCSRepair`, `SellItem`, `RepairItem` (`ServerPackets.cs:3074-3259`). Current NPC object/range/service context must still be valid. | Shop blocks world and closes with `CloseShop`; no optimistic gold/item mutation. Native `OpenNpcShop` is a route, not proof of Crystal visual/service parity. |
| Cash/Game shop — `GameshopDialog.cs:7-24`; `GameScene.cs:303-304` | Class/category buttons, item filters, search, page/scroll controls, gold/credit checkboxes, quantity and close. | `GameshopBuy` (`ClientPackets.cs:2434-2454`) and receipts/catalog `GameShopInfo`, `GameShopStock` (`ServerPackets.cs:5348-5389`). Catalog/payment/quantity are local draft; buy is authoritative. | Modal blocks world, duplicate buy is suppressed until receipt. Native/Web typed flow is `ui-core/src/effect.rs:150+`, `client-bevy/src/crystal_ui/overlays.rs:7958-8240`; P1 until source visual indices and live receipt ordering are captured. |
| Storage/Warehouse — `NPCDialogs.cs:2798-3310`; `ClientPackets.cs:113-168,322-420,487-630`; `ServerPackets.cs:3212-3244,3673-3717` | Storage item grid, tabs/scroll, rent/protect/close, locked page/password label; expanded storage is data-driven. | `StoreItem`, `TakeBackItem`, `MoveItem`, `SplitItem`, `MergeItem`, `UnlockStorage`, `SetStoragePassword`, `RemoveStoragePassword`, plus `UserStorage`, `NPCStorage`, `StorageUnlockResult`, `StoragePasswordResult`, `ResizeStorage`. | Storage service must be active/in-range and password state must be receipt-backed. Native typed pending request IDs are in `client-bevy/src/crystal_ui/overlays.rs:1643-1652,8291-8560`; Web receipt handling is `apps/web/app/page.tsx:7209-7260,9153-9241`. P0 for stale service, duplicate mutation, or storage data overwrite; P1 for panel geometry. |
| Trade — `TradeDialogs.cs:9-203`; `ClientPackets.cs:454-486,1413-1466`; `ServerPackets.cs:1896-1987` | Own/guest item grids, gold labels, Confirm/Close, dynamic selected cells. | Request/reply, deposit/retrieve item, gold, confirm lock and cancel; server sends request/accept/gold/item/confirm/cancel. | Trade is modal and must block world; confirmation is a two-party authoritative state, not a local toggle. Current native `OpenTrade`/Web stage5 support is not a source-complete implementation: P0/P1 `IMPLEMENTATION_GAP`. |
| Group — `GroupDialog.cs:10-18`; `ClientPackets.cs:1143-1198`; `ServerPackets.cs:3719-3770` | Switch, Add, Delete, Close and dynamic member rows. | `SwitchGroup`, `AddMember`, `DelMember`, `GroupInvite`; server `GroupMembersMap`, invite/delete receipts. | Group panel blocks only its own modal area; invite confirmation must not mutate before receipt. Native `OpenGroup` and intent mapping are present, but full Crystal live multi-user evidence is `BLOCKED_EXTERNAL`. |
| Guild — `GuildDialog.cs:12-111,2184-2189`; `ClientPackets.cs:1700-1840`; `ServerPackets.cs:4470-4709` | Notice/members/storage/rank/buff/status tabs; member rows/dropdowns/delete buttons; rank permission buttons; guild storage item cells and gold controls. | `EditGuildMember`, `EditGuildNotice`, `GuildInvite`, `RequestGuildInfo`, `GuildStorageGoldChange`, `GuildStorageItemChange`; receipts `GuildNoticeChange`, `GuildMemberChange`, `GuildStatus`, `GuildStorageList` and related. | Per-tab local drafts must commit only on server request/receipt; guild storage has independent slot/permission guards. Native overlay mapping exists (`client-bevy/src/crystal_ui/overlays.rs:1550-1607,3168-3368`); exact source control/image and live guild receipt parity remain P1/P0 `BLOCKED_EXTERNAL`. |

### 4.6 Additional Crystal dialogs that keep the denominator open

These are actual source classes and therefore cannot be silently excluded from
the contract even if the current Windows vertical slice does not expose them:

| Surface | Crystal source | Primary controls/data | Current disposition |
|---|---|---|---|
| Friends, memo, relationship, mentor | `FriendDialog.cs:9-14,480-484`; `RelationshipDialog.cs:9-13`; `MentorDialog.cs:9-13` | Page/add/remove/memo/mail/whisper; relationship allow/request/divorce; mentor allow/add/remove | `IMPLEMENTATION_GAP` unless packet and receipt rows are added to the generated registry |
| Ranking/help/keybind | `RankingDialog.cs:7-13`; `HelpDialog.cs:7-12`; `KeyboardLayoutDialog.cs:7-14` | Class tabs, online filter, rank rows; paged help; keybind rows/reset/enforce | Visible source surface; native registry currently treats several as disabled source controls. P1 |
| Intelligent creature/pet | `IntelligentCreatureDialogs.cs:10-16,1128-1146,1276-1289` | Summon/dismiss/release/mode/options/grade | `IMPLEMENTATION_GAP`; server packets exist but native UI/packet receipts need source trace |
| Mount/fishing | `MountDialog.cs:10-15`; `FishingDialog.cs:10-14,159-164` | Mount item grid/ride; fishing grid/cast/autocast/fish/ESC | `IMPLEMENTATION_GAP`; do not fall through to player inventory |
| Craft/refine/awakening/socket | `NPCDialogs.cs:1867-1874,2256-2276,2726-2729`; `SocketDialog.cs:7-10` | Material cells, recipe/refine/awakening selectors and confirm buttons | `IMPLEMENTATION_GAP`; packet families exist but exact UI and service guards are not included in native Candidate registry |
| Rental/market/consignment/trust merchant | `ItemRentalDialog.cs:11`; `ItemRentDialog.cs:9-203`; `ItemRentingDialog.cs:10-221`; `TrustMerchantDialog.cs:13-47` | Search/price/find/refresh/buy/sell/collect, rental cells and locks | `IMPLEMENTATION_GAP`; packet classes exist (`ClientPackets.cs:1513-1669,2540-2684`) |
| Guild territory/report/notice/roll/timer | `GuildTerritoryDialog .cs:78-84`; `ReportDialog.cs:7`; `NoticeDialog.cs:10-19`; `RollDialog.cs:9`; `TimerDialog.cs:7` | Purchase/notice/issue/roll/timer surfaces | `IMPLEMENTATION_GAP` or `BLOCKED_EXTERNAL` depending live protocol evidence |

## 5. Native action and state contract

The shared native contract is a useful semantic spine, but it must not be
mistaken for proof that every Crystal control has been implemented.

| Layer | Source | Contract |
|---|---|---|
| Input vector | `apps/game-client/platform-windows/src/input.rs:64-121,173-232,242-522` | Window focus, shell screen, pointer capture, chat focus, window/scrollbar/button dragging and pressed edges gate world actions. W/A/S/D and arrows walk; Shift runs; E turns; V revives; Digit1-6 use belt; F1-F8 use skill slots; native Magic carries target/direction data. |
| Native shell | `apps/game-client/platform-windows/src/shell_bridge.rs:73-274`; `client-bevy/src/native_shell.rs:964-1165` | Connection/login/select/create/delete/security/starting/in-game states are an explicit state machine. Network receipts, not button presses, advance authoritative states. |
| Platform-independent action | `apps/game-client/ui-core/src/action.rs:6-237` | `UiAction` contains login/security/select/panel/mail/shop/storage/item/quest/chat/combat/system actions. Secret credentials use `SecretText`; no secret belongs in persistent `UiState`. |
| Reducer | `apps/game-client/ui-core/src/reducer.rs:99-566` | `reduce(state, action)` produces a successor state and typed effects. Delete confirmation, ChangePassword pending, Observe pending, mail drafts, storage requests and option persistence have explicit guards. |
| State | `apps/game-client/ui-core/src/state.rs:9-55,384-432,717-730` | Screen, panel and security panel are separate. `blocks_world_click()` is a contract, not a rendering hint. |
| Effect/packet boundary | `apps/game-client/ui-core/src/effect.rs:41-238`; `platform-windows/src/native_protocol.rs:31-180` | Gateway commands are request-shaped and must be serialized once. Server results must be routed back as actions/events; local UI state must not impersonate a server receipt. |
| Bevy overlay | `apps/game-client/client-bevy/src/crystal_ui/overlays.rs:550-566,1987-2165,2204-2259,2905-3040` | Overlay buttons use `Interaction::Pressed`, explicit z layers and typed actions. Quest/Option replacement and world blocking are tested at `:8984-9077`. |
| Android reuse | `apps/game-client/platform-android/src/android_input.rs:1+` and `src/lib.rs:370-471` | Android may reuse `UiAction`/`UiState`/`GatewayCommand`, but touch/long-press/drag/back/IME/safe-area mapping remains a platform adapter. It does not prove Windows mouse/keyboard parity. |
| Web | `apps/web/app/page.tsx:7410-7615` and `apps/web/lib/world-model/*` | Web sends BrowserCommand/packet-shaped intents and applies authoritative receipts. Web code is a regression reference for backend semantics, not a substitute for Crystal native hit regions or Win32 focus/window behaviour. |

## 6. Special audits

### 6.1 Security and destructive flows

- Delete is two-stage: select -> local DeleteConfirm -> exact index/name
  confirmation -> `DeleteCharacter` -> matching server receipt -> roster
  refresh. Any shortcut from select to packet is P0.
- ChangePassword holds a pending request and redacted `SecretText`. A failed
  receipt keeps the panel and shows an error; a successful receipt closes it.
  Duplicate submit while pending is effect-free.
- SafeKey is local only. It must not call ChangePassword and must not claim
  server success.
- Login, RegisterAccount, StartGame, DeleteCharacter, ChangePassword and all
  item/gold/mail/shop/storage mutations must fail closed when unauthenticated,
  not InGame, malformed, duplicate-pending, or disconnected.

### 6.2 Quest versus Option routing

Crystal Q opens the quest diary; the HUD Option control opens the OptionDialog.
They are different source classes, local state, labels, sprites and close
paths. The native regression must assert both directions:

```text
QuestLog + click Option -> Options visible, QuestLog hidden,
                   blocks_world_click = true
Options + click Option -> Options hidden,
                   blocks_world_click = false (unless another modal remains)
QuestLog + click Quest -> QuestLog hidden
```

The Bevy test at `client-bevy/src/crystal_ui/overlays.rs:8984-9077` proves this
for the current adapter. It does not close the Crystal source audit because
the source image/rect/focus evidence is still separate.

### 6.3 Mail, Shop and Storage data flow

The required sequence for each request is:

```text
pointer/key -> local draft/selection -> one typed intent
  -> one BrowserCommand/native wire command
  -> one Crystal-shaped ClientPacket
  -> authoritative server validation
  -> receipt packet
  -> model/read-model update
  -> visible result / pending release
```

No optimistic removal of an item, gold, attachment or storage entry is
allowed. Mail claim/delete/send, NPC shop buy/sell/repair, GameShop buy and
Storage deposit/withdraw/password/expand each need independent pending keys;
one operation's receipt must not clear another operation. A reconnect must
drop/revalidate pending mutations rather than replaying them.

### 6.4 Overlay Z, blocking, focus and mouse capture

The current native overlay contract uses HUD below modal layers and explicit
`GlobalZIndex` values around `985` and higher (`overlays.rs:1987-2165`). The
audit invariant is ordering, not the numeric value alone:

```text
world/map < HUD < normal panel < modal/security/delete < pressed/focus layer
```

For every visible overlay, test:

- button click reaches exactly one control;
- a click inside a modal never reaches the world;
- a click outside a movable panel follows Crystal's source behaviour;
- scrollbar drag captures until release;
- window drag captures and does not issue movement;
- chat/storage/password text focus captures keyboard and IME input;
- closing a panel releases its blocker and stale `Interaction::Pressed` state;
- disabled controls draw the disabled sprite and emit no action.

### 6.5 Resize, DPI, focus, window drag and Alt-Tab

Crystal uses a fixed logical stage from `Settings.ScreenWidth/ScreenHeight`,
centres dialogs, clips the cursor when full-screen or `MouseClip` is enabled,
and clears mouse buttons/modifiers on `CMain_Deactivate` (`Crystal/Client/Forms/CMain.cs:42-130`).
The native host must preserve the logical 1024x768 contract while mapping
physical pixels through its window scale. 100%/125%/150% DPI, resize during a
drag, Alt-Tab, focus loss during a held key, and re-focus after reconnect are
`BLOCKED_EXTERNAL` until executed on a real Windows display; source and unit
tests alone cannot certify them.

## 7. Fail-closed acceptance matrix

Every row is a gate. A missing command, packet, receipt, result, or evidence
is a failure, not an inferred pass.

| Vector | Expected `UiAction`/intent | Expected command/packet | Expected server/UI result | Negative gate |
|---|---|---|---|---|
| Login button / Enter | `Login` | `GatewayCommand::Login` -> `ClientPackets.Login` | `LoginSuccess` -> CharacterSelect | Empty fields, duplicate submit, stale receipt and password log must all fail |
| New Account | `RegisterAccount` | `NewAccount` | NewAccount receipt/notice | Invalid field or unauthenticated path emits no packet |
| Change Password | Open + `SubmitChangePassword` | `SecurityRequest::ChangePassword` -> `ClientPackets.ChangePassword` | matching ChangePassword result closes or retains panel | duplicate pending, mismatched account, redaction failure |
| Safe Key | `SafeKey`, key/delete/random/enter | no server packet | local masked input only | any ChangePassword packet is failure |
| Select/Create/Delete | `SelectCharacter`, `StartGame`, `CreateCharacter`, two-stage delete | `StartGame`, `NewCharacter`, confirmed `DeleteCharacter` | roster/start receipt | empty slot, wrong index, first delete click, double confirm |
| HUD Character/Inventory/Skill/Quest/Option/Menu | open/toggle actions | usually none; selected actions go through typed adapter | correct panel replacement and blocker | Quest/Option cross-route or world click leak |
| Walk/Run/Turn | keyboard/mouse intent | `Walk`/`Run`/`Turn` | authoritative `UserLocation`/object motion | unfocused window, modal, chat, scrollbar/window drag must emit none |
| Attack/Harvest/Pickup | target or world action | `Attack`/`RangeAttack`/`Harvest`/`PickUp` | object health/death/drop/remove or pickup receipt | no local damage, no pickup of stale object, cooldown duplicate |
| Belt/Skill | Digit1-6, F1-F8 + modifier | `UseItem`, `MagicKey`/`Magic` | item/skill/effect packets | wrong bar, insufficient resource, repeated key edge |
| Inventory/Character item | inspect/use/equip/drop/move/merge/split | exact grid/id command | slot/item receipt | Quest grid fallback, invalid UID, local mutation before receipt |
| Quest/NPC | select/accept/finish/abandon/interact | quest/NPC packet | authoritative dialog/quest/inventory/gold result | wrong NPC/range, stale dialog, Option route conflict |
| Mail | read/claim/delete/compose/send | exact mail command | matching mail receipt and model update | duplicate pending, attachment/gold loss, local claim/delete |
| NPC Shop/GameShop | select/quantity/buy/sell/repair | `BuyItem`/`SellItem`/`RepairItem`/`GameshopBuy` | catalog/gold/item receipts | stale service, double buy, optimistic gold |
| Storage | open/select/deposit/withdraw/password/expand | storage command with request ID | UserStorage/NPCStorage/receipt | locked/out-of-range, duplicate request, storage overwrite |
| Trade/Group/Guild | invite/accept/items/gold/rank/notice/storage | exact shared packet family | two-party/permission receipt | local accept/permission mutation, wrong member/slot |
| Chat | focus/type/Enter/Escape/filter/resize | `Chat`/settings local state | `Chat`/`ObjectChat` or persisted local settings | world click while focused, draft leak |
| Resize/DPI | window resize/scale change | none | logical 1024x768 rects remain stable | clipped modal, wrong hit rect, sprite blur/scale drift |
| Alt-Tab/focus | focus lost/re-gained | none | modifiers/mouse state cleared; reconnect state explicit | held movement or spell fires after refocus |
| Web regression | same backend command/receipt flow | BrowserCommand/Gateway/Simulation | Web panels unchanged and packet-compatible | native change alters Web-only route or receipt shape |

## 8. P0/P1 disposition ledger

### P0

- P0-UI-001: Crystal semantic denominator and generated per-control leaf
  inventory are open because the source root is dirty and the current counts
  include helper/control references, not player-observable leaves.
- P0-UI-002: DeleteConfirm must remain two-stage and index-bound; no direct or
  duplicate `DeleteCharacter`.
- P0-UI-003: ChangePassword secrets must remain transient/redacted and receipt
  scoped; SafeKey must remain local-only.
- P0-UI-004: Mail, shop and storage must be receipt-backed with independent
  pending operations; no optimistic authoritative mutation.
- P0-UI-005: Overlay, text focus, window drag, scrollbar drag and unfocused
  window must suppress world input.
- P0-UI-006: Crystal player-drop/pickup UID and nested item metadata must not be
  rewritten by a lossy native/Web adapter. The exact stack-merge distinction is
  documented in `docs/parity/CRYSTAL-GROUND-ITEM-CONTRACT.md`.

### P1

- P1-UI-001: Exact sprite library/index, localization output, pressed/hover/
  disabled asset and per-resolution rect for every generated control.
- P1-UI-002: Full source surfaces not in the current native registry: hero,
  pet, mount, fishing, rental, market, refine/craft/awakening/socket, social,
  guild territory and trust merchant.
- P1-UI-003: Crystal fixed-stage scaling, 125%/150% DPI, resize, Alt-Tab,
  cursor clipping and movable dialog behaviour on real Windows.
- P1-UI-004: Human visual/feel comparison against the same Crystal scene,
  including GDI text, animation timing, focus ring, drag feel, sound and
  disabled-state affordance.
- P1-UI-005: Web regression after each native/shared adapter change, including
  Quest/Option, Mail/Shop/Storage and reconnect state.

## 9. Acceptance evidence required to close this contract

The contract can only move a row from open to verified when all of the
following exist for that row:

1. A clean, hash-bound Crystal source snapshot and generated per-control
   registry, with no unresolved source/semantic/inventory completion flags.
2. Source line and constructor evidence for label/localization, library/index,
   rect/layout, enabled/visible conditions and focus/mouse rules.
3. A deterministic native unit/integration trace from physical input through
   `UiAction`/intent, state reducer, typed command, packet and receipt.
4. Negative traces for disabled, duplicate, unauthenticated, stale, wrong
   overlay, wrong grid, resize/DPI and focus-loss cases.
5. Web regression evidence for shared Gateway/Simulation changes.
6. Real Windows screenshots and interaction evidence for visual/feel rows;
   automated source or unit checks cannot substitute for this gate.

Until those artifacts exist, the correct disposition is `INFERENCE`,
`IMPLEMENTATION_GAP`, or `BLOCKED_EXTERNAL`; do not convert it to Accepted by
counting existing native registry rows.
