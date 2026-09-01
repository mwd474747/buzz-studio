#!/usr/bin/env python3
"""Generate and validate immutable desktop release candidates."""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import plistlib
import re
import secrets
import stat
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
TEAM_ID_RE = re.compile(r"^[A-Z0-9]{10}$")
MAC_LEFTOVER_ID = "mac-packaged-app-build"
SIGNING_PIN_LEFTOVER_ID = "approved-macos-signing-pin"
PRODUCER_LEFTOVER_ID = "mac-controlled-candidate-producer"
ROLLBACK_LEFTOVER_ID = "historical-package-rollback"
GIT_OBJECT_RE = re.compile(r"^[0-9a-f]{40,64}$")
GIT_TREE_MODES = {
    ("blob", "100644"),
    ("blob", "100755"),
    ("blob", "120000"),
    ("commit", "160000"),
}
INDEPENDENT_ATTESTATION_CLASS = "independent-builder-attestation"
SELF_ATTESTED_CLASSES = frozenset(
    {None, "self-attested", "caller-supplied", "manufactured", "self-attested-disabled"}
)
CODESIGN = Path("/usr/bin/codesign")
SPCTL = Path("/usr/sbin/spctl")
XCRUN = Path("/usr/bin/xcrun")
TRUSTED_APPLE_TOOLS = {str(CODESIGN), str(SPCTL), str(XCRUN)}
APPLE_TOOL_ENV = {"PATH": "/usr/bin:/bin", "LANG": "C"}
LOCAL_DEV_PROFILE_NAME = "local-dev-production"
LIVE_IMMUTABLE_FIELDS = (
    "schema_version",
    "profile",
    "source_commit",
    "content_digest",
    "bundle_identifier",
    "keyring_service",
    "relay_ws_url",
    "owner_display_prefix",
    "owner_pin",
    "frontend",
    "state_root",
    "log_root",
    "buzz_transport",
    "transport_requires_desktop",
    "desktop_requires_relay",
    "lane",
    "compiled_profile_sha256",
)
APP_EMBEDDED_PROFILE = Path("Contents/Resources/.release/local-dev-production.json")
APP_SOURCE_RECEIPT = Path("Contents/Resources/.release/source-receipt.json")
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


