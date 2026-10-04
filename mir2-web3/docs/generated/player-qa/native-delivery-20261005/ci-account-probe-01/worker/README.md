# Fixed account-token authority probe: frozen external patch

This directory is an independent sibling of the unchanged user-token probe. The production diff is exactly two lines: the docstring identifies account-token verification and VERIFY_PATH uses the fixed account-scoped endpoint after the fixed ACCOUNT definition. There is no token-type selection, user-token fallback, new input key, new receipt key or permission request.

The official Cloudflare account-token [Verify Token API](https://developers.cloudflare.com/api/resources/accounts/subresources/tokens/methods/verify/) supplies the account-scoped endpoint and result.status shape. The bucket GET, TLS certificate/hostname checks, direct fixed host:443 connection, no redirects/proxy/retries, bounded 65KiB read, redaction, CLI exit rules, fresh-only receipt and all not_probed write authority fields remain unchanged.

`SOURCE-AND-TEST.patch` is the complete source/test diff from exact original frozen bytes. BASELINE files are byte-exact original copies. Original 31 test bodies are byte-exact; one added test pins both endpoint literals, verifies only two GETs, proves an account-verification 401 stops before any fallback/bucket request, and proves the old user endpoint is rejected before HTTPS construction.

Local Python 3.13 fake-env/transport results:

- RED: 32 tests, 31 pass, one genuine literal-endpoint failure on unchanged source. Raw log and source/test snapshots retained.
- GREEN: 32 tests, 32 pass, zero errors/skips. All negative redaction/TLS/status/JSON/path/receipt/close guards are retained.
- No mechanical preparation failure occurred in this sibling round. The older frozen probe retains its own qualified preparation and close-failure evidence; it was not edited.

All 65 objects in the original freeze manifest plus its final receipt were reverified unchanged. Only this new external directory was written. No real environment lookup, credential use, authenticated network request, CI dispatch, Git/repository edit, service/deployment operation or player-store action occurred. Public official documentation browsing was read-only.

The earlier real user-token-endpoint HTTP401 does not establish credential expiry or identify the token type. This round used only fake 401 fixtures. A future account endpoint response belongs to root's separate real CI receipt. If both fixed endpoint classes later return 401, describe exactly that evidence; do not claim expiry. R2 upload, Worker edit and deployment authority remain unprobed even if these two GETs succeed.

For root review/integration use the final source/test hashes and strict receipt contract, then a separately authorized opaque CI invocation. The worker has not validated any actual token. Require exit0 and the complete source-pinned receipt together; an interrupted/failed write must never be accepted by reading partial receipt bytes alone. Native/publication success is not implied.

The offline test command is recorded in COMMANDS.json. A rerun retains new fixture objects and should use a new evidence directory rather than modify this frozen directory.
