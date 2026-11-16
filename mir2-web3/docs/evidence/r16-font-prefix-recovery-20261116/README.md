# Interrupted whole-font prefix recovery

Parent: `7468b3337ac448baf18dc1b362b87acbac5fa635`.
Candidate: `r16-font-prefix-d10fbdb7`. New Worker: 68,357 bytes,
SHA-256 `9c301d24217f498408d77eb0b38f88f9c6825020d7d380ed2c383022db615f97`.
Application `e0ceea2296` is now published with a matching Gateway. Both original
public cohorts and simplified/traditional QR checks passed; see
[production qualification and visual limits](production/README.md).

## Observation and change

The preceding public R2 trace recorded an app SC font read at +29,920 ms,
`net::ERR_ABORTED` at +67,058 ms after 13,702,384 encoded bytes, and another full
app read ending at +138,252 ms. This is an encoded-byte observation, not a claim
about the exact decoded prefix received. The original whole-body reader discarded
its accumulated chunks; only separate Range responses could retain restart parts.
The [sanitized observation](public-r2-font-interruption.json) matches the retained
private trace hash and records no observer errors or dropped requests.

The Worker now independently checks complete 256 KiB units from an interrupted
whole SC response and keeps matching units in the existing part cache. A partial
tail remains unusable. A later request still verifies every reused part and the
complete 16,437,364-byte SHA before returning a font. Complete bodies with a bad
SHA are rejected as before.

The logical request captures cache namespace and configuration revision before
queued/range work. Expired permission, changed configuration and explicit owner
cancellation cannot start prefix persistence. One existing 750 ms cache budget
covers all preservation work; late storage opens cannot schedule writes. All
original 63 part pins, cache keys, two download lanes and network deadlines remain
unchanged. Other fonts and gameplay rules are unchanged.

## Verification

- **188 focused/adjacent tests passed** (114 plus 74 additional fallback/retry
  checks), zero failures/skips. This includes the
  36-test range suite and nine new prefix cases; the subset is not added again.
  Cases include broken and short bodies, incomplete tails, corrupt units,
  configuration changes during body/queued work, expired permission, cancellation,
  delayed/denied storage and malformed complete responses.
- **Actual Chrome reproduction:** the old Worker retained 0 parts after the same
  controlled whole-body interruption; the expected baseline assertion failed.
  The candidate retained exactly 60 full verified units, discarded the partial
  tail and exposed no incomplete whole font. After closing/reopening the browser
  without resending configuration or populating caches, it fetched only the three
  missing units in one Range request. All six complete original font hashes,
  actual FontFace decoding and canvas glyphs passed. Both browsers/servers closed.
- **Actual QR regression:** current Page/QR effects and real Worker retained the
  existing shared-fill/cancellation behavior, 16 requests / 63 verified SC units.
  This was a local pending QR fixture, not real wallet approval.
- **Strict isolated Next/TypeScript build passed**, 43,271 ms,
  build ID `4-_mxLjHUVfES0_RTDSkf`. All 2,181 app/library files match the committed
  parent except one locked generated input; the served Worker is the new draft.
  Non-runtime test drafts were validated separately in the current checkout.
  Dependencies, prebuilt game packages and unrelated WIP were excluded/preserved.

| Complete local game | Walk ACK | Native after first frame | First/reentry Q |
| --- | ---: | ---: | --- |
| English default | 111 ms | 22,449 ms | Native / Native |
| Traditional Chinese shared WebGL2 | 90 ms | 4,731 ms | Native / Native |

Both unchanged ordinary game cohorts passed complete walking delivery, six font
hashes, 113 HUD resources, configured Q open/close without world mutations, normal
Alt+X Logout, same-role saved-position reentry and a second normal save. They reused
owned CacheStorage while clearing HTTP/auth state. The font interruption was the
separate Chrome fixture; it was **not injected into these games**. These timings
do not measure public cold performance or prove a causal speedup.

All 12 private build overlays were restored after the games; the 29 recorded
unrelated WIP hashes remained unchanged. All owned browsers and local
Gateway/Next/proxy processes closed.

See [validation](validation.json), [114-test output](adjacent-tests.log),
[74 additional Worker checks](remaining-worker-tests.log),
[expected old-Worker failure](chrome-baseline.json),
[new-Worker Chrome result](chrome-candidate.json) and
[private audit pins](private-evidence-pins.json).
Screenshots: [six original fonts](six-original-fonts-after-restart.png),
[English Native Q](world-r1-native-quest.png),
[Traditional shared-WebGL2 Native Q](world-r2-native-quest.png).

The resource checks are reproducible from the repository root:

```text
node --test mir2-web3/apps/web/scripts/test-asset-worker-font-ranges.mjs
node mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs --output <new-directory> --failure-mode whole-prefix --chrome <browser-executable> --playwright-package <package.json-with-playwright>
```

## Remaining scope

Matching release packaging and both original public cohorts are now qualified.
At the timed shared-HUD gate both public Q windows were actually Native, while
the first Q remained a React fallback in both cohorts. One early PNG is uniformly
black and byte-identical to a previous 6ba capture; it is not visual acceptance.
See the separate phases and retained limitation in [production](production/README.md).
Full Windows/Web 1:1, mixed NPC/welcome language, broader
routes/maps/progression/combat/economy/Alo, existing Rust formatting and human-only
Windows/phone checks remain open. Goal stays active.
