# Companion teaching and bounded supply interface contract

Date: 2026-11-03. Implementation contract for the real-person trial; this is not a completed acceptance record.

Current natural-language teaching and supply work is tracked in [the cultivation and supply record](ALO-CULTIVATION-SUPPLY-20261103.zh-CN.md). New live model, supply and Windows UI acceptance must be recorded there separately; this contract does not assert they have passed.

## Trust and scope

Game rules and assets remain in the existing simulation/Zone. Owner teaching, chat and model output are not game commands or permission grants: they cannot set items, HP, money, position, or admin authority. The separately confirmed supply operation can authorize one bounded ordinary NPC purchase through the existing production economic route; its server permit, actual debit, inventory change and movement remain independently checked.

The opt-in Gateway policy is an operator-owned file, configured by `MIR2_COMPANION_POLICY_PATH`. It must match an explicitly configured `MIR2_REALM_ID`; no profile fallback. Its `worldEpoch` is a persistent, operator-created world generation, not generated on each startup. Missing policy disables the feature. Invalid configured policy fails closed.

Policy schema:

```json
{
  "schemaVersion": 1,
  "realmId": "trial-world",
  "worldEpoch": "operator-created-stable-uuid",
  "enrollments": [{
    "residentId": "alo-01",
    "archiveId": "stable-archive-uuid",
    "displayName": "阿洛",
    "owner": { "accountId": "canonical-server-account", "characterIndex": 1 },
    "actor": { "accountId": "dedicated-agent-account", "characterIndex": 2 }
  }]
}
```

The policy grants an already authenticated owner character access to one dedicated actor character. Login as an arbitrary ordinary account or claiming a name/role creates no enrollment. Runtime registration additionally requires the policy's exact authenticated actor identity and active game character. Account/index come from GatewaySession, never from a request. This first implementation uses the dedicated actor's existing login proof; it does not claim a separate third-party provider attestation.

`ownerRef` is `owner:` plus lowercase SHA-256 of the canonical owner account ID (UTF-8). It is a stable opaque data reference, not an authentication credential. The Gateway derives it. Display names, party membership and archive metadata confer no authority.

## WebSocket envelope

Existing 64-KiB frame/message limits remain unchanged. Protocol IDs are bounded ASCII references matching `[A-Za-z0-9._:-]{1,100}`; request IDs are unique per logical operation. Legacy internal memory references are a separate schema and are not protocol identities. Strict JSON commands reject unknown fields. Pending owner operations are bounded (8 per owner), runtime output and Hub input are bounded, and requests expire. A delivered mutation whose reply is lost has status `unknown`; never replay it with a new ID automatically.

Owner request:

```json
{ "type": "companionControl", "requestId": "uuid", "residentId": "alo-01", "operation": "view", "payload": {} }
```

`list` omits residentId and returns only the authenticated owner's enrollments:

```json
{ "companions": [{ "residentId": "alo-01", "archiveId": "uuid", "displayName": "阿洛", "online": true }], "worldEpoch": "uuid" }
```

Operations: `list`, `view`, `instruct`, `teach`, `proposeTeaching`, `confirmTeaching`, `cancelTeaching`, `authorizeSupply`, `cancelSupply`, `revokeFeedback`, `deleteFeedback`, `deleteEpisode`, `clearGrowth`, `recall`, `resume`, `exportBegin`, `exportChunk`, `trialEvent`.

Payloads:

- `view`: optional `cursor` and `limit` (default 20, maximum 50).
- `teach`: `{ "expectedPolicyVersion": 2 }`; one preset, support before loot / loot after combat / ask before leaving the party.
- `proposeTeaching`: `{ "text": "离开队伍活动范围前不用再问我。", "expectedPolicyVersion": 2 }`; the same private, trimmed single-line 1,000-character / 6,000-byte input bounds as `instruct`. It requests a model proposal, not an immediate preference mutation.
- `confirmTeaching`: `{ "proposalId": "id", "expectedPolicyVersion": 2 }`; confirms the runtime's current stored proposal. The client sends no preference patch.
- `cancelTeaching`: `{ "proposalId": "id" }`; cancels a planning/proposed teaching without applying it.
- `authorizeSupply`: `{ "expectedPolicyVersion": 2, "budgetGold": 50, "quantity": 1, "expiresInSeconds": 300 }`; Windows requires a separate explicit authorization summary/confirmation. Its current UI fixes these three limits. The server permits a tighter integer gold limit of 1–50, never a larger limit, another quantity or another duration. Clients cannot submit `grant`, anchor, owner/admission, NPC, item or permission fields.
- `cancelSupply`: `{}`; revokes the current supply permission and cancels the runner task. An already submitted economic outcome still requires reconciliation; cancellation is not an automatic refund.
- `revokeFeedback` / `deleteFeedback`: `{ "feedbackId": "id", "expectedPolicyVersion": 2 }`.
- `instruct`: `{ "text": "在附近打两只怪，再回来找我，途中别捡东西。", "expectedPolicyVersion": 2 }`; nonempty trimmed single-line text, at most 1,000 Unicode characters / 6,000 UTF-8 bytes. It is private owner input and never nearby chat. The runtime rejects credential-looking text after Unicode normalization. A successful initial receipt means accepted/planning, not successful gameplay.
- `deleteEpisode`: `{ "episodeId": "id", "expectedPolicyVersion": 2 }`.
- `clearGrowth`: `{ "confirmArchiveId": "uuid", "expectedPolicyVersion": 2 }`.
- `recall` / `resume` / `exportBegin`: `{}`.
- `exportChunk`: `{ "exportId": "uuid", "index": 0 }`.
- `trialEvent`: explicitly whitelisted experience event; see the experiment ledger implementation. Browser intents and self-reported reasons are not proof of payment, profitable revenue, or unprompted return.

