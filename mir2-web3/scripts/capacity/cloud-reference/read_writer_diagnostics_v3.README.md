# Read-only writer and outbound diagnostics

This reader selects existing TEST Gateway journal messages. It changes no
gameplay, protocol, driver, service, route, VPN or timeout. Live operations still
require the coordinator's authorization and fresh explicit activation pins.
Importing the module performs no query.

From the repository root:

```text
python -B mir2-web3/scripts/capacity/cloud-reference/read_writer_diagnostics_v3.py --output <new-public-receipt.json> --pid <PID> --start-ticks <ticks> --invocation-id <32hex> --revision <40hex> --since <UTC-start> --until <UTC-end>
```

Use a new output file in an existing public evidence directory outside the
checkout. The local terminal must support the memory-only password prompt.
The fixed ED25519 host key is verified before password authentication; remote
sudo is noninteractive and receives closed stdin. The target is the existing
TEST service, never another Gateway, Zone, proxy or VPN service.

The UTC window must be positive and at most 60 seconds. Remote query work stays
within 20 seconds, local wall time within 60 seconds, total stream within 8 MiB,
and selected journal rows within 12,000. Reaching a bound is explicit incomplete
or failed evidence. PID, InvocationID, process start ticks and source health are
verified before and after. Output creation is exclusive. Actual remote exit
status, framing, byte counts and hashes are preserved; raw stdout/stderr are not.

## Existing log families

`diagnosticEvents` retain the literal prefix and existing stage:

- `[gateway-slow-stage]`: the original Tick/owner/queue labels plus existing
  session execution/flush, response/external/save flush, Tick flush/checkpoint/
  gate, input-gate/turn and snapshot-build/send labels.
- `[gateway-response-write]`: counts and original serialization, writer-lock,
  feed, flush and elapsed microseconds. The production aggregate has no stage
  token or socket identifier; projected stage is null.
- `[gateway-zone-outbound-block]`: existing read-gate, health-validation, send
  and completion-to-pop spans, with anonymous registration/batch context.
- `[gateway-zone-delivery-trace]`: existing enqueue/dequeue/gate/send/sent and
  terminal stages, with anonymous server trace/registration context.

No stage name, socket ID, actor or public-flow binding is invented. Coordinates,
object IDs and position indicators are redacted. Unknown payload/account/wallet/
cookie tokens never enter the result and mark the projection invalid. Required
missing metrics stay null; invalid numbers, duplicates and unknown enums remain
explicit. Fields irrelevant to a log family are omitted.

`stageCounts` and `maximumDurationMsByStage` remain slow-stage-only.
`prefixCounts`, `stageCountsByPrefix` and `maximumElapsedUsByPrefix` keep the
families and their source units separate.

## Diagnostic enablement observations

A bounded in-memory read of only the verified PID's environ exports two
predicates: `zoneDeliveryTraceLiteral1` and `slowStagePositiveU64`, together with
Boolean `readComplete`. No environment values, threshold, recipient selector,
credential or URL is persisted. Missing flags produce false; unreadable,
oversized or duplicate watched keys produce unknown/null values. Flag-read
completeness is separate from journal completeness. Literal1 does not prove an
optional recipient selector included a particular owner.

Historical operator observation, **2026-11-12 17:56:20–17:56:40 UTC**
(2026-11-13 01:56:20–01:56:40 Asia/Shanghai): a read-only query of then-pinned
TEST PID 592567, source `7033a012bf39c3d8bd742874c3273ff9eacfd422`, reported
readComplete=true, zoneDeliveryTraceLiteral1=false and slowStagePositiveU64=true.
That dated receipt describes one activation/window; it is not a current
configuration guarantee or evidence that the trace flag should be enabled.

## Interpretation and offline checks

Live owner packets use the single-send path, so response-batch metrics do not
split their encoding, writer-lock, readiness/feed or flush time. Existing zone
send spans combine those phases. Successful sink flush does not prove Caddy
forwarding, kernel transmission, TCP ACK or remote WebSocket receipt. Missing
logs do not establish a short duration, enabled tracing or actor binding.

From this script directory:

```text
python -B -m unittest -v test_read_writer_diagnostics_v3.py
```

The 27 offline tests use only in-memory SSH/HTTP/proc fixtures. They cover
redaction, malformed metrics/frames, flag privacy, identity/budget preservation,
pre-authentication host-key rejection and exclusive output. The parent fixture
is the canonical sibling `read_tick_diagnostics_v2.py`, whose pinned SHA256 is
`c477d83fdc11c781b3e003e89b24ff54f21128712e178892b3a05b2e20de5dd7`.
