# Root Probe19 controller

This controller only seals reviewed source or writes a read-only guard receipt. It never provisions accounts, builds or starts services. The probe remains nonformal and capacityAccepted=false.

Root must use the exact final runtime-prepared revision and review pins. Mode1 is node root-seal-and-guard-v1.mjs --seal-reviewed-inputs --prepared-file=<absolute-final-prepared> --prepared-sha256=<actual-sha> --review-file=<absolute-final-review> --review-sha256=<actual-sha>. Mode2 is node root-seal-and-guard-v1.mjs --readonly-sourceguard. Only after both succeed does Root separately call prepare-inputs.mjs --prepare-authorized-inputs and run-owned.mjs --execute-authorized-probe --attack-results=v2.

Seal mode checks every local/external pin, declared preparation metadata and original baseline helper references. It performs actual Node --check and PowerShell AST parsing, calls the finalized new full16 validator with input.project, checks policy/driver/scene/wire/Store contracts, then writes the real syntax log before the source freeze. The syntax log is sealed and available before account preparation. No check assumes private ACL has two principals after the normal owner-only seeder.

Backend build/source is c743/50/dae1; current capacity driver and execution is J=4fb58/226. The existing full16 validator is pinned under full-lifecycle-scout-anti-convergence-v1. The final prepared path is selected through CLI, with no stale hardcoded prepared-v1/v2/v3 choice. The controller support record remains root-controller-preparation-v1.json. Prepared metadata and independent review are separate seal inputs to avoid hash cycles.

Preparation syntax checking only parses this controller; it is not actual seal/guard execution or ESM evaluation. Root must preserve every previous PENDING prepared revision and any real failure log.
