"""Owned child wrapper: ordinary stdin/marker shutdown, never a machine service stop.

The wrapper AND every child inherit the controller's one server Job budget.
Private environment values stay in the input file and are never printed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def write_new(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)


def child_environment(overrides: dict) -> dict:
    if not isinstance(overrides, dict) or any(
        not isinstance(k, str) or not (k.startswith("MIR2_") or k.startswith("CRYSTAL_"))
        or (v is not None and not isinstance(v, str)) for k, v in overrides.items()
    ):
        raise ValueError("Only explicit MIR2/CRYSTAL child environment overrides")
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("MIR2_", "CRYSTAL_"))}
    env.update({k: v for k, v in overrides.items() if v is not None})
    return env


def run(spec: dict, root: Path) -> int:
    if spec.get("schema") != "mir2.comparable.service-host.v1":
        raise ValueError("Wrong service specification")
    argv, cwd = spec["command"], Path(spec["cwd"]).resolve(strict=True)
    if not isinstance(argv, list) or not argv or any(not isinstance(a, str) or not a for a in argv):
        raise ValueError("Explicit child argv required")
    executable = Path(argv[0]).resolve(strict=True)
    if not executable.is_file() or cwd.drive.upper() != "C:":
        raise ValueError("Pinned executable and C-only cwd required")
    if spec["stopMode"] not in ("stdin", "marker"):
        raise ValueError("Only normal stdin or marker shutdown")
    overrides = dict(spec.get("environment", {}))
    if spec.get("environmentFile"):
        private_env = Path(spec["environmentFile"]).resolve(strict=True)
        if private_env.drive.upper() != "C:" or private_env.parent.name != "private":
            raise ValueError("Private C-only environment file required")
        overrides = {**json.loads(private_env.read_text(encoding="utf-8-sig")), **overrides}
    stop = Path(spec["stopFile"]).resolve()
    if stop.parent != root or stop.exists():
        raise ValueError("Fresh controller stop marker required")
    wall = spec["wallSeconds"]
    if type(wall) is not int or not 60 <= wall <= 7200:
        raise ValueError("Finite service wall required")
    marker = Path(spec["childStopFile"]).resolve() if spec["stopMode"] == "marker" else None
    if marker is not None and (marker.parent != root or marker.exists()):
        raise ValueError("Fresh child stop marker required")
    if spec["stopMode"] == "stdin" and overrides.get("MIR2_GATEWAY_STDIN_SHUTDOWN") != "1":
        raise ValueError("Normal operator stdin shutdown must be explicitly enabled")
    started = time.monotonic()
    out_path, err_path = root / "game-stdout.log", root / "game-stderr.log"
    with out_path.open("xb") as out, err_path.open("xb") as err:
        child = subprocess.Popen(argv, cwd=cwd, env=child_environment(overrides),
                                 stdin=subprocess.PIPE if marker is None else subprocess.DEVNULL,
                                 stdout=out, stderr=err, creationflags=0x08000000 if os.name == "nt" else 0)
        write_new(root / "service-started.json", {
            "schema": "mir2.comparable.service-started.v1", "wrapperPid": os.getpid(),
            "gamePid": child.pid, "executableSha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
            "normalStopMode": spec["stopMode"], "allDescendantsMustBeInServerJob": True,
        })
        reason = "child-exited-before-stop"
        while child.poll() is None:
            if stop.exists() or time.monotonic() - started >= wall:
                reason = "controller-marker" if stop.exists() else "service-wall-expired"
                if marker is not None:
                    with marker.open("x", encoding="utf-8") as stream:
                        stream.write(reason + "\n")
                else:
                    child.stdin.write(b"shutdown\n")
                    child.stdin.flush()
                    child.stdin.close()
                break
            time.sleep(0.1)
        # A stuck ordinary shutdown stays visible to the controller. No kill,
        # resource release, fake success or detached background child here.
        code = child.wait()
    result = {"schema": "mir2.comparable.service-result.v1", "gamePid": child.pid,
              "actualExitCode": code, "stopReason": reason, "elapsedSeconds": time.monotonic() - started,
              "normalStopRequested": reason in ("controller-marker", "service-wall-expired"),
              "forceKillUsed": False}
    write_new(root / "service-result.json", result)
    return 0 if code == 0 and reason == "controller-marker" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--run-root", type=Path, required=True)
    args = parser.parse_args()
    root = args.run_root.resolve(strict=True)
    if root.drive.upper() != "C:" or not root.is_dir():
        raise ValueError("C-only existing run directory required")
    return run(json.loads(args.spec.read_text(encoding="utf-8-sig")), root)


if __name__ == "__main__":
    raise SystemExit(main())
