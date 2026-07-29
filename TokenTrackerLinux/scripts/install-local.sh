#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LINUX_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$LINUX_DIR/.." && pwd)"

PREFIX="${TOKENTRACKER_LINUX_PREFIX:-${HOME:?HOME is required}/.local}"

# Collapse `//`, `.` and `..` lexically (never through symlinks — the guard
# below still has to see every real path component) so noncanonical spellings
# of root such as `//`, `/.` or `/tmp/..` cannot slip past the root check.
normalize_absolute_path() {
  local segments=() segment
  while IFS= read -r segment; do
    case "$segment" in
      "" | ".") ;;
      "..") if ((${#segments[@]} > 0)); then unset "segments[${#segments[@]} - 1]"; fi ;;
      *) segments+=("$segment") ;;
    esac
  done < <(printf '%s\n' "${1//\//$'\n'}")
  if ((${#segments[@]} == 0)); then
    printf '/'
  else
    printf '/%s' "${segments[@]}"
  fi
}

if [[ "$PREFIX" != /* ]]; then
  echo "Refusing unsafe TokenTracker installation prefix: $PREFIX" >&2
  exit 1
fi
PREFIX="$(normalize_absolute_path "$PREFIX")"
if [[ "$PREFIX" == "/" ]]; then
  echo "Refusing unsafe TokenTracker installation prefix: $PREFIX" >&2
  exit 1
fi

BINARY_SOURCE="${TOKENTRACKER_LINUX_BINARY:-$LINUX_DIR/src-tauri/target/release/tokentracker-linux}"
RUNTIME_SOURCE="${TOKENTRACKER_LINUX_RUNTIME:-$LINUX_DIR/EmbeddedServer}"
DESKTOP_SOURCE="$LINUX_DIR/packaging/arch/tokentracker-linux/tokentracker-linux.desktop"
ICON_SOURCE="${TOKENTRACKER_LINUX_ICON:-$REPO_ROOT/dashboard/public/icon-512.png}"

BINARY_TARGET="$PREFIX/bin/tokentracker-linux"
RUNTIME_TARGET="$PREFIX/lib/tokentracker-linux"
DESKTOP_TARGET="$PREFIX/share/applications/tokentracker-linux.desktop"
ICON_TARGET="$PREFIX/share/icons/hicolor/512x512/apps/tokentracker-linux.png"

assert_no_symlink_components() {
  local target="$1"
  local component="$target"
  while [[ "$component" != "/" ]]; do
    if [[ -L "$component" ]]; then
      echo "Refusing TokenTracker path containing a symlink: $target" >&2
      exit 1
    fi
    component="$(dirname -- "$component")"
  done
}

for target in "$BINARY_TARGET" "$RUNTIME_TARGET" "$DESKTOP_TARGET" "$ICON_TARGET"; do
  assert_no_symlink_components "$target"
done

refresh_desktop_caches() {
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
  fi
  # Mirrors the Arch install hook: GTK launchers keep serving a stale or
  # generic icon from icon-theme.cache until it is rebuilt.
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
  fi
}

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BINARY_TARGET" "$DESKTOP_TARGET" "$ICON_TARGET"
  rm -rf "$RUNTIME_TARGET"
  refresh_desktop_caches
  echo "Removed the local TokenTracker Linux installation from $PREFIX"
  exit 0
fi

for source in "$BINARY_SOURCE" "$RUNTIME_SOURCE/node" \
  "$RUNTIME_SOURCE/tokentracker/bin/tracker.js" "$DESKTOP_SOURCE" "$ICON_SOURCE"; do
  if [[ ! -e "$source" ]]; then
    echo "TokenTracker install source not found: $source" >&2
    echo "Run npm run linux:build first." >&2
    exit 1
  fi
done

mkdir -p \
  "$PREFIX/bin" \
  "$PREFIX/lib" \
  "$PREFIX/share/applications" \
  "$PREFIX/share/icons/hicolor/512x512/apps"

runtime_stage="$(mktemp -d "$PREFIX/lib/.tokentracker-linux.install.XXXXXX")"
binary_stage="$(mktemp "$PREFIX/bin/.tokentracker-linux.install.XXXXXX")"
desktop_stage="$(mktemp "$PREFIX/share/applications/.tokentracker-linux.desktop.XXXXXX")"
icon_stage="$(mktemp "$PREFIX/share/icons/hicolor/512x512/apps/.tokentracker-linux.png.XXXXXX")"
cleanup() {
  if [[ -n "${runtime_stage:-}" && -d "$runtime_stage" ]]; then
    rm -rf "$runtime_stage"
  fi
  for stale in "${binary_stage:-}" "${desktop_stage:-}" "${icon_stage:-}"; do
    if [[ -n "$stale" && -f "$stale" ]]; then
      rm -f "$stale"
    fi
  done
}
trap cleanup EXIT

cp -a "$RUNTIME_SOURCE/." "$runtime_stage/"
chmod 755 "$runtime_stage/node"

install -m755 "$BINARY_SOURCE" "$binary_stage"

while IFS= read -r line || [[ -n "$line" ]]; do
  if [[ "$line" == Exec=* ]]; then
    printf 'Exec="%s"\n' "$BINARY_TARGET"
  else
    printf '%s\n' "$line"
  fi
done < "$DESKTOP_SOURCE" > "$desktop_stage"
chmod 644 "$desktop_stage"

install -m644 "$ICON_SOURCE" "$icon_stage"

# Every artifact is staged and validated above, so promotion is nothing but
# same-filesystem renames. A failure before this point leaves the previous
# installation untouched instead of mixing new runtime with old binaries.
rm -rf "$RUNTIME_TARGET"
mv "$runtime_stage" "$RUNTIME_TARGET"
runtime_stage=""
mv "$binary_stage" "$BINARY_TARGET"
binary_stage=""
mv "$desktop_stage" "$DESKTOP_TARGET"
desktop_stage=""
mv "$icon_stage" "$ICON_TARGET"
icon_stage=""
refresh_desktop_caches

echo "Installed TokenTracker Linux in $PREFIX"
echo "Launch it from the application menu or run: $BINARY_TARGET"
