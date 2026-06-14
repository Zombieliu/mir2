# Project Status Snapshot — 2026-06-15

Current state of `mir2-web3` (Crystal → web port) on `main`. Evidence-focused:
what's done, what's measured, what still blocks "playable 1:1", what's next.
Supersedes the point-in-time `PROJECT-STATUS-2026-05-30.md` (which tracked PR #1).

## Headline

- **Protocol (web):** ServerPacket handlers **278/282 = 98.6%**; ClientPacket
  **68/153 = 44.4%** literal, **111/153 ≈ 72.5%** via the gateway
  `browser_command_to_action` bridge. (`measure:frontend-coverage`, 2026-06-14)
- **Frontend completeness:** "visual client" ≈ **90%**, "playable game" ≈ **74%**
  (`FRONTEND-COMPLETENESS-AUDIT.md`).
- **Backend gameplay depth (strict prod口径):** combat numerics, world authority,
  and map/world all rose to **~85%** in June; **per-monster AI breadth (~35 vs
  Crystal's 212) is now the single largest gap** (`PRODUCTION-GAP-ASSESSMENT.md`,
  6/15 update block).
- **Backend tests:** ~**1272** `#[test]`/`#[tokio::test]` in `mir2-simulation`.
- **Deploy:** Player Web live behind `https://mir2.obelisk.build` (Vercel +
  Cloudflare Worker); Gateway on UCloud `165.154.65.136` (Postgres + Redis,
  cap 30/15/15); active asset release `mir2/v/20260601-fullcrystal-a2f10be0`.

## Landed since 2026-05-30 (selected, 178 commits)

**Gameplay feel (web):** floating damage numbers + hit flash (#98), all Crystal
sound effects wired with faithful triggers (#99), real item icons on ground drops
+ walk-to-pick-up (#97), loading overlay instead of black stage (#95),
continuous monster-click AutoHit, mount rendered as a layered riding sprite (#84).

**World / simulation:** full Crystal world activation — all maps live when
occupied, dormant when empty (#80); on-demand monster pool (#83); Crystal-faithful
zone-authoritative combat numerics (`Random(MinDC..=MaxDC)` + AC/MAC by damage
type + crit + agility dodge); mining (`Map.CreateMine` 1:1) + dynamic doors;
per-city reputation currencies (#78).

**Ops / parity:** full Crystal GM @-command set (~78 commands) on real backends;
zero-config GM provisioning (`MIR2_GM_ACCOUNTS` / `MIR2_GM_PASSWORD`); security
audit + remediation (#77: salted password hashing, packet DoS bounds, trade-dupe
guard, admin auth + rate limiting).

**Rendering / perf:** GPU map-tile + entity atlas served same-origin (#74, #85);
off-main-thread alpha-keying + movement (#93, #96); stop wiping the asset cache
each deploy (#100).

**On-chain (testnet):** smart-mine M1–M4 + Dubhe session keys SK0–SK2 (#92);
draggable/collapsible testnet HUD (#94).

## Still blocking "playable 1:1"

| Gap | Type | Path to close |
|---|---|---|
| Per-monster AI breadth (~35 → Crystal's 212 subclasses) | code, large + parallelizable | port逐怪 AI from `Crystal/Server/MirObjects/Monsters/` |
| Cross-process Zone sharding / single-owner handoff | design + infra | durable Zone snapshot/log + real RPC (`WORLD-AUTHORITY-STATUS.md`) |
| Persistence normalization (inventory/mail/economy/auction) | code | move from per-account JSON blobs to normalized tables |
| VFX real atlases + audio *bytes* | asset-gated | extract Crystal `.Lib`/`Sound` on a real machine → R2 publish |
| Real-GPU / mobile actor-render sign-off | hardware-gated | headed device QA (sandbox lacks GPU) |
| A few unwirable window actions (conquest gate/tax, hero dismiss/recall) | protocol-gated | new packet or NPC-script flow |

## Verify / reproduce

```bash
# frontend coverage numbers in this doc
cd mir2-web3/apps/web && node ./scripts/measure-frontend-coverage.mjs

# backend tests
cd mir2-web3 && cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1

# live health
curl https://165.154.65.136.sslip.io/health
curl https://mir2.obelisk.build/api/asset-manifest   # remoteAssets.assetBaseUrl non-null
```

See also: `ARCHITECTURE-CURRENT.md`, `FRONTEND-COMPLETENESS-AUDIT.md`,
`PRODUCTION-GAP-ASSESSMENT.md`, `CRYSTAL-1TO1-ROADMAP.md`,
`INFRASTRUCTURE-AND-SECRETS.md`.
