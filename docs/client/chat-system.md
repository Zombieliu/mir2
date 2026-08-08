# 聊天 / 系统消息 (Chat & system messages) — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

The single client-side **message log** that backs the in-game chat box: player chat
(normal / shout / whisper / group / guild / lover / mentor), server **system / hint /
announcement** lines, NPC dialog echoes, and the outbound chat-send path (a typed line
or `/Name …` whisper → a `chat` `BrowserCommand`). One flat `logs` state holds every
line; a `tone` + `channel` pair drives colour and visibility. The chat-settings window
(channel toggles, font size) is presentation-only and currently **not wired to filter
the feed** — the live filtering is a separate mechanism in the game UI scene.

The outbound classification (whether `Hello` becomes a shout/whisper/guild line) is done
by **prefix on the server** (Crystal `PlayerObject.Chat`); the client just prepends the
prefix (`!`, `/`, `!!`, `!~`, `:)`, `!#`) and sends the raw string.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/page.tsx` | log state + append + inbound chat/system cases + outbound send | `const [logs]` :1453 · `appendLog` :3986 · `createLogLine` :11731 · `defaultLogChannel` :11748 · `gatewayChatChannel` :11756 · `gatewayChatTone` :11795 · `whisperPlayer` :5723 |
| `apps/web/app/page.tsx` | inbound ServerPacket cases | `case "Chat"` :7024 · `case "ObjectChat"` :7031 · `case "NPCResponse"` :7877 · `case "SendOutputMessage"` :7898 · `case "Roll"` :7905 · `case "OpenBrowser"` :7916 · `case "UpdateNotice"` :8857 |
| `apps/web/app/page.tsx` | log types + outbound wiring | `type UiLogTone/UiLogChannel/UiLogLine` :183-203 · `onSendChat={(message)=>send({type:"chat",message})}` :11171 · `chatSettings={{open,onClose}}` :11257 |
| `apps/web/app/components/original-client-panels.tsx` | the chat box render + filter prefixes + visibility filter | `ChatFilterKey`/`ChatOptionFilterKey` :16-25 · `CHAT_FILTER_PREFIX` :95 · `formatChatMessageForFilter` :124 · `ChatFrame` :151 · `playerFacingChatLines` :531 · `matchesChatVisibility` :556 |
| `apps/web/app/components/original-client-game-ui-scene.tsx` | active-filter state + send glue | `activeChatFilter`/`hiddenChatFilters` :146-147 · `selectChatFilter` :165 · `sendActiveChatMessage` :180 |
| `apps/web/app/components/original-client-chat-settings-window.tsx` | channel-toggle / font / timestamp settings window | `ChatSettings` :27 · `CHANNEL_ROWS` :48 · `ChatSettingsWindow` :81 |
| `apps/gateway/src/web.rs` | outbound + inbound wire mapping | `BrowserCommand::Chat` :658 + arm :2662 · `ServerPacket::Chat` arm :4376 · `ObjectChat` :4384 · `SendOutputMessage` :5471 |
| `apps/simulation/src/runtime/packets.rs` | server-side chat handling | `handle_chat_packet` :4911 · `prepare_chat_packet` :4876 · `apply_crystal_chat_spam_guard` :4954 · `system_message_key` :129 · `hint_chat_key` :134 |
| `packages/protocol/src/types.rs` | the `ChatType` enum (0-16) | `enum ChatType` :91-109 |

## 数据流 (How it threads the layers)

**Inbound (server line → on-screen):**

```
sim emits ServerPacket::Chat{message,chat_type} | ObjectChat{object_id,text,chat_type}
  → gateway server_packet_to_event (web.rs:4376 / :4384) → JSON {packet:"Chat", payload:{message, chatType:"Normal"}}
        (chat_type is Debug-formatted: format!("{:?}", chat_type) → the STRING "Normal","Shout",…)
  → page.tsx switch case "Chat" :7024 / case "ObjectChat" :7031
        appendLog( payload.message|payload.text, gatewayChatTone(payload.chatType), gatewayChatChannel(payload.chatType) )
  → appendLog :3986 → createLogLine (prefix timestamp) → setLogs( [line, …prev].slice(0,24) )
  → panels.tsx ChatFrame reads `logs` → playerFacingChatLines :531 (filter + reverse) → <div class="channel-${channel}">
