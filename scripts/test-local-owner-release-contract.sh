#!/bin/bash
set -euo pipefail

script_path=${BASH_SOURCE[0]}
if [[ $script_path != /* ]]; then
  script_path=$(builtin pwd -P)/$script_path
fi
script_dir=${script_path%/*}
repo_root=$(builtin cd -- "$script_dir/.." && builtin pwd -P)
builtin cd -- "$repo_root"

/bin/bash -n scripts/build-local-owner-macos.sh

clean_environment=(/usr/bin/env)
while IFS= read -r name; do
  case "$name" in
    CARGO_HOME)
      clean_environment+=(-u "$name")
      ;;
    BUZZ_* | VITE_* | TAURI_* | NOSTR_PRIVATE_KEY | NOSTR_SECRET_KEY | NSEC | \
      APPLE_CERTIFICATE | APPLE_CERTIFICATE_PASSWORD | APPLE_SIGNING_IDENTITY | \
      APPLE_ID | APPLE_PASSWORD | APPLE_TEAM_ID | AC_API_KEY | AC_API_KEY_ID | \
      AC_API_ISSUER_ID | NODE_OPTIONS | NODE_PATH | RUSTC | RUSTC_* | RUSTFLAGS | \
      RUSTDOC | RUSTDOCFLAGS | RUSTUP_* | CARGO_* | npm_config_* | NPM_CONFIG_* | \
      PNPM_* | GIT_* | CC | CXX | CPP | AR | LD | NM | OBJC | OBJCXX | RANLIB | \
      STRIP | CFLAGS | CXXFLAGS | CPPFLAGS | LDFLAGS | ARFLAGS | DYLD_* | LD_* | \
      BASH_ENV | ENV | BASHOPTS | SHELLOPTS | CDPATH | GLOBIGNORE | HERMIT_* | XDG_* | \
      MACOSX_DEPLOYMENT_TARGET | SDKROOT | DEVELOPER_DIR | PKG_CONFIG_* | \
      CMAKE_* | MAKEFLAGS | MFLAGS | SCCACHE_* | CCACHE_*)
      clean_environment+=(-u "$name")
      ;;
  esac
done < <(compgen -e)

"${clean_environment[@]}" scripts/build-local-owner-macos.sh --contract-check >/dev/null

fake_tools=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/buzz-local-owner-fake-tools.XXXXXX")
trap '/bin/rm -rf -- "$fake_tools"' EXIT
for tool in git jq shasum awk cmp mktemp mkdir mv uname dirname; do
  printf '#!/bin/sh\nexit 97\n' >"$fake_tools/$tool"
  /bin/chmod 0755 "$fake_tools/$tool"
done
"${clean_environment[@]}" PATH="$fake_tools:/usr/bin:/bin" \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null

if "${clean_environment[@]}" BUZZ_BUILD_AGENT_ENV=unexpected \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted a forbidden BUZZ_* override" >&2
  exit 1
fi
if "${clean_environment[@]}" VITE_RELAY_URL=ws://example.invalid \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted a forbidden VITE_* override" >&2
  exit 1
fi
if "${clean_environment[@]}" TAURI_SIGNING_PRIVATE_KEY=unexpected \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted a forbidden Tauri signing override" >&2
  exit 1
fi
if "${clean_environment[@]}" NODE_OPTIONS=--require=/tmp/untrusted-node-hook \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted a forbidden NODE_OPTIONS override" >&2
  exit 1
fi
if "${clean_environment[@]}" RUSTC_WRAPPER=/tmp/untrusted-rustc-wrapper \
  scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted a forbidden RUSTC_WRAPPER override" >&2
  exit 1
fi
for name in \
  HERMIT_EXE \
  HERMIT_STATE_DIR \
  HERMIT_DIST_URL \
  RUSTUP_HOME \
  XDG_CACHE_HOME \
  XDG_CONFIG_HOME; do
  if "${clean_environment[@]}" "$name=/tmp/untrusted-build-tool-state" \
    scripts/build-local-owner-macos.sh --contract-check >/dev/null 2>&1; then
    echo "local-owner wrapper accepted forbidden $name override" >&2
    exit 1
  fi
done

/usr/bin/grep -Fq 'cargo:rerun-if-env-changed=BUZZ_DESKTOP_SOURCE_TREE' desktop/src-tauri/build.rs
/usr/bin/grep -Fq 'receipt.source_tree != expected_tree' desktop/src-tauri/build.rs
/usr/bin/grep -Fq 'builder_class != "buzz-local-owner-tauri-wrapper"' desktop/src-tauri/build.rs
/usr/bin/grep -Fq '.on_navigation(|_webview, url|' desktop/src-tauri/src/lib.rs
/usr/bin/grep -Fq 'crate::local_owner_profile::packaged_navigation_allowed(url)' \
  desktop/src-tauri/src/lib.rs
/usr/bin/grep -Fq 'export BUZZ_DESKTOP_SOURCE_COMMIT="$source_commit"' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export BUZZ_DESKTOP_SOURCE_TREE="$source_tree"' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq \
  '"$verified_hermit" --level=fatal exec "$build_repo_root/bin/$tool" -- "$@"' \
  scripts/build-local-owner-macos.sh
if /usr/bin/grep -Fq \
  '"$verified_hermit" --level=fatal exec "$build_repo_root" -- "$@"' \
  scripts/build-local-owner-macos.sh; then
  echo "local-owner wrapper passes a directory where Hermit requires a tool shim" >&2
  exit 1
fi
/usr/bin/grep -Fq 'hermit_exec pnpm install --frozen-lockfile' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'dependency installation changed the local-owner source; refusing provenance' \
  scripts/build-local-owner-macos.sh
install_line=$(/usr/bin/grep -nF 'hermit_exec pnpm install --frozen-lockfile' \
  scripts/build-local-owner-macos.sh | /usr/bin/awk -F: 'NR == 1 { print $1 }')
receipt_line=$(/usr/bin/grep -nF 'receipt_tmp=$(/usr/bin/mktemp' \
  scripts/build-local-owner-macos.sh | /usr/bin/awk -F: 'NR == 1 { print $1 }')
verification_line=$(/usr/bin/grep -nF \
  'dependency installation changed the local-owner source; refusing provenance' \
  scripts/build-local-owner-macos.sh | /usr/bin/awk -F: 'NR == 1 { print $1 }')
if [[ -z $install_line || -z $verification_line || -z $receipt_line \
  || $install_line -ge $verification_line || $verification_line -ge $receipt_line ]]; then
  echo "local-owner receipt must follow dependency installation and source verification" >&2
  exit 1
fi
if /usr/bin/grep -Eq 'cargo build|bundle-sidecars' scripts/build-local-owner-macos.sh; then
  echo "local-owner wrapper still builds legacy sidecars" >&2
  exit 1
fi
if /usr/bin/grep -Fq 'com.apple.security.cs.disable-library-validation' \
  desktop/src-tauri/Entitlements.local-owner.plist; then
  echo "local-owner entitlements disable library validation" >&2
  exit 1
fi
/usr/bin/plutil -lint desktop/src-tauri/Info.local-owner.plist >/dev/null
for forbidden_key in \
  NSMicrophoneUsageDescription \
  NSCameraUsageDescription \
  NSLocalNetworkUsageDescription \
  CFBundleURLTypes; do
  if /usr/libexec/PlistBuddy -c "Print :$forbidden_key" \
    desktop/src-tauri/Info.local-owner.plist >/dev/null 2>&1; then
    echo "local-owner Info.plist contains forbidden metadata: $forbidden_key" >&2
    exit 1
  fi
done
if /usr/bin/plutil -convert xml1 -o - desktop/src-tauri/Info.local-owner.plist \
  | /usr/bin/grep -Eiq \
    '(microphone|camera|local[[:space:]-]+network|share[[:space:]-]+compute)'; then
  echo "local-owner Info.plist contains a legacy device/network purpose string" >&2
  exit 1
fi
/usr/bin/grep -Fq \
  'assert_interaction_only_info_plist "$app_path/Contents/Info.plist"' \
  scripts/build-local-owner-macos.sh
if [[ -e desktop/src-tauri/Info.plist || -L desktop/src-tauri/Info.plist ]]; then
  echo "conventional Info.plist would be merged into the local-owner artifact" >&2
  exit 1
fi
/usr/bin/jq -e \
  '.bundle.macOS.infoPlist == "Info.standard.plist"' \
  desktop/src-tauri/tauri.conf.json >/dev/null
/usr/bin/jq -e '
  .bundle.macOS.entitlements == "Entitlements.local-owner.plist"
  and .bundle.macOS.infoPlist == "Info.local-owner.plist"
  and .app.security.capabilities == ["local-owner"]
  and .plugins["deep-link"].desktop == []
  and (.app.security.csp | contains("ws://localhost:3300"))
  and (.app.security.csp | contains("buzz-media:"))
  and (.app.security.csp | contains("http://127.0.0.1:") | not)
  and (.app.security.csp | contains("https://") | not)
' desktop/src-tauri/tauri.local-owner.conf.json >/dev/null
/usr/bin/jq -s -e '
  .[0] * .[1]
  | .plugins["deep-link"].desktop == []
  and .bundle.macOS.infoPlist == "Info.local-owner.plist"
' desktop/src-tauri/tauri.conf.json \
  desktop/src-tauri/tauri.local-owner.conf.json >/dev/null
/usr/bin/jq -e '
  .identifier == "local-owner"
  and (.permissions | index("notification:allow-notify") != null)
  and (.permissions | index("core:event:allow-emit-to") == null)
  and ([.permissions[] | select(
    startswith("opener:")
    or startswith("dialog:")
    or startswith("updater:")
    or startswith("global-shortcut:")
    or startswith("window-state:")
    or startswith("core:path:")
  )] | length == 0)
' desktop/src-tauri/capabilities/local-owner.json >/dev/null
/usr/bin/grep -Fq 'build_root=$(/usr/bin/mktemp -d' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export CARGO_TARGET_DIR="$build_root/target"' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'local-owner app must not contain symlinks' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'expected_hermit_sha256=61935bf58de3930bbec196d7c79d2a4d14d9e967670786d0eb433e1c4f567c05' \
  scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'HERMIT_STATE_DIR="$hermit_root/state"' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'isolated_git clone --no-checkout "$source_bundle" "$build_repo_root"' \
  scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export CARGO_HOME="$build_root/cargo-home"' \
  scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export HOME="$build_home"' scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export XDG_CONFIG_HOME="$build_root/xdg-config"' \
  scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'export RUSTUP_HOME="$build_root/rustup-home"' \
  scripts/build-local-owner-macos.sh
/usr/bin/grep -Fq 'pnpm install --frozen-lockfile --store-dir "$build_root/pnpm-store"' \
  scripts/build-local-owner-macos.sh

symlink_root=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/buzz-local-owner-wrapper-link.XXXXXX")
/bin/mkdir -p "$symlink_root/scripts"
/bin/ln -s "$repo_root/scripts/build-local-owner-macos.sh" \
  "$symlink_root/scripts/build-local-owner-macos.sh"
if "${clean_environment[@]}" "$symlink_root/scripts/build-local-owner-macos.sh" \
  --contract-check >/dev/null 2>&1; then
  echo "local-owner wrapper accepted symlink invocation" >&2
  exit 1
fi
/bin/rm -rf -- "$symlink_root"

echo "local-owner release contract checks passed"
