#!/usr/bin/env python3
"""Content-addressed immutable Desktop release packager (source-only).

Does not install, start, or replace a live Mac Buzz process.
Does not mint, import, export, print, or rotate keys.
Does not send Buzz messages.
Does not emit a fake Buzz.app on Linux.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROFILE_PATH = ROOT / "desktop" / "release" / "local-dev-production.profile.json"
SCHEMA_PATH = ROOT / "desktop" / "release" / "immutable-desktop-manifest.schema.json"
TAURI_CONF = ROOT / "desktop" / "src-tauri" / "tauri.conf.json"
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
LEFTOVER_ID = "mac-packaged-app-build"

PINNED = {
    "schema_version": 1,
    "profile": "local-dev-production",
    "bundle_identifier": "xyz.block.buzz.app",
    "keyring_service": "buzz-desktop",
    "relay_ws_url": "ws://localhost:3300",
    "expected_owner_pubkey_prefix": "ea840b3e",
}


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def git_head() -> str:
    import subprocess

    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def expand_user_path(template: str) -> str:
    home = os.environ.get("HOME", "")
    xdg = os.environ.get("XDG_STATE_HOME") or str(Path(home) / ".local" / "state")
    expanded = template.replace("${XDG_STATE_HOME:-${HOME}/.local/state}", xdg)
    expanded = expanded.replace("${XDG_STATE_HOME}", xdg)
    return expanded.replace("${HOME}", home)


def forbidden_runtime_root(checkout: Path, candidate: Path) -> bool:
    checkout = checkout.resolve()
    try:
        candidate_resolved = candidate.resolve()
    except FileNotFoundError:
        candidate_resolved = candidate
    if checkout == candidate_resolved or checkout in candidate_resolved.parents:
        return True
    parts = {part.lower() for part in candidate_resolved.parts}
    if "dawsos" in parts and ({"reports", "ops"} & parts):
        return True
    text = str(candidate_resolved)
    return "/reports/ops/" in text or text.endswith("/reports/ops")


def prove_frontend_dist(tauri_conf: dict) -> dict:
    frontend_dist = tauri_conf.get("build", {}).get("frontendDist")
    identifier = tauri_conf.get("identifier")
    if identifier != PINNED["bundle_identifier"]:
        raise SystemExit(
            f"tauri identifier {identifier!r} is not {PINNED['bundle_identifier']}"
        )
    if frontend_dist != "../dist":
        raise SystemExit(
            f"tauri frontendDist {frontend_dist!r} is not ../dist; "
            "production packaging must not use Vite devUrl"
        )
    return {
        "mode": "frontendDist",
        "path": "../dist",
        "dev_url_active": False,
        "identifier": identifier,
        "devUrl_present_for_tauri_dev_only": "devUrl" in tauri_conf.get("build", {}),
    }


def hashed_inputs(frontend_dist_dir: Path | None) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    for rel in (
        "desktop/src-tauri/tauri.conf.json",
        "desktop/release/local-dev-production.profile.json",
        "desktop/release/immutable-desktop-manifest.schema.json",
        "desktop/src-tauri/src/app_state_keyring.rs",
        "crates/buzz-desktop-immutable-profile/src/lib.rs",
        "desktop/package.json",
        "desktop/src-tauri/Cargo.toml",
    ):
        path = ROOT / rel
        rows.append((rel, sha256_file(path)))
    if frontend_dist_dir is not None and frontend_dist_dir.is_dir():
        for path in sorted(p for p in frontend_dist_dir.rglob("*") if p.is_file()):
            rel = path.relative_to(ROOT).as_posix()
            rows.append((rel, sha256_file(path)))
    return rows


def content_digest(rows: list[tuple[str, str]]) -> str:
    payload = "".join(f"{rel}\0{digest}\n" for rel, digest in rows).encode()
    return sha256_bytes(payload)


def validate_manifest(manifest: dict) -> None:
    for key, value in PINNED.items():
        if manifest.get(key) != value:
            raise SystemExit(f"manifest {key}={manifest.get(key)!r} != {value!r}")
    if not COMMIT_RE.fullmatch(str(manifest.get("source_commit", ""))):
        raise SystemExit("manifest source_commit must be a full 40-char SHA")
    if not DIGEST_RE.fullmatch(str(manifest.get("content_digest", ""))):
        raise SystemExit("manifest content_digest must be sha256:<64 hex>")
    rollback = manifest.get("rollback_target")
    if rollback is not None and not DIGEST_RE.fullmatch(str(rollback)):
        raise SystemExit("manifest rollback_target must be sha256:<64 hex> or null")
    frontend = manifest.get("frontend") or {}
    if frontend.get("mode") != "frontendDist" or frontend.get("path") != "../dist":
        raise SystemExit("manifest frontend must be frontendDist ../dist")
    if frontend.get("dev_url_active") is not False:
        raise SystemExit("manifest frontend.dev_url_active must be false")
    leftovers = manifest.get("leftovers") or []
    if not any(item.get("id") == LEFTOVER_ID for item in leftovers):
        raise SystemExit(f"manifest must record leftover {LEFTOVER_ID}")
    macos_app = (manifest.get("artifacts") or {}).get("macos_app")
    leftover = next(item for item in leftovers if item.get("id") == LEFTOVER_ID)
    if macos_app is not None and leftover.get("status") != "satisfied":
        raise SystemExit("macos_app present but leftover not satisfied")
    if macos_app is None and leftover.get("status") != "needed":
        raise SystemExit("Linux/source package must leave mac-packaged-app-build needed")


def default_runtime_roots(profile: dict) -> tuple[str, str]:
    if sys.platform == "darwin":
        state = expand_user_path(profile["state_root"]["macos"])
        logs = expand_user_path(profile["log_root"]["macos"])
    else:
        state = expand_user_path(profile["state_root"]["linux_test"])
        logs = expand_user_path(profile["log_root"]["linux_test"])
    return state, logs


def pointer_path(release_root: Path, name: str) -> Path:
    return release_root / name


def read_pointer(release_root: Path, name: str) -> str | None:
    path = pointer_path(release_root, name)
    if not path.is_file():
        return None
    value = path.read_text().strip()
    return value or None


def write_pointer(release_root: Path, name: str, digest: str) -> None:
    pointer_path(release_root, name).write_text(digest + "\n")


def package(release_root: Path, source_commit: str | None) -> Path:
    if forbidden_runtime_root(ROOT, release_root):
        raise SystemExit(
            f"release root {release_root} must be outside the source checkout "
            "and outside DawsOS reports/ops"
        )
    profile = load_json(PROFILE_PATH)
    tauri_conf = load_json(TAURI_CONF)
    frontend_proof = prove_frontend_dist(tauri_conf)
    state_root, log_root = default_runtime_roots(profile)
    for label, path in ("state_root", state_root), ("log_root", log_root):
        if forbidden_runtime_root(ROOT, Path(path)):
            raise SystemExit(f"{label} {path} is forbidden")

    dist_dir = ROOT / "desktop" / "dist"
    frontend_dir = dist_dir if dist_dir.is_dir() else None
    rows = hashed_inputs(frontend_dir)
    digest = content_digest(rows)
    commit = source_commit or git_head()
    if not COMMIT_RE.fullmatch(commit):
        raise SystemExit(f"source_commit {commit!r} is not a full SHA")

    previous = read_pointer(release_root, "current")
    rollback_target = previous if previous and previous != digest else None

    leftovers = [
        {
            "id": LEFTOVER_ID,
            "status": "needed",
            "reason": (
                "This host cannot produce a signed macOS .app. A Mac worker "
                "must build Buzz.app from this manifest without installing "
                "over the live Mac Buzz process."
            ),
        }
    ]
    manifest = {
        **PINNED,
        "source_commit": commit,
        "content_digest": digest,
        "frontend": {
            "mode": "frontendDist",
            "path": "../dist",
            "dev_url_active": False,
            "digest": sha256_file(dist_dir / "index.html")
            if frontend_dir and (dist_dir / "index.html").is_file()
            else None,
        },
        "state_root": state_root,
        "log_root": log_root,
        "rollback_target": rollback_target,
        "artifacts": {"macos_app": None},
        "leftovers": leftovers,
    }
    validate_manifest(manifest)

    dest = release_root / "releases" / digest.replace(":", "-")
    dest.mkdir(parents=True, exist_ok=True)
    (dest / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (dest / "profile.json").write_text(PROFILE_PATH.read_text())
    proofs = dest / "proofs"
    proofs.mkdir(exist_ok=True)
    (proofs / "frontend-dist.json").write_text(
        json.dumps(frontend_proof, indent=2) + "\n"
    )
    (proofs / "content-inputs.json").write_text(
        json.dumps([{"path": rel, "digest": d} for rel, d in rows], indent=2) + "\n"
    )
    (dest / "artifacts").mkdir(exist_ok=True)
    (dest / "artifacts" / "README").write_text(
        "No Buzz.app is produced on Linux. Leftover: mac-packaged-app-build.\n"
        "Do not treat any file in this directory as a macOS application.\n"
    )
    if previous and previous != digest:
        write_pointer(release_root, "previous", previous)
    write_pointer(release_root, "current", digest)
    return dest / "manifest.json"


def rollback(release_root: Path) -> str:
    current = read_pointer(release_root, "current")
    target = read_pointer(release_root, "previous")
    if current is None:
        raise SystemExit("no current release to roll back from")
    if target is None:
        raise SystemExit("no rollback_target / previous pointer")
    if target == current:
        raise SystemExit("rollback target must not equal current")
    dest = release_root / "releases" / target.replace(":", "-")
    manifest_path = dest / "manifest.json"
    if not manifest_path.is_file():
        raise SystemExit(f"rollback target {target} is not a published digest")
    write_pointer(release_root, "previous", current)
    write_pointer(release_root, "current", target)
    return target


def verify(release_root: Path) -> None:
    current = read_pointer(release_root, "current")
    if current is None:
        raise SystemExit("no current pointer")
    manifest = load_json(
        release_root / "releases" / current.replace(":", "-") / "manifest.json"
    )
    validate_manifest(manifest)
    if manifest["content_digest"] != current:
        raise SystemExit("current pointer does not match manifest content_digest")
    prove_frontend_dist(load_json(TAURI_CONF))
    if SCHEMA_PATH.is_file():
        schema = load_json(SCHEMA_PATH)
        if schema.get("properties", {}).get("schema_version", {}).get("const") != 1:
            raise SystemExit("schema_version const drifted")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    pkg = sub.add_parser("package", help="write a content-addressed source package")
    pkg.add_argument("--release-root", required=True, type=Path)
    pkg.add_argument("--source-commit", default=None)
    rb = sub.add_parser("rollback", help="activate the exact previous digest")
    rb.add_argument("--release-root", required=True, type=Path)
    ver = sub.add_parser("verify", help="verify current manifest + frontendDist proof")
    ver.add_argument("--release-root", required=True, type=Path)
    args = parser.parse_args()
    if args.cmd == "package":
        path = package(args.release_root.expanduser(), args.source_commit)
        print(path)
        return 0
    if args.cmd == "rollback":
        print(rollback(args.release_root.expanduser()))
        return 0
    verify(args.release_root.expanduser())
    return 0


if __name__ == "__main__":
    sys.exit(main())
