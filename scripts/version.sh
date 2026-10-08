#!/bin/sh
# Show or set the app version.
#   scripts/version.sh            -> print the version everywhere it lives
#   scripts/version.sh 0.1.2      -> set it (app/package.json is the source;
#                                    tauri.conf.json reads it; Cargo follows)
# Patch bumps for fixes and polish; minor for something worth calling a release.
set -e
cd "$(dirname "$0")/.."
pkg() { node -p "require('./app/package.json').version"; }
cargo_v() { sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1; }

if [ -z "$1" ]; then
  echo "app/package.json  $(pkg)"
  echo "Cargo.toml        $(cargo_v)"
  [ "$(pkg)" = "$(cargo_v)" ] && echo "in sync" || { echo "OUT OF SYNC"; exit 1; }
  exit 0
fi

case "$1" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must look like 1.2.3" >&2; exit 2 ;;
esac
node -e "const f='app/package.json',p=require('./'+f);p.version='$1';require('fs').writeFileSync(f,JSON.stringify(p,null,2)+'\n')"
sed -i '' "s/^version = \".*\"/version = \"$1\"/" Cargo.toml
cargo update --workspace --offline --quiet   # bump our crates in Cargo.lock, nothing else
echo "version -> $1"
