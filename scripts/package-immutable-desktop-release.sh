#!/usr/bin/env bash
# Source-only / build-only packager for the immutable local-dev Desktop profile.
#
# Does not install, start, or replace any live Mac Buzz process.
# Does not mint, import, export, print, or rotate keys.
# Does not send Buzz messages.
# Does not produce or claim a Buzz.app on Linux.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
release_root="${1:-}"
if [[ -z "$release_root" ]]; then
  echo "usage: $0 <release-root-outside-checkout>" >&2
  echo "The release root must be outside this checkout and outside DawsOS reports/ops." >&2
  echo "This script does not install or launch Buzz." >&2
  exit 1
fi

resolved=$(realpath -m "$release_root" 2>/dev/null || echo "$release_root")
if [[ "$resolved" == "$repo_root" || "$resolved" == "$repo_root"/* ]]; then
  echo "error: release root must not be inside the source checkout ($repo_root)" >&2
  exit 1
fi
case "$release_root" in
  *DawsOS*/reports*|*DawsOS*/ops*|*dawsos*/reports*|*dawsos*/ops*|*reports/ops*)
    echo "error: release root must not be a DawsOS reports/ops path" >&2
    exit 1
    ;;
esac

mkdir -p "$release_root"
python3 "$repo_root/scripts/immutable_desktop_release.py" package \
  --release-root "$release_root"
echo "leftover: mac-packaged-app-build (signed Buzz.app requires a Mac worker)"
echo "this host did not produce Buzz.app and must not be installed onto a live Mac"
