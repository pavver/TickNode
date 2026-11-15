#!/usr/bin/env bash
# Prepares a release: bumps every first-party crate and the add-on to VERSION, turns the
# "Unreleased" changelog section into "VERSION", commits and tags vVERSION.
# Pushing the tag (git push origin master vVERSION) publishes the add-on images; Home Assistant
# then offers the update. Run the pre-release ub-mode pass (AGENTS.md, Releases) first.
#
#   scripts/release.sh 0.2.0
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "usage: $0 X.Y.Z" >&2
    exit 1
fi
cd "$(git rev-parse --show-toplevel)"
if [[ -n "$(git status --porcelain)" ]]; then
    echo "the working tree is not clean" >&2
    exit 1
fi
if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
    echo "tag v$version already exists" >&2
    exit 1
fi
changelog=ha-addon/ticknode/CHANGELOG.md
if ! grep -q '^## Unreleased' "$changelog"; then
    echo "$changelog has no '## Unreleased' section to release" >&2
    exit 1
fi

# The package version is the first `version = ` line of each workspace member manifest.
for manifest in $(cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json,sys; [print(p["manifest_path"]) for p in json.load(sys.stdin)["packages"]]'); do
    sed -i "0,/^version = \".*\"/s//version = \"$version\"/" "$manifest"
done
sed -i "s/^version: \".*\"/version: \"$version\"/" ha-addon/ticknode/config.yaml
sed -i "s/^## Unreleased.*/## $version/" "$changelog"
cargo update --workspace --quiet

git add -A
git commit -q -m "chore(release): v$version"
git tag -a "v$version" -m "TickNode $version"
echo "Tagged v$version. Publish with: git push origin master v$version"
