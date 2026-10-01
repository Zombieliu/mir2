# Native Windows automatic updates

This implements the native Windows distribution path independently of the old
web-only Tauri shell. The installer installs the verified r7 game Candidate plus
a separately attested launcher/updater bundle. It preserves the stable AppId.

Desktop/start-menu shortcut -> app/Mir2Launcher.exe -> authenticated engine copy
in app/.update -> signed HTTPS update -> app/game/mir2-platform-windows.exe.

Trust and activation:

- Detached one-signer RSA/SHA256 CMS; embedded RSA-public-key SHA256 pin.
  A same-key renewed code-signing certificate works without importing roots.
  Fresh feed issue time/expiry and signing-certificate validity are checked.
  Historic installed descriptors remain valid at their authenticated build time.
- Channel/windows-x64 sequence, minimum launcher ABI, protocol/content contract,
  exact candidate/version/manifest/attestation and size/hash bindings.
- Fixed HTTPS origin, certificate validation, no redirects; bounded streamed
  downloads and ASCII Windows-compatible manifest paths. Reparse points,
  hardlinks, case aliases, file/directory collisions, ADS-style paths and
  unexpected executable assets fail closed.
- Only changed/missing/corrupted managed game files are downloaded on an update.
  Completed verified downloads are cached; interrupted partial files are retried.
  Startup with an unchanged release checks metadata and executable without
  hashing all123031 assets. Missing game EXE bypasses that fast path for repair.
- Immutable engine generations and atomic active pointer; engine executes from
  a verified scratch copy, so it can update itself without replacing its image.
- Per-install named mutex plus exclusive file lock across Windows sessions.
  Exact process-path detection covers old/direct game shortcuts. Neither game
  nor installer force-terminates the player. Installer probes and holds the same
  file lock for fresh and existing installations, releasing it before launch.
  Successful full reinstalls retire an older journal without losing the signed
  sequence history. A newer signed installer seed also raises that history floor.
- Flush backups and bounded journal before mutation, activate metadata last.
  Recover applying transactions before launch. Retain committed first-launch
  obligation across check-only runs/power loss. A failed process creation or
  nonzero exit within10s restores the previous package and quarantines that feed.
  Highest signed sequence survives rollback. Zero exit or10s of process survival
  is only a startup heuristic, not proof of gameplay/server compatibility.
- Offline/unavailable/invalid feed can use the verified installed game; invalid
  recovery/installed signatures prevent launch. Closing the update window
  cancels before activation/game start. Logs and personal non-manifest files,
  per-user language/display preferences and server accounts/saves are preserved.

Nine native updater languages: en/zh-TW/pt-BR/ru/hi/id/vi/th/ar. The native dialog
uses Windows font fallback and DPI scaling, separately from game font atlases.
It reads the same roaming language preference as the installer/game, with a
legacy local-preference fallback.

Release workflow (PowerShell7.2+; known internal signing identity required):

1. Run focused/native updater tests including generated CMS fixtures.
2. Commit all source and use exact clean source revision.
3. Run scripts/build-updater-release.ps1 with CandidateRoot, fresh OutputRoot,
   SourceRevision, SignerThumbprint, Sequence and optional TargetDir. It always
   rebuilds both small Rust binaries from the locked source. The game Candidate
   keeps its own original source revision; it is never relabelled.
4. Run the existing strict game Candidate verifier and
   scripts/verify-updater-bundle.ps1. Run independent literal game/supplemental
   installer input generators. Compile the Inno recipe.
5. scripts/archive-update-release.py uses the emitted source-map.json and a
   fresh absolute tar.gz output. It validates all bytes/closure but explicitly
   does not replace the CMS gate. Upload to private staging; server validates
   extraction and every hash before immutable release promotion. The workstation
   CMS gate remains explicit, with all approved uploads SHA256 pinned.
6. Use scripts/deploy-prepare-plan.py with the approved archive/feed, an explicit
   fresh output directory and current Caddy configuration hash. Review the plan,
   then execute its preflight, private staging/upload and apply commands through
   the trusted SSH transport. The root-owned publisher keeps existing routes,
   validates Caddy using only its existing service environment references, and
   atomically publishes the signed discovery directory last under
   https://165.154.65.136.sslip.io/client-updates/. Discovery is no-store.
   Release paths are immutable. Gateway routes/binaries/save data are separate.
   Sequence must increase for every changed feed, including expiry renewal.

Old clients need a one-time r7 installer migration to get the launcher. Subsequent
client/asset/engine releases use per-file updates. The bootstrap stays fixed; a
future incompatible bootstrap ABI or signing-key replacement needs an explicitly
authenticated migration/new installer. A renewed certificate for the same RSA
key is supported. The feed expires after30days and must be renewed/published.

Limits: internal CMS is not public Windows Authenticode signing and does not
solve application-control4551 on the other laptop. Existing signing identity
expires2026-12-28; renew the same protected key/cert before publishing later
feeds. An absent/corrupted active engine, whole game metadata loss, or missing
signed release history needs repair with the trusted installer. This work does
not resume capacity testing or claim50–100-player stability.