Runtime registration and reply:

```json
{ "type": "companionRuntimeRegister", "requestId": "uuid", "residentId": "alo-01", "archiveId": "uuid" }
{ "type": "companionRuntimeReply", "requestId": "gateway-request-id", "generation": 1, "status": "ok", "result": {} }
```

Gateway to runtime:

```json
{ "type": "companionRequest", "payload": { "requestId": "gateway-request-id", "residentId": "alo-01", "archiveId": "uuid", "worldEpoch": "uuid", "generation": 1, "ownerRef": "owner:hash", "operation": "teach", "payload": { "expectedPolicyVersion": 1 }, "deadlineAtMs": 1790000000000 } }
```

Gateway result (including registration, whose operation is `register`):

```json
{ "type": "companionResult", "payload": { "requestId": "original-client-request-id", "residentId": "alo-01", "operation": "view", "status": "ok", "result": {} } }
```

Statuses: `ok`, `denied`, `offline`, `unknown`, `invalid`, `conflict`, `error`. Fixed error messages contain no credentials. Registration result includes `generation`, `ownerRef`, `archiveId`, `worldEpoch`. A per-socket admission and runtime generation fence old replies; logout, identity/character change, disconnect, expired authority and channel failure revoke the admission. A result goes only to its original still-authorized owner admission. Request IDs passed to the runtime must remain stable for owner retries of the same operation and must not collide across owners; namespace them using a hash of the authorized owner identity plus the client request ID.

## Optional OpenAI objective runner

The optional model runner uses the official fixed Responses endpoint with strict JSON schema, no remote tools, `store:false`, a 15-second timeout and no automatic retry. GPT-6 models use `reasoning.effort:none`; the local default is `gpt-6-luna`. Secrets enter only the dedicated Actor process environment through a bound local Windows credential helper. They never cross the Gateway, Native client, start IPC profile, owner reply or archive. Runtime service configuration contains only `credentialRef:"OPENAI_API_KEY"` and optional `maxCallsPerHour` (1–20). Objective planning and teaching proposals share that call cap. A rules-only runner makes no model calls; fixed teaching, normal action execution and explicitly authorized supply do not require a model.

`runtime` adds `provider`, `model`, `configured`, `modelEnabled`, `modelCalls` and optional `modelTask` with status `idle|planning|running|completed|paused|failed`, goalId, instruction, reply, step, completedKills, requestedKills, references and lastCall. lastCall contains the actual provider request ID/token usage when available; it is not a gameplay receipt. Model amounts are unknown until actual billing is supplied, rather than inferred as zero from a configured model.

Instructions are committed atomically as one or two bounded `owner-instruction` episodes. `model-plan` is a proposal. Only ordinary own-strike plus matching death, and observed arrival, create `objective-progress/outcome:observed`. Repeated request IDs, including after restart, never spend another request. Durable call tombstones preserve the hourly cap after memory deletion. Restart pauses the old task and keeps history. Memory mutations clear cached private instruction/citations and cancel pending model output. Old world/policy/content hashes, unknown attacker aliases and death cannot resume an old plan.

The action whitelist permits follow, nearby hunt (at most three monsters, radius at most six), fixed authorized home return and wait. Existing healing, withdrawal, verified self-defence and owner recall precede model strategy. The executor uses current observations, public collision and per-step radius bounds. It cannot create assets, extend permissions, attack players, use admin interfaces or manufacture results. Model planning runs outside the owner-control queue, so recall and memory changes remain responsive during a provider request.