def source_tree_rows(cwd: Path | None = None) -> list[tuple[str, str, str, str]]:
    """Git tree identity: path, type, mode, and blob/gitlink object. No working-tree subset."""
    repo = cwd or ROOT
    out = subprocess.check_output(
        ["git", "ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        cwd=repo,
    )
    rows: list[tuple[str, str, str, str]] = []
    for entry in out.split(b"\0"):
        if not entry:
            continue
        if b"\t" not in entry:
            raise SystemExit("git ls-tree entry is missing a path separator")
        meta, path_raw = entry.split(b"\t", 1)
        parts = meta.split(b" ")
        if len(parts) != 3:
            raise SystemExit("git ls-tree entry is not mode type object")
        mode = parts[0].decode()
        typ = parts[1].decode()
        obj = parts[2].decode()
        path = path_raw.decode()
        if (typ, mode) not in GIT_TREE_MODES:
            raise SystemExit(f"unsupported git tree entry {typ} {mode} at {path}")
        if not GIT_OBJECT_RE.fullmatch(obj):
            raise SystemExit(f"git tree object is not a blob/gitlink id at {path}")
        if not path or path.startswith("/") or ".." in path.split("/"):
            raise SystemExit(f"git tree path is not a contained relative path: {path}")
        rows.append((path, typ, mode, obj))
    rows.sort()
    if not rows:
        raise SystemExit("source tree hash is empty; refuse a selected-file subset")
    return rows


def content_digest(rows: list[tuple[str, ...]]) -> str:
    payload = "".join("\0".join(row) + "\n" for row in rows).encode()
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
    if profile.get("macos_signing_pin_required") is not True:
        raise SystemExit("macos signing pin is required and must fail closed when empty")
    for key in ("approved_team_id", "approved_codesign_identity"):
        value = profile.get(key)
        if value is None:
            continue
        if not isinstance(value, str) or not value.strip():
            raise SystemExit(f"{key} must be a non-empty string or null")
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


def compiled_profile_raw() -> bytes:
    return LOCAL_DEV_PROFILE.read_bytes()


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


def pin_from_compiled_json(raw: bytes | None = None) -> dict:
    return compiled_owner_pin(json.loads(raw if raw is not None else compiled_profile_raw()))


def pin_equals_compiled(pin: dict, compiled: dict) -> bool:
    return (
        pin.get("owner_pubkey") == compiled.get("owner_pubkey")
        and pin.get("owner_pubkey_sha256") == compiled.get("owner_pubkey_sha256")
        and pin.get("status") == compiled.get("status")
        and pin.get("admission") == compiled.get("admission")
    )


def pin_is_compiled(pin: dict) -> bool:
    """True only when pin equals the pin derived from compiled profile JSON bytes."""
    raw = compiled_profile_raw()
    try:
        compiled = pin_from_compiled_json(raw)
    except SystemExit:
        return False
    if compiled.get("status") in {None, "unpinned", "missing"}:
        return False
    if not (compiled.get("owner_pubkey") or compiled.get("owner_pubkey_sha256")):
        return False
    return pin_equals_compiled(pin, compiled)


def require_manifest_pin_matches_compiled(pin: dict) -> dict:
    raw = compiled_profile_raw()
    compiled = pin_from_compiled_json(raw)
    if pin.get("status") in {"exact", "digest", "exact+digest"} and compiled.get("status") == "unpinned":
        raise SystemExit("forged exact manifest pin denied; compiled profile JSON is unpinned")
    if not pin_equals_compiled(pin, compiled):
        raise SystemExit("manifest owner pin does not equal the compiled profile JSON")
    return compiled


def compiled_macos_signing_pins(profile: dict | None = None) -> dict:
    data = profile if profile is not None else json.loads(compiled_profile_raw())
    team = data.get("approved_team_id")
    identity = data.get("approved_codesign_identity")
    team = str(team).strip() if isinstance(team, str) and team.strip() else None
    identity = str(identity).strip() if isinstance(identity, str) and identity.strip() else None
    if team and not TEAM_ID_RE.fullmatch(team):
        raise SystemExit("approved_team_id must be a 10-char Apple Team ID or null")
    filled = bool(team and identity)
    return {
        "approved_team_id": team,
        "approved_codesign_identity": identity,
        "required": data.get("macos_signing_pin_required") is True,
        "filled": filled,
    }


def require_live_immutable_equals_source(source: dict, live: dict) -> None:
    for key in LIVE_IMMUTABLE_FIELDS:
        if live.get(key) != source.get(key):
            raise SystemExit(f"live manifest immutable field {key} drifted from source")


def default_runtime_roots(profile: dict) -> tuple[str, str]:
    """Expanded host paths for local forbidden-root checks only. Not authority."""
    if sys.platform == "darwin":
        state = expand_user_path(profile["state_root"]["macos"])
        logs = expand_user_path(profile["log_root"]["macos"])
    else:
        state = expand_user_path(profile["state_root"]["linux_test"])
        logs = expand_user_path(profile["log_root"]["linux_test"])
    return state, logs


def platform_neutral_runtime_roots(profile: dict) -> tuple[dict, dict]:
    """Source-package roots are templates so Linux and Mac reconstruct the same manifest."""
    state = profile.get("state_root")
    logs = profile.get("log_root")
    if not isinstance(state, dict) or not isinstance(logs, dict):
        raise SystemExit("state_root and log_root must be platform-neutral template objects")
    for name, templates in ("state_root", state), ("log_root", logs):
        if "macos" not in templates or "linux_test" not in templates:
            raise SystemExit(f"{name} templates must include macos and linux_test")
        for key, template in templates.items():
            if not isinstance(template, str) or not template.strip():
                raise SystemExit(f"{name}.{key} must be a non-empty template string")
            expanded = expand_user_path(template)
            if forbidden_runtime_root(ROOT, Path(expanded)):
                raise SystemExit(f"{name}.{key} {expanded} is forbidden")
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
    with held_release_root(release_root) as root:
        root.write_rel(name, digest + "\n", replace=True)


def package_dir_name(digest: str) -> str:
    if not DIGEST_RE.fullmatch(digest):
        raise SystemExit("current must be exactly sha256:<64 hex>")
    return digest.replace(":", "-")


def package_relpath(digest: str) -> str:
    return f"releases/{package_dir_name(digest)}"


def _relative_write_parts(relative_path: str) -> tuple[str, ...]:
    rel = Path(relative_path)
    if rel.is_absolute() or rel.anchor:
        raise SystemExit("package write path must be relative")
    parts = rel.parts
    if not parts or any(part in {".", "..", ""} for part in parts):
        raise SystemExit("package write path traversal denied")
    return parts


def _dir_open_flags() -> int:
    if not hasattr(os, "O_NOFOLLOW") or not hasattr(os, "O_DIRECTORY"):
        raise SystemExit("descriptor-anchored writes require O_NOFOLLOW and O_DIRECTORY")
    return os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW


def _file_excl_flags() -> int:
    if not hasattr(os, "O_NOFOLLOW"):
        raise SystemExit("descriptor-anchored writes require O_NOFOLLOW")
    return os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW


def _open_dirfd(path: str | None = None, *, dir_fd: int | None = None, name: str | None = None) -> int:
    flags = _dir_open_flags()
    try:
        if dir_fd is None:
            if path is None:
                raise SystemExit("directory descriptor root is missing")
            return os.open(path, flags)
        if name is None:
            raise SystemExit("directory descriptor name is missing")
        return os.open(name, flags, dir_fd=dir_fd)
    except OSError as exc:
        raise SystemExit(f"directory descriptor open failed: {exc}") from exc


def _lstat_child(dir_fd: int, name: str) -> os.stat_result:
    try:
        return os.stat(name, dir_fd=dir_fd, follow_symlinks=False)
    except OSError as exc:
        raise SystemExit(f"contained lstat failed: {exc}") from exc


def _mkdirat_open(dir_fd: int, name: str, *, exclusive: bool = False) -> int:
    try:
        os.mkdir(name, 0o755, dir_fd=dir_fd)
    except FileExistsError:
        if exclusive:
            raise SystemExit(f"digest directory {name} already exists; publication is write-once")
    except OSError as exc:
        raise SystemExit(f"contained mkdir failed: {exc}") from exc
    existing = _lstat_child(dir_fd, name)
    if stat.S_ISLNK(existing.st_mode):
        raise SystemExit(f"symlink destination denied: {name}")
    if not stat.S_ISDIR(existing.st_mode):
        raise SystemExit(f"contained path is not a directory: {name}")
    return _open_dirfd(name=name, dir_fd=dir_fd)


def _write_all(fd: int, payload: bytes) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise SystemExit("contained write made no progress")
        view = view[written:]


def _unlink_under(dir_fd: int, name: str) -> None:
    try:
        os.unlink(name, dir_fd=dir_fd)
    except FileNotFoundError:
        return
    except OSError as exc:
        raise SystemExit(f"contained temp unlink failed: {exc}") from exc


def _publish_temp(dir_fd: int, tmp_name: str, dest_name: str, *, replace: bool) -> None:
    """Publish a fsynced exclusive temp inode via descriptor-relative rename or link."""
    if replace:
        try:
            os.rename(tmp_name, dest_name, src_dir_fd=dir_fd, dst_dir_fd=dir_fd)
        except OSError as exc:
            _unlink_under(dir_fd, tmp_name)
            raise SystemExit(f"descriptor-relative rename failed: {exc}") from exc
        return
    try:
        os.link(
            tmp_name,
            dest_name,
            src_dir_fd=dir_fd,
            dst_dir_fd=dir_fd,
            follow_symlinks=False,
        )
    except FileExistsError:
        _unlink_under(dir_fd, tmp_name)
        raise SystemExit(f"immutable publish refused existing path: {dest_name}")
    except OSError as exc:
        _unlink_under(dir_fd, tmp_name)
        raise SystemExit(f"immutable no-replace publish failed: {exc}") from exc
    _unlink_under(dir_fd, tmp_name)


def _write_under_fd(root_fd: int, relative_path: str, data: str | bytes, *, replace: bool) -> None:
    """Write a fresh exclusive temp inode, fsync it, then publish from root_fd.

    Never reopens a descendant pathname and never truncates an existing inode.
    Intermediate components are opened only with openat(..., O_NOFOLLOW).
    """
    parts = _relative_write_parts(relative_path)
    payload = data.encode() if isinstance(data, str) else data
    dir_fd = root_fd
    opened: list[int] = []
    try:
        for part in parts[:-1]:
            child_fd = _mkdirat_open(dir_fd, part)
            opened.append(child_fd)
            dir_fd = child_fd
        dest_name = parts[-1]
        tmp_name = f".{dest_name}.tmp.{os.getpid()}.{secrets.token_hex(8)}"
        try:
            fd = os.open(tmp_name, _file_excl_flags(), 0o644, dir_fd=dir_fd)
        except OSError as exc:
            raise SystemExit(f"exclusive temp open failed: {exc}") from exc
        try:
            _write_all(fd, payload)
            os.fsync(fd)
        finally:
            os.close(fd)
        _publish_temp(dir_fd, tmp_name, dest_name, replace=replace)
        try:
            os.fsync(dir_fd)
        except OSError as exc:
            raise SystemExit(f"containing directory fsync failed: {exc}") from exc
    finally:
        for child_fd in reversed(opened):
            os.close(child_fd)


def _exists_under_fd(root_fd: int, relative_path: str) -> bool:
    parts = _relative_write_parts(relative_path)
    dir_fd = root_fd
    opened: list[int] = []
    try:
        for part in parts[:-1]:
            try:
                existing = os.stat(part, dir_fd=dir_fd, follow_symlinks=False)
            except FileNotFoundError:
                return False
            except OSError as exc:
                raise SystemExit(f"contained lstat failed: {exc}") from exc
            if stat.S_ISLNK(existing.st_mode):
                raise SystemExit(f"symlink destination denied: {part}")
            if not stat.S_ISDIR(existing.st_mode):
                raise SystemExit(f"contained path is not a directory: {part}")
            child_fd = _open_dirfd(name=part, dir_fd=dir_fd)
            opened.append(child_fd)
            dir_fd = child_fd
        try:
            os.stat(parts[-1], dir_fd=dir_fd, follow_symlinks=False)
        except FileNotFoundError:
            return False
        except OSError as exc:
            raise SystemExit(f"contained lstat failed: {exc}") from exc
        return True
    finally:
        for child_fd in reversed(opened):
            os.close(child_fd)


class HeldReleaseRoot:
    """One trusted release-root directory descriptor for a whole write operation."""

    def __init__(self, path: Path, fd: int) -> None:
        self.path = path
        self.fd = fd

    def child_exists(self, relative_path: str) -> bool:
        return _exists_under_fd(self.fd, relative_path)

    def mkdir_rel(self, relative_path: str, *, exclusive_leaf: bool = False) -> Path:
        parts = _relative_write_parts(relative_path)
        dir_fd = self.fd
        opened: list[int] = []
        try:
            for index, part in enumerate(parts):
                child_fd = _mkdirat_open(
                    dir_fd, part, exclusive=exclusive_leaf and index == len(parts) - 1
                )
                opened.append(child_fd)
                dir_fd = child_fd
        finally:
            for child_fd in reversed(opened):
                os.close(child_fd)
        return self.path.joinpath(*parts)

    def write_rel(self, relative_path: str, data: str | bytes, *, replace: bool = False) -> Path:
        _write_under_fd(self.fd, relative_path, data, replace=replace)
        return self.path.joinpath(*_relative_write_parts(relative_path))


@contextlib.contextmanager
def held_release_root(release_root: Path):
    """Open the trusted release-root directory once; close it after the operation."""
    path = Path(release_root).expanduser().resolve()
    fd = _open_dirfd(str(path))
    try:
        yield HeldReleaseRoot(path, fd)
    finally:
        os.close(fd)


def mkdir_contained(root: Path, relative_path: str = ".") -> Path:
    """Create directories under a trusted root via no-follow directory descriptors."""
    if relative_path in {".", ""}:
        return Path(root)
    with held_release_root(root) as held:
        return held.mkdir_rel(relative_path)


def write_package_file(
    package_root: Path,
    relative_path: str,
    data: str | bytes,
    *,
    replace: bool = False,
    overwrite_regular: bool | None = None,
) -> Path:
    """Single-root convenience write. Callers must pass the trusted root, not a descendant pathname.

    Multi-file operations must use one held_release_root() for the whole operation.
    Existing inodes are never truncated: publication is exclusive temp + fsync +
    descriptor-relative rename (replace) or no-replace link (immutable).
    `overwrite_regular` is accepted as an alias for replace-via-rename; it does not
    open or truncate the destination inode.
    """
    if overwrite_regular is not None:
        replace = overwrite_regular
    with held_release_root(package_root) as root:
        return root.write_rel(relative_path, data, replace=replace)


def digest_dir(release_root: Path, digest: str, *, must_exist: bool = False) -> Path:
    """Resolve a package dir as a child of release_root/releases. No path traversal."""
    if not DIGEST_RE.fullmatch(digest):
        raise SystemExit("current must be exactly sha256:<64 hex>")
    root = release_root.expanduser().resolve()
    releases = root / "releases"
    if releases.exists() or releases.is_symlink():
        resolved_releases = releases.resolve()
        if resolved_releases.parent != root:
            raise SystemExit("releases/ must be a direct child of the release root")
        dest = (releases / digest.replace(":", "-")).resolve()
        if dest.parent != resolved_releases:
            raise SystemExit("package destination escaped the release root")
    else:
        if must_exist:
            raise SystemExit("authenticated package destination is missing")
        dest = releases / digest.replace(":", "-")
    if must_exist and not (dest / "manifest.json").is_file():
        raise SystemExit("authenticated package manifest is missing")
    return dest


def source_package_leftovers(signing_pins: dict) -> list[dict]:
    leftovers = [
        {
            "id": MAC_LEFTOVER_ID,
            "status": "needed",
            "reason": (
                "This host cannot produce a signed macOS .app. leftover "
                "mac-packaged-app-build stays needed until independent "
                "codesign, compiled Team ID / identity, Gatekeeper, stapler, "
                "bundle identifier, executable, version, source receipt, and "
                "embedded profile succeed on this Buzz.app. live/ is not "
                "written for unsigned evidence. Any Apple-notarized app is not enough."
            ),
        }
    ]
    if not signing_pins["filled"]:
        leftovers.append(
            {
                "id": SIGNING_PIN_LEFTOVER_ID,
                "status": "needed",
                "reason": (
                    "approved_team_id and approved_codesign_identity are empty. "
                    "Leftover until the real Team ID is compiled in. "
                    "Do not invent a Team ID."
                ),
            }
        )
    leftovers.append(
        {
            "id": PRODUCER_LEFTOVER_ID,
            "status": "needed",
            "stage": 3,
            "reason": (
                "Stage 3 leftover: the self-attesting Mac producer is hard-disabled. "
                "It must not hash a caller-supplied .app and emit a matching "
                "receipt plus external provenance. Stage 3 recreates the package "
                "on the isolated Mac from the approved commit, then builds from "
                "that recreated source or consumes a builder attestation that "
                "Stage 3 authenticates itself. JSON attestation_class strings "
                "are not builder authority. Until then, admission refuses live/ "
                "for self-attested or caller-supplied provenance. Do not fake a signed .app."
            ),
        }
    )
    leftovers.append(
        {
            "id": ROLLBACK_LEFTOVER_ID,
            "status": "needed",
            "reason": (
                "local-dev-rollback is hard-disabled. It must not mutate current "
                "on an unauthenticated historical package. Stage 3 recreates the "
                "package on the isolated Mac from the approved commit."
            ),
        }
    )
    return leftovers


def build_canonical_source_manifest() -> dict:
    """Reconstruct the complete authority-bearing source package from live source."""
    profile = load_local_dev_profile()
    frontend_proof = prove_frontend_dist(load_json(TAURI_CONF))
    state_root, log_root = platform_neutral_runtime_roots(profile)
    head = require_clean_head()
    rows = source_tree_rows()
    digest = content_digest(rows)
    profile_bytes = compiled_profile_raw()
    pin = pin_from_compiled_json(profile_bytes)
    require_manifest_pin_matches_compiled(pin)
    signing_pins = compiled_macos_signing_pins(profile)
    return {
        "schema_version": 1,
        "profile": LOCAL_DEV_PROFILE_NAME,
        "source_commit": head,
        "content_digest": digest,
        "bundle_identifier": BUNDLE_IDENTIFIER,
        "keyring_service": PRODUCTION_KEYRING,
        "relay_ws_url": PRODUCTION_RELAY,
        "owner_display_prefix": OWNER_DISPLAY_PREFIX,
        "owner_pin": pin,
        "compiled_profile_sha256": sha256_bytes(profile_bytes),
        "frontend": frontend_proof,
        "state_root": state_root,
        "log_root": log_root,
        "buzz_transport": "optional-to-transport",
        "transport_requires_desktop": False,
        "desktop_requires_relay": True,
        "artifacts": {"macos_app": None},
        "leftovers": source_package_leftovers(signing_pins),
        "lane": "scripts/desktop_release.py",
    }


def require_complete_manifest_match(stored: dict, canonical: dict) -> None:
    """Reject missing, extra, or changed authority-bearing fields. Subset hash is not enough."""
    if not isinstance(stored, dict) or not isinstance(canonical, dict):
        raise SystemExit("source manifest must be an object")
    stored_keys = set(stored)
    canonical_keys = set(canonical)
    missing = canonical_keys - stored_keys
    extra = stored_keys - canonical_keys
    if missing:
        raise SystemExit(
            "source manifest missing authority-bearing fields: "
            + ", ".join(sorted(missing))
        )
    if extra:
        raise SystemExit(
            "source manifest has extra authority-bearing fields: "
            + ", ".join(sorted(extra))
        )
    for key in sorted(canonical_keys):
        if stored[key] != canonical[key]:
            raise SystemExit(
                f"source manifest field {key} does not match reconstructed "
                "canonical source"
            )


def authenticate_source_package(release_root: Path) -> tuple[str, Path, dict]:
    """Authenticate the current source package before any admission write."""
    root = release_root.expanduser().resolve()
    current = read_pointer(root, "current")
    if current is None:
        raise SystemExit("no current pointer")
    if not DIGEST_RE.fullmatch(current):
        raise SystemExit("current must be exactly sha256:<64 hex>")
    dest = digest_dir(root, current, must_exist=True)
    manifest = load_json(dest / "manifest.json")
    if manifest.get("content_digest") != current:
        raise SystemExit("manifest digest must equal current")
    stored = recompute_stored_digest(dest)
    if stored != current:
        raise SystemExit("stored source-tree proof does not recompute to current")
    live_digest = content_digest(source_tree_rows())
    if live_digest != current:
        raise SystemExit("recomputed source-tree digest does not equal current")
    head = require_clean_head()
    if head != manifest.get("source_commit"):
        raise SystemExit("HEAD does not match recorded source_commit")
    stored_profile = dest / "profile.json"
    if not stored_profile.is_file() or stored_profile.read_bytes() != compiled_profile_raw():
        raise SystemExit("stored profile.json does not match compiled profile JSON bytes")
    canonical = build_canonical_source_manifest()
    require_complete_manifest_match(manifest, canonical)
    stored_frontend = dest / "proofs" / "frontend-dist.json"
    if not stored_frontend.is_file() or load_json(stored_frontend) != canonical["frontend"]:
        raise SystemExit("stored frontend proof does not match reconstructed canonical frontend")
    validate_local_dev_manifest(manifest, live=False)
    return current, dest, manifest


def validate_local_dev_manifest(
    manifest: dict, *, live: bool, source: dict | None = None
) -> None:
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
    if not DIGEST_RE.fullmatch(str(manifest.get("compiled_profile_sha256", ""))):
        raise SystemExit("manifest compiled_profile_sha256 must be sha256:<64 hex>")
    if manifest["compiled_profile_sha256"] != sha256_bytes(compiled_profile_raw()):
        raise SystemExit("manifest compiled_profile_sha256 does not match compiled JSON bytes")
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
    require_manifest_pin_matches_compiled(pin)
    signing_pins = compiled_macos_signing_pins()
    signing_leftover = next(
        (item for item in leftovers if item.get("id") == SIGNING_PIN_LEFTOVER_ID), None
    )
    if live:
        if source is None:
            raise SystemExit("live validation requires the source manifest")
        require_live_immutable_equals_source(source, manifest)
        if leftover.get("status") != "satisfied":
            raise SystemExit("live/ is only for a proven candidate; leftover must be satisfied")
        if not pin_is_compiled(pin):
            raise SystemExit("live package admission fails closed without a compiled owner public-key pin")
        if not signing_pins["filled"]:
            raise SystemExit(
                "live/ requires compiled approved_team_id and approved_codesign_identity"
            )
        if not proven_macos_artifact(macos_app):
            raise SystemExit(
                "live/ requires this bundle's identifier, compiled signing pin, "
                "executable, version, source receipt, embedded profile, "
                "and independent codesign/Gatekeeper/stapler"
            )
        producer_leftover = next(
            (item for item in leftovers if item.get("id") == PRODUCER_LEFTOVER_ID), None
        )
        if producer_leftover is None or producer_leftover.get("status") != "satisfied":
            raise SystemExit(
                "live/ refused: mac-controlled-candidate-producer leftover is "
                "needed; self-attested or caller-supplied provenance is not enough"
            )
        return
    if leftover.get("status") != "needed":
        raise SystemExit("Linux/source package must leave mac-packaged-app-build needed")
    producer_leftover = next(
        (item for item in leftovers if item.get("id") == PRODUCER_LEFTOVER_ID), None
    )
    if producer_leftover is None or producer_leftover.get("status") != "needed":
        raise SystemExit("source package must leave mac-controlled-candidate-producer needed")
    rollback_leftover = next(
        (item for item in leftovers if item.get("id") == ROLLBACK_LEFTOVER_ID), None
    )
    if rollback_leftover is None or rollback_leftover.get("status") != "needed":
        raise SystemExit("source package must leave historical-package-rollback needed")
    if macos_app is not None:
        raise SystemExit("source-only package must not claim a macOS .app artifact")
    if not signing_pins["filled"] and (
        signing_leftover is None or signing_leftover.get("status") != "needed"
    ):
        raise SystemExit(
            "empty approved Team ID / identity pin must leave "
            f"{SIGNING_PIN_LEFTOVER_ID} needed"
        )


def package_local_dev(args: argparse.Namespace) -> None:
    release_root = args.release_root.expanduser().resolve()
    if forbidden_runtime_root(ROOT, release_root):
        raise SystemExit(
            f"release root {release_root} must be outside the source checkout "
            "and outside DawsOS reports/ops"
        )
    if getattr(args, "owner_pubkey", None) or getattr(args, "owner_pubkey_sha256", None):
        raise SystemExit(
            "owner pin CLI override is forbidden; pin must come from the compiled profile"
        )
    requested = args.source_commit
    head = require_clean_head()
    if requested and requested != head:
        raise SystemExit(
            f"source_commit {requested} does not match HEAD {head}; "
            "an arbitrary 40-char SHA is not accepted"
        )
    rows = source_tree_rows()
    manifest = build_canonical_source_manifest()
    digest = manifest["content_digest"]
    validate_local_dev_manifest(manifest, live=False)

    dest_rel = package_relpath(digest)
    with held_release_root(release_root) as root:
        if root.child_exists(dest_rel):
            raise SystemExit(
                f"digest directory {release_root / dest_rel} already exists; publication is write-once"
            )
        dest = root.mkdir_rel(dest_rel, exclusive_leaf=True)
        root.write_rel(f"{dest_rel}/manifest.json", json.dumps(manifest, indent=2) + "\n")
        root.write_rel(f"{dest_rel}/profile.json", compiled_profile_raw())
        root.write_rel(
            f"{dest_rel}/proofs/frontend-dist.json",
            json.dumps(manifest["frontend"], indent=2) + "\n",
        )
        root.write_rel(
            f"{dest_rel}/proofs/source-tree.json",
            json.dumps(
                [
                    {"path": path, "type": typ, "mode": mode, "object": obj}
                    for path, typ, mode, obj in rows
                ],
                indent=2,
            )
            + "\n",
        )
        root.write_rel(
            f"{dest_rel}/artifacts/README",
            "No Buzz.app is produced here. Leftover: mac-packaged-app-build.\n"
            "Do not treat any file in this directory as a macOS application.\n"
            "A boolean admit_macos_app_artifact(true) is not proof of a signed app.\n"
            "The self-attesting Mac producer is hard-disabled (Stage 3 leftover).\n",
        )
        previous = read_pointer(release_root, "current")
        if previous and previous != digest:
            root.write_rel("previous", previous + "\n", replace=True)
        root.write_rel("current", digest + "\n", replace=True)
    print(dest / "manifest.json")


def recompute_stored_digest(dest: Path) -> str:
    proof = load_json(dest / "proofs" / "source-tree.json")
    if not isinstance(proof, list) or not proof:
        raise SystemExit("stored source-tree proof is missing or empty")
    rows: list[tuple[str, str, str, str]] = []
    for item in proof:
        if not isinstance(item, dict):
            raise SystemExit("stored source-tree proof entry must be an object")
        try:
            row = (item["path"], item["type"], item["mode"], item["object"])
        except KeyError as exc:
            raise SystemExit(f"stored source-tree proof missing {exc}") from exc
        if any(not isinstance(part, str) or not part for part in row):
            raise SystemExit("stored source-tree proof fields must be non-empty strings")
        if (row[1], row[2]) not in GIT_TREE_MODES or not GIT_OBJECT_RE.fullmatch(row[3]):
            raise SystemExit("stored source-tree proof has an unsupported git identity")
        rows.append(row)
    return content_digest(rows)


def verify_local_dev(args: argparse.Namespace) -> None:
    current, dest, manifest = authenticate_source_package(args.release_root)
    live_dest = dest / "live"
    if live_dest.is_dir():
        live_manifest = load_json(live_dest / "manifest.json")
        validate_local_dev_manifest(live_manifest, live=True, source=manifest)
        artifact = (live_manifest.get("artifacts") or {}).get("macos_app") or {}
        app_path = artifact.get("app_path")
        if not app_path:
            raise SystemExit("live/ manifest is missing the real .app path")
        evidence = macos_signing_evidence(
            Path(app_path), source_manifest=manifest, dest=dest
        )
        if not evidence["signed"] or not evidence["notarized"]:
            raise SystemExit(
                "live/ artifact failed independent re-verification; "
                f"{evidence.get('reason')}"
            )
        if evidence["sha256"] != artifact.get("sha256"):
            raise SystemExit("live/ artifact digest does not match independent recompute")
    print(f"verified local-dev production digest {current}")


def rollback_local_dev(args: argparse.Namespace) -> None:
    """Hard-disabled. Must not mutate current on an unauthenticated historical package."""
    raise SystemExit(
        "local-dev-rollback leftover: historical-package-rollback is needed. "
        "It must not mutate current on an unauthenticated historical package. "
        "Stage 3 recreates the package on the isolated Mac from the approved commit."
    )


def _entry_mode(path: Path) -> str:
    return format(stat.S_IMODE(path.lstat().st_mode), "03o")


def sha256_tree(path: Path) -> str:
    """Digest a bundle tree including regular files, symlinks, and modes.

    Unreadable or unsupported entries fail closed instead of being skipped.
    """
    rows: list[tuple[str, str]] = []

    def record(rel: str, kind: str, entry: Path, payload: str) -> None:
        try:
            mode = _entry_mode(entry)
        except OSError as exc:
            raise SystemExit(f"unreadable tree entry {entry}: {exc}") from exc
        rows.append((rel, f"{kind}:{mode}:{payload}"))

    if path.is_symlink():
        record(".", "symlink", path, os.readlink(path))
        return content_digest(rows)
    if path.is_file():
        record(".", "file", path, sha256_file(path))
        return content_digest(rows)
    if not path.is_dir():
        raise SystemExit(f"unsupported tree root {path}")

    def walk(current: Path) -> None:
        try:
            names = sorted(os.listdir(current))
        except OSError as exc:
            raise SystemExit(f"unreadable tree directory {current}: {exc}") from exc
        for name in names:
            child = current / name
            rel = str(child.relative_to(path))
            try:
                st = child.lstat()
            except OSError as exc:
                raise SystemExit(f"unreadable tree entry {child}: {exc}") from exc
            if stat.S_ISLNK(st.st_mode):
                try:
                    target = os.readlink(child)
                except OSError as exc:
                    raise SystemExit(f"unreadable symlink {child}: {exc}") from exc
                record(rel, "symlink", child, target)
            elif stat.S_ISREG(st.st_mode):
                try:
                    digest = sha256_file(child)
                except OSError as exc:
                    raise SystemExit(f"unreadable file {child}: {exc}") from exc
                record(rel, "file", child, digest)
            elif stat.S_ISDIR(st.st_mode):
                record(rel, "dir", child, "")
                walk(child)
            else:
                raise SystemExit(f"unsupported tree entry {rel}")

    walk(path)
    if not rows:
        record(".", "dir", path, "")
    # Leftover: a non-empty bundle omits the root-directory mode from the
    # digest. Do not "fix" that here by opening live/ or weakening signing
    # holds. See .release/LEFTOVER-release-root-writer-containment.md.
    return content_digest(rows)


def _run_macos_tool(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    if not cmd:
        raise SystemExit("Apple tool command is empty")
    if cmd[0] not in TRUSTED_APPLE_TOOLS:
        raise SystemExit(
            f"Apple tool {cmd[0]!r} is not a trusted absolute path "
            "(PATH spoofing denied)"
        )
    if cmd[0] == str(XCRUN) and (len(cmd) < 2 or cmd[1] != "stapler"):
        raise SystemExit("xcrun must invoke stapler only")
    env = dict(APPLE_TOOL_ENV)
    home = os.environ.get("HOME")
    if home:
        env["HOME"] = home
    return subprocess.run(cmd, capture_output=True, text=True, check=False, env=env)


def _read_info_plist(app: Path) -> dict | None:
    plist_path = app / "Contents" / "Info.plist"
    if not plist_path.is_file():
        return None
    try:
        with plist_path.open("rb") as handle:
            data = plistlib.load(handle)
    except (OSError, plistlib.InvalidFileException, ValueError):
        return None
    return data if isinstance(data, dict) else None


def provenance_canonical_bytes(provenance: dict) -> bytes:
    return json.dumps(provenance, sort_keys=True, separators=(",", ":")).encode()


def load_build_provenance(dest: Path) -> dict | None:
    path = dest / "candidate" / "unsigned" / "build-provenance.json"
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def macos_signing_evidence(
    app_path: Path | None,
    *,
    source_manifest: dict | None = None,
    dest: Path | None = None,
) -> dict:
    """Independently inspect a real Buzz.app. Caller strings never set signed/notarized."""
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
        "bundle_identifier": None,
        "executable": None,
        "version": None,
        "receipt": None,
        "embedded_profile_matches": False,
        "receipt_matches": False,
        "executable_sha256": None,
        "provenance_matches": False,
        "provenance_attestation_class": None,
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

    signing_pins = compiled_macos_signing_pins()
    info = _read_info_plist(resolved)
    if info is None:
        evidence["reason"] = "Info.plist missing or unreadable; fail closed"
        return evidence
    bundle_id = info.get("CFBundleIdentifier")
    evidence["bundle_identifier"] = bundle_id
    if bundle_id != BUNDLE_IDENTIFIER:
        evidence["reason"] = (
            f"bundle identifier {bundle_id!r} is not {BUNDLE_IDENTIFIER}; "
            "an unrelated Apple-notarized app is not admitted"
        )
        return evidence
    executable_name = info.get("CFBundleExecutable")
    if not executable_name:
        evidence["reason"] = "CFBundleExecutable missing; fail closed"
        return evidence
    executable = resolved / "Contents" / "MacOS" / str(executable_name)
    if not executable.exists():
        evidence["reason"] = f"bundle executable {executable} missing; fail closed"
        return evidence
    evidence["executable"] = str(executable)
    try:
        executable_digest = sha256_file(executable)
    except OSError as exc:
        evidence["reason"] = f"bundle executable unreadable; fail closed: {exc}"
        return evidence
    evidence["executable_sha256"] = executable_digest
    version = info.get("CFBundleShortVersionString") or info.get("CFBundleVersion")
    evidence["version"] = version
    if not version:
        evidence["reason"] = "bundle version missing; fail closed"
        return evidence

    embedded = resolved / APP_EMBEDDED_PROFILE
    if not embedded.is_file() or embedded.read_bytes() != compiled_profile_raw():
        evidence["reason"] = (
            "embedded production profile missing or does not match compiled JSON bytes"
        )
        return evidence
    evidence["embedded_profile_matches"] = True

    receipt_path = resolved / APP_SOURCE_RECEIPT
    if not receipt_path.is_file():
        evidence["reason"] = "source/build receipt missing; fail closed"
        return evidence
    try:
        receipt = json.loads(receipt_path.read_text())
    except (OSError, json.JSONDecodeError):
        evidence["reason"] = "source/build receipt is not JSON; fail closed"
        return evidence
    evidence["receipt"] = receipt
    if not isinstance(receipt, dict):
        evidence["reason"] = "source/build receipt must be an object"
        return evidence
    if receipt.get("version") != version:
        evidence["reason"] = "receipt version does not match Info.plist version"
        return evidence
    if receipt.get("compiled_profile_sha256") != sha256_bytes(compiled_profile_raw()):
        evidence["reason"] = "receipt compiled_profile_sha256 does not match compiled JSON"
        return evidence
    if source_manifest is not None:
        if receipt.get("source_commit") != source_manifest.get("source_commit"):
            evidence["reason"] = "receipt source_commit does not match the source manifest"
            return evidence
        if receipt.get("content_digest") != source_manifest.get("content_digest"):
            evidence["reason"] = "receipt content_digest does not match the source manifest"
            return evidence
        if receipt.get("compiled_profile_sha256") != source_manifest.get("compiled_profile_sha256"):
            evidence["reason"] = "receipt profile digest does not match the source manifest"
            return evidence
    if receipt.get("executable_sha256") != executable_digest:
        evidence["reason"] = "receipt executable digest does not match the bundle executable"
        return evidence
    evidence["receipt_matches"] = True
    provenance = load_build_provenance(dest) if dest is not None else None
    attestation_class = None if provenance is None else provenance.get("attestation_class")
    evidence["provenance_attestation_class"] = attestation_class
    if provenance is None or attestation_class in SELF_ATTESTED_CLASSES:
        evidence["reason"] = (
            "self-attested / caller-supplied provenance is not executable "
            "provenance; refuse live/"
        )
        return evidence
    if attestation_class != INDEPENDENT_ATTESTATION_CLASS:
        evidence["reason"] = "unsupported provenance attestation class; fail closed"
        return evidence
    # JSON attestation_class is not builder authority. Stage 3 must
    # authenticate the builder attestation itself. This hold keeps live/ closed.
    if provenance.get("builder") != "isolated-mac-lane":
        evidence["reason"] = (
            "independent builder attestation must name isolated-mac-lane; "
            "caller-supplied app bytes are not a builder"
        )
        return evidence
    if (
        provenance.get("executable_sha256") != executable_digest
        or provenance.get("content_digest") != (source_manifest or {}).get("content_digest")
        or provenance.get("compiled_profile_sha256") != sha256_bytes(compiled_profile_raw())
        or provenance.get("source_commit") != (source_manifest or {}).get("source_commit")
        or receipt.get("provenance_sha256") != sha256_bytes(provenance_canonical_bytes(provenance))
    ):
        evidence["reason"] = "wrong executable provenance; fail closed"
        return evidence
    evidence["provenance_matches"] = True

    if not signing_pins["filled"]:
        evidence["reason"] = (
            "approved_team_id / approved_codesign_identity compiled pins are empty; "
            f"leftover {SIGNING_PIN_LEFTOVER_ID} stays needed "
            "(do not invent a Team ID; do not admit any Apple-notarized app)"
        )
        return evidence

    if sys.platform != "darwin":
        evidence["reason"] = (
            "this host cannot run trusted Apple tools; fail closed"
        )
        return evidence
    verify = _run_macos_tool(
        [str(CODESIGN), "--verify", "--deep", "--strict", "--verbose=2", str(resolved)]
    )
    evidence["codesign_verify"] = verify.returncode == 0
    if verify.returncode != 0:
        evidence["reason"] = "codesign --verify failed"
        return evidence
    display = _run_macos_tool([str(CODESIGN), "--display", "--verbose=4", str(resolved)])
    text = f"{display.stderr}\n{display.stdout}"
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
    if team_id != signing_pins["approved_team_id"]:
        evidence["reason"] = "codesign Team ID does not match the compiled approved_team_id"
        return evidence
    if identity != signing_pins["approved_codesign_identity"]:
        evidence["reason"] = (
            "codesign identity does not match the compiled approved_codesign_identity"
        )
        return evidence
    evidence["team_id"] = team_id
    evidence["codesign_identity"] = identity
    gate = _run_macos_tool(
        [str(SPCTL), "--assess", "--type", "execute", "--verbose", str(resolved)]
    )
    evidence["gatekeeper"] = gate.returncode == 0
    if gate.returncode != 0:
        evidence["reason"] = "Gatekeeper assessment failed"
        return evidence
    staple = _run_macos_tool([str(XCRUN), "stapler", "validate", str(resolved)])
    evidence["stapled"] = staple.returncode == 0
    if not evidence["stapled"]:
        evidence["reason"] = "stapled notarization ticket missing"
        return evidence
    evidence["notarization"] = "stapler-validate"
    evidence["signed"] = True
    evidence["notarized"] = True
    evidence["reason"] = (
        "independent Buzz.app admission: bundle identifier, compiled signing pin, "
        "executable, version, source receipt, embedded profile, codesign, "
        "Gatekeeper, and stapler succeeded"
    )
    return evidence


def proven_macos_artifact(artifact: dict | None) -> bool:
    if not artifact:
        return False
    signing_pins = compiled_macos_signing_pins()
    if not signing_pins["filled"]:
        return False
    return (
        artifact.get("signed") is True
        and artifact.get("notarized") is True
        and artifact.get("stapled") is True
        and artifact.get("codesign_verify") is True
        and artifact.get("gatekeeper") is True
        and artifact.get("embedded_profile_matches") is True
        and artifact.get("receipt_matches") is True
        and artifact.get("provenance_matches") is True
        and bool(DIGEST_RE.fullmatch(str(artifact.get("executable_sha256") or "")))
        and artifact.get("bundle_identifier") == BUNDLE_IDENTIFIER
        and artifact.get("team_id") == signing_pins["approved_team_id"]
        and artifact.get("codesign_identity") == signing_pins["approved_codesign_identity"]
        and bool(DIGEST_RE.fullmatch(str(artifact.get("sha256") or "")))
        and bool(artifact.get("executable"))
        and bool(artifact.get("version"))
        and bool(artifact.get("receipt"))
        and bool(artifact.get("app_path"))
        and artifact.get("notarization") == "stapler-validate"
        and artifact.get("provenance_attestation_class") == INDEPENDENT_ATTESTATION_CLASS
    )


def write_candidate_evidence(root: HeldReleaseRoot, dest_rel: str, evidence: dict) -> Path:
    return root.write_rel(
        f"{dest_rel}/candidate/evidence/macos-app.json",
        json.dumps(evidence, indent=2) + "\n",
        replace=True,
    )


def producer_leftover_needed(source_manifest: dict) -> bool:
    leftovers = source_manifest.get("leftovers") or []
    producer = next((item for item in leftovers if item.get("id") == PRODUCER_LEFTOVER_ID), None)
    return producer is None or producer.get("status") != "satisfied"


def write_live_if_proven(
    root: HeldReleaseRoot, dest: Path, dest_rel: str, source_manifest: dict, evidence: dict
) -> Path:
    provenance = load_build_provenance(dest)
    attestation_class = None if provenance is None else provenance.get("attestation_class")
    leftover_needed = (
        not evidence.get("signed")
        or not evidence.get("notarized")
        or not pin_is_compiled(source_manifest.get("owner_pin") or {})
        or not compiled_macos_signing_pins()["filled"]
        or not proven_macos_artifact(evidence)
        or producer_leftover_needed(source_manifest)
        or attestation_class in SELF_ATTESTED_CLASSES
        or attestation_class != INDEPENDENT_ATTESTATION_CLASS
        or (provenance or {}).get("builder") != "isolated-mac-lane"
    )
    if leftover_needed:
        raise SystemExit(
            "live/ refused: Stage 3 leftover; self-attested or caller-supplied "
            "provenance cannot write live/"
        )
    live = dict(source_manifest)
    live["artifacts"] = {"macos_app": evidence}
    # Leftover: this future live-manifest construction still drops the
    # needed historical-package-rollback leftover. Do not satisfy it here
    # or open live/ while signing/producer holds remain.
    # See .release/LEFTOVER-release-root-writer-containment.md.
    live["leftovers"] = [
        {
            "id": MAC_LEFTOVER_ID,
            "status": "satisfied",
            "reason": evidence.get("reason"),
        },
        {
            "id": PRODUCER_LEFTOVER_ID,
            "status": "satisfied",
            "stage": 3,
            "reason": "independent isolated-mac-lane builder attestation",
        },
    ]
    validate_local_dev_manifest(live, live=True, source=source_manifest)
    live_rel = f"{dest_rel}/live"
    if root.child_exists(live_rel):
        raise SystemExit(f"{root.path / live_rel} already exists; live publication is write-once")
    return root.write_rel(
        f"{live_rel}/manifest.json", json.dumps(live, indent=2) + "\n", replace=False
    )


def embed_profile_and_receipt(app: Path, manifest: dict) -> dict:
    """Hard-disabled. Do not manufacture provenance from caller-supplied app bytes."""
    raise SystemExit(
        "self-attesting producer is hard-disabled; "
        "do not manufacture provenance from caller-supplied app bytes"
    )


def produce_macos_candidate(args: argparse.Namespace) -> None:
    """Hard-disabled self-attesting producer. Stage 3 leftover on every host."""
    # Leftover: this write command does not repeat forbidden_runtime_root.
    # See .release/LEFTOVER-release-root-writer-containment.md.
    current, dest, _manifest = authenticate_source_package(args.release_root)
    leftover = {
        "id": PRODUCER_LEFTOVER_ID,
        "status": "needed",
        "stage": 3,
        "attestation_class": "self-attested-disabled",
        "reason": (
            "Stage 3 leftover: controlled Mac candidate producer is hard-disabled. "
            "It must not hash a caller-supplied .app and emit matching receipt plus "
            "external provenance. Stage 3 recreates the package on the isolated Mac "
            "from the approved commit. JSON attestation_class strings are not "
            "builder authority. Until then, admission refuses live/ for "
            "self-attested or caller-supplied provenance. Do not fake a signed .app."
        ),
        "content_digest": current,
        "platform": sys.platform,
    }
    dest_rel = package_relpath(current)
    with held_release_root(args.release_root) as root:
        path = root.write_rel(
            f"{dest_rel}/candidate/unsigned/producer-leftover.json",
            json.dumps(leftover, indent=2) + "\n",
            replace=True,
        )
    print(path)
    if args.unsigned_app is not None:
        raise SystemExit(
            "self-attesting producer is hard-disabled; "
            "do not manufacture provenance from caller-supplied app bytes"
        )
    raise SystemExit(
        "mac-controlled-candidate-producer leftover (Stage 3); "
        "self-attesting producer hard-disabled"
    )


def admit_local_dev_app(args: argparse.Namespace) -> None:
    # Leftover: this write command does not repeat forbidden_runtime_root.
    # See .release/LEFTOVER-release-root-writer-containment.md.
    current, dest, manifest = authenticate_source_package(args.release_root)
    if args.boolean_true:
        raise SystemExit("admit_macos_app_artifact(true) is not proof of a signed app")
    for forbidden in ("codesign_identity", "team_id", "notarization", "stapled"):
        if getattr(args, forbidden, None):
            raise SystemExit(f"{forbidden} CLI strings are not signing evidence")
    require_manifest_pin_matches_compiled(manifest.get("owner_pin") or {})
    evidence = macos_signing_evidence(
        args.app, source_manifest=manifest, dest=dest
    )
    if args.app_hash:
        claimed = normalize_digest(args.app_hash)
        if evidence["sha256"] and evidence["sha256"] != claimed:
            raise SystemExit("claimed --app-hash does not match the recomputed artifact digest")
        if evidence["sha256"] is None:
            evidence["claimed_sha256"] = claimed
    leftover_needed = (
        not evidence["signed"]
        or not evidence["notarized"]
        or not pin_is_compiled(manifest.get("owner_pin") or {})
        or not compiled_macos_signing_pins()["filled"]
        or not proven_macos_artifact(evidence)
        or producer_leftover_needed(manifest)
        or evidence.get("provenance_attestation_class") != INDEPENDENT_ATTESTATION_CLASS
    )
    dest_rel = package_relpath(current)
    with held_release_root(args.release_root) as root:
        if leftover_needed:
            evidence_path = write_candidate_evidence(root, dest_rel, evidence)
            print(evidence_path)
            raise SystemExit(
                "incomplete observation written under candidate/evidence; "
                "live/ is write-once for a proven signed and notarized candidate only"
            )
        path = write_live_if_proven(root, dest, dest_rel, manifest, evidence)
    print(path)


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
    rb = sub.add_parser(
        "local-dev-rollback",
        help="leftover: must not mutate current on an unauthenticated historical package",
    )
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
    produce = sub.add_parser(
        "local-dev-produce-app",
        help="Stage 3 leftover: self-attesting Mac producer is hard-disabled",
    )
    produce.add_argument("--release-root", required=True, type=Path)
    produce.add_argument("--unsigned-app", type=Path, help="unsigned .app to embed into; never signed here")
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
    elif args.command == "local-dev-produce-app":
        produce_macos_candidate(args)
    else:
        raise SystemExit(f"unknown command {args.command}")


if __name__ == "__main__":
    main()
