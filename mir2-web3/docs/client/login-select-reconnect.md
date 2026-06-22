# 登录 / 选角 / 重连 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

The client's session lifecycle: the three-screen state machine (`login` → `select` → `game`),
the single WebSocket to the gateway (`connectGateway`), and the **auto-reconnect-resume** flow that
silently re-authenticates + re-enters the world after a dropped socket. It also owns the auth
surface — classic account/password, on-chain **passkey**, and **wallet** logins — and the helper
sequences that fire the right ordered `clientVersion`/`login`/`startGame` commands for each path.

登录态机 + 唯一一条到 gateway 的 WebSocket + 掉线后静默重连复活。三种登录方式（密码 / passkey /
钱包），每种用一组有序命令序列进入世界。

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/page.tsx` | screen state machine (`login`/`select`/`game`) | `const [screen, setScreen]` :1428 · `screenRef` :1338 (mirror :1821) · `ClientScreen` = `lib/original-ui.ts:12` |
| `apps/web/app/page.tsx` | socket lifecycle | `connectGateway(bootstrapAfterOpen)` :4518 (`open` :4535, `close` :4575, `error` :4595, `message`→`handleGatewayEvent` :4603) · `send(command,{quiet})` :4026 |
| `apps/web/app/page.tsx` | the login/account actions | `createAccount` :4618 · `submitLogin` :4632 · `submitSuiLogin` :4652 · `submitPasskeyLogin` :4680 · `submitWalletLogin` :4689 · `startSelectedCharacter` :4694 · `quickEnterWorld` :4702 · `resetClient` :4717 |
| `apps/web/app/page.tsx` | reconnect machine | `sendGatewayReconnectSequence` :4436 · `scheduleGatewayReconnect` :4469 · `completeGatewayReconnect` :4449 · `failGatewayReconnect` :4460 · `captureGatewayReconnectSnapshot` :4405 · `resetGatewayReconnectState` :4398 |
| `apps/web/app/page.tsx` | inbound session packets | `switch (event.packet)` :6422 — `Connected` :6423, `ClientVersion` :6426, `Disconnect` :6431, `NewAccount` :6461, `Login` (fail) :6469, `LoginBanned` :6479, `LoginSuccess` :6532, `StartGame` :6557, `UserInformation` :6614, `LogOutSuccess` :7127, `StartGameBanned` :7156, `ReturnToLogin` :8245 |
| `apps/web/lib/client-login-runtime.ts` | the ordered command sequences (the only place that *composes* login commands) | `sendBootstrapSequence` :32 · `sendPasswordLoginCommand` :42 · `sendNewAccountCommand` :52 · `sendSuiLoginCommand` :70 · `requestSuiLoginToken` :79 |
| `apps/web/lib/passkey-auth.ts` | passkey/wallet token mint + wallet-standard discovery (the **definitions** re-exported by client-login-runtime) | `requestPasskeyLoginToken` :87 · `requestWalletLoginToken` :124 · `getSuiWalletSummaries` :147 · `getActiveSuiWalletSession` :49 · `connectSuiWalletForSigning` :58 |
| `apps/web/app/original-client-shell.tsx` | renders login/select screens, calls back via `on*` props | wired at `<OriginalClientShell …>` page.tsx :11107 (`onSubmitLogin` :11165, `onPasskeyLogin` :11166, `onQuickEnter` :11169, `onEnterWorld` :11236, …) |
| `apps/gateway/src/web.rs` | outbound BrowserCommand→ClientPacket + the **server-side** reconnect-grace store | `browser_command_to_action` :2570 (`Login` :2577, `PasskeyLogin` :2584, `NewAccount` :2587, `StartGame` :2648) · `ReconnectSessionStore` :304 · `reconnect_grace_ttl_seconds` :2129 · grace-restore on StartGame :1750 |

## 数据流 (How it threads the layers)

**Outbound — a password login (the canonical path):**

1. UI button → `onSubmitLogin` prop → `submitLogin()` (page.tsx:4632). It records
   `activeReconnectAuthRef = {kind:"password",accountId,password}` (this is what makes reconnect
   work — see gotchas), sets `loginBusy`, then either sends immediately (socket OPEN) or sets
   `pendingLoginRef=true` and calls `connectGateway()`.
2. On socket `open` (page.tsx:4535) the pending flag fires `sendPasswordLoginCommand(send, …)`
   (`client-login-runtime.ts:42`) → emits `{type:"clientVersion"}` then `{type:"login",accountId,password}`.
3. `send()` (page.tsx:4026) `socket.send(JSON.stringify(command))`.
4. Gateway `browser_command_to_action` (web.rs:2570) maps `BrowserCommand::Login` → `ClientPacket::Login`
   (web.rs:2577); `clientVersion`→`ClientPacket::ClientVersion`.
5. Sim authenticates → emits `ServerPacket::LoginSuccess { characters }`.
6. Gateway `server_packet_to_event` → JSON `{type:"packet",packet:"LoginSuccess",payload:{characters:[…]}}`.
7. page.tsx `case "LoginSuccess"` (:6532): `parseCharacters(payload,…)` → `setCharacters`, then (no reconnect)
   `setScreen("select")`.
8. Player picks a slot + `onEnterWorld`→`startSelectedCharacter()` (:4694) → `send({type:"startGame",characterIndex})`
   → `BrowserCommand::StartGame` → `ClientPacket::StartGame`.
9. Sim emits the world bootstrap; the decisive packet is **`UserInformation`** (page.tsx:6614) — *that* arm
   does `setScreen("game")` + `completeGatewayReconnect()`. `case "StartGame"` (:6557) only **error-checks**
   `result !== 4`; it does **not** flip the screen.

**Inbound — server-driven kicks:** `Disconnect` (:6431), `LogOutSuccess` (:7127), `ReturnToLogin`
(:8245) each set `screenRef.current="login"` + `setScreen("login")`; `LogOutSuccess` keeps
`connected:true` (you stay on the socket, back at the character roster). `Login`/`LoginBanned`
(:6469/:6479) = auth failure → back to `login` with `loginErrorKey`.

## 状态形状 (State shape)

**Screen + auth (React state, page.tsx ~1428-1479):**
- `screen: ClientScreen` (`"login"|"select"|"game"`) — mirrored synchronously into `screenRef` (:1338, effect :1821) because packet handlers run before React flushes and read `screenRef.current`.
- `accountId`/`password: string` (default `"demo"`), `loginBusy: boolean`, `loginErrorKey: string|null`
  (rendered as `loginError={loginErrorKey ? t(loginErrorKey) : null}`).
- `characters: SelectCharacterEntry[]` (type :729; `{index,name,level,classKey,gender,lastAccess,synthetic?}`),
  `selectedCharacterIndex: number`.
- `wsState: "closed"|"connecting"|"open"`, `reconnectStatus: ReconnectStatus`.
- `suiWallets: SuiWalletSummary[]`, `walletPickerOpen: boolean`.

**Refs (the truth packet handlers read; React state lags):**
- `socketRef` — the live `WebSocket`. `send()` no-ops unless `readyState===OPEN`.
- `accountIdRef`/`passwordRef` (:1413-1414, **default `"demo"`**), `charactersRef`, `selectedCharacterIndexRef`
  — all mirrored from state by the effect at page.tsx:1655-1660.
- `activeReconnectAuthRef: ReconnectAuthSnapshot|null` (type :776) — the credentials a successful login captured;
  `password` variant carries the plaintext password, `sui` variant carries `{token,expiresAt}`.
- `reconnectSnapshotRef: ReconnectSnapshot|null` (type :789) = `{auth,characterIndex,characterName}` — the in-flight resume plan.
- `reconnectStatusRef: ReconnectStatus` (type :770) = `{mode: "idle"|"scheduled"|"connecting"|"resuming"|"failed", attempt, nextAttemptAt}`; `reconnectAttemptRef: number`; `reconnectTimerRef` (the `setTimeout` handle).
- `pendingLoginRef`/`pendingNewAccountRef`/`pendingSuiLoginRef` (:1339-1341) — "fire this once the socket opens" latches.
- `manualSocketCloseRef` — true when *we* closed the socket (logout/reset), suppresses auto-reconnect on `close`.

**Constants:** `RECONNECT_DELAYS_MS = [1000,2000,4000,8000,12000]` (:802), `MAX_RECONNECT_ATTEMPTS = 6` (:803),
`createIdleReconnectStatus()` (:814). Gateway grace TTL: `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` default **15s**, clamp 1–120 (web.rs:2129).

## 坑 & 不变量 (Invariants & gotchas)

- **`UserInformation`, not `StartGame`, enters the world.** `case "StartGame"` (:6557) only checks
  `result===4` for failure; the screen flips to `game` and reconnect completes inside `case "UserInformation"`
  (:6663-6665). If you add a new "entered world" side-effect, hook it to `UserInformation`, not `StartGame`.

- **Why a *raw* `send({type:"login",…})` breaks reconnect (the "refs at `demo`" trap).** Reconnect replays
  credentials from `activeReconnectAuthRef`. Only `submitLogin`/`submitSuiLogin`/`quickEnterWorld` set that ref.
  If you log in by calling `send(...)` directly (e.g. a QA/console harness, or the bootstrap path that *doesn't*
  set it), `activeReconnectAuthRef` stays `null`. On the next drop, `captureGatewayReconnectSnapshot` (:4405)
  falls back to `accountIdRef`/`passwordRef`, which **default to `"demo"/"demo"`** (:1413-1414) unless the login
  form was actually typed into — so the gateway tries to resume the **wrong (demo) account**. Always log in
  through `submitLogin`/`submitSuiLogin`, or set `activeReconnectAuthRef.current` yourself. (`sendBootstrapSequence`
  in `client-login-runtime.ts:32` is fine because `quickEnterWorld` :4702 sets the auth ref before calling it.)

- **Two reconnect graces, on different sides, that can collide.** (a) **Client** waits `RECONNECT_DELAYS_MS`
  before retrying. (b) **Gateway** keeps the *live sim session* warm in `ReconnectSessionStore` for
  `reconnect_grace_ttl_seconds()` (default 15s, web.rs:2129) so a returning `StartGame` with the **same
  `{account_id, character_index}`** key (web.rs:1750-1768) reclaims the in-progress character instead of a cold
  re-login. If you re-login *too fast* the grace lease may still hold the session; if too slow (>15s) it's purged
  and you get a fresh bootstrap. This is the documented "reconnect-grace ~15s collides with fast re-login" hazard
  in the persistence-QA notes.

- **The resume replays the WHOLE handshake quietly.** `sendGatewayReconnectSequence` (:4436) re-sends
  `clientVersion`+`login`(or `passkeyLogin`)+`startGame` with `{quiet:true}` (no chat-log spam). It runs from the
  socket `open` handler only when `reconnectSnapshotRef && reconnectAttemptRef>0` (:4542). Order matters — the
  pending-login / pending-sui / pending-newaccount / bootstrap branches are mutually exclusive `return`s in `open`
  (:4542-4572); a reconnect snapshot wins over all of them.

- **`sui` reconnect tokens expire.** `captureGatewayReconnectSnapshot` refuses to build a snapshot if a `sui`
  token is within 5s of expiry (:4419) → a passkey/wallet session that drops near token expiry will `failGatewayReconnect`
  rather than resume with a dead token. The wallet itself is retained module-locally in `activeWalletSession`
  (`passkey-auth.ts:46`) only for on-chain signing, **not** for re-login.

- **`close` only auto-reconnects when it wasn't us + not already failed.** Guard at :4590:
  `isCurrentSocket && !closedManually && reconnectStatusRef.current.mode !== "failed"`. `resetClient`/logout set
  `manualSocketCloseRef` so a deliberate close stays closed. Cap is `MAX_RECONNECT_ATTEMPTS` (:4480) → `failGatewayReconnect`.

- **`screenRef` must be written *before* `setScreen`.** Every screen-changing arm sets `screenRef.current="…"`
  immediately then `setScreen("…")` (e.g. :6663-6664, :7130-7131). Don't call only `setScreen` — the next packet in
  the same microtask reads the stale `screenRef` and `captureGatewayReconnectSnapshot`'s `screenRef.current!=="game"`
  guard (:4406) would misfire.

- **Phantom character slot.** A fresh account's `LoginSuccess` can carry `characters:[]`; `parseCharacters`
  (:12599) then synthesizes a `fallbackCharacter` (`synthetic:true`, `index 0`, :12646). `NewCharacterSuccess`
  drops synthetic entries before appending the real one (:6501). `isFallbackCharacter` (:12659) /
  `!isFallbackCharacter` is how reconnect picks a real slot. Don't `startGame` a synthetic slot.

- **`gatewayWs` query override is localhost-only by design.** `resolveGatewayWebSocketUrl` (:988) honors
  `?gatewayWs=` only when the host is local (:995) — a hosted origin ignores it so a crafted link can't redirect a
  player's socket (and credentials) to an attacker gateway. QA against a worktree gateway via `?gatewayWs=` works
  *only* on `localhost`.

- **Passkey `rp.id` is the hostname.** `requestPasskeyLoginToken` (`passkey-auth.ts:87`) sets `rp.id = window.location.hostname`;
  WebAuthn needs `localhost`, not `127.0.0.1`. Wallet discovery re-dispatches `wallet-standard:app-ready` on every scan
  (`rescanWalletStandard`, :205) to catch extensions that inject late — without it Slush/Suiet stay invisible.

## 如何扩展 (How to extend / add to this area)

**Add a new auth method (e.g. a new SSO):**
1. `apps/web/lib/passkey-auth.ts` — add a `requestXxxLoginToken(): Promise<SuiLoginToken>` that returns
   `{accountId, token, expiresAt}` (mint via `/api/...`). Re-export through `client-login-runtime.ts` if other
   modules need it.
2. `apps/web/lib/client-login-runtime.ts` — if the wire sequence differs, add a `sendXxxLoginCommand(send,…)`;
   otherwise reuse `sendSuiLoginCommand`. Extend `SuiLoginKind` + `requestSuiLoginToken` (:79) if it routes through the picker.
3. `apps/web/app/page.tsx` — add `submitXxxLogin()` modeled on `submitSuiLogin` (:4652): **set `activeReconnectAuthRef`**,
   set `loginBusy`, handle the socket-not-open case with a `pendingXxxRef` latch, and consume that latch in the `open`
   handler (:4556-4571). Wire the new `pendingXxxRef` into the `close` reset (:4581-4583) and `resetClient` (:4726-4728).
4. Add the `ReconnectAuthSnapshot` variant (:776) if the resume needs different fields, and a branch in
   `sendGatewayReconnectSequence` (:4436).
5. `apps/gateway/src/web.rs` — `enum BrowserCommand` (:585) + an arm in `browser_command_to_action` (:2570) →
   `SessionAction`/`ClientPacket`. Mirror the `PasskeyLogin` pattern (:2584) for token auth.
6. UI: pass a new `on*` prop into `<OriginalClientShell>` (page.tsx :11160-11169) and render the button in
   `apps/web/app/original-client-shell.tsx` (presentation only — never `send` from the component).

**Tune reconnect behavior:** edit `RECONNECT_DELAYS_MS`/`MAX_RECONNECT_ATTEMPTS` (page.tsx :802-803) for the
client backoff; set `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` for the server warm-session window (web.rs:2129).
Keep client backoff and the server grace in a sane relation (don't let the first client retry land *after* the grace expires unless you intend a cold re-login).

## 相关 (Related)

- [`page-tsx-map.md`](./page-tsx-map.md) — the block map for the ~12.7k-line `page.tsx`.
- [`protocol-cross-layer.md`](./protocol-cross-layer.md) — the 5-layer wiring + add-a-packet recipe (both directions).
- [`onchain-mine.md`](./onchain-mine.md) — the Sui wallet session retained by wallet login (`getActiveSuiWalletSession`).
- [`world-scene-render.md`](./world-scene-render.md) — what `UserInformation`/`MapInformation` bootstrap into on entering `game`.
- Source: `apps/web/app/page.tsx` (login/connect/reconnect block ~4026-4798 + session packet arms ~6422-8249),
  `apps/web/lib/client-login-runtime.ts`, `apps/web/lib/passkey-auth.ts`, `apps/web/lib/load-starter-scene.ts`
  (server-side starter scene blueprint), `apps/gateway/src/web.rs` (`browser_command_to_action` :2570, `ReconnectSessionStore` :304).
