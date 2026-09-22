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
when the preference is disabled. The native release build/package is recorded
at handoff. Native visual verification remains separate.

Logs: `C:/mir2-ui-repair-20260921/daylight-lighting-tests.log`,
`daylight-config-tests.log`, and `daylight-client-build.log`.
