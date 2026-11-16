# Actual Cloudflare native publication — 2026-10-06

Signed sequence13 is now public at the original source and
`https://assets.mir2.obelisk.build/client-updates/`. The new
[Windows Bootstrap](https://assets.mir2.obelisk.build/client-updates/installers/6175bd72f273c2f00d96117183fc1bf4d1f987c15455a1064e4f0e975ca70af5/Mir2Setup.exe)
is27,496,643 bytes/SHA6175bd72f273c2f00d96117183fc1bf4d1f987c15455a1064e4f0e975ca70af5.
It seeds the resume/progress engine48454; live gameR17/source6032 is unchanged.
The user's installed Cloudflare connector authorized management; existing
opaque Actions upload authorization performed the actual object publication.
No secret value was retrieved, rotated or recorded.

[Raw public archive](raw-publication.zip) contains234 exact files/1,082,301
uncompressed bytes. ZIP299,599 bytes/SHA
`68b1a9ef9193bd32289dc8c6ae58431496a581f8cd75cfb20fae2ed42f38c46b`.
[Inventory](ARCHIVE.json) pins every member; all ZIP members were rehashed.
Only an explicit allowlist is included. Private Caddy configuration/environment,
SSH credential material and large executable/download bodies are excluded.
The original external files and payload SHA receipts remain retained.

Sources remain distinct:

| Role | Actual pushed revision |
| --- | --- |
| Upload Worker and stage/promote tooling | afc3f7561fbd412c8d4afad3082724e511913bed |
| Dedicated reader | 4c60c323aed11829abae6ee5ce6e13c675a3c1c3 |
| Bootstrap update engine | 48454bba7623694ca7432613af52bee28e83970e |
| Unchanged live game | 6032ef8b3e27dd97bad0b20c8676ef9f185db64b |

The three reader code/test/config files are byte-exact admissions, independently
pinned in [READER-ADMISSION.json](READER-ADMISSION.json). The original reader
README is in the raw archive; the current README describes the actual CI route.

Actual deployment and publication:

- `raw/CF-READ-PREFLIGHT-01.json`, `BUILD-INPUTS.json`, upload/reader deploy
  receipts bind real account/bucket/route and exact code readback. Upload secret
  binding is inherited. Reader uses R2 only and workers.dev/previews are disabled.
- Frozen original proof closes36 objects/817,719,767 bytes. Fixed6 origin append
  imports the byte-pinned existing Guard/Operations, checks all30 old hashes,
  rejects late conflicts before any public write, and adds only the six admitted
  paths. Actual Linux11 negative checks include real renameat2 NOREPLACE,
  symlinks, hardlinks, writable ancestors and full-hash tampering. The initial
  SFTP-mode seal refusal happened before copying; mode repair and seal2 passed.
- Initial Caddy candidate validation failed on missing live environment before
  config replacement. The subsequent exact candidate validates with live
  environment held only in memory. Public update static paths were extended.
- [Stage1](https://github.com/Zombieliu/mir2/actions/runs/37346004215) retains
  HTTP502/uncertain object0 import with no pointer attempt. Actual read-only
  Worker probe then observes origin200/exact SHA but no Content-Length and
  R2 object0 absent. Only update-file HTTP compression is excluded; strict
  Worker length/SHA validation is preserved. Before/after probe shows identical
 1159 bytes/SHA and recovered Content-Length1159. A reload attempt stops on
  incoming connection before writing; the reconciled operation later validates
  and reloads the exact candidate. Gateway units are not restarted.
- [Stage2](https://github.com/Zombieliu/mir2/actions/runs/37348106566) succeeds,
  verifies every public object/817,719,767 bytes, and does not write the pointer.
  Artifact11361272735 ZIP4912/SHAca3732b20263fe6eb3a3e6220423dde135308e8e0989705834bb4a48de20b939;
  receipt SHA5cd36d6d74ae3e2dd4c16e421389d6204587f5baa1777065a92d04a43064b4fd.
- Origin latest13 is exchanged atomically with the old signed12 pair, which
  remains privately retained with its original bytes/fingerprints. Local TLS
  reads validate both aliases/no-store. No new CMS result is inferred from
  that equality check.
- Separate [CAS promotion](https://github.com/Zombieliu/mir2/actions/runs/37350173926)
  binds stage2/run/receipt SHA and succeeds with private pointer and public
  aliases verified, no unknown outcome. Artifact11361299115 ZIP6258/SHA
 9182c3a50a047326eb177ef9bd9dfb1e4c65c9923ffb456ac2e12654ece6199e;
  promotion receipt SHA78f3ab0dd9869aa915d4b9e4b54cd7aa3064a5247171a2e8fe4a0a7ce6d8d79e.
  Tooling138Node/64Python/2actualSIGINT passes with zero skips in each actual
  publication run; these repeated checks overlap and are not summed.
- Temporary read-only origin probe route and Worker are both deleted with
  successful API receipts; native and original routes remain.

Public delivery verification:

| Same immutable Bootstrap, TLS and full SHA | Origin | CDN MISS | CDN HIT |
| --- | ---: | ---: | ---: |
| Wire bytes | 27,496,643 | 27,496,643 | 27,496,643 |
| Curl total seconds | 15.858007 | 5.404556 | 5.132961 |
| Bytes/second | 1,733,928 | 5,087,681 | 5,356,880 |
| Time to first byte | 2.293581 | 1.472617 | 1.062833 |

Actual CDN POP isSIN. Warm speed is about3.09× origin in this local comparison.
This does not establish every player's rate or complete installation duration.
The five HTTPS range checks include real `curl --continue-at -` from a retained
1MiB prefix: only26,448,067 new bytes are sent; final full Bootstrap SHA matches.
Suffix1024,416 and stale If-Range200/full-body checks also pass.

After promotion, default Python public readback returned403. The original
failure receipt incorrectly attributed it to origin/zero completed checks;
`FAILURE-CORRECTION.json` retains the original and identifies the actual failed
CDN endpoint after both origin files succeeded. Curl/default ureq/CI agents
returned200, while Python yielded Browser Integrity Check error1010. Real
configuration reads show global BIC on/security level medium, with no existing
configuration entrypoint. One new rule exempts only the exact download host's
public update GET/HEAD paths via `bic:false`. No managed rules or global
security settings are disabled.
[Cloudflare1010](https://developers.cloudflare.com/support/troubleshooting/http-status-codes/cloudflare-1xxx-errors/error-1010/),
[scoped configuration](https://developers.cloudflare.com/rules/configuration-rules/settings/#browser-integrity-check).
API creation/readback agree; global settings remain unchanged. Actual Python
public JSON/signature/HEAD are now200; private/write requests retain1010/403.
With the genuine native default UA, private paths404 and write methods405
verify Worker enforcement. Source/CDN latest pair7 checks pass with sequence13,
no-store and exact original signed bytes.

An additional release-profile native Windows probe depends on the unchanged
production updater/source48454 and actual locked ureq2.12.1. Its source pins
match before/after; build and actual run pass. Direct CDN requests confirm13,
production HttpsSource discovery reports2773 bytes, Windows pinned CMS verifies
both discovery and engine descriptions, the full1,859,072-byte engine SHA
matches, and HTTPS206 prefix equals the original Bootstrap bytes. This is a
real native TLS/CMS read path, not execution of the released installer or game.

All configuration/feed steps compare their own before/after PIDs. This does
not claim every QA service stayed in the same state throughout the whole run:
one periodic QA unit was observed inactive between phases. No Gateway restart,
native game launch, F installation replacement or real-save edit was performed.
Actual full-installer and affected-laptop timing, human frontend acceptance,
R18 rollout, multiplayer capacity and the broader blocked Goal remain OPEN.
