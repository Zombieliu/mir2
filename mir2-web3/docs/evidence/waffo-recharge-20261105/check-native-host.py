"""Check the Windows host using a temporary, guarded lockfile update.

Original lock bytes are restored only while the lock still matches the exact
generated snapshot used by the check. No runtime manifest/lock is modified.
"""

from pathlib import Path
import hashlib
import json
import os
import subprocess
import tempfile
import uuid


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    project = Path(__file__).resolve().parents[3]
    evidence = Path(__file__).resolve().parent
    manifest = project / "apps/game-client/platform-windows/Cargo.toml"
    lock = manifest.with_name("Cargo.lock")
    runtime_lock = project / "apps/game-client/runtime/Cargo.lock"
    runtime_manifest = project / "apps/game-client/runtime/Cargo.toml"
    original = lock.read_bytes()
    protected = {path: path.read_bytes() for path in (manifest, runtime_lock, runtime_manifest)}
    backup = Path(tempfile.gettempdir()) / f"mir2-waffo-platform-lock-{uuid.uuid4().hex}.backup"
    with backup.open("xb") as output:
        output.write(original)
    target = Path(tempfile.gettempdir()) / "mir2-waffo-native-target"
    env = os.environ.copy()
    env.update(CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_DEV_OPT_LEVEL="0", CARGO_INCREMENTAL="0")
    stages = []
    generated = None
    result_code = 1
    online_used = False
    summary = {"schema": "mir2.waffo.native-host-check.v1", "originalLockSha256": sha(original),
               "lockBackup": str(backup), "temporaryLockRestored": False, "stages": stages}

    def run(name: str, arguments: list[str]) -> tuple[int, str]:
        print(name, flush=True)
        log_path = evidence / f"native-host-{name}.log"
        with log_path.open("w", encoding="utf-8") as output:
            output.write(" ".join(arguments) + "\n")
            output.flush()
            result = subprocess.run(arguments, cwd=project, env=env, stdout=output, stderr=subprocess.STDOUT)
        contents = log_path.read_text(encoding="utf-8")
        stages.append({"name": name, "command": arguments, "exitCode": result.returncode, "log": log_path.name})
        print(f"{name}: exit {result.returncode}", flush=True)
        return result.returncode, contents

    try:
        base = ["cargo", "+1.95.0"]
        result_code, _ = run("resolve-offline", base + ["update", "--workspace", "--offline", "--manifest-path", str(manifest)])
        if result_code:
            if lock.read_bytes() != original:
                raise RuntimeError("failed resolution changed lock unexpectedly; refusing an unverified overwrite")
            online_used = True
            result_code, _ = run("resolve-online", base + ["update", "--workspace", "--manifest-path", str(manifest)])
        if result_code:
            return result_code
        generated = lock.read_bytes()
        summary["generatedLockSha256"] = sha(generated)
        check = base + ["check", "--locked", "--manifest-path", str(manifest), "--tests", "--target-dir", str(target),
                        "--config", 'profile.dev.package."*".opt-level=0']
        result_code, contents = run("check-offline", check + ["--offline"])
        missing_cache = any(marker in contents for marker in ["failed to download", "attempting to make an HTTP request", "no matching package named"])
        if result_code and missing_cache and not online_used:
            if lock.read_bytes() != generated:
                raise RuntimeError("temporary lock changed concurrently; refusing to continue")
            online_used = True
            result_code, _ = run("check-online", check)
        summary["hostCompilePassed"] = result_code == 0
        return result_code
    finally:
        summary["onlineResolutionOrFetchUsed"] = online_used
        current = lock.read_bytes()
        if generated is not None and current == generated:
            restore = lock.with_name(f"Cargo.lock.waffo-restore-{uuid.uuid4().hex}.tmp")
            with restore.open("xb") as output:
                output.write(original)
            if lock.read_bytes() != generated:
                raise RuntimeError("lock changed before restore; backup retained and current lock left untouched")
            os.replace(restore, lock)
            summary["temporaryLockRestored"] = lock.read_bytes() == original
        elif current == original:
            summary["temporaryLockRestored"] = True
        else:
            summary["restoreBlockedByConcurrentChange"] = True
        summary["protectedFilesUnchanged"] = {str(path.relative_to(project)): path.read_bytes() == data for path, data in protected.items()}
        summary["finalLockSha256"] = sha(lock.read_bytes())
        (evidence / "native-host-validation.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({key: value for key, value in summary.items() if key != "stages"}, indent=2), flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
