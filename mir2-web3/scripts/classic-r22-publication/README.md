# R22/s17 offline preparation

Frozen game: `7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc`, Candidate `WN-CANDIDATE-20261008-invited-22`. Reused engine: `b4390ee4c987bbf000ba1d95761151e39fc0d54f`, exe SHA256 `9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8`, unchanged `releases/updater-9213EC3FE1A992F7-s15`.

These are preparation tools. No new feed, root CMS receipt, publication plan, Worker admission, origin append or public promotion has been produced or executed in this phase. CI `37712122111` and the actual R22 bundle/delivery must be verified by the release lead before proceeding. No new artifact hash is prefilled.

## Required inputs

- Actual clean game checkout at the frozen game revision, actual candidate bytes, signed RELEASE-STATEMENT, signed DELIVERY and passed full decoded DELIVERY-VERIFICATION. Old `npc-map-go` is not used.
- Exact public s16 `latest.json` and `latest.p7s`, respectively SHA256 `fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca` and `1f881c23724b892ab9814eb2eb9002a365df159a7220bb5316538a0ea181a5c7`.
- Old unchanged signed engine and independently hashed bootstrap exe. Engine tooling checkout remains clean at b4390ee4; the closure tool checkout must be clean at `4c60c323aed11829abae6ee5ce6e13c675a3c1c3`, with prepare-native-publication.py SHA256 `da2bfe2c8358fe28aee10396c1b173f7d81e1a998d4a3f2ea024c0e7a9cd5535`.
- Expected-current JSON containing exactly the genuine s16 publication plan's `candidate` pointer, and the existing pinned signing identity. Only the release lead runs signing; this preparation did not read a private key.

## Ordered local procedure

Use actual input paths, fresh output directories under `C:/mir2-playtest-releases/20261008-classic-r22`, and preserve failed attempts. Do not reuse failed output directories.

Before signing the feed, build delivery with the unchanged clean4c60 tooling,
then run `sign-native-delivery-r22-reviewed.ps1` with explicit `GameProjectRoot`
and `VerificationToolProjectRoot`, the actual `CandidateRoot`/`DeliveryRoot`,
frozen `SourceRevision` and existing signer. This retains the original decoded
byte reconstruction and six validation functions, adding independently clean
game7fea/tool4c60/verifier-hash and signed R22 identity checks. Its dedicated
derivation receipt records exact inverse byte recovery,15 retained safety
fragments and11 offline gate cases. Signing still requires the actual completed
Candidate, every original32MiB metadata bound and the original CMS/NoLinks gates.

1. Run `sign-reused-engine-feed-r22-s17.ps1` with mandatory `CandidateRoot`, `GameProjectRoot`, `PreviousFeedRoot`, `EngineRoot`, `OutputRoot`, `SignerThumbprint`. It rechecks clean frozen source, all pinned CMS signers, candidate manifest/version/statement binding, exact predecessor bytes and unchanged engine payload before signing only the new sequence-17 feed.
2. Run `prepare-signed-native-publication-r22-s17.ps1` with `CandidateRoot`, `DeliveryRoot`, `EngineRoot`, `FeedRoot`, `OutputRoot`, `EngineProjectRoot`, `PublicationToolProjectRoot`, `ExpectedCurrent`, `BootstrapExe`, `PythonPath` and the normal `IncludeRootFiles` flag. It performs actual Windows CMS checks, requires the decoded delivery receipt and delegates the complete closure to the pinned committed tool. Publication pointer source remains the reused engine revision; signed game source is checked independently.
3. From mir2-web3 run `python -B scripts/prepare_native_r22_s17_fixed.py --publication-root <actual-proof-directory> --feed-root <actual-signed-feed-directory> --bootstrap-exe <actual-exe>`. It reads exact reviewed R20 template Git blobs at publication base aba54763 (shallow-clone fallback accepts only explicitly pinned unchanged LF/CRLF historical code variants; proof/feed/CMS bytes remain exact) and full-hashes every actual new object. Outputs are create-only R22/s17 files; no s16 reader or object is overwritten. It does not replace actual CMS verification with the generator's own claim.
4. Preserve exact generated plan and fixture bytes in Git: the new fixture directory already has `-text` attributes. Add the single `src/.gitattributes` rule `/native-r22-s17-publication-plan.json -text` in the Worker tree before committing generated admission. The driver verifies the literal length/hash; CRLF conversion of that JSON would correctly reject it. No CMS/proof normalization is permitted.
5. Run the generated Python delivery, interruption and Worker Node suites. Then review `worker-route-reviewed.patch`; `git apply --check` already passes against aba54763. Only the release lead may add the new route after genuine admission generation and all corresponding tests. This phase does not edit Worker index.ts or wrangler configuration.
6. Calculate exact SHA256 of actual PUBLICATION-PLAN.json and ROOT-VERIFICATION.json. Run `python -B scripts/classic-r22-publication/prepare_origin_r22_s17.py --publication-root <actual-proof-directory> --plan-sha256 <actual-plan-sha> --root-proof-sha256 <actual-proof-sha>`. It consumes old R20 closure/proof by pinned bytes, hashes all new objects, and emits only fresh `C:/mir2-playtest-releases/20261008-classic-r22/operators-01` artifacts. Historical old game source paths are inert; they are never used to bypass new source hashing.

Origin output commands are reviewable Linux-root operators for private staging, seal, preflight, immutable append and later feed exchange. They retain root ownership/modes, NOFOLLOW/O_EXCL, inode/mtime checks, source bounds, publisher lock, unchanged old objects and exact s16 feed CAS. Reuse copies from the strict public origin into private root stage; it does not relink or replace old public content. Caddy config and all existing service PIDs must remain unchanged. Do not execute any emitted command during this offline task.

## Current verification

`python -B scripts/test_prepare_native_r22_s17_offline.py -v`: 9/9 GREEN in `C:/mir2-build/classic-r22-offline-06.log`. Tests use real s16 proof bytes; synthetic future render data stays in memory and is never emitted as an admitted plan. Node syntax, template parsing and retained guards passed. Both PowerShell recipes parse with zero errors. Logs 01/02 retain test marker-name mistakes; no production guard was weakened. Full new signed-byte/Worker suites remain pending real R22 artifacts.
