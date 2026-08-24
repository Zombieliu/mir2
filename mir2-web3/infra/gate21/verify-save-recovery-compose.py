#!/usr/bin/env python3
"""Static, secret-safe verification of Compose Gateway recovery wiring."""

from __future__ import annotations

import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SINGLE = "MIR2_GATEWAY_SAVE_RECOVERY_MAC_KEY"
HA = tuple(f"MIR2_GATEWAY_{index}_SAVE_RECOVERY_MAC_KEY" for index in range(1, 4))
KNOWN_KEYS = (SINGLE, *HA)
ROLE_LABEL = "com.obelisk.mir2.role"
GATEWAY_ROLE = "gateway"
PROJECT_NAMES = (None, "mir2-recovery-a", "mir2-recovery-b")
HEX_64 = re.compile(r"(?i)(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])")
RUNTIME_STRENGTH_TEST = (
    "cargo +1.95.0 test --manifest-path apps/gateway/Cargo.toml "
    "--bin mir2-gateway --jobs 1 "
    "tests::empty_malformed_and_weak_recovery_keys_are_rejected "
    "-- --exact --test-threads=1"
)
_TEST_SECRETS: set[str] = set()


class Failure(RuntimeError):
    pass


def gateway_specs(prefix: str, logical_prefix: str) -> dict[str, dict[str, str]]:
    logical_stem = f"{logical_prefix}-" if logical_prefix else ""
    return {
        f"gateway-{index}": {
            "instance": f"{prefix}-gateway-{index}",
            "root": f"/var/lib/obelisk/save-recovery/gateway-{index}",
            "key": HA[index - 1],
            "logical_volume": (
                f"{logical_stem}gateway-{index}-save-recovery"
            ),
            "physical_volume": (
                f"mir2-{prefix}-gateway-{index}-save-recovery-v1"
            ),
        }
        for index in range(1, 4)
    }


CASES: tuple[dict[str, Any], ...] = (
    {
        "name": "early",
        "files": ("infra/early/docker-compose.yml",),
        "keys": (SINGLE,),
        "fixtures": {},
        "example": "infra/early/.env.example",
        "gateways": {
            "gateway": {
                "instance": "early-gateway-1",
                "root": "/var/lib/obelisk/save-recovery/gateway",
                "key": SINGLE,
                "logical_volume": "gateway-save-recovery",
                "physical_volume": "mir2-early-gateway-save-recovery-v1",
            }
        },
    },
    {
        "name": "gate12",
        "files": ("infra/gate12/docker-compose.yml",),
        "keys": (SINGLE,),
        "fixtures": {
            "GATE12_ZONE_A_SIGNING_KEY_FILE": "/tmp/gate12-zone-a.key",
            "GATE12_ZONE_B_SIGNING_KEY_FILE": "/tmp/gate12-zone-b.key",
        },
        "example": "infra/gate12/.env.example",
        "gateways": {
            "gateway": {
                "instance": "gate12-gateway-1",
                "root": "/var/lib/obelisk/save-recovery/gateway",
                "key": SINGLE,
                "logical_volume": "gateway-save-recovery",
                "physical_volume": "mir2-gate12-gateway-save-recovery-v1",
            }
        },
    },
    {
        "name": "gate19",
        "files": ("infra/gate19/docker-compose.yml",),
        "keys": HA,
        "fixtures": {},
        "example": "infra/gate19/.env.example",
        "gateways": gateway_specs("gate19", ""),
    },
    {
        "name": "gate20",
        "files": ("infra/gate20/docker-compose.yml",),
        "keys": (),
        "fixtures": {},
        "example": None,
        "gateways": {},
    },
    {
        "name": "gate21",
        "files": (
            "infra/gate19/docker-compose.yml",
            "infra/gate21/docker-compose.yml",
        ),
        "keys": HA,
        "fixtures": {},
        "example": "infra/gate21/.env.example",
        "gateways": gateway_specs("gate21", "gate21"),
    },
)


