#!/bin/bash
# The move from the old file names: a settings file as Atlas Settings wrote
# it (`atlas-settingsrc`, group [Atlas], the page last shown) is adopted as
# `telamon-settingsrc` and the window opens on that page. Inside the dev
# container (scripts/dev.sh scripts/smoke-migrate.sh); a headless smoke run on
# its own XDG folders, never the user's.
#   Env: SMOKE_OUT (default /work/smoke/migrate)
set -euo pipefail

root=/work/smoke/xdg
out=${SMOKE_OUT:-/work/smoke/migrate}
mkdir -p "$root/config" "$out"
rm -f "$root"/config/atlas-settingsrc "$root"/config/telamon-settingsrc
printf '[Atlas]\nFormat=1\n\n[Window]\nPage=sound\n' >"$root/config/atlas-settingsrc"

# The run's exit status is not used: smoke.sh says "smoke: ok" itself (its
# xvfb-run ends with the status of the process it stopped).
SMOKE_OUT=$out "$(dirname "$0")/smoke.sh" | tee "$out/run.txt" || true
grep -q "^smoke: ok" "$out/run.txt" || { echo "migrate: the smoke run failed" >&2; exit 1; }

new=$root/config/telamon-settingsrc
[ -f "$new" ] || { echo "migrate: $new was not made" >&2; exit 1; }
grep -qx 'Page=sound' "$new" || { echo "migrate: the page was not carried over:" >&2; cat "$new" >&2; exit 1; }
grep -q '^\[Atlas\]' "$new" && { echo "migrate: the new file still has [Atlas]" >&2; exit 1; }
grep -qx '\[Telamon\]' "$new" || { echo "migrate: the new file has no [Telamon] group" >&2; exit 1; }
[ -f "$root/config/atlas-settingsrc" ] || { echo "migrate: the old file was removed (the old app still reads it)" >&2; exit 1; }
echo "migrate: ok"