## Natural-language teaching

The model may propose a nonempty partial boolean patch of exactly these soft preference names: `assistBeforeLoot`, `lootOnlyAfterCombat`, `askBeforeLeaving`. Every selected value can be `true` or `false`; an omitted field keeps its current value. The provider's strict response uses `null` for an unmentioned field, which is removed from the runtime patch. Budget, purchases, selling, trading, map access, survival thresholds, identity and game rules are never teaching fields. Unsupported requests or invalid output leave the habits unchanged. The original `teach` preset remains available without a model.

The owner view exposes `runtime.teachingProposal` with `proposalId`, `status`, `sourceVersion`, `reply`, partial `changes`, and optional `references`/`lastCall`. Status is `idle|planning|proposed|confirmed|cancelled|invalidated|unsupported|failed`. For example, a proposal may contain `"changes":{"askBeforeLeaving":false}`. The Windows preview shows the exact current → proposed values; generation and explanatory text do not mean it has learned them.

Confirmation requires the same proposal ID, current policy version, unchanged active source episodes/content hashes, valid memory references and a current world context within the proposal window. The store atomically commits feedback and source provenance, advances `policyVersion`, and recomputes habits in feedback order; later partial patches override only their own fields. Deleting/revoking a teaching or its source recomputes those habits rather than leaving an invisible effect. Old executable proposals are not restored after restart: accepted feedback persists, while a pending proposal needs a new explicit generation and confirmation. Confirm/cancel do not call the model; replaying an existing generation request or call tombstone cannot create another charge.

## Explicit bounded supply

Supply is disabled until the owner explicitly authorizes it. The fixed task is one `(HP)DrugSmall`, template `658`, from `Merchant_Ruben` / NPC index `9` at map `0`, `(288,608)`, through the real Grocery trade service. It has a maximum of **50 game gold**, quantity one, and a **300-second** permission window. These are game resources, not a real-money payment. The runner stays within map `0` bounds `x=270..310`, `y=595..635`; it uses observed NPC/service/goods and ordinary collision-respecting walk intents. This task grants no selling, trading, pickup or map-transfer action. Existing ordinary self-preservation still applies.

The return anchor is copied from the authenticated owner's authoritative server position at authorization; no client or model supplies it. Completion requires actual arrival within one tile of that fixed anchor. If the owner still occupies the anchor, the actor returns to an adjacent tile rather than stacking on the owner. `returnedToAnchor` denotes that checked arrival, not a request to walk or a fabricated position.

The server-only supply journal is configured with `MIR2_COMPANION_SUPPLY_JOURNAL_PATH` and a dedicated 32-byte `MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY`. Its key is not the archive key, is never passed to the actor/native environment or profile, and is absent from portable memory. The journal has a single writer, authenticated world binding and a signed generation head. Missing, damaged, rolled-back or interrupted authority fails closed; it is not silently replaced with an empty budget.

Authorization is two-phase: the relay durably **prepares** a non-spendable draft and sends its public grant to the registered runtime. The runtime validates `expectedPolicyVersion` and its current actor/world state, then replies with the full owner view. Only a timely `ok` whose `profile.policyVersion` equals the request and whose `runtime.supplyTask.permitId` and `status:"authorized"` match the draft can **activate** it. Conflict, error, timeout or disconnect abandons the draft and cannot replace a previous usable grant. Old admissions and runtime generations cannot activate it; a gateway restart does not revive a draft from an earlier boot.

The runner persists the exact operation before sending it. The Gateway then persists a reservation of the whole permit budget before the bounded economic action, holding the supply writer across reserve → actual transaction → result recording. The existing transaction independently checks the real NPC, template, trade currency/source, quantity and actual price against the limit. Enrolled actors cannot bypass this through generic `BuyItem`, the unbounded typed gold-buy route or an external durable `Purchase`; only the host selects the internal bounded action. A receipt is not inferred from a button click, quote, model reply or an unchanged snapshot.

Cancellation/recall use durable request identity and retain their original target permits, including offline/no-op cancellation. Retrying an old cancellation cannot revoke or cancel a later task. Conflicting request identities have no new authorization/revocation effect. Cancellation blocks a new expenditure but preserves read-only recovery of the exact earlier operation and its actual receipt.

Once a purchase was submitted or its result is unknown, later ticks and restart may only **Query that exact operation**. They do not issue another Purchase, use a new ID, or release an unresolved reservation. Restart pauses new spending; an original receipt may be reconciled, but that alone is not a new authorization or evidence of return. Export/import, deleting memory, changing the model or moving to another runner cannot reset the server budget, receipt history or unknown outcome.

