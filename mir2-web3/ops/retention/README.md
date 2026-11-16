# Server retention policy

These files define the reviewed storage policy and metadata collector. Journal
limits and the daily metadata timer were installed on test host 165.154.65.136
on 2026-10-30; see the [deployment and evidence](../../docs/SERVER-RETENTION-20261030.zh-CN.md).
Stage, public-release and backup pruning are not scheduled. The collector only
retains its own metadata reports. Another host requires a separate review and
installation; copying these repository files performs no deployment. The
machine-readable rules are in [policy.json](policy.json).

## Staging directories

Use [server_retention_cleanup.py](../../scripts/server_retention_cleanup.py) only
for individually approved, exact staging directories in its fixed allowlist.
`--inspect` reads metadata and emits a tree fingerprint, size and entry count;
it does not inspect file contents or independently establish absence of global
references. The default plan invocation checks without deleting. Deletion needs
`--execute` and `--expected-sha256` matching the complete raw plan bytes.

Before each batch, complete a fresh reference audit, resolve every read gap,
exclude every candidate writer, and approve the exact candidates and plan hash.
The plan must be at most 120 seconds old. The newest mtime or ctime anywhere in
each candidate must be at least 48 hours old. The engine checks every target
before the first deletion and revalidates each target before deletion. Stop on
any failure and preserve the partial result; do not blindly retry a failed plan.
The fingerprint covers metadata rather than file contents. Deletion is not an
atomic transaction against concurrent writers, and its final stdout receipt is
not a durable per-entry journal for process termination or host failure.

## Public releases and backups

Keep **all public releases and all backups** in this phase. No prune is enabled.
Future public pruning must retain every channel's current release and at least
two validated rollback releases, including their complete transitive dependency
closures. Shared objects stay while any retained release or live consumer needs
them; age alone cannot authorize their deletion.

Daily 7, weekly 4 and monthly 3 are future backup retention upper-bound plans,
not authorization to delete existing backups. Automatic pruning remains false
until a coherent PostgreSQL/state/recovery/release-pair backup, protected key
material, verified off-host copies and a successful documented restore exist,
followed by explicit operator approval of an implementation.

## Journal deployment and rollback

[journald.conf](journald.conf) proposes a drop-in at
`/etc/systemd/journald.conf.d/90-mir2-retention.conf`. Preserve any existing file
at that exact path before installation, review other journal overrides and the
installed systemd version, then have the operator install and activate the
reviewed drop-in. Verify the effective settings and `journalctl --disk-usage`
after activation. No journal operation is performed by these repository files.

The proposed limits are 512 MiB persistent journal use, 3 GiB space to keep
free, 64 MiB journal files, 64 MiB runtime journal use, 14-day retention and
compression. These are journal limits, not a guarantee that the entire disk
stays below a threshold. Journal retention removes archived files; active files
and other applications can keep usage above a configured limit. Existing
rsyslog rotation is to remain weekly with four retained rotations; verify its
host configuration before reporting it as active.

Rollback restores the prior exact drop-in, or removes only this new drop-in if
none existed, then reactivates and verifies the previous journal configuration.
Rollback does not recover logs already removed under an active retention rule.
See the official [journald configuration reference](https://www.freedesktop.org/software/systemd/man/latest/journald.conf.html)
and [journalctl reference](https://www.freedesktop.org/software/systemd/man/latest/journalctl.html).

## Metadata audits and disk alerts

[daily_audit.py](daily_audit.py) collects root filesystem `statvfs`, existence of
the configured protected directories, `systemctl is-active` for four configured
live units, and metadata of the six configured local feed files. It reads no
environment, feed contents or database data and makes no service changes.
`--skip-feed-metadata` omits the optional feed metadata checks. Disk warning or
critical is recorded with exit zero; collection or storage errors exit nonzero.

The script holds a nonblocking lock in `/var/lib/mir2-retention-audit` (root-owned
0700), exclusively publishes `audit-YYYYMMDDTHHMMSSZ.json` files (0600), and keeps
the most recent 14 reports of at most 5 MiB each. It prunes only matching,
single-link, root-owned regular files with its own JSON producer marker and
matching timestamp. It refuses symlinks and directories, never recurses, and
leaves unrelated regular files alone. Failure can leave a newly published audit
or partial metadata-file pruning; it reports the number removed. It never prunes
staging files, logs, player data, releases, feeds or backups.

After reviewing the script and running [test_daily_audit.py](test_daily_audit.py),
the operator may install the reviewed script at
`/usr/local/lib/mir2-retention/daily_audit.py` and the
[service](mir2-retention-audit.service) and [timer](mir2-retention-audit.timer)
templates in the system unit directory. The oneshot service uses root, a private
state directory, a strict system sandbox with that state directory explicitly
writable, and low CPU/I/O priority. The timer runs daily 03:20 UTC plus up to
five minutes of random delay, with persistent catch-up. Verify the installed
units, trigger one audited run, inspect its report and exit status, then explicitly
enable the timer. This test-host installation completed those gates; its timer
is enabled, and the initial report is preserved in the deployment evidence.

Rollback stops and disables only this new timer, then removes or restores only
its exact service/timer/script files and reloads the unit manager. Preserve the
private audit directory and reports for evidence; unrelated services are outside
the rollback scope.

Warn when available disk space is at most 20% **or** 10 GiB; mark critical at
at most 10% **or** 5 GiB, with critical taking precedence. These are alert and
operator-review thresholds, never permission for automatic deletion. Reports
and status are stored locally in JSON and the system journal; external alert
delivery is not configured. Current free space may still report warning under
the 20% threshold even after successful recovery.
