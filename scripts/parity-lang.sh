#!/usr/bin/env bash
# Compare the language commands of this `mz` with the language repo's own `mz` (built from
# the same tag) over its examples and primitives: stdout, stderr and exit status must match.
# usage: scripts/parity-lang.sh <path to mzizi-dev/mzizi checkout> [path to this mz]
set -euo pipefail
lang=${1:?path to a mzizi-dev/mzizi checkout, with target/debug/mz built}
new=${2:-target/debug/mz}
old="$lang/target/debug/mz"
cd "$lang"
n=0
bad=0
for f in examples/*.mz examples/components/*.mz primitives/*.mz; do
  for c in "check" "check --agent" contract outline hash ir; do
    n=$((n + 1))
    # shellcheck disable=SC2086
    a=$("$old" $c "$f" 2>&1 | sed -E 's/[0-9]+ ?ms/Nms/g'; echo "rc=${PIPESTATUS[0]}")
    # shellcheck disable=SC2086
    b=$("$OLDPWD/$new" $c "$f" 2>&1 | sed -E 's/[0-9]+ ?ms/Nms/g'; echo "rc=${PIPESTATUS[0]}")
    [ "$a" = "$b" ] || { bad=$((bad + 1)); echo "DIFF $c $f"; }
  done
done
echo "cases=$n diffs=$bad"
[ "$bad" -eq 0 ]