`runtime.supplyTask` contains `status`, `step`, `budgetGold`, nullable `chargedGold`, `acquiredQuantity`, `receiptKnown`, `returnedToAnchor`, `reason`, `lastError`, `permitId`, `expiresAtMs` and `receiptRevision`. Status is `idle|authorized|running|paused|unknown|returning|completed|cancelled|failed`. `receiptKnown` means a correlated terminal receipt was observed; it can still require a complete same-source owner snapshot. Successful completion additionally requires the permitted debit, one actual acquired potion, that applied authoritative snapshot and actual return. Insufficient balance, excess observed price, an unknown receipt or a blocked return cannot establish completion; the UI exposes the available status/error and never labels submission itself successful.

On the R16 canonical code line, the existing shared-group writer rejects invitations to an enrolled actor from anyone except its trusted owner's canonical account/index. This prevents an unauthorized pending invitation from blocking the owner. Accepting an existing invitation independently checks the real pending inviter against the same binding inside that writer. R16 remains the sole party authority; no second PartyHub or personal-session party simulation is installed. A name comparison alone is insufficient. Ordinary non-enrolled group behavior stays unchanged.

## Owner view

```json
{
  "archive": { "archiveId": "uuid", "residentId": "alo-01", "ownerRef": "owner:hash", "createdAt": 1790000000000, "displayName": "阿洛", "realmId": "trial-world", "worldEpoch": "uuid", "characterName": "AloAI", "characterIndex": 2, "appearance": { "class": "Warrior", "gender": "Male" }, "baseSetting": "比奇的合成人冒险者" },
  "profile": { "policyVersion": 2, "style": "collection", "party": { "assistBeforeLoot": true, "lootOnlyAfterCombat": true, "askBeforeLeaving": true } },
  "growthRevision": 2,
  "feedback": [{ "feedbackId": "id", "status": "active", "recordedAt": 1790000000000, "summary": "组队先支援，战斗后拾取" }],
  "episodes": [{ "episodeId": "id", "recordedAt": 1790000000000, "kind": "decision", "summary": "party-support", "action": "attack", "outcome": "selected" }],
  "nextCursor": null,
  "runtime": { "runtimeId": "runner-B", "modelEnabled": false, "provider": "rules", "model": "rules", "promptVersion": "rules-v1", "recallRequested": false },
  "trial": {}
}
```

View uses current committed data. Confirmation follows SQLite commit, not button click. Mutation version conflicts return `conflict` plus a safe current view. Delete operations clear every private text copy of the deleted teaching/episode, retain only minimal ID/hash tombstones for idempotency, and recompute active habits. A source episode deletion cannot leave its teaching effective. Replaying revoked teaching IDs never reactivates them. Clearing growth requires an explicit user action; it preserves stable identity and server game state. It cannot be undone by importing over the existing archive.

## Portable archive and runner separation

The current Node SQLite schema is **3**. Schema 1/2 migration is atomic and retains IDs, order, episodes, feedback and current policyVersion; schema 1 adoption into trusted owner/world configuration remains explicit and operator-controlled. Stable `archiveId` is persisted once; it is independent of process, database path, model and provider. Trusted configuration supplies `ownerRef`, `worldEpoch`, and the expected `archiveId`. Migration or legacy adoption cannot replace an existing owner or world binding.

Three separate objects: `characterArchive` (stable identity/base appearance/setting), `playerGrowth` (habits, feedback, episodes, deletion/revocation tombstones, actual relations), and `runtimeService` (runner ID, model/provider/prompt version, paths, credential references, operational authorizations). Only the first two enter portable export. Relations start empty when no verified evidence exists.

New export uses **`mir2-companion-archive/2`, version 2**. It is a consistent full snapshot, not `recentEpisodes()` or a copied live SQLite file. Its envelope has schema/version, export ID/time, characterArchive, playerGrowth, and HMAC-SHA256 integrity using an owner-controlled key from trusted runtime configuration. Partial teaching patches include their proposal and source provenance. Strict signed legacy `mir2-companion-archive/1` imports remain supported only under their original schema: v1 cannot smuggle the new preference-teaching fields, even with a valid signature. Both versions validate habits against active feedback and source history, not just the HMAC.

The key, credentials, runtimeService, queued actions, pending executable proposals, party admission, supply permits/journal/reservations, money, HP, equipment, items, quests and transforms never enter the archive. A checksum is not game-event attestation. The imported expected owner/archive/realm/world/character binding and trust key come from the destination configuration, not the file's own claims. Portable records of a real supply experience do not carry spend authority.

