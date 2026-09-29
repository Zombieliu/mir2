# Three-class saved-route verification

Run the external read-only gate from PowerShell or Node:

```powershell
node C:\mir2-game-shop-audit-20260915\verify-three-class-saved-route.mjs > C:\mir2-game-shop-audit-20260915\verify-three-class-saved-route.json
```

Optional roots are `MIR2_PROTOCOL_ROOT` and `MIR2_NEWCOMER_CONFIG`. The helper reads the natural account store and the three named class artifacts, derives each class’s 55 route IDs from `newcomer-journey-v1.json`, and checks the four milestone IDs `2100015`, `2100020`, `2100025`, `2100030`.

It emits only safe character/account mapping, revision, level, map, position, HP/MP, counts, missing IDs, mismatches, final-evidence status, and SHA-256 file hashes. It never prints passwords, auth, session, or seal-ledger fields and never mutates storage. For each trace it selects the last owner-bearing `worldSnapshot` before an outbound `logOut` request, requires that request to be sent, and then requires a later received `LogOutSuccess`; a post-logout character-list/empty snapshot cannot replace the saved owner state. The selected owner must match the expected name/class/index, have a finite owner id, `dead:false`, and authoritative exact HP above zero. The derived class route must contain exactly 55 mandatory IDs. An active-run snapshot/report without `completed:true` plus received `LogOutSuccess` is explicitly ineligible as final evidence, so the gate remains `FAIL_UNTIL_FINAL_PROOF` until all three ordinary saved characters have 55/55 mandatory, 4/4 milestones, level 30+, and matching logged-out final snapshots.

Exit status is 0 only when all three classes have complete final evidence, 55/55 + 4/4, level 30+, and all saved/final fields level/map/x/y/hp/mp present and equal. Any missing or mismatched field exits 2. The isolated synthetic validator is:

```powershell
node C:\mir2-game-shop-audit-20260915\verify-three-class-saved-route-fixture.mjs
```

It creates only temporary synthetic files, proves a valid baseline can pass, and proves wrong-map and unknown-level cases exit 2.