```

`gatewayChatChannel` (page.tsx:11756) maps the **string** `chatType` → a `UiLogChannel`
(`shout/shout2/shout3 → "shout"`, `whisperin/whisperout → "whisper"`, `group → "group"`,
`guild → "guild"`, `mentor → "mentor"`, `relationship → "relationship"`,
`system/system2 → "system"`, `hint → "hint"`, `announcement/levelup/linemessage →
"announcement"`, else `"normal"`). `gatewayChatTone` (:11795) collapses that to
`"system"` for `system|hint|announcement`, else `"chat"`.

**Outbound (typed line → sim):**

```
player types in ChatFrame input (panels.tsx) → onSendChat() = sendActiveChatMessage (game-ui-scene.tsx:180)
   formatChatMessageForFilter(activeFilter, text)   ← prepends CHAT_FILTER_PREFIX[filter] ('!','/','!!','!~',':)','!#')
  → page.tsx onSendChat={(message)=>send({type:"chat",message})} (page.tsx:11171)
  → send() :4026 → WS JSON {type:"chat", message}
  → gateway BrowserCommand::Chat{message} (web.rs:658) → browser_command_to_action arm :2662
        → SessionAction::Packet(ClientPacket::Chat{ message, linked_items: Vec::new() })
  → sim handle_chat_packet (packets.rs:4911): @LOGIN-password check → GM @-command dispatch → prepare → emit ObjectChat
