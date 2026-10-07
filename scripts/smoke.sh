#!/bin/bash
# Headless smoke run, inside the dev container (scripts/dev.sh scripts/smoke.sh):
# starts the built app under Xvfb on a private session bus, with XDG dirs
# under /work/smoke, opens it with the given arguments, waits, screenshots
# the window and checks the log for QML errors.
#   scripts/smoke.sh [app arguments...]
# Env: ATLAS_SETTINGS_BIN (default /work/cmake/dev/atlas-settings),
#      SMOKE_OUT (default /work/smoke/out), SMOKE_WAIT seconds (default 3),
#      SMOKE_SCALE (QT_SCALE_FACTOR, default 1).
set -euo pipefail

bin=${ATLAS_SETTINGS_BIN:-/work/cmake/dev/atlas-settings}
out=${SMOKE_OUT:-/work/smoke/out}
root=/work/smoke/xdg
mkdir -p "$out" "$root"/{config,data,cache,runtime}
chmod 700 "$root/runtime"

if [ "${1:-}" != --inner ]; then
    # A session bus that can't start services (no portals, no daemons).
    cat >/work/smoke/bus.conf <<'EOF'
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
EOF
    exec dbus-run-session --config-file=/work/smoke/bus.conf -- xvfb-run -a -s "-screen 0 1920x1080x24" "$0" --inner "$@"
fi
shift

export XDG_CONFIG_HOME=$root/config XDG_DATA_HOME=$root/data XDG_CACHE_HOME=$root/cache XDG_RUNTIME_DIR=$root/runtime
export LANG=C.UTF-8 LC_ALL=C.UTF-8
export QT_QPA_PLATFORM=xcb QT_FORCE_STDERR_LOGGING=1 QT_SCALE_FACTOR=${SMOKE_SCALE:-1}
unset WAYLAND_DISPLAY

# SMOKE_PRE: a program to run first in the session (services to mock on the
# session bus, fixtures); see smoke-mock.sh.
if [ -n "${SMOKE_PRE:-}" ]; then
    "$SMOKE_PRE" &
    sleep 1
fi
log=$out/app.log
"$bin" "$@" >"$log" 2>&1 &
pid=$!
# The app goes with the script, however the script ends.
trap 'kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null' EXIT
trap 'exit 143' TERM INT HUP
sleep "${SMOKE_WAIT:-3}"
if ! kill -0 "$pid" 2>/dev/null; then
    echo "smoke: the app exited early" >&2
    cat "$log" >&2
    exit 1
fi
# SMOKE_XDO: xdotool commands to run before the screenshot, e.g. to open a
# sheet ("mousemove 600 300 click 1 sleep 1"); the window is the screen's top left.
if [ -n "${SMOKE_XDO:-}" ]; then
    # shellcheck disable=SC2086 # split on purpose
    xdotool $SMOKE_XDO
    sleep 1
fi
w=$(xdotool search --onlyvisible --name '^Settings' | head -1 || true)
if [ -n "$w" ]; then
    import -window "$w" "$out/window.png"
    echo "screenshot: $out/window.png"
else
    echo "smoke: no Settings window found" >&2
fi
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
if grep -nE 'qrc:|\.qml:[0-9]+|TypeError|ReferenceError|is not defined|Cannot assign|Unable to assign' "$log"; then
    echo "smoke: QML warnings above (full log $log)" >&2
    exit 1
fi
echo "smoke: ok (log $log)"
