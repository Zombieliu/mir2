# Bounded cloud inventory

`read_inventory.py --output <new C evidence JSON>` performs one read-only query
against the explicitly authorized TEST host. Start it in the credential owner's
PTY. It restricts SSH to ED25519 and verifies the pinned `6M4…` fingerprint before
showing a memory-only getpass prompt. Echo fallback is an error. There is one
authentication candidate per invocation; no credential enters argv/files/logs or
sudo stdin. The coordinator controls the maximum of two authorized candidates.

The authenticated command is `sudo -n` with closed stdin and a 20-second remote
query budget. A 60-second local query watchdog closes only this SSH connection.
Only `systemctl show`, `/proc`, cgroup kernel files and one loopback health GET are
read. The target PID/invocation/start tick/cgroup are discovered freshly and checked
again at the end. Historical PIDs are presence observations, never identity pins.

Output contains host CPU/memory, top RSS PID/comm, executable SHA, cgroup effective
limits/pressure/events, health revision and strictly projected numeric/enum config.
Topology/config-document environment values are presence booleans only. Missing
or invalid fields remain unknown; configured values are not effective defaults.
No argv, raw environment, stderr, unframed stdout or credentials are persisted.
There are no service, package, route, firewall, VPN, update or load operations.

Every output is exclusive. An aborted credential prompt is not an authentication
attempt or an inventory result. Preserve such records and use a new filename.
Some execution hosts isolate PTY registries per agent: in that case the credential
owner must launch and feed its own PTY; credentials should not be relayed in chat.
