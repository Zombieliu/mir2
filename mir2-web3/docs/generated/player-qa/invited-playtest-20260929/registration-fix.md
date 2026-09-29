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

Clean attested rebuild, signed package, revised x64 installer and installed-file
verification follow this code checkpoint. Inno's SetupArchitecture=x64 is needed
so its runtime probe inspects the x64 system DLL. Do not reuse the old EXE's
attestation or silently overwrite the previously delivered installer.

The user paused the capacity GOAL. These registration and installer corrections
do not resume capacity work or establish 50–100 stable players. Original
production, the isolated service, and accepted character saves are unchanged.
