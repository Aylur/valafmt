#!/usr/bin/env bash
# Clone a corpus of real Vala projects and run the parse/format/idempotency
# sweep over them. Usage: scripts/corpus.sh
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p corpus
repos=(
    elementary/files
    elementary/terminal
    elementary/calendar
    elementary/music
    elementary/photos
    elementary/code
    GNOME/gitg
    GNOME/shotwell
    GNOME/gnome-clocks
    GNOME/geary
)
for repo in "${repos[@]}"; do
    name=$(basename "$repo")
    if [ ! -d "corpus/$name" ]; then
        echo "cloning $repo..."
        git clone -q --depth 1 "https://github.com/$repo" "corpus/$name"
    fi
done

cargo run --release --example corpus -- corpus/ "$@"
