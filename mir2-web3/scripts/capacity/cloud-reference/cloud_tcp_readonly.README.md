# TCP read-only diagnostic companion

The reviewed source and 16 offline tests are integrated on the canonical branch. A separate six-second read-only preflight completed with three real samples and observed remote exit 0. That preflight covered a quiescent server; it supplies no TCP coverage for earlier failed load cases. A new 2/25-player diagnostic uses this companion alongside the unchanged load harness and primary monitor.

The fixed host/user and ED25519 pin match the reviewed cloud monitor V3 principles. A local PTY getpass stays in memory; host key is checked before the one password-auth attempt. Remote uses `sudo -n`, closed stdin, and `python3 -B` (no bytecode writes). No install, payload capture, network/VPN/firewall/power changes or service/client signals. On read timeout/output overflow only this collector's own `ss`/`systemctl show` subprocess may be cancelled; it does not control a service or client. This is a diagnostic, never pass its output as the load harness monitor.

## Input and exact operator CLI

`cloud_tcp_readonly.pins.template.json` is an input template, not a fresh status query or pre-certified activation. Copy it to a NEW public pins file, fill the actual existing V9 status/source SHA256 values and reviewed target identities, and set templateOnly=false and rootReviewedIdentityPins=true after review. No new status read while players are active is required. The template refuses before getpass/network. Runtime independently checks target unit PID/startTicks/InvocationID/health revision and Caddy PID/startTicks/comm before AND after each sample; drift ends the diagnostic. It queries no full environment, argv, state, accounts or ledgers.

Run from the canonical repository root, in a local PTY:

```powershell
python -B mir2-web3/scripts/capacity/cloud-reference/cloud_tcp_readonly.py --pins "$Case/tcp-pins.actual.json" --output "$Case/tcp.latest.json" --history "$Case/tcp.history.jsonl" --stop-file "$Case/tcp-diagnostic.stop" --max-seconds 7200
```

The same command with NEW output/control paths and `--max-seconds 6` is a finite read-only preflight. Known fields not printed by the installed ss/kernel remain null and diagnosticComplete=false, not zero. Thus exit2 with reason duration_completed can denote incomplete optional TCP information; ss_unavailable/read failure is distinct. No fallback or tuning occurs.

## Scope, output and limits

At most one sample per2s; finite1..10800seconds. Fixed read-only commands: `systemctl show mir2-playtest.service --property=MainPID,InvocationID,ActiveState` and `ss -Htinpe state established ( sport = :443 or sport = :7210 or dport = :7210 )`. Only known-Caddy-owned local443 sockets and known TEST/Caddy-owned7210 sockets with BOTH endpoints loopback are exported. Foreign/unowned selected rows are counted, not exported, and diagnostic completeness is false. This is established TCP coverage only, not handshake/FIN/TIME_WAIT/QUIC/UDP coverage.

Selected /proc/net/snmp Tcp and /proc/net/netstat TcpExt counters include retransmissions/timeouts/errors/resets/segments; these network-namespace totals include cohost traffic and cannot be attributed to a game actor. Socket fields include send/receive queue bytes, rtt/rtt variation/rto in milliseconds, explicit retrans outstanding/total, byte and segment counters. Each field is strictly parsed; absent stays null. No zero is inferred from omission. SocketId and flowId are salted HMACs over endpoints held only in remote memory; raw endpoints, ports, names, fds, inode, argv, payloads and environment are never emitted. IDs only correlate within one collector execution, and do not map a TCP connection to an actor or prove a Caddy-public/loopback application pairing.

Limits: ss stdout1MiB,512 exported sockets, per JSON256KiB, total history256MiB (a terminal reserve is retained), stderr64KiB hash-only, each helper<=1.5s, health<=.75s with proxies/redirects disabled. A hard local setup20s/stale12s/duration+6s watchdog closes only the owned SSH resources, including an internally blocked exec call. Only read helpers are spawned; no file is written remotely. stdout raw records are validated fixed projections, not raw ss text; remote errors emit only an allowlisted code.

Only the new tool's own stop-file is read. Local stop/interrupt closes the owned channel; no main-monitor stop-file is written and no service/client fuse is triggered. Remote exit unavailable/-1 remains null with remoteExitStatusReceived=false. Finite normal completion requires observed remote0 for diagnosticComplete=true. CoverageFirstSampledAt/CoverageLastSampledAt are actual samples; coverageFromCampaignStartClaimed is always false, so starting after admission never supplies earlier coverage. Missing samples/fields, read races and a2s cadence limit attribution. Low kernel retransmission counts would not prove an application pipeline is fast.

References for interpretation: [ss(8)](https://man7.org/linux/man-pages/man8/ss.8.html), [Linux kernel SNMP counters](https://docs.kernel.org/networking/snmp_counter.html). No benchmark or real TCP conclusion is produced by these offline tests.

## Offline tests

`python -B -m unittest discover -s <this source directory> -p test_cloud_tcp_readonly.py -v`

16 unique synthetic tests actual0: same shared remote parser, real zero vs missing null, salt/IPv6/loopback role pairing, unknown/foreign owner, malformed fields, counter header pairs, identity drift before/after, command whitelist, wrong host key before auth, sudo stdin empty, credential non-export, unknown exit remains null, missing ss no invented zero. Fake SSH and temporary public fixtures only; no real sockets, private file reads or game loads. Duplicate test runs do not increase the unique count.
