# Android NI-14: bounded read-only mailbox ingress

Source: `3003f0780c7254b69ff880a32161cce17142dab2`.
Frozen full Windows denominator: `3d735745f1117d42a7859e87604a106351dca935`.
Read-only remote check before this delivery: Windows `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`;
Android PR253 was OPEN/Draft on `codex/android-shared-sync`, base `codex/playtest-registration`.
This leaf is **NI-14 PARTIAL**. The whole Android/Windows goal remains Active.

## What changed

- Shared read-only `native_mail_ingress` contains fourteen frozen Windows pure
  projections/helpers. Visibility/crate path are normalized, then rustfmt1.95.0
  and whitespace canonicalization prove equivalence. This is not a new mobile
  battle, save, authentication, postage or attachment-grant rule.
- Java forwards only ReceiveMail, MailSendRequest, MailCost, MailLockedItem during
  an authenticated, listed-character selected Start or its same-owner map load.
  ReceiveMail alone has a 512KiB UTF-8 bound for a complete256-row mailbox.
  Services/other packets retain16KiB. Unsupported anonymous MailSent and
  ParcelCollected cannot settle a transaction or manufacture successful feedback.
- Android binds mail to a nonzero, unique selfPlayer identity and map. Unbound
  packet-first FIFO stays inert; the first snapshot base precedes newer packets.
  Missing mailbox fields do not invent an empty mailbox. Duplicates, malformed
  lists, >256 rows, >5 attachments and oversized typed output reject atomically
  instead of silently trimming. Queue bounds64/4MiB refer to serialized strings,
  not a resident-memory measurement.
- Escaped host envelopes admit the dedicated bounded mailbox while retaining
  the original32-event and8MiB serialized accounting ceilings. Oversized
  unrelated/service envelopes remain rejected. No new privilege is granted.
- Existing runtime mailbox/service consumers are factored as a plugin, with
  unchanged bodies and the same wallet-before-mail-before-shop lifecycle order.
  Rejected Start now retires staged mail. DataReset clears it; same-owner
  SceneReset keeps personal mailbox/parcel data and rejects stale map tags.

## Exact-source verification

All seven final gates bind the same clean committed source and53 input hashes.
The eleven ignored tests were already ignored; they are not counted as passes.

| Gate | Actual result |
| --- | --- |
| Android Rust normal | 378 passed, 0 failed |
| Android Rust ui-preview | 406 passed, 0 failed |
| Shared native-player-ui | 1287 passed, 10 pre-existing ignored |
| Runtime | 292 passed, 1 pre-existing ignored |
| Java normal / ui-preview, fresh full run | 57 / 57 passed; no skipped/errors |
| API31 arm64 normal | cargo-ndk check passed |
| API31 arm64 ui-preview | cargo-ndk check passed |

[Machine-readable source binding, original runs and guards](source-evidence.json)
contains commands, exact source/input hashes, full-gate exits, XML counts and
raw log/XML hashes. Raw local archive (not a portable release artifact):
`apps/game-client/platform-android/target/mail-ingress-20261003-UNXFiI/`.
The Rust host tests exercise production receive and shared consumer functions
without a renderer. Java uses local TLS MockWebServer fixtures, not a real account.

## Failures retained

- The initial Java red ran1 test/1failure, then all3 new transport tests/3failures.
  A complete mailbox demonstrated why the old16KiB packet cap could not be
  reused. The oversized mailbox fixture was explicitly raised to >512KiB;
  full256×5 and oversized16KiB service tests were added. Old test assertions
  were not weakened. The working Java follow-up actually ran Debug57/preview5;
  only the final committed full gate ran57/57.
- First Rust host-focused run:23passed/5failed. Four failures came from the
  lightweight fixture lacking actual mail consumers; one found missing rejected
  Start retirement. Factoring the same existing consumers and adding retirement
  yields28/28 with those original assertions retained.
- New shared test preparation failed E0308 (expected catalog image Optionu16
  versus the model's Optionu32); only the expected value's conversion changed.
  After that preparation correction,6/6 shared projector tests passed.
- Audit preparation initially used nonexistent Windows `src/lib.rs`; the real
  `src/main.rs` corrected that check. A malformed trailing backslash-n in the
  generated JSON was also corrected, with the invalid original retained.
  The first documentation patch was rejected for targeting the same file twice;
  it was combined into one operation before any document edits.
  These are preparation errors, not product red/green claims.

## UI rollback and package identity

The failed v36 storage/Bag candidate `11d28f02a6e975224de51ec54993273cd9958f52`
was reverted by `f07a9152a3349b3aa0861f05d87d8582d5476fa4`. This source leaf
retains published v35 UI plus mail DATA ingress. Blank button/count text in v36
remains a failed visual gate. The isolated text-layout diagnosis is not a fixed
Android screenshot or GPU proof. UI repair remains awaiting user confirmation.

This leaf did **not** build/install a new APK, change APK version/signing/assets,
or replace the previously installed failed v36 normal/preview packages.
Therefore those packages/screenshots cannot be rebound to this source or used
to claim its JNI/visible mail UI has passed. Prior GL506 failures remain open.

## Not completed / next bounded work

- Actual Java-to-JNI-to-runtime mailbox/service producer on exact fresh APKs,
  source/resource/hash/install/version evidence and visible simulator mail UI.
- Full outward read/delete/quote/lock/send/claim paths, delivery and failure
  correlation, authoritative money/item settlement. No local success fallback.
- Approved real HTTPS/WSS login and online character loop, mailbox during
  logout/reconnect and saved state, production isolation.
- Phone mailbox/composer hit targets, IME, multiline/scroll, every supported
  locale/window/operation and confirmed storage/Bag UI repair.
- Complete remaining NI15–20 / G2–G6, resources/audio/update, zero-error rendering,
  physical-device multitouch/background/network and final human acceptance.

No production deployment, real-save edit, password/key disclosure, server rule,
Windows host write or original-workspace mutation was performed. Original
checkout and prior Android checkout retain their verified Git status; this is
not a recursive hash guarantee for unrelated untracked content.
