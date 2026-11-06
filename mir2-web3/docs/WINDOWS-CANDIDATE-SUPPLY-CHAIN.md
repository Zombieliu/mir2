# Windows Candidate Supply-Chain Gate

This document defines the fail-closed publication boundary implemented by
`.github/workflows/cross-platform-client.yml`. It covers workflow artifacts;
the formal Authenticode/CMS package-signing gate remains separate.

## Repository controls

- Every external Action in the workflow is pinned to a full 40-character
  commit SHA. Major versions are comments only.
- Pull requests produce only
  `mir2-windows-x86_64-unpublished-build`. They cannot enter the Candidate
  publication job.
- `windows-candidate-publish` runs only from `refs/heads/main`, after the
  Windows build job, and requires the `windows-candidate-publisher` GitHub
  Environment.
- The downloaded EXE must match `mir2.windows.build-attestation.v2`, the exact
  source revision, a clean worktree, and a locked Cargo build.
- The release ZIP and `SUPPLY-CHAIN-PROVENANCE.json` both receive GitHub SLSA
  provenance attestations. Both are verified with `gh attestation verify`
  before the Candidate artifact upload step becomes reachable.

## Protected publisher configuration

Create a GitHub Environment named `windows-candidate-publisher` and configure:

1. Deployment branch restriction: `main` only.
2. Required reviewers and prevention of self-review where supported.
3. Environment secret `MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID`, set to the exact
   positive numeric GitHub actor ID of the dedicated release publisher.

There is intentionally no default or repository-variable fallback. The value
must exactly equal `${{ github.actor_id }}`. Missing, malformed, or mismatched
configuration fails before packaging, attestation, or upload.

Artifact attestations must be available for the repository plan. If GitHub's
attestation service is unavailable or verification cannot retrieve both
subjects after the bounded retries, publication fails closed.

## Local static gates

Run from the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File mir2-web3/apps/game-client/platform-windows/scripts/verify-windows-publisher-contract.ps1 -SelfTest
powershell -NoProfile -ExecutionPolicy Bypass -File mir2-web3/apps/game-client/platform-windows/scripts/test-windows-candidate-supply-chain.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File mir2-web3/apps/game-client/platform-windows/scripts/build-attested-windows-candidate.ps1 -SelfTest
git diff --check
```

The publisher self-test covers a valid exact match and rejects missing,
non-numeric, and mismatched identities. The supply-chain test rejects mutable
Action tags, missing pins, a non-protected publisher source, and any ordering
other than package -> attest -> verify -> upload.

## External acceptance still required

- Configure the protected Environment and publisher UID.
- Run the hosted workflow from `main` and retain the successful attestation
  verification evidence.
- Require the `Attested Windows Candidate publication gate` job in release
  policy before calling its output publishable.
- Provision the existing external code-signing certificate requirement before
  calling the formal package signed; this workflow hardening does not create,
  read, or use that private key.
