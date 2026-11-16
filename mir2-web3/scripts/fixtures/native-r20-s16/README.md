# Fixed R20 / signed16 preparation

This directory is a new append-only channel. The original s13, s14 and s15
files, receipts and fixed transport remain immutable. Actual root-verified
s16 inputs now produced the fixed plan, module, driver and test fixtures.
The new fixed publication is CMS-admitted using root's genuine raw verification
receipt; the generator itself does not perform CMS verification.
No cloud stage, promotion or Windows game acceptance was performed by this
publisher lane.

The offline generator fixes game `WN-CANDIDATE-20261007-invited-20` to source
`8faedd89fbcdda7d4f814c05f7d2f56979ab5bf5`. It fixes the predecessor to actual
s15 and requires its signed engine component, directory
`releases/updater-9213EC3FE1A992F7-s15`, metadata and executable to be reused
with the same full-byte sizes and SHA256 values. Engine source stays
`b4390ee4c987bbf000ba1d95761151e39fc0d54f`.

The actual pinned-Windows-CMS/delivery-byte verification outputs were supplied
by root and admitted before invoking:

```powershell
python -B scripts/prepare_native_r20_s16_fixed.py `
  --publication-root <absolute-reviewed-publication-directory> `
  --feed-root <absolute-signed-feed-directory> `
  --bootstrap-exe <absolute-root-reviewed-reused-R19-Bootstrap.exe>
```

The generator reads `PUBLICATION-PLAN.json`, `ROOT-VERIFICATION.json`,
`latest.json` and `latest.p7s`; it does not accept a source/sequence override,
perform CMS verification, access credentials, sign or publish. It hashes
every complete source artifact, rejects linked/replaced inputs, keeps the
64 KiB control, 32 MiB metadata and 128 MiB artifact limits, and never replaces
a conflicting previous attempt. An identical rerun is a no-write operation.
The genuine source plan requires the exact complete signed15 predecessor
pointer as `expectedCurrent`. Only new16 generator/driver/Worker admission
adds that fixed equality guard; it rejects null or changed fields. Runtime
pointer observation, conditional CAS, reconciliation of an already promoted
16 and the original finite transport/deadline policies are unchanged.

Only then does it emit the new fixed Worker plan/module, delivery driver,
Worker/Python/cancellation checks, exact raw actual-cms16/predecessor15
fixtures and a generator report. That report means offline generation only;
its actual stage, promotion and Windows fields remain false. It does not
change the official downloader reader.

The sequence16 CI branch is wired to the generated checks and fixed driver.
Promotion for both15 and16
requires the exact same HEAD as the separate successful stage run; deadlines,
receipt checks, CAS and all older sequence paths are preserved.

The minimal Worker import and exact route `/upload/native-r20-s16` were added
before the existing s15 route after actual generation. `WORKER-ROUTE.patch`
records the applied four-line wiring diff against the frozen index.

`qa-runs/generator-policy/` contains offline source/hash/negative policy
checks. Mutated inputs stay in memory and are negative-only; pure rendering
sets admission to prepared, parses source without loading it, and never
emits a publication. Those checks are separate from actual R20 CMS and
full-closure acceptance.

The initial source-tail comparison failed because its normalization omitted
the identity-only `R19/s15` to `R20/s16` comment change. That assertion was
corrected; the subsequent25 checks passed. No runtime semantics or admission
guard was changed to resolve it.

Actual generation attempt1 also failed closed because the genuine plan bound
exact15 while the old template expected null. The source plan was not changed
or reissued with a weaker expectation. Root explicitly approved only the new16
fixed admission guard described above, then generation attempt2 passed. The
first failure is retained in `qa-runs/generation-01-failed.json`.

Actual immutable preparation binds41 objects (35 game objects),
1,077,405,420 bytes, including the unchanged signed s15 engine and the full
hash of root-reviewed reused R19 Bootstrap bytes. Metadata remains within
31,537,753 bytes; the largest artifact is111,160,320 bytes. The fixed literal
plan is29,254 bytes.

- Source plan SHA256: `905bc48ea9a210d5cb8ef5d82f748b341d9654cf287f1c3803ce74a1a7ae556d`.
- Root verification SHA256: `6a59813e8e3451820a064e1ff1c9f7525f77e0aab6ed85ffa791c8679c3192e8`.
- Native canonical plan SHA256: `87f52ab83eaa188b15e3d6dd313ec6b8e13047e7b10e720637e36a79101636e7`.
- Literal plan SHA256: `c9b149a3ef6351100eecbc43fc889b1262031824b00ee9169faa4b8d5808c1c9`.
- Closed object SHA256: `3dbb02448bf6c7e090f860714239ab6261255d81d9d67344fbb54a5ff696eaf3`.

Offline checks passed: independent generator26, Node138, Python76 and actual
Windows SIGINT1. Actual GNU timeout/owned-child1 is explicitly skipped on
Windows and must run in Linux CI. The complete outputs of the generated
suites are in `qa-runs/full-suite-01/`. All14 generated outputs were reread
and matched their recorded full SHA/size; ten frozen s15 inputs remain byte
exact. YAML parsing and `git diff --check` passed.

Source comparison excludes the explicit new exact15 admission checks and
fixed identity/hash constants. Other Python runtime function ASTs and the
Worker JSON/stream/deadline/CAS/HEAD tail are unchanged. This is not a claim
that the entire new module is byte identical to old15.

CMS verification was performed by root before generation; it is not reverified
by these Node/R2/transport models. Actual network/R2 stage, separate promotion,
Linux timeout proof and Windows game acceptance remain separate root gates.
