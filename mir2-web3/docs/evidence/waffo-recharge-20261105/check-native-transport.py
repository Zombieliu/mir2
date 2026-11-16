"""Run the exact Windows payment wire/browser modules without a game build.

The Windows host lock is currently stale relative to unrelated runtime WIP.
This isolated harness never edits that lock or copies/modifies source modules.
"""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile


def main() -> int:
    project = Path(__file__).resolve().parents[3]
    evidence = Path(__file__).resolve().parent
    harness = Path(tempfile.gettempdir()) / "mir2-waffo-native-harness"
    target = Path(tempfile.gettempdir()) / "mir2-waffo-native-target"
    harness.mkdir(parents=True, exist_ok=True)
    (harness / "src").mkdir(exist_ok=True)
    # Preserve the existing accepted crate versions (including yanked versions
    # already pinned by the source project), then resolve only the tiny harness.
    if not (harness / "Cargo.lock").exists():
        shutil.copyfile(project / "apps/game-client/client-bevy/Cargo.lock", harness / "Cargo.lock")
    native = project / "apps/game-client/platform-windows/src"
    bevy = (project / "apps/game-client/client-bevy").as_posix()
    protocol = (project / "packages/protocol").as_posix()
    (harness / "Cargo.toml").write_text(
        f'''[package]
name = "mir2-waffo-native-transport-check"
version = "0.1.0"
edition = "2021"
[workspace]
[dependencies]
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
mir2-client-bevy = {{ path = "{bevy}", features = ["native-ui"] }}
mir2-protocol = {{ path = "{protocol}" }}
bevy = {{ version = "0.19.0", default-features = false }}
tokio-tungstenite = {{ version = "0.29", features = ["rustls-tls-native-roots"] }}
url = "2"
''',
        encoding="utf-8",
    )
    (harness / "src/lib.rs").write_text(
        '#![allow(dead_code)]\n'
        f'#[path = r"{native / "native_protocol.rs"}"]\nmod native_protocol;\n'
        f'#[path = r"{native / "billing_browser.rs"}"]\nmod billing_browser;\n',
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update(
        CARGO_PROFILE_DEV_DEBUG="0",
        CARGO_PROFILE_TEST_DEBUG="0",
        CARGO_INCREMENTAL="0",
        RUST_TEST_THREADS="1",
    )
    command = [
        "cargo", "+1.95.0", "test", "--offline", "--manifest-path",
        str(harness / "Cargo.toml"), "--target-dir", str(target), "billing", "--lib",
    ]
    print("Running exact source modules:", native, flush=True)
    print("Harness:", harness, flush=True)
    with (evidence / "native-transport-tests.log").open("w", encoding="utf-8") as log:
        log.write(" ".join(command) + "\n")
        log.flush()
        result = subprocess.run(command, cwd=project, env=env, stdout=log, stderr=subprocess.STDOUT)
    print("Exit code:", result.returncode, flush=True)
    print("Log:", evidence / "native-transport-tests.log", flush=True)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
