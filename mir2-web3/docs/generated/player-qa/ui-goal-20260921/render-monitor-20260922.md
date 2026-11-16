# Native running presentation diagnostics — 2026-09-22

Opt-in instrumentation adds evidence for running stutter and pullback. It does
not declare either defect fixed or visual acceptance complete.

## Recorded evidence

- `MIR2_NATIVE_RENDER_TRACE_PATH` enables bounded JSONL recording; default off.
- Main-world `Last` captures propagated camera/body transforms, committed actor
  root and applied map/entity centers. Units are stage pixels, not framebuffer
  pixels. This is CPU presentation state, not GPU execution or physical display.
- Frame intervals flag >=33/50/100 ms. Applied-center disagreement and >4 px
  screen-anchor drift are recorded with boundary suppression.
- Movement markers correlate actual world-view motion with one command segment.
  Reverse motion >4 px is a candidate, including when camera and player move
  together. Turn, correction, degraded run, focus, map boundary, dropped records
  and new commands reset the comparison. Confirmed late ACKs remain observable.
- Map/entity sync, map cache-miss loading/parsing and entity PNG decoding provide
  CPU timing context; input/ACK markers share the trace.
- Anomaly capture retains up to 10 seconds / 1,200 preceding frames and a fixed
  two-second tail. Ten-second health summaries cover uncaptured frames too.
- Nonblocking queue capacity 2,048, recorded drop count, background I/O, exclusive
  writer and 256 MiB file budget bound diagnostic cost. Startup/focus changes and
  record drops must be considered when interpreting evidence.

## Validation

- Runtime full suite: 260/260.
- Windows full suite: 696/696.
- Final focused diagnostics suite (including degraded-run reset): 9/9.
- Streaming Node summary test: 1/1; launcher PowerShell syntax validated.
- Logs: `C:/mir2-ui-repair-20260921/render-diagnostics-budget-runtime-full.log`,
  `render-diagnostics-windows-full.log`, `render-diagnostics-release-build.log`.

No newly instrumented live running trace has been accepted yet. Existing running
client PID 42684 belongs to the previous package and cannot gain telemetry by
changing environment variables after launch. Do not kill it to deploy.

## Normal-player collection

After normal logout and closing the previous client, use the platform-windows
`scripts/Start-RenderDiagnosticClient.ps1` with `-PackageDirectory` pointing to
the new package and `-LogDirectory C:/mir2-ui-repair-20260921/render-live`.
The launcher refuses an already-running client and enables movement plus render
traces only in the new process. It does not change saves, levels or server rules.

Run `node scripts/summarize-render-trace.mjs <render.jsonl>` from
platform-windows. Compare anomaly frames, CPU stages and movement outcomes at the
same timestamps. Captured frames are selected by anomalies; do not calculate
whole-session frame rates from them. Health summaries are separate. Screen/video
review and GPU-present measurements remain separate acceptance evidence.

## Windows startup correction

The first live startup rejected the append-only file handle at `try_lock`; the
trace remained empty. Added read access (required by Windows file locking) and
a real-filesystem writer regression. Focused diagnostics now pass 10/10, including
opening, locking, writing, flushing and closing the actual Windows trace file.
The failed startup is retained in `render-live/20260922-193711-015-client.stderr.log`.
