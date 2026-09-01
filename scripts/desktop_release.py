#!/usr/bin/env python3
"""Generate and validate immutable desktop release candidates."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"
METADATA = ROOT / ".release" / "desktop-candidate.json"
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
DESKTOP_PATHS = (
    "desktop/",
    "crates/buzz-core/",
    "crates/buzz-persona/",
    "crates/buzz-sdk/",
    "crates/buzz-agent/",
    "crates/buzz-media/",
)
CANDIDATE_FILES = {
    ".release/desktop-candidate.json",
    "CHANGELOG.md",
    "desktop/package.json",
    "desktop/src-tauri/tauri.conf.json",
    "desktop/src-tauri/Cargo.toml",
    "desktop/src-tauri/Cargo.lock",
    "pnpm-lock.yaml",
}
REQUIRED_CANDIDATE_FILES = {
    ".release/desktop-candidate.json",
    "CHANGELOG.md",
    "desktop/package.json",
    "desktop/src-tauri/tauri.conf.json",
    "desktop/src-tauri/Cargo.toml",
}

LOCAL_DEV_PROFILE = ROOT / ".release" / "local-dev-production.json"
TAURI_CONF = ROOT / "desktop" / "src-tauri" / "tauri.conf.json"
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
PUBKEY_RE = re.compile(r"^[0-9a-f]{64}$")
MAC_LEFTOVER_ID = "mac-packaged-app-build"
LOCAL_DEV_PROFILE_NAME = "local-dev-production"
BUNDLE_IDENTIFIER = "xyz.block.buzz.app"
PRODUCTION_KEYRING = "buzz-desktop"
PRODUCTION_RELAY = "ws://localhost:3300"
OWNER_DISPLAY_PREFIX = "ea840b3e"
FRONTEND_DIST = "../dist"


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def commit_list(range_spec: str, paths: tuple[str, ...] | None = None) -> list[dict[str, str]]:
    args = ["log", range_spec, "--no-merges", "--format=%H%x00%s"]
    if paths:
        args += ["--", *paths]
    out = git(*args)
    if not out:
        return []
    return [dict(zip(("sha", "subject"), line.split("\0", 1))) for line in out.splitlines()]


def stable_tags(base_sha: str) -> list[tuple[int, str, str]]:
    tags: list[tuple[int, str, str]] = []
    for tag in git("tag", "--merged", base_sha, "--list").splitlines():
        if not re.fullmatch(r"(?:desktop-)?v[0-9]+\.[0-9]+\.[0-9]+", tag):
            continue
        sha = git("rev-list", "-n", "1", tag)
        distance = int(git("rev-list", "--count", f"{sha}..{base_sha}"))
        tags.append((distance, tag, sha))
    return tags


def previous_tag(base_sha: str) -> str:
    tags = stable_tags(base_sha)
    if not tags:
        return ""
    min_distance = min(item[0] for item in tags)
    nearest = [item for item in tags if item[0] == min_distance]
    commits = {item[2] for item in nearest}
    if len(commits) != 1:
        detail = ", ".join(f"{tag}@{sha}" for _, tag, sha in nearest)
        raise SystemExit(f"ambiguous previous desktop release tags: {detail}")
    # During migration, prefer the namespaced tag when aliases share a commit.
    nearest.sort(key=lambda item: (not item[1].startswith("desktop-v"), item[1]))
    return nearest[0][1]


def bullet(commit: dict[str, str], repo: str) -> str:
    sha, subject = commit["sha"], commit["subject"]
    short = sha[:12]
    pr_match = re.search(r" \(#([0-9]+)\)$", subject)
    if pr_match:
        pr = pr_match.group(1)
        subject = subject[: pr_match.start()]
        return f"- {subject} ([#{pr}](https://github.com/{repo}/pull/{pr})) ([`{sha}`](https://github.com/{repo}/commit/{sha}))"
    return f"- {subject} ([`{sha}`](https://github.com/{repo}/commit/{sha}))"


def expected(base_sha: str, previous: str) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    # With no prior desktop tag, account for the repository's root commit too.
    # A ``root..base`` range silently drops that first commit.
    range_spec = f"{previous}..{base_sha}" if previous else base_sha
    all_commits = commit_list(range_spec)
    relevant_shas = {c["sha"] for c in commit_list(range_spec, DESKTOP_PATHS)}
    relevant = [c for c in all_commits if c["sha"] in relevant_shas]
    other = [c for c in all_commits if c["sha"] not in relevant_shas]
    return relevant, other


def render(version: str, base_sha: str, previous: str, repo: str) -> tuple[str, list[str]]:
    relevant, other = expected(base_sha, previous)
    lines = [f"## v{version}", "", "### Desktop and shared changes", ""]
    lines += [bullet(c, repo) for c in relevant] or ["- None"]
    lines += ["", "### Other repository changes", ""]
    lines += [bullet(c, repo) for c in other] or ["- None"]
    compare_start = previous or git("rev-list", "--max-parents=0", base_sha).splitlines()[0]
    lines += ["", f"[Compare {compare_start}...desktop-v{version}](https://github.com/{repo}/compare/{compare_start}...desktop-v{version})"]
    return "\n".join(lines) + "\n", [c["sha"] for c in relevant + other]


def generate(args: argparse.Namespace) -> None:
    if not SEMVER.fullmatch(args.version):
        raise SystemExit(f"invalid semver: {args.version}")
    base_sha = git("rev-parse", args.base)
    previous = previous_tag(base_sha)
    repo = args.repo or re.sub(r".*github\.com[:/]", "", git("remote", "get-url", "origin")).removesuffix(".git")
    block, commits = render(args.version, base_sha, previous, repo)
    old = CHANGELOG.read_text() if CHANGELOG.exists() else "# Changelog\n"
    if not old.startswith("# Changelog"):
        raise SystemExit("CHANGELOG.md must begin with '# Changelog'")
    remainder = old.split("\n", 1)[1].lstrip("\n") if "\n" in old else ""
    CHANGELOG.write_text(f"# Changelog\n\n{block}\n{remainder}")
    METADATA.parent.mkdir(parents=True, exist_ok=True)
    METADATA.write_text(json.dumps({
        "schema": 1,
        "version": args.version,
        "base_sha": base_sha,
        "previous_tag": previous or None,
        "tag": f"desktop-v{args.version}",
        "commit_count": len(commits),
        "local_dev_production_profile": ".release/local-dev-production.json",
    }, indent=2) + "\n")


def validate(args: argparse.Namespace) -> None:
    data = json.loads(METADATA.read_text())
    version = args.version or data["version"]
    if data != {**data, "version": version}:
        raise SystemExit("candidate version does not match metadata")
    if data["tag"] != f"desktop-v{version}":
        raise SystemExit("candidate tag does not match version")
    candidate = git("rev-parse", args.candidate)
    parents = git("show", "-s", "--format=%P", candidate).split()
    if len(parents) != 1 or parents[0] != data["base_sha"]:
        raise SystemExit("candidate must be one commit directly above recorded base_sha")
    changed = set(git("diff-tree", "--no-commit-id", "--name-only", "-r", candidate).splitlines())
    unexpected = changed - CANDIDATE_FILES
    missing = REQUIRED_CANDIDATE_FILES - changed
    if unexpected or missing:
        detail = []
        if unexpected:
            detail.append(f"unexpected files: {', '.join(sorted(unexpected))}")
        if missing:
            detail.append(f"missing required files: {', '.join(sorted(missing))}")
        raise SystemExit("candidate is not version-only (" + "; ".join(detail) + ")")
    previous = data["previous_tag"] or ""
    actual_previous = previous_tag(data["base_sha"])
    if previous != actual_previous:
        raise SystemExit(
            f"recorded previous tag {previous or '<none>'} does not match "
            f"nearest release tag {actual_previous or '<none>'}"
        )
    repo = args.repo or "block/buzz"
    expected_block, shas = render(version, data["base_sha"], previous, repo)
    text = CHANGELOG.read_text()
    blocks = re.findall(rf"(?ms)^## v{re.escape(version)}\n.*?(?=^## v|\Z)", text)
    if len(blocks) != 1:
        raise SystemExit(f"expected exactly one changelog block for v{version}")
    if blocks[0].rstrip() != expected_block.rstrip():
        raise SystemExit("changelog block is not deterministic for recorded candidate base")
    found = re.findall(r"\[`([0-9a-f]{40})`\]", blocks[0])
    if len(found) != len(set(found)) or set(found) != set(shas) or len(found) != data["commit_count"]:
        raise SystemExit("changelog does not account for every expected non-merge commit exactly once")
    manifests = {
        ROOT / "desktop/package.json": json.loads((ROOT / "desktop/package.json").read_text())["version"],
        ROOT / "desktop/src-tauri/tauri.conf.json": json.loads((ROOT / "desktop/src-tauri/tauri.conf.json").read_text())["version"],
    }
    cargo = re.search(r'(?m)^version = "([^"]+)"', (ROOT / "desktop/src-tauri/Cargo.toml").read_text())
    manifests[ROOT / "desktop/src-tauri/Cargo.toml"] = cargo.group(1) if cargo else ""
    bad = [str(path.relative_to(ROOT)) for path, value in manifests.items() if value != version]
    if bad:
        raise SystemExit(f"version mismatch in: {', '.join(bad)}")
    author = git("show", "-s", "--format=%an <%ae>", candidate)
    body = git("show", "-s", "--format=%B", candidate)
    if author != "Wes <wesbillman@users.noreply.github.com>":
        raise SystemExit(f"unexpected candidate author: {author}")
    if "Signed-off-by: Wes <wesbillman@users.noreply.github.com>" not in body:
        raise SystemExit("candidate is missing Wes Signed-off-by trailer")
    if not re.search(r"(?m)^Co-authored-by: .+ <.+>$", body):
        raise SystemExit("candidate is missing automation Co-authored-by trailer")
    print(f"validated immutable desktop candidate {candidate} for desktop-v{version}")
    profile_path = data.get("local_dev_production_profile")
    if profile_path:
        if profile_path != ".release/local-dev-production.json":
            raise SystemExit("candidate local_dev_production_profile pointer drifted")
        if not LOCAL_DEV_PROFILE.is_file():
            raise SystemExit("local-dev production profile is missing from the release lane")


def sha256_bytes(data: bytes) -> str:
    import hashlib

    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def expand_user_path(template: str) -> str:
    import os

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


def require_clean_head(cwd: Path | None = None) -> str:
    repo = cwd or ROOT
    status = subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True)
    if status.strip():
        raise SystemExit("working tree is not clean; refuse to hash a dirty source tree")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()


def source_tree_rows(cwd: Path | None = None) -> list[tuple[str, str]]:
    repo = cwd or ROOT
    out = subprocess.check_output(["git", "ls-files", "-z"], cwd=repo)
    rows: list[tuple[str, str]] = []
    for rel in out.split(b"\0"):
        if not rel:
            continue
        path = repo / rel.decode()
        if path.is_file() and not path.is_symlink():
            rows.append((rel.decode(), sha256_file(path)))
    rows.sort()
    if not rows:
        raise SystemExit("source tree hash is empty; refuse a selected-file subset")
    return rows


def content_digest(rows: list[tuple[str, str]]) -> str:
    payload = "".join(f"{rel}\0{digest}\n" for rel, digest in rows).encode()
    return sha256_bytes(payload)


def load_local_dev_profile() -> dict:
    profile = load_json(LOCAL_DEV_PROFILE)
    if profile.get("profile") != LOCAL_DEV_PROFILE_NAME:
        raise SystemExit("local-dev production profile name drifted")
    if profile.get("bundle_identifier") != BUNDLE_IDENTIFIER:
        raise SystemExit("local-dev production profile bundle identifier drifted")
    if profile.get("keyring_service") != PRODUCTION_KEYRING:
        raise SystemExit("local-dev production profile keyring service drifted")
    if profile.get("relay_ws_url") != PRODUCTION_RELAY:
        raise SystemExit("local-dev production profile relay drifted (must be :3300)")
    if profile.get("owner_display_prefix") != OWNER_DISPLAY_PREFIX:
        raise SystemExit("local-dev production display prefix drifted")
    if profile.get("frontend_dist") != FRONTEND_DIST:
        raise SystemExit("local-dev production frontendDist drifted")
    if profile.get("buzz_transport") != "optional-to-transport":
        raise SystemExit(
            "buzz_transport must be optional to transport; Desktop requires the relay"
        )
    if profile.get("desktop_requires_relay") is not True:
        raise SystemExit("desktop_requires_relay must be true")
    if profile.get("owner_pin_required") is not True:
        raise SystemExit("production profile pin is required")
    return profile


def prove_frontend_dist(tauri_conf: dict) -> dict:
    frontend_dist = tauri_conf.get("build", {}).get("frontendDist")
    identifier = tauri_conf.get("identifier")
    if identifier != BUNDLE_IDENTIFIER:
        raise SystemExit(f"tauri identifier {identifier!r} is not {BUNDLE_IDENTIFIER}")
    if frontend_dist != FRONTEND_DIST:
        raise SystemExit(
            f"tauri frontendDist {frontend_dist!r} is not {FRONTEND_DIST}; "
            "production packaging must not use Vite devUrl"
        )
    return {
        "mode": "frontendDist",
        "path": FRONTEND_DIST,
        "dev_url_active": False,
        "identifier": identifier,
    }


def normalize_digest(value: str) -> str:
    raw = value.strip().lower()
    hex_part = raw.removeprefix("sha256:")
    if not re.fullmatch(r"[0-9a-f]{64}", hex_part):
        raise SystemExit(f"digest {value!r} is not sha256:<64 hex>")
    return f"sha256:{hex_part}"


def normalize_owner_pubkey(value: str) -> str:
    hex_part = value.strip().lower()
    if not PUBKEY_RE.fullmatch(hex_part):
        raise SystemExit("owner pubkey pin must be the complete 64-char hex public key")
    if not hex_part.startswith(OWNER_DISPLAY_PREFIX):
        raise SystemExit(
            f"owner pubkey pin display prefix is not {OWNER_DISPLAY_PREFIX} "
            "(prefix is display-only; the complete key is the boundary)"
        )
    return hex_part


def compiled_owner_pin(profile: dict) -> dict:
    """Pin comes only from the compiled in-tree profile. CLI/env cannot claim exact."""
    import hashlib

    pubkey = profile.get("owner_pubkey")
    digest = profile.get("owner_pubkey_sha256")
    pin: dict[str, str | None] = {
        "owner_pubkey": None,
        "owner_pubkey_sha256": None,
        "status": "unpinned",
        "admission": "fail-closed",
    }
    if pubkey:
        pin["owner_pubkey"] = normalize_owner_pubkey(str(pubkey))
        pin["status"] = "exact"
        pin["admission"] = "pinned"
    if digest:
        pin["owner_pubkey_sha256"] = normalize_digest(str(digest))
        pin["status"] = "digest" if pin["status"] == "unpinned" else "exact+digest"
        pin["admission"] = "pinned"
    if pin["owner_pubkey"] and pin["owner_pubkey_sha256"]:
        hashed = "sha256:" + hashlib.sha256(bytes.fromhex(pin["owner_pubkey"])).hexdigest()
        if hashed != pin["owner_pubkey_sha256"]:
            raise SystemExit("owner pubkey pin does not match owner_pubkey_sha256")
    if pin["status"] == "unpinned":
        pin["admission"] = "fail-closed"
    return pin


def pin_is_compiled(pin: dict) -> bool:
    return pin.get("status") not in {None, "unpinned", "missing"} and bool(
        pin.get("owner_pubkey") or pin.get("owner_pubkey_sha256")
    )


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


def digest_dir(release_root: Path, digest: str) -> Path:
    return release_root / "releases" / digest.replace(":", "-")


def validate_local_dev_manifest(manifest: dict, *, live: bool) -> None:
    if manifest.get("profile") != LOCAL_DEV_PROFILE_NAME:
        raise SystemExit("manifest profile drifted")
    if manifest.get("bundle_identifier") != BUNDLE_IDENTIFIER:
        raise SystemExit("manifest bundle identifier drifted")
    if manifest.get("keyring_service") != PRODUCTION_KEYRING:
        raise SystemExit("manifest keyring service drifted")
    if manifest.get("relay_ws_url") != PRODUCTION_RELAY:
        raise SystemExit("manifest relay drifted")
    if manifest.get("owner_display_prefix") != OWNER_DISPLAY_PREFIX:
        raise SystemExit("manifest display prefix is not a substitute for the owner pin")
    if not COMMIT_RE.fullmatch(str(manifest.get("source_commit", ""))):
        raise SystemExit("manifest source_commit must be a full 40-char SHA")
    if not DIGEST_RE.fullmatch(str(manifest.get("content_digest", ""))):
        raise SystemExit("manifest content_digest must be sha256:<64 hex>")
    if manifest.get("buzz_transport") != "optional-to-transport":
        raise SystemExit("manifest must keep Desktop optional to buzz_transport")
    if manifest.get("desktop_requires_relay") is not True:
        raise SystemExit("manifest must require the relay for Desktop")
    leftovers = manifest.get("leftovers") or []
    leftover = next((item for item in leftovers if item.get("id") == MAC_LEFTOVER_ID), None)
    if leftover is None:
        raise SystemExit(f"manifest must record leftover {MAC_LEFTOVER_ID}")
    macos_app = (manifest.get("artifacts") or {}).get("macos_app")
    pin = manifest.get("owner_pin") or {}
    if pin.get("status") in {"exact", "digest", "exact+digest"} and not pin_is_compiled(pin):
        raise SystemExit("manifest must not claim an exact pin when the compiled profile is unpinned")
    if live:
        if leftover.get("status") != "satisfied":
            raise SystemExit("live/ is only for a proven candidate; leftover must be satisfied")
        if not pin_is_compiled(pin):
            raise SystemExit("live package admission fails closed without a compiled owner public-key pin")
        if not proven_macos_artifact(macos_app):
            raise SystemExit(
                "live/ requires independent codesign, Team ID, Gatekeeper, stapler, and digest"
            )
        return
    if leftover.get("status") != "needed":
        raise SystemExit("Linux/source package must leave mac-packaged-app-build needed")
    if macos_app is not None:
        raise SystemExit("source-only package must not claim a macOS .app artifact")
    if pin.get("status") == "exact" and not pin_is_compiled(pin):
        raise SystemExit("compiled pins are null; manifest must not say exact")


def package_local_dev(args: argparse.Namespace) -> None:
    release_root = args.release_root.expanduser().resolve()
    if forbidden_runtime_root(ROOT, release_root):
        raise SystemExit(
            f"release root {release_root} must be outside the source checkout "
            "and outside DawsOS reports/ops"
        )
    profile = load_local_dev_profile()
    tauri_conf = load_json(TAURI_CONF)
    frontend_proof = prove_frontend_dist(tauri_conf)
    state_root, log_root = default_runtime_roots(profile)
    for label, path in ("state_root", state_root), ("log_root", log_root):
        if forbidden_runtime_root(ROOT, Path(path)):
            raise SystemExit(f"{label} {path} is forbidden")

    head = require_clean_head()
    requested = args.source_commit
    if requested and requested != head:
        raise SystemExit(
            f"source_commit {requested} does not match HEAD {head}; "
            "an arbitrary 40-char SHA is not accepted"
        )
    rows = source_tree_rows()
    digest = content_digest(rows)
    if getattr(args, "owner_pubkey", None) or getattr(args, "owner_pubkey_sha256", None):
        raise SystemExit(
            "owner pin CLI override is forbidden; pin must come from the compiled profile"
        )
    pin = compiled_owner_pin(profile)

    leftovers = [
        {
            "id": MAC_LEFTOVER_ID,
            "status": "needed",
            "reason": (
                "This host cannot produce a signed macOS .app. leftover "
                "mac-packaged-app-build stays needed until independent "
                "codesign, Team ID, Gatekeeper, stapler, and digest succeed "
                "on a real .app. live/ is not written for unsigned evidence."
            ),
        }
    ]
    manifest = {
        "schema_version": 1,
        "profile": LOCAL_DEV_PROFILE_NAME,
        "source_commit": head,
        "content_digest": digest,
        "bundle_identifier": BUNDLE_IDENTIFIER,
        "keyring_service": PRODUCTION_KEYRING,
        "relay_ws_url": PRODUCTION_RELAY,
        "owner_display_prefix": OWNER_DISPLAY_PREFIX,
        "owner_pin": pin,
        "frontend": frontend_proof,
        "state_root": state_root,
        "log_root": log_root,
        "buzz_transport": "optional-to-transport",
        "transport_requires_desktop": False,
        "desktop_requires_relay": True,
        "artifacts": {"macos_app": None},
        "leftovers": leftovers,
        "lane": "scripts/desktop_release.py",
    }
    validate_local_dev_manifest(manifest, live=False)

    dest = digest_dir(release_root, digest)
    if dest.exists():
        raise SystemExit(f"digest directory {dest} already exists; publication is write-once")
    dest.mkdir(parents=True, exist_ok=False)
    (dest / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (dest / "profile.json").write_text(LOCAL_DEV_PROFILE.read_text())
    proofs = dest / "proofs"
    proofs.mkdir()
    (proofs / "frontend-dist.json").write_text(json.dumps(frontend_proof, indent=2) + "\n")
    (proofs / "source-tree.json").write_text(
        json.dumps([{"path": rel, "digest": d} for rel, d in rows], indent=2) + "\n"
    )
    (dest / "artifacts").mkdir()
    (dest / "artifacts" / "README").write_text(
        "No Buzz.app is produced on Linux. Leftover: mac-packaged-app-build.\n"
        "Do not treat any file in this directory as a macOS application.\n"
        "A boolean admit_macos_app_artifact(true) is not proof of a signed app.\n"
    )
    previous = read_pointer(release_root, "current")
    if previous and previous != digest:
        write_pointer(release_root, "previous", previous)
    write_pointer(release_root, "current", digest)
    print(dest / "manifest.json")


def recompute_stored_digest(dest: Path) -> str:
    proof = load_json(dest / "proofs" / "source-tree.json")
    rows = [(item["path"], item["digest"]) for item in proof]
    return content_digest(rows)


def verify_local_dev(args: argparse.Namespace) -> None:
    release_root = args.release_root.expanduser().resolve()
    current = read_pointer(release_root, "current")
    if current is None:
        raise SystemExit("no current pointer")
    dest = digest_dir(release_root, current)
    manifest = load_json(dest / "manifest.json")
    validate_local_dev_manifest(manifest, live=False)
    if manifest["content_digest"] != current:
        raise SystemExit("current pointer does not match manifest content_digest")
    stored = recompute_stored_digest(dest)
    if stored != manifest["content_digest"]:
        raise SystemExit("stored source-tree proof does not recompute to content_digest")
    live_rows = source_tree_rows()
    live_digest = content_digest(live_rows)
    if live_digest != manifest["content_digest"]:
        raise SystemExit(
            f"verification recomputed source-tree digest {live_digest} "
            f"!= manifest {manifest['content_digest']}"
        )
    head = require_clean_head()
    if head != manifest["source_commit"]:
        raise SystemExit(
            f"verification HEAD {head} does not match recorded source_commit "
            f"{manifest['source_commit']}"
        )
    prove_frontend_dist(load_json(TAURI_CONF))
    live_dest = dest / "live"
    if live_dest.is_dir():
        live_manifest = load_json(live_dest / "manifest.json")
        validate_local_dev_manifest(live_manifest, live=True)
        artifact = (live_manifest.get("artifacts") or {}).get("macos_app") or {}
        app_path = artifact.get("app_path")
        if not app_path:
            raise SystemExit("live/ manifest is missing the real .app path")
        evidence = macos_signing_evidence(Path(app_path))
        if not evidence["signed"] or not evidence["notarized"]:
            raise SystemExit(
                "live/ artifact failed independent re-verification; "
                f"{evidence.get('reason')}"
            )
        if evidence["sha256"] != artifact.get("sha256"):
            raise SystemExit("live/ artifact digest does not match independent recompute")
    print(f"verified local-dev production digest {current}")


def rollback_local_dev(args: argparse.Namespace) -> None:
    release_root = args.release_root.expanduser().resolve()
    current = read_pointer(release_root, "current")
    if current is None:
        raise SystemExit("no current release to roll back from")
    target = normalize_digest(args.target_digest)
    dest = digest_dir(release_root, target)
    if not (dest / "manifest.json").is_file():
        raise SystemExit(f"rollback target {target} is not a published digest")
    manifest = load_json(dest / "manifest.json")
    recomputed = recompute_stored_digest(dest)
    if recomputed != target or recomputed != manifest["content_digest"]:
        raise SystemExit(
            "rollback target tree digest did not recompute; "
            "mutable pointer files are not authority"
        )
    if target == current:
        raise SystemExit("rollback target must not equal current")
    write_pointer(release_root, "previous", current)
    write_pointer(release_root, "current", target)
    print(target)


TEAM_ID_RE = re.compile(r"^[A-Z0-9]{10}$")


def sha256_tree(path: Path) -> str:
    if path.is_file():
        return sha256_file(path)
    rows: list[tuple[str, str]] = []
    for child in sorted(path.rglob("*")):
        if child.is_file() and not child.is_symlink():
            rows.append((str(child.relative_to(path)), sha256_file(child)))
    if not rows:
        return sha256_bytes(path.as_posix().encode())
    return content_digest(rows)


def _run_macos_tool(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def macos_signing_evidence(app_path: Path | None) -> dict:
    """Independently inspect a real .app. Caller strings never set signed/notarized."""
    evidence: dict = {
        "app_path": None,
        "sha256": None,
        "signed": False,
        "notarized": False,
        "codesign_identity": None,
        "team_id": None,
        "notarization": None,
        "stapled": False,
        "codesign_verify": False,
        "gatekeeper": False,
        "reason": None,
    }
    if app_path is None:
        evidence["reason"] = "no real .app path; fail closed"
        return evidence
    resolved = app_path.expanduser().resolve()
    evidence["app_path"] = str(resolved)
    if not resolved.exists():
        evidence["reason"] = f"{resolved} does not exist; fail closed"
        return evidence
    if not resolved.name.endswith(".app"):
        evidence["sha256"] = sha256_tree(resolved)
        evidence["reason"] = "path is not a .app bundle; fail closed"
        return evidence
    evidence["sha256"] = sha256_tree(resolved)
    if sys.platform != "darwin":
        evidence["reason"] = (
            "this host cannot run codesign, spctl, or stapler; "
            "fail closed (do not fake macOS tools)"
        )
        return evidence
    verify = _run_macos_tool(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(resolved)])
    evidence["codesign_verify"] = verify.returncode == 0
    if verify.returncode != 0:
        evidence["reason"] = "codesign --verify failed"
        return evidence
    info = _run_macos_tool(["codesign", "--display", "--verbose=4", str(resolved)])
    text = f"{info.stderr}\n{info.stdout}"
    team_id = None
    identity = None
    for line in text.splitlines():
        if line.startswith("TeamIdentifier="):
            team_id = line.split("=", 1)[1].strip()
        if line.startswith("Authority=") and identity is None:
            identity = line.split("=", 1)[1].strip()
    if not team_id or team_id == "not set" or not TEAM_ID_RE.fullmatch(team_id):
        evidence["reason"] = "Team ID missing or invalid from codesign display"
        return evidence
    evidence["team_id"] = team_id
    evidence["codesign_identity"] = identity
    gate = _run_macos_tool(["spctl", "--assess", "--type", "execute", "--verbose", str(resolved)])
    evidence["gatekeeper"] = gate.returncode == 0
    if gate.returncode != 0:
        evidence["reason"] = "Gatekeeper assessment failed"
        return evidence
    staple = _run_macos_tool(["stapler", "validate", str(resolved)])
    if staple.returncode != 0:
        staple = _run_macos_tool(["xcrun", "stapler", "validate", str(resolved)])
    evidence["stapled"] = staple.returncode == 0
    if not evidence["stapled"]:
        evidence["reason"] = "stapled notarization ticket missing"
        return evidence
    evidence["notarization"] = "stapler-validate"
    evidence["signed"] = True
    evidence["notarized"] = True
    evidence["reason"] = (
        "independent codesign verify, Team ID, Gatekeeper, and stapler validation succeeded"
    )
    return evidence


def proven_macos_artifact(artifact: dict | None) -> bool:
    if not artifact:
        return False
    return (
        artifact.get("signed") is True
        and artifact.get("notarized") is True
        and artifact.get("stapled") is True
        and artifact.get("codesign_verify") is True
        and artifact.get("gatekeeper") is True
        and bool(DIGEST_RE.fullmatch(str(artifact.get("sha256") or "")))
        and bool(artifact.get("team_id"))
        and bool(artifact.get("codesign_identity"))
        and bool(artifact.get("notarization"))
        and bool(artifact.get("app_path"))
    )


def write_candidate_evidence(dest: Path, evidence: dict) -> Path:
    candidate = dest / "candidate" / "evidence"
    candidate.mkdir(parents=True, exist_ok=True)
    path = candidate / "macos-app.json"
    path.write_text(json.dumps(evidence, indent=2) + "\n")
    return path


def admit_local_dev_app(args: argparse.Namespace) -> None:
    release_root = args.release_root.expanduser().resolve()
    current = read_pointer(release_root, "current")
    if current is None:
        raise SystemExit("no current source package")
    dest = digest_dir(release_root, current)
    manifest = load_json(dest / "manifest.json")
    validate_local_dev_manifest(manifest, live=False)
    if args.boolean_true:
        raise SystemExit("admit_macos_app_artifact(true) is not proof of a signed app")
    for forbidden in ("codesign_identity", "team_id", "notarization", "stapled"):
        if getattr(args, forbidden, None):
            raise SystemExit(f"{forbidden} CLI strings are not signing evidence")
    evidence = macos_signing_evidence(args.app)
    if args.app_hash:
        claimed = normalize_digest(args.app_hash)
        if evidence["sha256"] and evidence["sha256"] != claimed:
            raise SystemExit("claimed --app-hash does not match the recomputed artifact digest")
        if evidence["sha256"] is None:
            evidence["claimed_sha256"] = claimed
    evidence_path = write_candidate_evidence(dest, evidence)
    leftover_needed = (
        not evidence["signed"]
        or not evidence["notarized"]
        or not pin_is_compiled(manifest.get("owner_pin") or {})
    )
    if leftover_needed:
        print(evidence_path)
        raise SystemExit(
            "incomplete observation written under candidate/evidence; "
            "live/ is write-once for a proven signed and notarized candidate only"
        )
    live = dict(manifest)
    live["artifacts"] = {"macos_app": evidence}
    live["leftovers"] = [
        {
            "id": MAC_LEFTOVER_ID,
            "status": "satisfied",
            "reason": evidence["reason"],
        }
    ]
    validate_local_dev_manifest(live, live=True)
    live_dest = dest / "live"
    if live_dest.exists():
        raise SystemExit(f"{live_dest} already exists; live publication is write-once")
    live_dest.mkdir()
    (live_dest / "manifest.json").write_text(json.dumps(live, indent=2) + "\n")
    print(live_dest / "manifest.json")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    gen = sub.add_parser("generate")
    gen.add_argument("version")
    gen.add_argument("--base", required=True)
    gen.add_argument("--repo")
    val = sub.add_parser("validate")
    val.add_argument("--candidate", default="HEAD")
    val.add_argument("--version")
    val.add_argument("--repo")
    pkg = sub.add_parser("local-dev-package", help="content-addressed local-dev production source package")
    pkg.add_argument("--release-root", required=True, type=Path)
    pkg.add_argument("--source-commit", default=None)
    ver = sub.add_parser("local-dev-verify", help="recompute the complete source-tree digest")
    ver.add_argument("--release-root", required=True, type=Path)
    rb = sub.add_parser("local-dev-rollback", help="activate an authenticated target tree digest")
    rb.add_argument("--release-root", required=True, type=Path)
    rb.add_argument("--target-digest", required=True)
    admit = sub.add_parser(
        "local-dev-admit-app",
        help="independently inspect a real .app; live/ only if signed and notarized",
    )
    admit.add_argument("--release-root", required=True, type=Path)
    admit.add_argument("--app", type=Path, help="path to a real Buzz.app bundle")
    admit.add_argument("--app-hash", help="optional claimed digest; must match the recomputed tree")
    admit.add_argument("--boolean-true", action="store_true", help="rejected: boolean is not proof")
    args = parser.parse_args()
    if args.command == "generate":
        generate(args)
    elif args.command == "validate":
        validate(args)
    elif args.command == "local-dev-package":
        package_local_dev(args)
    elif args.command == "local-dev-verify":
        verify_local_dev(args)
    elif args.command == "local-dev-rollback":
        rollback_local_dev(args)
    elif args.command == "local-dev-admit-app":
        admit_local_dev_app(args)
    else:
        raise SystemExit(f"unknown command {args.command}")


if __name__ == "__main__":
    main()
