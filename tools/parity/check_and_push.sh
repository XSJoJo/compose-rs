#!/bin/bash
# Merge the given SHAs into main (in order), then run the checks and push only if all pass.
# Stops at the first conflict outside docs/decisions.md, leaving the merge for a person to resolve.
# Usage: tools/parity/check_and_push.sh [--gradle] <sha>...   (run from anywhere in the checkout, on main)
# With no SHAs it only checks and pushes what main already has.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
out=${PARITY_OUT:-${TMPDIR:-/tmp}/compose-rs-parity}
mkdir -p "$out"
gradle=0
if [ "${1:-}" = "--gradle" ]; then gradle=1; shift; fi
if [ "$(git branch --show-current)" != main ]; then echo "STOP: not on main"; exit 1; fi
if git rev-parse -q --verify MERGE_HEAD >/dev/null; then echo "STOP: a merge is already in progress"; exit 3; fi
git fetch -q origin
for sha in "$@"; do
  if ! git merge --no-edit "$sha" >/dev/null 2>&1; then
    u=$(git ls-files -u | cut -f2 | sort -u)
    if [ "$u" != "docs/decisions.md" ]; then echo "STOP: conflicts merging $sha: $u"; exit 4; fi
    python3 -I "$here/resolve_decisions.py" || { echo "STOP: decision index check failed"; exit 5; }
    git add docs/decisions.md
    printf 'Merge %s\n' "$(git log -1 --format=%s "$sha" | cut -c1-60)" | git commit -q -F - || { echo "STOP: commit failed"; exit 6; }
  fi
  echo "merged $sha as $(git log -1 --format=%h)"
done
python3 -I "$here/resolve_decisions.py" || { echo "STOP: decision index check failed"; exit 5; }
if ! git diff --quiet docs/decisions.md; then
  git add docs/decisions.md
  printf 'The decision index counts after the merges\n' | git commit -q -F -
fi
if grep -rln -E '^(<<<<<<<|>>>>>>>) ' crates android docs examples benchmarks --include=*.rs --include=*.kt --include=*.md --include=*.toml 2>/dev/null | head -1 | grep -q .; then echo "STOP: conflict markers left"; exit 7; fi
n=$(git rev-list --objects origin/main..HEAD | grep -c /target/); [ "$n" = 0 ] || { echo "STOP: target objects in range"; exit 8; }
cargo test --workspace -q --no-fail-fast > "$out/tests.txt" 2>&1; t=$?
echo "tests exit $t"; awk '/^test result/{p+=$4;f+=$6}END{print p" passed "f" failed"}' "$out/tests.txt"
grep -E '^(error|test .* FAILED)' "$out/tests.txt" | sort | uniq -c | head
cargo clippy --workspace --all-targets -q > "$out/clippy.txt" 2>&1; c=$?; echo "clippy exit $c"
cargo fmt --check >/dev/null; d=$?; echo "fmt exit $d"
g=0
if [ $gradle = 1 ]; then (cd android && ./gradlew --offline :bench-rs:assembleRelease :compose-rs:lintRelease -q > "$out/gradle.txt" 2>&1); g=$?; echo "gradle exit $g"; fi
if [ $t = 0 ] && [ $c = 0 ] && [ $d = 0 ] && [ $g = 0 ]; then git push origin main 2>&1 | tail -1; else echo "not pushed"; exit 9; fi
