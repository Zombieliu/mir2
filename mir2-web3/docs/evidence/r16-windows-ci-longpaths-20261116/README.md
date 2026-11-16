# Windows CI checkout long paths

Source parent: `6ba38557ddb298df72cb9e4a23c48caf542a598e`.

The preceding Windows Candidate and Developer Handoff jobs stopped during
checkout, before their game checks. A recorded failing file path was reproduced
with Git for Windows 2.50.1 in a separate one-file repository: the 320-character
absolute path failed with `core.longpaths=false` (exit 128), then the same commit
checked out cleanly after `git config --global core.longpaths true`. Its 44-byte
contents matched SHA-256 `41254b410a99383aa6946a9fb339779622fbff4de9b2e97f68303e840cd21d3d`.
The reproduction used its own `GIT_CONFIG_GLOBAL`; the developer's global config
was unchanged. See [checkout receipt](checkout-receipt.json).

Five Windows jobs across four workflows now enable this setting before checkout.
The matrix job conditions it on Windows. The step runs in `runner.temp`, which
already exists when Home Agent's default `mir2-web3` directory does not yet exist.
YAML parsing and comparison with the parent confirm that all existing steps,
permissions, conditions and gates are preserved; the 29 recorded unrelated WIP
hashes remain unchanged. See [configuration verification](workflow-verification.json).

This follows [Git for Windows' documented long-path setting](https://gitforwindows.org/git-cannot-create-a-file-or-directory-with-a-long-path.html).
The pinned [checkout action copies the existing global configuration](https://github.com/actions/checkout/blob/3d3c42e5aac5ba805825da76410c181273ba90b1/src/git-auth-helper.ts#L76-L114)
before using its temporary configuration.

Hosted follow-up: all four applicable Windows checkout jobs passed the new
configuration and actual checkout on `f16f42c9ea`; the protected main-branch
publication job did not run. See [hosted results](hosted-checkout.json). The
Windows host job then reported existing Rust formatting differences; those
checks remain open and are not skipped. This checkout repair does not certify
the full Candidate gate or modify gameplay.