def report(message: str, *, error: bool = False) -> None:
    if any(secret in message for secret in _TEST_SECRETS):
        message = "FAIL: verifier suppressed a secret-bearing diagnostic"
        error = True
    print(message, file=sys.stderr if error else sys.stdout)


def make_key() -> str:
    value = secrets.token_hex(32)
    _TEST_SECRETS.add(value)
    return value


def compose_command(
    case: dict[str, Any],
    empty_env: Path,
    project_name: str | None,
    *config_args: str,
) -> list[str]:
    command = ["docker", "compose", "--env-file", str(empty_env)]
    if project_name is not None:
        command.extend(("--project-name", project_name))
    for file_name in case["files"]:
        command.extend(("-f", str(ROOT / file_name)))
    command.extend(("--profile", "acceptance", "config", *config_args))
    return command


def compose_environment(
    case: dict[str, Any], key_values: dict[str, str]
) -> dict[str, str]:
    environment = dict(os.environ)
    environment.pop("COMPOSE_PROJECT_NAME", None)
    for key in KNOWN_KEYS:
        environment.pop(key, None)
    environment.update(case["fixtures"])
    environment.update(key_values)
    return environment


def run_compose(
    case: dict[str, Any],
    empty_env: Path,
    key_values: dict[str, str],
    project_name: str | None,
    *config_args: str,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        compose_command(case, empty_env, project_name, *config_args),
        cwd=ROOT,
        env=compose_environment(case, key_values),
        capture_output=True,
        text=True,
        check=False,
    )


def parse_environment(service: dict[str, Any]) -> dict[str, str]:
    environment = service.get("environment", {})
    if isinstance(environment, dict):
        return {str(key): str(value) for key, value in environment.items()}
    if isinstance(environment, list):
        parsed: dict[str, str] = {}
        for entry in environment:
            name, separator, value = str(entry).partition("=")
            if separator:
                parsed[name] = value
        return parsed
    return {}


def parse_labels(service: dict[str, Any]) -> dict[str, str]:
    labels = service.get("labels", {})
    if isinstance(labels, dict):
        return {str(key): str(value) for key, value in labels.items()}
    if isinstance(labels, list):
        parsed: dict[str, str] = {}
        for entry in labels:
            name, separator, value = str(entry).partition("=")
            parsed[name] = value if separator else ""
        return parsed
    return {}


def image_is_gateway(image: object) -> bool:
    image_name = str(image or "").split("@", 1)[0].rsplit("/", 1)[-1]
    repository = image_name.rsplit(":", 1)[0].lower()
    return "gateway" in re.split(r"[^a-z0-9]+", repository)


def service_is_likely_gateway(service: dict[str, Any]) -> bool:
    build = service.get("build")
    build_target = build.get("target") if isinstance(build, dict) else None
    environment = parse_environment(service)
    runtime_gateway = {
        "MIR2_GATEWAY_TCP_ADDR",
        "MIR2_GATEWAY_WEB_ADDR",
    }.issubset(environment)
    return (
        build_target == "gateway"
        or image_is_gateway(service.get("image"))
        or runtime_gateway
    )


def actual_gateways(model: dict[str, Any]) -> set[str]:
    """Return the authoritative Gateway inventory declared by role label."""
    return {
        name
        for name, service in model.get("services", {}).items()
        if isinstance(service, dict)
        and parse_labels(service).get(ROLE_LABEL) == GATEWAY_ROLE
    }


def likely_gateways(model: dict[str, Any]) -> set[str]:
    """Return heuristic candidates used only to guard the role contract."""
    return {
        name
        for name, service in model.get("services", {}).items()
        if isinstance(service, dict) and service_is_likely_gateway(service)
    }