```

The server, not the client, interprets the prefix — the web client never sets a
`chatType` outbound, it sends the raw prefixed string. **Two server-side chat handlers
exist, and they differ:** the active single-session path the gateway dispatches to,
`handle_chat_packet` (packets.rs:4911, via `ClientPacket::Chat` at packets.rs:7457), only
handles `@LOGIN`/`@`-commands/`@ADDSTORAGE` and otherwise emits a plain
`ObjectChat{ chat_type: ChatType::Normal }` — it does **not** classify `!`/`/`/`!!`
prefixes. The full prefix→channel classification (the real port of Crystal
`PlayerObject.Chat`) lives in the **zone-sharded** path `ZoneRuntime::chat`
(zone/runtime.rs:1484): `strip_prefix('/')→whisper`, `"!!"→group`, `"!~"→guild`,
`"!#"→mentor`, `"!"→shout`, `":)"→relationship`, `"@!"→announcement`, `'@'→dropped`. So
shout/whisper/group tone only renders when chat flows through the zone path.

## 状态形状 (State shape)

- `logs: UiLogLine[]` (page.tsx:1453) — the ONLY chat-log store. `UiLogLine = { text:
  string; tone: "chat"|"system"|"network"; channel: UiLogChannel }` (page.tsx:199).
  `text` is timestamp-prefixed by `createLogLine`. **Capped at 24 lines**, newest first
  (`appendLog` :3997 `.slice(0,24)`).
- `chatMessage: string` (page.tsx:1456) — the controlled chat input value. `whisperPlayer`
  (:5723) and account/select reset (:4768) write it; the chat box reads it via the
  `chatMessage` prop and `onChatMessageChange={setChatMessage}` (:11163). NOTE: this is
  the **login/select** input binding; in-game the same `chatMessage`/`onChatMessageChange`
  flow through `OriginalClientGameUiScene`.
- `showChatSettings: boolean` (page.tsx:1499) — toggled by hotkey `c` (page.tsx:1604) and
  the chat-bar settings button.
- `activeChatFilter: ChatFilterKey` + `hiddenChatFilters: ChatOptionFilterKey[]`
  (game-ui-scene.tsx:146-147) — **local React state inside the game UI scene**, not on
  `world`. `activeChatFilter` chooses the outbound prefix; `hiddenChatFilters` is the
  real per-channel show/hide list applied by `matchesChatVisibility`.
- `world.interactionHints: string[]` — passed to `ChatFrame` as `hints` (game-ui-scene.tsx:254),
  rendered alongside chat (interaction prompts, not chat lines).
- **No `world.*` / `world.stage5Systems.*` slice for chat.** Chat is pure local UI state.
  This is unusual for this codebase — most areas live on `world`; chat does not.

`channel` colour is applied by CSS class `channel-${channel}` (panels.tsx:213) in
`globals.css`; `tone === "system"` also adds a `system` class.

## 坑 & 不变量 (Invariants & gotchas)

- **`tone === "network"` lines are DROPPED.** `appendLog` early-returns for `"network"`
  (page.tsx:3991) and `playerFacingChatLines` filters them again (panels.tsx:533).
  Many call sites pass `"network"` (the `send`/`recv` packet trace, WS open/close); those
  never reach the chat box by design — they exist only for the debug log path.
- **Inbound `chatType` is a Debug STRING, not a number.** The gateway emits
  `format!("{:?}", chat_type)` → `"Normal"`,`"WhisperIn"`,… `gatewayChatChannel`
  lower-cases and matches those. If you add a `ChatType` variant in
  `packages/protocol/src/types.rs`, you MUST add its lower-cased name to the
  `gatewayChatChannel` switch (page.tsx:11761) or it silently degrades to `"normal"`.
- **`ChatType` enum has 17 values (0-16); the client only distinguishes ~9 channels.**
  `Trainer`(9) and `LevelUp`(10) have NO explicit case — `levelup` IS mapped (→
  `announcement`) but `trainer` is not, so a `Trainer` line falls through to `"normal"`.
- **The chat-settings window is cosmetically wired but does NOTHING to the feed.**
  page.tsx mounts it with only `{open, onClose}` (page.tsx:11257); `settings` and
  `onApply` are `undefined`, so `ChatSettingsWindow` (settings-window.tsx:81) keeps a
  purely-local `DEFAULT_SETTINGS` draft. Toggling a channel there does not hide lines.
  **The REAL in-game channel filter is `hiddenChatFilters` + `matchesChatVisibility`
  (panels.tsx:556), driven by a different UI** (the chat-option filter buttons in
  `ChatFrame`). Two separate "channel toggle" surfaces; only the latter works.
- **Two `ChatChannelKey` vocabularies.** The settings window uses `lover` (settings-window.tsx:20);
  the log/feed uses `relationship` (page.tsx:192). `matchesChatVisibility` bridges them
  (`case "relationship": return !hidden.has("lover")`, panels.tsx:570). Keep them aligned
  when adding a channel.
- **`shout` and `announcement` share one filter toggle** (`matchesChatVisibility` :561-563),
  so hiding "shout" also hides server announcements/level-up lines.
- **Two `trimLogTimestamp` definitions** — page.tsx:11744 and panels.tsx:552 (identical
  regex). Edit both or neither.
- **`SendOutputMessage` ignores `outputType`.** The gateway forwards `outputType`
  (web.rs:5479) but page.tsx (:7898) always logs it as `("system","server")` regardless —
  Crystal's red/blue/green output tinting is not reproduced.
- **Spam guard + chat-ban are SERVER-side** (`apply_crystal_chat_spam_guard`
  packets.rs:4954): too-fast chat → a 5-minute ban that the sim enforces and reports as a
  system line; the client has no rate limiter and shows whatever the server sends.
- **`@`-commands never echo.** Any line starting with `@` is consumed by the sim as a GM
  command attempt and is never shown as chat (packets.rs:4923-4932, citing Crystal
  `PlayerObject.Chat` GMLogin branch). `@LOGIN` arms a one-line password prompt; the very
  next chat line is swallowed as the password (packets.rs:4919). So a user typing a real
  message right after `@LOGIN` will see it vanish — expected, matches Crystal.
- **`@ADDSTORAGE`** is special-cased in `prepare_chat_packet` (packets.rs:4900) → storage
  rental, not chat.
- **Safe-zone note:** normal/shout chat is NOT safe-zone gated, but several adjacent
  surfaces are — guild chat/war and some social actions require the player be in a safe
  zone (`stage5_guild_player_in_safe_zone` packets.rs:2108). Chat lines themselves flow
  regardless of zone.

## 如何扩展 (How to extend / add to this area)

**Add a new inbound chat/system channel (e.g. a new `ChatType`):**
1. `packages/protocol/src/types.rs` — add the `ChatType` variant + its `TryFrom<u8>`
   number (keep existing numbers stable; append).
2. `apps/simulation/…` — emit `ServerPacket::Chat { chat_type: ChatType::New, … }` where
   the semantic fires (cite Crystal `file:line`).
3. `apps/gateway/src/web.rs` — no change needed for `Chat`/`ObjectChat` (they already
   `format!("{:?}", chat_type)`); the new variant name flows automatically.
4. `apps/web/app/page.tsx` `gatewayChatChannel` (:11761) — add `case "new":` →
   the `UiLogChannel` it should colour as. If it should ever read as system, also confirm
   `gatewayChatTone` (:11795).
5. `apps/web/app/page.tsx` — widen `type UiLogChannel` (:184) with the new channel string.
6. `apps/web/app/components/original-client-panels.tsx` — widen `DisplayLogLineLike.channel`
   (:30), add a `matchesChatVisibility` case (:558) and a `channel-<name>` CSS rule in
   `globals.css`.

**Add a new outbound chat shortcut (e.g. a new filter prefix):**
1. `apps/web/app/components/original-client-panels.tsx` — add to `ChatFilterKey` (:16),
   `CHAT_FILTER_PREFIX` (:95), and a `CHAT_FILTER_BUTTONS` entry (:85). `formatChatMessageForFilter`
   (:124) and `selectChatFilter` (game-ui-scene.tsx:165) pick it up automatically.
2. No protocol/gateway/sim change: the prefix is interpreted **server-side** by Crystal
   `PlayerObject.Chat`. The web only needs the prefix string to match Crystal's.

**Make the chat-settings window actually filter (close the cosmetic gap):**
1. `apps/web/app/page.tsx` — add a `chatSettings` React state (`ChatSettings`), pass
   `settings` + `onApply` in the `chatSettings={…}` prop (page.tsx:11257).
2. Reconcile `ChatSettings.channels` (the `lover`/`system` vocabulary) with
   `hiddenChatFilters` so the window drives `matchesChatVisibility` instead of the
   duplicate filter-button state — or remove one of the two toggle surfaces.

## 相关 (Related)

- [`protocol-cross-layer.md`](./protocol-cross-layer.md) — the 5-layer wiring + add-a-feature recipe.
- [`page-tsx-map.md`](./page-tsx-map.md) — locating handlers in the ~12.7k-line `page.tsx`.
- [`stage5-social.md`](./stage5-social.md) — group/guild/friends (guild chat & relationship lines originate there).
- [`shell-rendering.md`](./shell-rendering.md) — `OriginalClientShell` + `on*` callback surface (where `onSendChat` is threaded).
- Source: `apps/web/app/page.tsx` (`appendLog` :3986, `gatewayChatChannel` :11756) ·
  `apps/web/app/components/original-client-panels.tsx` (`ChatFrame` :151) ·
  `apps/web/app/components/original-client-chat-settings-window.tsx` ·
  `apps/gateway/src/web.rs` (:2662, :4376) · `apps/simulation/src/runtime/packets.rs` (`handle_chat_packet` :4911) ·
  `packages/protocol/src/types.rs` (`ChatType` :91).
