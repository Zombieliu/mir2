"""Read-only guard against continuing or publishing an obsolete worktree."""
from pathlib import Path
import argparse
import json
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-detached", action="store_true",
                        help="Verify an immutable build snapshot of the integrated line")
    parser.add_argument("--require-clean", action="store_true",
                        help="Also require no tracked source modifications")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    config = json.loads((root / "mir2-web3/config/unified-codeline.json").read_text(encoding="utf-8"))

    def git(*parts):
        return subprocess.run(["git", "-C", str(root), *parts], capture_output=True,
                              text=True, encoding="utf-8", errors="replace")

    head = git("rev-parse", "HEAD")
    branch = git("symbolic-ref", "--quiet", "--short", "HEAD")
    errors = []
    if head.returncode:
        errors.append("Cannot resolve the source commit")
    name = branch.stdout.strip()
    if name != config["canonicalBranch"] and not (not name and args.allow_detached):
        errors.append(f"Use {config['canonicalBranch']}; this is {name or 'a detached snapshot'}")
    for topic, commit in config["requiredAncestors"].items():
        if git("merge-base", "--is-ancestor", commit, "HEAD").returncode:
            errors.append(f"Missing integrated history: {topic} ({commit})")
    for app in ("web", "website"):
        deployment = json.loads((root / f"mir2-web3/apps/{app}/vercel.json").read_text(encoding="utf-8"))
        enabled = deployment.get("git", {}).get("deploymentEnabled", {})
        if not isinstance(enabled, dict) or enabled != {config["canonicalBranch"]: False}:
            errors.append(f"{app}: canonical source push must retain the manual release boundary")
    if args.require_clean and git("diff", "HEAD", "--quiet", "--").returncode:
        errors.append("Tracked source changes must be committed before release")
    print(json.dumps({"schema": "mir2.unified-codeline-verification.v1",
                      "branch": name or None, "head": head.stdout.strip(),
                      "requiredAncestors": config["requiredAncestors"],
                      "passed": not errors, "errors": errors}, ensure_ascii=False, indent=2))
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())