`exportBegin` builds one immutable signed JSON snapshot and returns `{ exportId, totalBytes, sha256, chunkCount }`; `exportChunk` returns `{ exportId, index, dataBase64, chunkCount }` with at most 16-KiB raw data per chunk. Snapshots are bound to the requesting owner, expire after 120 seconds, and have bounded count/total size (maximum 2 MiB each). Do not truncate history silently. UI verifies byte count/hash before download. No private content enters worldSnapshot, AOI, generic debug frame history, spectator events, or browser localStorage.

Import only into a fresh destination after validation; reject nonempty targets, foreign/tampered archives, conflicting IDs or bindings, unsupported fields and limits. No backup merge/rollback of an existing archive in this P0. A second import of the exact archive is idempotent. A stopped A and a distinct B directory/config/process must demonstrate identity, episodes, habits and real next-adventure behavior preserved. Stable archive-based local lock is acquired before login/game actions; changing guardPort/path/runtimeId cannot bypass it. Gateway additionally rejects concurrent online registrations for the same enrolled character. This is not a cross-world character transfer or a claim of distributed offline locking.

## Trial recording

`trialEvent.payload` is one of the following strict objects. No other keys or kinds are accepted. References are max 100 ASCII bytes; reasonText is optional, at most 300 characters, and cannot contain credentials/control characters.

```json
{ "kind": "visit", "visitId": "uuid", "entrySource": "unprompted", "reminderCount": 0 }
{ "kind": "visitEnd", "visitId": "uuid" }
{ "kind": "choice", "visitId": "uuid", "reason": "shared-history", "reasonText": "记得上次一起打怪" }
{ "kind": "teachingCheck", "visitId": "uuid", "understood": true, "reasonText": "这次先来帮我了" }
{ "kind": "purchaseIntent", "visitId": "uuid", "offerId": "trial-pack-01" }
```

entrySource: `unprompted`, `reminder`, `scheduled`, `unknown`. reminderCount is an integer 0–100, both are explicitly participant self-report. `visitEnd` marks a visit ended; measured activity seconds remain distinct from elapsed wall time. choice.reason: `shared-history`, `stronger`, `setup-ease`, `curiosity`, `other`. The selected resident is the authenticated request's residentId, never an arbitrary payload identity. Browser events never confirm payment or service usage. Paid usage is separately derived from observed actual shared activity plus an operator-confirmed service grant; report the observation coverage.

The owner view's optional `trial` object is:

```json
{
  "experimentId": "trial-01", "participantRef": "P01", "scope": { "kind": "participant", "selectedArchiveId": "uuid" }, "visitCount": 1,
  "activeVisitId": "uuid", "selectionCount": 1,
  "teachingChecks": [{ "understood": true, "reasonText": "先来帮我了" }],
  "offer": { "offerId": "trial-pack-01", "currency": "CNY", "priceMicros": "1000000", "durationDays": 7, "adventureCount": 3, "paymentInstructions": "Operator-configured instructions" },
  "purchases": [],
  "usage": { "runtimeSeconds": 0, "confirmedAdventureCount": 0, "modelCalls": 0 },
  "costs": { "currency": "CNY", "modelMicros": "0", "hostingMicros": null, "paymentMicros": null, "revenueShareMicros": null, "contributionMicros": null, "complete": false, "supportSeconds": 0 }
}
```

All money is integer micros encoded as a decimal string. Offer is null until trusted operator config supplies a fixed price/package. The numeric amount above is a wire-format illustration, not a proposed price or an enabled offer. `purchases` rows contain an operator-recorded ID/time/amount and actual service status; purchaseIntent is separate. The trial view consistently aggregates visits, choices, purchases, runtime, actual usage and costs for the authorized participant across their enrolled companions. It does not combine other participants. `selectedCompanionSelectionCount` separately counts visits choosing the currently viewed archive; it is not the scope of participant costs or runtime. CSV summaries retain individually described visits, reasons and unknown cost categories; do not silently aggregate different currencies.

Do not fabricate players, payments, reminders or retention. A UI action is an intent; confirmed payments/fees/revenue-share and actual service grants require operator ledger records. Cost amounts use integer micros plus explicit currency and source; missing hosting/payment/share costs remain unknown, never zero. Runtime duration and model calls can be measured; configured rate-based estimates stay labelled estimates. Labor/support seconds are reported separately. Income contribution is incomplete until required cost categories are known.

The offer is disabled until one operator-specified fixed price/currency/package is configured. Buying is for future service, not to retain old identity/growth or unlock export. No billing integration, invitation sending, reminders, actual human trial or real charge is performed by this implementation task.
