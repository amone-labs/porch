#!/bin/sh
# Publish the build in release/ as the update every installed porch picks up.
#
# Run after scripts/release.sh. Uploads the updater archive and the .dmg under
# the version, then writes latest.json last, so an app never reads a manifest
# whose files are not there yet. Goes through aws-vault, as the profile in
# PORCH_AWS_PROFILE (see AGENTS.md).
#
#   scripts/publish-update.sh notes.md notes-en.md   (files: "- " lines become the list in the app)
#   scripts/publish-update.sh "One line on what changed" "The same in English"
#
# The second argument is the English notes (`notes_en` in latest.json), shown on
# English screens; apps before English support ignore it. Leave it out and
# English screens show a generic line instead of the Korean notes.
set -eu
cd "$(dirname "$0")/.."

BUCKET=${PORCH_UPDATE_BUCKET:-porch-updates-6f3a11bb}
AWS_PROFILE_NAME=${PORCH_AWS_PROFILE:?set PORCH_AWS_PROFILE to the aws-vault profile that can write the update bucket}
BASE="https://$BUCKET.s3.ap-northeast-2.amazonaws.com"
VERSION=$(node -p "require('./app/package.json').version")
NOTES=${1:-"porch $VERSION"}
[ -f "$NOTES" ] && NOTES=$(cat "$NOTES")
NOTES_EN=${2:-}
[ -n "$NOTES_EN" ] && [ -f "$NOTES_EN" ] && NOTES_EN=$(cat "$NOTES_EN")
ARCHIVE=release/Porch.app.tar.gz
DMG=release/Porch_${VERSION}_aarch64.dmg

for f in "$ARCHIVE" "$ARCHIVE.sig" "$DMG"; do
  [ -f "$f" ] || { echo "missing $f: run scripts/release.sh first" >&2; exit 1; }
done
# The archive must come from the same build as the dmg (release.sh writes both).
AGE=$(( $(stat -f %m "$DMG") - $(stat -f %m "$ARCHIVE") ))
[ "${AGE#-}" -lt 600 ] || { echo "$ARCHIVE is not from the $VERSION build" >&2; exit 1; }

NAME="Porch_${VERSION}_aarch64.app.tar.gz"
MANIFEST=$(mktemp)
node -e '
const [version, notes, notesEn, url, sigFile] = process.argv.slice(1);
const signature = require("fs").readFileSync(sigFile, "utf8").trim();
const entry = { signature, url };
console.log(JSON.stringify({ version, notes, ...(notesEn ? { notes_en: notesEn } : {}), pub_date: new Date().toISOString(),
  platforms: { "darwin-aarch64": entry, "darwin-aarch64-app": entry } }, null, 2));
' "$VERSION" "$NOTES" "$NOTES_EN" "$BASE/$VERSION/$NAME" "$ARCHIVE.sig" > "$MANIFEST"

aws-vault exec --no-session "$AWS_PROFILE_NAME" -- sh -c "
  set -e
  aws s3 cp '$ARCHIVE' 's3://$BUCKET/$VERSION/$NAME' --only-show-errors
  aws s3 cp '$DMG' 's3://$BUCKET/$VERSION/Porch_${VERSION}_aarch64.dmg' --only-show-errors
  aws s3 cp '$DMG' 's3://$BUCKET/Porch.dmg' --cache-control no-cache --only-show-errors
  aws s3 cp '$DMG' 's3://$BUCKET/porch.dmg' --cache-control no-cache --only-show-errors
  aws s3 cp '$MANIFEST' 's3://$BUCKET/latest.json' --content-type application/json --cache-control no-cache --only-show-errors
"
rm -f "$MANIFEST"

echo "published $VERSION"
curl -fsS "$BASE/latest.json" | node -p 'JSON.parse(require("fs").readFileSync(0)).version'
curl -fsSI "$BASE/$VERSION/$NAME" | head -1
echo "latest dmg: $BASE/Porch.dmg (porch.dmg kept for old links)"
