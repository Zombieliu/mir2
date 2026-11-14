# Development client defaults to daylight

At the user's request, the Windows development client now defaults to a fixed
daytime display. This is a local display preference, independent of server
time and character state. The configuration is:

```toml
[display]
force_daylight = true
```

An omitted setting also defaults to true for these development launches.
Set it to false and restart the client to restore the original server/map
lighting, including dawn/evening, night and dark cave overrides. This override
must be disabled for original Crystal lighting acceptance.

Native startup latches the preference before starting the gateway connection.
The per-connection lighting bridge preserves it over scene reset, logout and
reconnect generation changes, while retaining received source light fields.
Only the generated lighting/HUD output is projected to Day (2), map darkness
zero and the sun icon. No per-frame filesystem polling or server commands
are added. Other render layers and the movement-cadence repair remain active.

Verification: lighting regressions 14/14 and session-config regressions 11/11
pass. The new lifecycle regression covers server Night, dark map override,
scene reset, normal logout, reconnect, and restoration of source lighting
when the preference is disabled. The native release build passes. Native visual
verification remains separate.

Logs: `C:/mir2-ui-repair-20260921/daylight-lighting-tests.log`,
`daylight-config-tests.log`, and `daylight-client-build.log`.

Prepared package: `C:/numeron-legend-of-rebirth-20260922-daylight`.
Client source `e3d32fa9fed3069024bdf822f95ca751453ebdf4`, SHA256
`2575F495C1C4EF3CE1A7F1BDB73FD89C9E1F9F0930BD354E4E5727B3BF24D53D`.
Its config explicitly sets `force_daylight = true`, SHA256
`01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.
Gateway source remains `1b60931f7` with the movement-deadline repair; no server
replacement is required for this display setting. After the user confirmed
normal logout/close, native PID44840 launched from this package at 23:01
Asia/Shanghai. Startup reports `force_daylight=true` and connects to the
existing Gateway. Movement/render diagnostics use prefix
`C:/mir2-ui-repair-20260921/render-live/20260922-230135-354`.
Assets remain a shared junction. User visual testing is pending;
`visualAccepted=false`.
