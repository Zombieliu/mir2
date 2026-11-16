# Recharge deployment regression, 2026-10-14 Beijing

The activity website operator now writes the exact seven public-file buffers
and `release.json` that passed hash/length verification. It no longer reopens
the upload directory after validation. Exclusive creation and flush/fsync retain
the original immutable-release behavior.

The interrupted activity publication reported `existingSiteRoutesChanged`.
Its inspected incoming snippet used CRLF while the existing site block used LF;
the normalized blocks were identical. Only the incoming snippet is now
normalized for first installation. Updates also admit the previous operator's
exact uploaded bytes and LF/CRLF equivalents, with a single occurrence across
all variants. Existing Caddy bytes and their SHA256 compare-and-swap remain exact.
Duplicate/missing/changed snippets, duplicate or unexpected authenticated
routes, ambiguous upgrades, previous-site identity, gateway identity and
activation/rollback checks remain enforced.

`verified_release`, `write_verified_release` and `proxy_candidate` are extracted
from the existing operator so the actual code can be checked without executing
`main`, reading credentials or contacting services. Offline checks on Windows:

| Command from `mir2-web3` | Result |
| --- | --- |
| `python -B -m unittest discover -s apps/recharge-web/tests -p test_deploy.py -v` | 20 passed, 0 failed/skipped |
| `node --test apps/recharge-web/tests/core.test.mjs apps/recharge-web/tests/activity-core.test.mjs apps/recharge-web/tests/activities.test.mjs` | 29 passed, 0 failed/skipped |
| `git diff --check` | Passed |

The Python regression replaces every staged public file and the manifest after
verification, removes staging altogether, then verifies every published byte,
length and SHA256 against the original manifest. It also rejects pre-verification
tampering and unsafe manifest names, preserves pre-existing release files, and
checks initial/update/logout/activity route changes with LF/CRLF input while
retaining unrelated mixed-ending configuration bytes and refusal gates.

Raw bounded outputs and source hashes:
[`deployment regression evidence`](generated/player-qa/recharge-deploy-20261014/RESULT.json).

This is a source and offline regression checkpoint. No Caddy reload, gateway
restart, database migration, payment, game-account mutation or website rollout
was performed here. The independent recharge site's earlier Stripe TEST / game
save acceptance belongs to `e1ec0ae8fb`; the activity gateway's recorded rollout
belongs to `bad3737f26`. They are separate from a fresh activity-page publication
and browser acceptance, which remain pending. Global Candidate100 is unchanged.
