#!/usr/bin/env bash
# Assemble the GitHub Pages site locally, exactly as .github/workflows/pages.yml does:
#   site/              -> <out>/            (landing page)
#   docs/manual/book/  -> <out>/manual/     (mdBook manual)
# Default output: .scratch/site-build/. Preview with
#   python3 -m http.server -d .scratch/site-build 8000
# and open http://localhost:8000/ (links are relative, so the /dioptra/ prefix that
# GitHub Pages adds is not needed locally).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

OUT="${1:-.scratch/site-build}"

if ! command -v mdbook >/dev/null 2>&1; then
  echo "mdbook not found; install it with: cargo install mdbook --version 0.5.4 --locked" >&2
  exit 1
fi

mdbook build docs/manual

rm -rf "$OUT"
mkdir -p "$OUT/manual"
cp -R site/. "$OUT/"
cp -R docs/manual/book/. "$OUT/manual/"
touch "$OUT/.nojekyll"

test -f "$OUT/index.html"
test -f "$OUT/manual/index.html"
echo "Site assembled in $OUT ($(du -sh "$OUT" | cut -f1))."
