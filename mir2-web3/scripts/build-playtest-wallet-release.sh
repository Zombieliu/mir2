#!/usr/bin/env bash
# Native Linux CI only. This script never connects to a game server, reuses a
# database, starts containers, or publishes a release outside the CI artifact.
set -euo pipefail
umask 077

task_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_root"
task_toolchain=1.95.0
task_revision="$(git rev-parse HEAD)"
task_release_tag="${INPUT_RELEASE_TAG:-playtest-wallet-${task_revision:0:12}}"
task_output="$task_root/dist/playtest-wallet-release"
task_evidence="$task_output/evidence"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

[[ "$(uname -s)" == Linux ]] || fail 'playtest release verification requires native Linux'
[[ "$EUID" -ne 0 ]] || fail 'playtest release verification requires a non-root publisher'
[[ "$task_revision" =~ ^[0-9a-f]{40}$ ]] || fail 'source revision must be a full Git SHA'
[[ "${GITHUB_SHA:-$task_revision}" == "$task_revision" ]] || fail 'checkout differs from the authenticated CI revision'
[[ "$task_release_tag" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] || fail 'release tag must be one safe path component'
git diff --quiet --ignore-submodules -- || fail 'tracked source is dirty'
git diff --cached --quiet --ignore-submodules -- || fail 'source index is dirty'
mkdir -p "$task_evidence"

# Git archive applies the current repository prefix even to a subtree object.
# Run from the repository root, and inspect maps before the expensive builds.
task_repository_root="$(git rev-parse --show-toplevel)"
task_maps="$task_output/mir2-crystal-map-pack-$task_revision.tar.gz"
git -C "$task_repository_root" archive --format=tar.gz --prefix=crystal-map-pack/ "$task_revision:mir2-web3/apps/web/lib/generated/crystal-map-pack" >"$task_maps"
python3 - "$task_maps" <<'PY'
import pathlib
import sys
import tarfile
with tarfile.open(sys.argv[1], 'r:gz') as bundle:
    files = [member for member in bundle.getmembers() if not member.isdir()]
    if not files or not any(member.name.endswith('.map.gz') for member in files):
        raise SystemExit('authenticated map tree has no collision maps')
    for member in files:
        path = pathlib.PurePosixPath(member.name)
        if not member.isfile() or not path.is_relative_to('crystal-map-pack') or '..' in path.parts:
            raise SystemExit('map package contains an unsafe member')
print(f'PASS: authenticated map archive contains {len(files)} files')
PY

task_pg_bin=''
for task_candidate in /usr/lib/postgresql/*/bin; do
  if [[ -x "$task_candidate/initdb" && -x "$task_candidate/pg_ctl" && -x "$task_candidate/psql" ]]; then
    task_pg_bin="$task_candidate"
  fi
done
[[ -n "$task_pg_bin" ]] || fail 'native initdb, pg_ctl and psql must be installed'
task_pg_root="$(mktemp -d "${RUNNER_TEMP:-/tmp}/mir2-playtest-wallet-pg.XXXXXXXX")"
task_pg_data="$task_pg_root/data"
task_pg_socket="$task_pg_root/socket"
mkdir -m 0700 "$task_pg_socket"
task_pg_started=0
cleanup() {
  local task_exit="$?"
  trap - EXIT
  if [[ "$task_pg_started" == 1 ]]; then
    if ! "$task_pg_bin/pg_ctl" -D "$task_pg_data" -m fast -w -t 30 stop >"$task_evidence/postgres-stop.log" 2>&1; then
      printf 'FAIL: temporary PostgreSQL did not stop cleanly\n' >&2
      task_exit=1
    fi
  fi
  # The cluster belongs to this ephemeral CI runner. Keep its private directory
  # for failure diagnosis; it is never included in an uploaded artifact.
  exit "$task_exit"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

task_pg_port="$(python3 - <<'PY'
import socket
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    print(listener.getsockname()[1])
PY
)"
[[ "$task_pg_port" =~ ^[0-9]+$ ]] || fail 'could not reserve an isolated PostgreSQL port'
# Trust is limited to a new, private, loopback-only CI cluster. These roles and
# databases are generated here and never point at a developer or player store.
"$task_pg_bin/initdb" -D "$task_pg_data" --username=mir2_ci_admin --auth-local=trust --auth-host=trust --encoding=UTF8 --no-locale >"$task_evidence/postgres-init.log" 2>&1
task_pg_started=1
"$task_pg_bin/pg_ctl" -D "$task_pg_data" -l "$task_evidence/postgres.log" -o "-h 127.0.0.1 -p $task_pg_port -k $task_pg_socket -c max_connections=40" -w -t 30 start
export PGHOST=127.0.0.1 PGPORT="$task_pg_port" PGUSER=mir2_ci_admin PGDATABASE=postgres
unset PGOPTIONS PGSERVICE PGSERVICEFILE PGPASSWORD PGPASSFILE
task_stamp="$(date -u +%Y%m%d%H%M%S)_$$"
for task_domain in monthly billing activities startup; do
  task_role="${task_domain}_qa"
  task_database="mir2_${task_domain}_qa_${task_stamp}"
  "$task_pg_bin/createuser" --no-superuser --no-createdb --no-createrole --no-replication "$task_role"
  "$task_pg_bin/createdb" --owner="$task_role" "$task_database"
  "$task_pg_bin/psql" -X -v ON_ERROR_STOP=1 -q -c "REVOKE ALL ON DATABASE $task_database FROM PUBLIC;"
  "$task_pg_bin/psql" -X -v ON_ERROR_STOP=1 -At -d "$task_database" -c "SELECT current_database(), '$task_role', (SELECT rolsuper FROM pg_roles WHERE rolname = '$task_role');" >>"$task_evidence/postgres-isolation.log"
  task_uri="postgres://$task_role@127.0.0.1:$task_pg_port/$task_database"
  case "$task_domain" in
    monthly) export MIR2_MONTHLY_CARD_TEST_DATABASE_URL="$task_uri" MIR2_MONTHLY_CARD_TEST_ISOLATED=1 ;;
    billing) export MIR2_BILLING_TEST_DATABASE_URL="$task_uri" MIR2_BILLING_TEST_ISOLATED=1 ;;
    activities) export MIR2_ACTIVITY_TEST_DATABASE_URL="$task_uri" MIR2_ACTIVITY_TEST_ISOLATED=1 ;;
    startup) export MIR2_STARTUP_TEST_DATABASE_URL="$task_uri" ;;
  esac
done
unset task_uri
export MIR2_STRIPE_ENABLED=0 RUST_TEST_THREADS=1
task_test_args=(+"$task_toolchain" --config 'profile.test.opt-level=1' --config 'profile.test.debug=0' --config 'profile.test.package.argon2.opt-level=3' --config 'profile.test.package.blake2.opt-level=3' test --locked -p mir2-simulation)
cargo "${task_test_args[@]}" --lib schema_union -- --test-threads=1 2>&1 | tee "$task_evidence/schema-union.log"
grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed;' "$task_evidence/schema-union.log" || fail 'schema union did not execute any passing tests'
for task_test in monthly_card_postgres billing_postgres activities_postgres; do
  cargo "${task_test_args[@]}" --test "$task_test" -- --ignored --test-threads=1 2>&1 | tee "$task_evidence/$task_test.log"
  grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed;' "$task_evidence/$task_test.log" || fail "$task_test did not execute any passing tests"
done

MIR2_REQUIRE_LINUX_SECURITY_GATE=1 bash scripts/selftest-gateway-save-recovery.sh 2>&1 | tee "$task_evidence/linux-release-security.log"
grep -q '^PASS:' "$task_evidence/linux-release-security.log" || fail 'Linux security gate did not attest success'

(
  # The secure packager requires its staged README.txt to be exactly 0644.
  # Its private stage stays 0700; keep PostgreSQL/evidence creation at 0077 in
  # the parent while giving only the packager its expected creation mask.
  umask 022
  MIR2_RELEASE_PUBLISHER_UID="$(id -u)" \
  MIR2_RUST_TOOLCHAIN="$task_toolchain" \
  MIR2_RELEASE_REQUIRE_CLEAN=1 \
  MIR2_RELEASE_TAG="$task_release_tag" \
  bash scripts/package-gateway-release.sh
) 2>&1 | tee "$task_evidence/gateway-package.log"

# The library tests do not execute async_main. Exercise the packaged release
# with real isolated PostgreSQL and Redis while both recovery features are off.
# GitHub's ephemeral Ubuntu image may omit the native Redis executable.
if ! command -v redis-server >/dev/null 2>&1; then
  sudo -n apt-get update -qq
  sudo -n apt-get install --no-install-recommends -y redis-server
fi
python3 scripts/verify-playtest-postgres-startup.py \
  --binary "$task_root/target/release/mir2-gateway" \
  --maps "$task_root/apps/web/lib/generated/crystal-map-pack" \
  --psql "$task_pg_bin/psql" \
  --redis-binary "$(command -v redis-server)" \
  --revision "$task_revision" \
  --evidence "$task_evidence" 2>&1 | tee "$task_evidence/gateway-postgres-startup.log"

# Archive exactly the authenticated Git tree, rather than an untracked local
# map directory. No archive member is extracted or executed here.
python3 - "$task_revision" "$task_maps" "$task_output" "$task_release_tag" <<'PY'
import hashlib
import json
import pathlib
import sys
import tarfile

revision, archive_name, output_name, release_tag = sys.argv[1:]
archive = pathlib.Path(archive_name)
output = pathlib.Path(output_name)
files = []
with tarfile.open(archive, 'r:gz') as bundle:
    for member in bundle.getmembers():
        if member.isdir():
            continue
        path = pathlib.PurePosixPath(member.name)
        if not member.isfile() or not path.is_relative_to('crystal-map-pack') or '..' in path.parts:
            raise SystemExit('map package contains an unsafe member')
        stream = bundle.extractfile(member)
        if stream is None:
            raise SystemExit('map package member is unreadable')
        data = stream.read()
        if len(data) != member.size:
            raise SystemExit('map package member length mismatch')
        files.append({'path': member.name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
if not files or not any(entry['path'].endswith('.map.gz') for entry in files):
    raise SystemExit('authenticated map tree has no collision maps')
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_name(archive.name + '.sha256').write_text(f'{digest}  {archive.name}\n', encoding='utf-8')
manifest = {
    'sourceRevision': revision,
    'sourceTree': f'{revision}:mir2-web3/apps/web/lib/generated/crystal-map-pack',
    'archive': archive.name,
    'archiveSha256': digest,
    'archiveBytes': archive.stat().st_size,
    'fileCount': len(files),
    'totalFileBytes': sum(entry['bytes'] for entry in files),
    'files': sorted(files, key=lambda entry: entry['path']),
}
(output / 'crystal-map-pack-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
verification = {
    'sourceRevision': revision,
    'releaseTag': release_tag,
    'rustToolchain': '1.95.0',
    'buildEnvironment': 'native-linux-non-root',
    'postgres': 'private-temporary-native-cluster-loopback-only',
    'containersStarted': False,
    'productionPublish': False,
    'passed': ['schema_union', 'monthly_card_postgres', 'billing_postgres', 'activities_postgres', 'linux-release-security', 'gateway-package', 'gateway-postgres-startup'],
    'mapManifest': 'crystal-map-pack-manifest.json',
}
(output / 'playtest-release-verification.json').write_text(json.dumps(verification, indent=2) + '\n', encoding='utf-8')
PY
git diff --quiet --ignore-submodules -- || fail 'verification changed tracked source'
git diff --cached --quiet --ignore-submodules -- || fail 'verification changed the source index'
printf 'PASS: native playtest Gateway, PostgreSQL persistence and matching maps at %s\n' "$task_revision"