def verify_gateway_role_labels(model: dict[str, Any], context: str) -> None:
    services = model.get("services", {})
    for name in sorted(likely_gateways(model)):
        service = services[name]
        role = parse_labels(service).get(ROLE_LABEL)
        if role == GATEWAY_ROLE:
            continue
        state = "missing" if role is None else "misstated"
        raise Failure(
            f"{context}/{name}: likely Gateway has {state} required role "
            f"label; expected {ROLE_LABEL}={GATEWAY_ROLE}"
        )


def physical_volume_map(model: dict[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    for logical_name, value in model.get("volumes", {}).items():
        physical_name = logical_name
        if isinstance(value, dict) and value.get("name"):
            physical_name = str(value["name"])
        result[str(logical_name)] = str(physical_name)
    return result


def verify_env_example(case: dict[str, Any]) -> None:
    example_name = case["example"]
    if example_name is None:
        return

    text = (ROOT / example_name).read_text(encoding="utf-8")
    if HEX_64.search(text):
        raise Failure(f"{case['name']}: env example contains a real 64-hex value")

    assignments: dict[str, list[str]] = {}
    for line in text.splitlines():
        if "=" not in line or line.lstrip().startswith("#"):
            continue
        name, value = line.split("=", 1)
        assignments.setdefault(name.strip(), []).append(value.strip())

    expected_keys = set(case["keys"])
    present_recovery_keys = {
        name for name in assignments if name in KNOWN_KEYS
    }
    if present_recovery_keys != expected_keys:
        raise Failure(f"{case['name']}: env example recovery-key set mismatch")
    for key in case["keys"]:
        values = assignments.get(key, [])
        if len(values) != 1:
            raise Failure(f"{case['name']}: {key} must appear exactly once")
        if values[0] != "":
            raise Failure(f"{case['name']}: {key} must be empty in env example")


def verify_no_embedded_compose_key(case: dict[str, Any]) -> None:
    for file_name in case["files"]:
        text = (ROOT / file_name).read_text(encoding="utf-8")
        if HEX_64.search(text):
            raise Failure(
                f"{case['name']}: Compose source contains a real 64-hex value"
            )


def verify_expected_volume_names() -> None:
    seen: dict[str, str] = {}
    for case in CASES:
        for service_name, expected in case["gateways"].items():
            physical = expected["physical_volume"]
            owner = f"{case['name']}/{service_name}"
            if physical in seen:
                raise Failure(
                    f"physical volume {physical} reused by {seen[physical]} and {owner}"
                )
            seen[physical] = owner

    gate19 = {
        value["physical_volume"] for value in CASES[2]["gateways"].values()
    }
    gate21 = {
        value["physical_volume"] for value in CASES[4]["gateways"].values()
    }
    if gate19 & gate21:
        raise Failure("Gate19 and Gate21 physical volume names overlap")


def verify_rendered_contract(
    case: dict[str, Any],
    model: dict[str, Any],
    key_values: dict[str, str],
) -> tuple[tuple[str, str, str, str], ...]:
    expected_services = set(case["gateways"])
    verify_gateway_role_labels(model, case["name"])
    discovered_services = actual_gateways(model)
    if discovered_services != expected_services:
        raise Failure(
            f"{case['name']}: role-labelled Gateway coverage mismatch; "
            f"discovered={sorted(discovered_services)} "
            f"expected={sorted(expected_services)}"
        )

    top_level_volumes = physical_volume_map(model)
    roots: set[str] = set()
    physical_sources: set[str] = set()
    instance_ids: set[str] = set()
    fingerprint: list[tuple[str, str, str, str]] = []

    for service_name, expected in case["gateways"].items():
        service = model["services"][service_name]
        environment = parse_environment(service)
        instance = environment.get("MIR2_GATEWAY_INSTANCE_ID")
        root = environment.get("MIR2_SAVE_RECOVERY_DIR")

        if instance != expected["instance"]:
            raise Failure(
                f"{case['name']}/{service_name}: unstable instance identity"
            )
        if root != expected["root"]:
            raise Failure(f"{case['name']}/{service_name}: wrong recovery root")
        path = PurePosixPath(str(root))
        if not path.is_absolute() or ".." in path.parts:
            raise Failure(
                f"{case['name']}/{service_name}: recovery root is not absolute"
            )
        if environment.get("MIR2_SAVE_RECOVERY_MAC_KEY") != key_values[expected["key"]]:
            raise Failure(
                f"{case['name']}/{service_name}: key interpolation mismatch"
            )

        recovery_mounts = [
            mount
            for mount in service.get("volumes", [])
            if str(mount.get("target", "")).startswith(
                "/var/lib/obelisk/save-recovery"
            )
        ]
        if len(recovery_mounts) != 1:
            raise Failure(
                f"{case['name']}/{service_name}: final recovery mount count != 1"
            )
        mount = recovery_mounts[0]
        source = str(mount.get("source", ""))
        if mount.get("type") != "volume":
            raise Failure(
                f"{case['name']}/{service_name}: recovery mount is not a volume"
            )
        if mount.get("target") != root:
            raise Failure(
                f"{case['name']}/{service_name}: recovery target/root mismatch"
            )
        logical = expected["logical_volume"]
        if source != logical:
            raise Failure(
                f"{case['name']}/{service_name}: wrong logical volume source"
            )

        physical = top_level_volumes.get(source)
        if physical != expected["physical_volume"]:
            raise Failure(
                f"{case['name']}/{service_name}: top-level volume.name mismatch"
            )
        if case["name"] == "gate21" and not physical.startswith("mir2-gate21-"):
            raise Failure(
                f"{case['name']}/{service_name}: inherited Gate19 source remained"
            )

        if root in roots or physical in physical_sources or instance in instance_ids:
            raise Failure(
                f"{case['name']}: recovery root, volume, or instance identity shared"
            )
        roots.add(str(root))
        physical_sources.add(physical)
        instance_ids.add(str(instance))
        fingerprint.append((service_name, str(instance), str(root), physical))

    return tuple(sorted(fingerprint))


def expect_interpolation_failure(
    case: dict[str, Any],
    empty_env: Path,
    key_values: dict[str, str],
    key_name: str,
    variant: str,
) -> None:
    result = run_compose(case, empty_env, key_values, None, "--quiet")
    if result.returncode == 0:
        raise Failure(
            f"{case['name']}: {variant} {key_name} passed Compose interpolation"
        )
    if key_name not in result.stderr:
        raise Failure(
            f"{case['name']}: {variant} failure did not identify {key_name}"
        )


def verify_interpolation_boundary(
    case: dict[str, Any],
    empty_env: Path,
    strong_keys: dict[str, str],
) -> int:
    checks = 0
    for key_name in case["keys"]:
        missing = dict(strong_keys)
        missing.pop(key_name)
        expect_interpolation_failure(
            case, empty_env, missing, key_name, "missing"
        )
        checks += 1

        empty = dict(strong_keys)
        empty[key_name] = ""
        expect_interpolation_failure(case, empty_env, empty, key_name, "empty")
        checks += 1

    nonempty_samples = (
        "abcd",
        "replace-with-save-recovery-key",
        "00" * 32,
    )
    for sample in nonempty_samples:
        weak_keys = {key_name: sample for key_name in case["keys"]}
        result = run_compose(case, empty_env, weak_keys, None, "--quiet")
        if result.returncode != 0:
            raise Failure(
                f"{case['name']}: Compose unexpectedly enforced non-empty key strength"
            )
    return checks


def verify_case(case: dict[str, Any], empty_env: Path) -> None:
    verify_env_example(case)
    verify_no_embedded_compose_key(case)
    strong_keys = {key_name: make_key() for key_name in case["keys"]}
    interpolation_checks = verify_interpolation_boundary(
        case, empty_env, strong_keys
    )

    expected_fingerprint: tuple[tuple[str, str, str, str], ...] | None = None
    for project_name in PROJECT_NAMES:
        result = run_compose(
            case,
            empty_env,
            strong_keys,
            project_name,
            "--format",
            "json",
        )
        if result.returncode != 0:
            raise Failure(
                f"{case['name']}: valid Compose config failed for "
                f"project={project_name or 'default'}"
            )
        try:
            model = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise Failure(f"{case['name']}: Compose JSON was invalid") from error

        fingerprint = verify_rendered_contract(case, model, strong_keys)
        if expected_fingerprint is None:
            expected_fingerprint = fingerprint
        elif fingerprint != expected_fingerprint:
            raise Failure(
                f"{case['name']}: recovery identity changed with project name"
            )

    report(
        f"PASS {case['name']}: Compose wiring for "
        f"{len(case['gateways'])} role-labelled Gateway service(s); "
        f"{interpolation_checks} missing/empty key check(s); "
        f"physical identity stable across {len(PROJECT_NAMES)} project names"
    )


def verify_role_label_discovery_regression() -> None:
    role_only_model = {
        "services": {
            "arbitrary-renamed-runtime": {
                "image": "registry.example/obelisk/generic-runtime:latest",
                "labels": {ROLE_LABEL: GATEWAY_ROLE},
                "environment": {},
            },
            "gateway-docs-only": {
                "image": "nginx:alpine",
                "environment": {},
            },
        },
        "volumes": {},
    }
    if actual_gateways(role_only_model) != {"arbitrary-renamed-runtime"}:
        raise Failure("authoritative role-label discovery regression failed")
    if likely_gateways(role_only_model):
        raise Failure("role-only Gateway unexpectedly depended on heuristic discovery")

    unprotected_case = {
        "name": "role-only-unprotected-regression",
        "gateways": {},
    }
    try:
        verify_rendered_contract(unprotected_case, role_only_model, {})
    except Failure as error:
        if "role-labelled Gateway coverage mismatch" not in str(error):
            raise
    else:
        raise Failure("role-only unprotected Gateway did not fail coverage")

    guard_regressions = (
        (
            "missing",
            {
                "build": {"target": "gateway"},
                "image": "registry.example/obelisk/generic-runtime:latest",
                "environment": {},
            },
        ),
        (
            "misstated",
            {
                "image": "registry.example/obelisk/mir2-gateway:canary",
                "labels": {ROLE_LABEL: "worker"},
                "environment": {},
            },
        ),
    )
    for expected_state, service in guard_regressions:
        guard_model = {"services": {"renamed-service": service}, "volumes": {}}
        try:
            verify_gateway_role_labels(guard_model, "role-guard-regression")
        except Failure as error:
            diagnostic = str(error)
            if expected_state not in diagnostic or ROLE_LABEL not in diagnostic:
                raise
        else:
            raise Failure(
                f"likely Gateway with {expected_state} role label passed guard"
            )

    report(
        "PASS role-label discovery regression: role-only renamed Gateway is "
        "discovered; likely missing/mislabelled roles are rejected"
    )


def main() -> int:
    if shutil.which("docker") is None:
        report("FAIL: docker CLI is required", error=True)
        return 1

    try:
        verify_expected_volume_names()
        verify_role_label_discovery_regression()
        with tempfile.TemporaryDirectory(
            prefix="mir2-recovery-compose-"
        ) as temporary:
            empty_env = Path(temporary) / "empty.env"
            empty_env.write_text("", encoding="utf-8")
            for case in CASES:
                verify_case(case, empty_env)
    except Failure as error:
        report(f"FAIL: {error}", error=True)
        return 1

    report(
        "BOUNDARY: Compose accepts non-empty malformed, placeholder, and "
        "repeated weak probes; runtime strength was not executed"
    )
    report(f"NOT RUN Gateway runtime strength gate: {RUNTIME_STRENGTH_TEST}")
    report(
        "PASS: Compose Gateway save-recovery wiring and physical-volume "
        "invariants verified"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
