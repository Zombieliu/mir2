# Invited Windows registration correction — 2026-09-29

The public realm rejected repeated registration attempts, but the native client
only decoded nested Gateway errors. Real top-level messages became the generic
`gateway error`. The native registration form also advertised 5–15 character
passwords while the commercial server requires at least 10 characters.

## Change

- Keep existing login and password-change input behavior. New-account passwords
  use the supported native 10–15 ASCII alphanumeric subset, reject account-equal
  and the same four common weak passwords as the commercial server. The server
  remains authoritative; its policies and counters are unchanged.
- Field colors, enabled submission, intent construction and raw model submission
  use the same field validation. Optional profile fields remain optional and the
  existing Crystal email pattern is retained. `1S` is rejected; empty is allowed.
- Read canonical top-level error messages/codes and retain legacy nested errors.
  Display the server's retry seconds in Chinese; preserve unknown server reasons.
- Actual keyboard/paste edits clear stale registration feedback. Pending requests
  reject keyboard and paste edits; no duplicate request is queued.
- Registration guidance/local errors and common server rejections are Chinese.
  This is not a whole-client Chinese-localization claim.

## Evidence

The read-only isolated-realm audit found the reported account absent, with four
registration attempts exceeding its existing three-per-hour account limit. The
peer bucket was four of five; this was not a load-test peer-bucket exhaustion.
The account bucket was expected to expire at 2026-09-29 10:17:36 UTC. No account,
password, counter, server restart, or production data was changed during this fix.
At 10:20:42 UTC a second read-only observation confirmed its natural expiry
(`GET=null`, `PTTL=-2`); the account was still absent. This is a timed observation,
not a guarantee that later repeated attempts cannot trigger the same policy.

Shared `native_shell` regressions: 64 passed, zero failures/ignores. The focused
registration filter also passed 17 checks; these overlap and are not additive.
Native final regression: 762 passed, zero failures, three existing explicit
ignores, with the verified invited package selected by MIR2_NATIVE_ASSET_ROOT.
The first complete run lacked runtime asset fixtures (589 pass/173 fail/3 ignore);
that failed run is retained and was corrected by declaring the existing complete
asset bundle, without altering tests or gameplay. An initial parser borrow-check
failure was fixed before the passing native regression.

Independent review covered production-password compatibility, flat/nested error
handling and clipboard behavior. `git diff --check` passes. No new live account
was created as a substitute for the player's acceptance.

Private local evidence directory (no credentials exported):
`C:/mir2-ui-repair-20260921/registration-fix-20260929`.
Shared logs: `C:/mir2-build/client-bevy-r29-tests/registration-*-20260929.log`.

## Release status

Clean attested source `6b55b668c72ce56a95bc53c62cea22fcf03502d8` was pushed on
`codex/playtest-registration`. Its pinned, locked Rust 1.95.0 release build
completed successfully; the native EXE SHA-256 is
`9A640907332E02907A7A19056AB3CDCE3DB7269614A6A9CDEB0BA50B7F33FA4A`.

Candidate `WN-CANDIDATE-20260929-invited-02` contains 123,029 files and passes
the strict package verifier with `sourceRepoCheck=checked`, a valid detached
CMS signature, zero failures and `visualAccepted=false`. A separate exact-file,
size and SHA-256 check verified all 741,601,990 installer-input bytes. The first
packaging attempt failed Windows ADS enumeration on the long staging path;
the unchanged script passed using its supported shorter `dist/r2` output path.
No filesystem or signature checks were disabled.

New artifact:
`C:/mir2-playtest-releases/20260929-registration/Mir2-Invite-20260929-r2-Setup.exe`
(594,793,744 bytes), SHA-256
`DED4F0E306DF12ABD41EA7FFAD1E76CD644F64A074E24CF144ADDB0964340771`.
The old delivered installer is retained unchanged. Inno Setup compilation exits
zero, version is 2026.9.29.2 and the actual Setup PE machine is AMD64.
`SetupArchitecture=x64` makes the runtime probe inspect the x64 system DLL.
Bundled Microsoft runtime and the compiler have valid publisher signatures;
the final Setup itself still has no publicly trusted Authenticode signature.

The owned C-drive upgrade fixture was checked before installation. An initial
guard detected the active F-drive client; a path-specific follow-up detected
that the desktop shortcut also belongs to that F-drive client. Both stopped
before launching Setup or changing installed files, shortcuts or registration.
Installed-upgrade/uninstall and a new authenticated GUI registration remain
unverified. The delivered artifact is a verified package/installer build, not
an assertion that those user-machine flows have passed. The user can close the
old client and install this revision; no running client was closed automatically.

Installer recipe, exact input allowlist, compile log, CMS/package report,
checksums and guard logs are retained under
`C:/mir2-playtest-releases/20260929-registration/installer-build`.
`registration-release.json` beside the Setup records the completed and open
checks. No fresh account or password was created for the user.

The user paused the capacity GOAL. These registration and installer corrections
do not resume capacity work or establish 50–100 stable players. Original
production, the isolated service, and accepted character saves are unchanged.
