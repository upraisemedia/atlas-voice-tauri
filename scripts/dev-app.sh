#!/usr/bin/env bash
# Build a signed debug .app bundle and launch it.
#
# Why: macOS privacy permissions (Microphone, Accessibility) are tied to the
# app's code signature. `tauri dev` runs an unsigned binary whose permissions
# end up attributed to the terminal, and ad-hoc signatures change on every
# build. Signing every build with the same certificate keeps granted
# permissions across rebuilds. See docs/decisions/0002-identity-and-signing.md.
set -euo pipefail

cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

identity="${ATLAS_SIGNING_IDENTITY:-}"
if [ -z "$identity" ]; then
  identity="$(security find-identity -v -p codesigning \
    | sed -n 's/.*"\(Apple Development: [^"]*\)".*/\1/p' | head -n1)"
fi
if [ -z "$identity" ]; then
  echo "No 'Apple Development' signing identity found." >&2
  echo "Create one in Xcode (Settings > Accounts) or set ATLAS_SIGNING_IDENTITY." >&2
  exit 1
fi

echo "Signing with: $identity"
APPLE_SIGNING_IDENTITY="$identity" npm run tauri -- build --debug --bundles app

app="src-tauri/target/debug/bundle/macos/Atlas Voice.app"
codesign --verify --strict "$app"

# Quit a running instance first so the fresh build is the one that launches.
osascript -e 'tell application id "nl.atlasvoice.desktop" to quit' >/dev/null 2>&1 || true
sleep 1
open "$app"
