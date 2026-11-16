# Local OpenAI model settings

This standalone Windows helper belongs beside `Numeron-Alo.exe` as
`Numeron-Alo-Settings.exe`. Opening and saving its GUI makes no API request.
The API-key edit control uses the native Windows password style. A blank key
preserves an existing valid credential; closing the window preserves settings.

Credentials use the current Windows user's Generic Credential Manager store,
with local-machine persistence for that user. The target is
`NumeronAlo/OpenAI/v1/<sha256>`: canonicalize the executable's parent directory,
normalize separators, remove the extended Windows path prefix (preserving UNC),
lowercase, trim trailing separators, and hash its UTF-8 bytes with SHA-256.
Moving a bundle requires entering its key again; a mismatched copied metadata
file fails closed. There is no command-line target override.

The only persisted application metadata is
`apps/ai-player/.data/local-playtest/openai.local.json` under that bundle:

```json
{
  "schemaVersion": 1,
  "provider": "openai",
  "enabled": true,
  "model": "gpt-6-luna",
  "credentialTarget": "NumeronAlo/OpenAI/v1/<bundle-path-sha256>",
  "maxCallsPerHour": 20
}
```

`--read-key` verifies the bundle-bound metadata
and emits `{credentialTarget,apiKey}` only to a pipe. Other arguments, missing
credentials, file/console output handles, and read failures produce fixed error
text and a nonzero exit. The Node loader captures that pipe with a five-second
timeout and an 8 KiB output limit. It never forwards Codex, AI Gateway, or OpenAI
credentials in the helper's environment.

`--configure-stdin` is the private configuration mode for an explicitly
authorized local caller. It accepts only an actual OS stdin pipe, reads at most
8192 bytes, and accepts exactly a JSON object with `model` and `apiKey` strings.
The key must be nonempty and valid; the GUI's empty-key preservation behavior is
unchanged. Console/file/NUL input handles, unknown or duplicate fields, and
target/root/path arguments fail with fixed stderr text. The caller must close
the input pipe after sending its bounded JSON. Never put a key in arguments,
environment variables for this helper, temporary files, or GUI automation.

Both modes derive the same credential target solely from the executable's
parent directory. Configuration reuses the GUI's prepare-metadata, credential
write, atomic-replace and rollback sequence. Raw input and the decoded key use
zeroing secret containers with no Debug/Serialize implementations. Successful
configuration returns only `{"saved":true,"model":"gpt-6-luna"}`; it does not
return the credential or make an OpenAI request. The model remains editable.

`loadLocalModelSettings({bundleDir,env})` reads local metadata first. With no
metadata it returns rules, or accepts only an explicitly supplied
`env.OPENAI_API_KEY` with the default model. It does not inspect other credential
variables. The returned API key is memory-only: callers must pass it directly to
the model client and must not serialize, log, persist, or send it to the game.
The runtime enforces the returned twenty-attempt hourly limit; this settings
loader does not perform model calls or maintain a usage counter.

This boundary follows [OpenAI's API authentication guidance](https://developers.openai.com/api/reference/overview#authentication):
keep API credentials secret and load them in the trusted application rather
than embedding them in game/browser messages or source.

From the repository's `mir2-web3` directory:

```powershell
cargo +1.95.0 test --locked --offline --manifest-path apps/ai-player/model-settings/Cargo.toml --target-dir F:/mir2-builds/alo-model-settings
cargo +1.95.0 build --release --locked --offline --manifest-path apps/ai-player/model-settings/Cargo.toml --target-dir F:/mir2-builds/alo-model-settings
node --test apps/ai-player/tests/local-model-settings.test.mjs
```

The Windows credential roundtrip test creates a unique temporary bundle target,
uses a synthetic nonfunctional key, and deletes that credential afterward.
It does not access a user's installed bundle or call OpenAI.
