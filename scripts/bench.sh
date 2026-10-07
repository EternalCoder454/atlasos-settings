#!/bin/bash
# Footprint against docs/DESIGN.md's budgets, inside the dev container
# (scripts/dev.sh scripts/bench.sh), on a release build: launch to a shown
# window (median of 7 warm launches), RSS after launch, RSS after visiting
# every page, and idle CPU over 60 s. Same private bus, Xvfb and XDG dirs as
# scripts/ui-stress.sh. Search per keystroke is the ignored test
# crates/settings-registry/tests/search_speed.rs.
#   scripts/bench.sh
# Env: TELAMON_SETTINGS_BIN (default /work/cmake/rel/telamon-settings).
# Prints one line per measure and writes /work/bench/results.txt; it only
# reports against the budgets, it doesn't fail on them. Page IDs are read
# from the registry's source (pages.rs).

bin=${TELAMON_SETTINGS_BIN:-/work/cmake/rel/telamon-settings}
base=/work/bench
root=$base/xdg

if [ "${1:-}" != --inner ]; then
    rm -rf -- "$root"
    mkdir -p -- "$root"/{config,data,cache,runtime} || exit 2
    chmod 700 "$root/runtime"
    cat >"$base/bus.conf" <<'CONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
    exec dbus-run-session --config-file="$base/bus.conf" -- xvfb-run -a -s "-screen 0 1920x1080x24" "$0" --inner
fi
if [ ! -e /run/.containerenv ] || [ ! -d /work ]; then
    echo "bench: runs only inside the dev container (scripts/dev.sh)" >&2
    exit 2
fi

export XDG_CONFIG_HOME=$root/config XDG_DATA_HOME=$root/data XDG_CACHE_HOME=$root/cache XDG_RUNTIME_DIR=$root/runtime
export LANG=C.UTF-8 LC_ALL=C.UTF-8
export QT_QPA_PLATFORM=xcb QT_FORCE_STDERR_LOGGING=1 QT_SCALE_FACTOR=1
unset WAYLAND_DISPLAY

results=$base/results.txt
: >"$results"
say() { echo "$*" | tee -a "$results"; }
pid=
stop() {
    if [ -n "$pid" ]; then
        kill "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
    fi
    pid=
}
trap 'stop; pkill -x telamon-settings 2>/dev/null; pkill -x kcmshell6 2>/dev/null' EXIT
# A signal (run.sh's timeout, Ctrl+C) runs the EXIT trap too.
trap 'exit 143' TERM INT HUP

if [ ! -x "$bin" ]; then
    say "FAIL: $bin is missing"
    exit 1
fi

now_ms() { echo $(($(date +%s%N) / 1000000)); }
rss_mb() { awk '/^Rss:/ { printf "%.1f", $2 / 1024 }' "/proc/$1/smaps_rollup"; }
cpu_ticks() { awk '{ print $14 + $15 }' "/proc/$1/stat"; }

# Launch to a shown window. The first launch warms the caches and isn't counted.
times=()
for i in $(seq 0 7); do
    t0=$(now_ms)
    "$bin" >"$base/app-$i.log" 2>&1 &
    pid=$!
    if ! timeout 15 xdotool search --sync --onlyvisible --name '^Telamon Settings' >/dev/null 2>&1 || ! kill -0 "$pid" 2>/dev/null; then
        say "FAIL: launch $i showed no window"
        exit 1
    fi
    t1=$(now_ms)
    [ "$i" -gt 0 ] && times+=($((t1 - t0)))
    [ "$i" -lt 7 ] && stop
done
median=$(printf '%s\n' "${times[@]}" | sort -n | sed -n 4p)
say "launch to window: median ${median} ms of 7 (${times[*]}); budget 250 ms"

sleep 3
say "RSS after launch: $(rss_mb "$pid") MB; budget 140 MB"

# Idle: the window shown, nothing happening.
hz=$(getconf CLK_TCK)
c0=$(cpu_ticks "$pid")
sleep 60
c1=$(cpu_ticks "$pid")
say "idle CPU over 60 s: $(((c1 - c0) * 1000 / hz)) ms of CPU time; budget 0"

# Every page, through the running instance. The IDs come from the registry,
# the one list of pages.
pages=$(grep -oP '^\s+id: "\K[^"]+' /src/crates/settings-registry/src/pages.rs)
if [ -z "$pages" ]; then
    say "FAIL: no page IDs found in the registry"
    exit 1
fi
for page in $pages; do
    timeout 10 "$bin" "$page" >/dev/null 2>&1
    sleep 0.4
    pkill -x kcmshell6 2>/dev/null
done
sleep 2
say "RSS after every page: $(rss_mb "$pid") MB; budget 220 MB"
stop
