# Companion P0 interface contract

Date: 2026-11-03. Implementation contract for the real-person trial; this is not a completed acceptance record.

## Trust and scope

Game rules and assets remain in the existing simulation/Zone. This feature has no WorldCommand representation and cannot grant items, HP, money, movement, admin actions, or permissions.

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

Operations: `list`, `view`, `teach`, `revokeFeedback`, `deleteFeedback`, `deleteEpisode`, `clearGrowth`, `recall`, `resume`, `exportBegin`, `exportChunk`, `trialEvent`.

Payloads:

- `view`: optional `cursor` and `limit` (default 20, maximum 50).
- `teach`: `{ "expectedPolicyVersion": 2 }`; one preset, support before loot / loot after combat / ask before leaving the party.
- `revokeFeedback` / `deleteFeedback`: `{ "feedbackId": "id", "expectedPolicyVersion": 2 }`.
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

For an enrolled actor, accepting a party invitation additionally checks the real pending inviter's owner account/index in the same PartyHub action. A name comparison alone is insufficient. Ordinary non-enrolled group behavior stays unchanged.

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

The Node store migrates schema 1 atomically without losing IDs, order, episodes, feedback or current policyVersion. Stable `archiveId` is persisted once; it is independent of process, database path, model and provider. Trusted P0 configuration supplies `ownerRef`, `worldEpoch`, and the expected `archiveId`. Legacy adoption is explicit, local-operator controlled, and cannot replace an existing owner or world binding.

Three separate objects: `characterArchive` (stable identity/base appearance/setting), `playerGrowth` (habits, feedback, episodes, deletion/revocation tombstones, actual relations), and `runtimeService` (runner ID, model/provider/prompt version, paths, credential references, operational authorizations). Only the first two enter portable export. Relations start empty when no verified evidence exists.

Export is a consistent full snapshot, not `recentEpisodes()` or a copied live SQLite file. Its envelope has schema/version, export ID/time, characterArchive, playerGrowth, and HMAC-SHA256 integrity using an owner-controlled key from trusted runtime configuration. The key, credentials, runtimeService, queued actions, party admission, money, HP, equipment, items, quests and transforms never enter the archive. A checksum is not game-event attestation. The imported expected owner/archive/realm/world/character binding and trust key come from the destination configuration, not the file's own claims.

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
