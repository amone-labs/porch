#!/bin/sh
# Build a signed, notarized, stapled .dmg ready to hand to someone.
#
# One-time setup on this Mac (asks for an app-specific password from
# appleid.apple.com; nothing is stored in the repo):
#   xcrun notarytool store-credentials porch \
#     --apple-id <your Apple ID> --team-id <Team ID>
#
# Then:  scripts/release.sh
# Output: release/Porch_<version>_<arch>.dmg (+ updater archive and .sig)
set -e
cd "$(dirname "$0")/.."
PROFILE="${PORCH_NOTARY_PROFILE:-porch}"

scripts/version.sh >/dev/null || { echo "versions out of sync: run scripts/version.sh" >&2; exit 1; }
VERSION=$(node -p "require('./app/package.json').version")

# Developer ID Application identity from the login keychain.
APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)
[ -n "$APPLE_SIGNING_IDENTITY" ] || { echo "no 'Developer ID Application' identity in the keychain" >&2; exit 1; }
export APPLE_SIGNING_IDENTITY

# Minisign key for updater artifacts (kept outside the repo).
KEY="$HOME/.tauri/porch.key"
[ -f "$KEY" ] || { echo "missing $KEY (updater signing key)" >&2; exit 1; }
TAURI_SIGNING_PRIVATE_KEY=$(cat "$KEY")
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# PostHog project key for the usage events of ADR 0006 (kept outside the repo).
# Without it the build works but sends nothing.
PH_KEY="$HOME/.tauri/porch-posthog.key"
if [ -z "$VITE_POSTHOG_KEY" ] && [ -f "$PH_KEY" ]; then
  VITE_POSTHOG_KEY=$(cat "$PH_KEY")
fi
if [ -n "$VITE_POSTHOG_KEY" ]; then
  export VITE_POSTHOG_KEY
else
  echo "warning: no PostHog key ($PH_KEY or VITE_POSTHOG_KEY); this build sends no usage events" >&2
fi

echo "building $VERSION, signing as: $APPLE_SIGNING_IDENTITY"
BUNDLE=target/release/bundle
# macOS paths ignore case: an old bundle/porch.app would hand its lowercase
# name to the new Porch.app build. Start the bundle folder clean.
rm -rf "$BUNDLE"
(cd app && pnpm tauri build)

DMG=$(ls -t "$BUNDLE"/dmg/*.dmg | head -1)
APP="$BUNDLE/macos/Porch.app"
codesign --verify --deep --strict "$APP"
codesign --verify "$APP/Contents/MacOS/porch-hook"
codesign --verify "$APP/Contents/MacOS/porch"

mkdir -p release
if xcrun notarytool history --keychain-profile "$PROFILE" >/dev/null 2>&1; then
  echo "notarizing $(basename "$DMG") (usually a few minutes)"
  xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
  xcrun stapler staple "$DMG"
  spctl -a -t open --context context:primary-signature -v "$DMG"
else
  echo ""
  echo "NOT NOTARIZED: no notarytool profile '$PROFILE' on this Mac."
  echo "The .dmg is signed, but recipients will see an 'unidentified developer' block."
  echo "Run once:  xcrun notarytool store-credentials $PROFILE --apple-id <Apple ID> --team-id <Team ID>"
  NOTARIZED=no
fi

cp "$DMG" release/
cp "$BUNDLE"/macos/*.app.tar.gz* release/ 2>/dev/null || true
ls -la release/
[ "$NOTARIZED" = "no" ] && exit 3
echo "ready: release/$(basename "$DMG")"
