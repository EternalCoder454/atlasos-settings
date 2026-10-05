#!/bin/bash
# Headless UI stress run, inside the dev container (scripts/dev.sh scripts/ui-stress.sh):
# starts the built app under Xvfb on a private session bus, with XDG dirs
# under /work/ui-stress/xdg, and drives it with xdotool through first run,
# sidebar clicks, search, deep links to the running instance, a narrow window,
# rapid input, restart and 1.5x scale. Saves numbered screenshots and a
# PASS/FAIL summary; exits non-zero if any check failed.
#   scripts/ui-stress.sh
# Env: ATLAS_SETTINGS_BIN (default /work/cmake/dev/atlas-settings),
#      UI_STRESS_OUT (default /work/ui-stress/out; must be under
#      /work/ui-stress; wiped at start).
# Sidebar click coordinates are window-relative at QT_SCALE_FACTOR=1
# (window 1008x720) and may need adjusting if the layout changes.

bin=${ATLAS_SETTINGS_BIN:-/work/cmake/dev/atlas-settings}
out=${UI_STRESS_OUT:-/work/ui-stress/out}
base=/work/ui-stress
root=$base/xdg

if [ "${1:-}" != --inner ]; then
    if ! command -v pgrep >/dev/null 2>&1 || ! command -v pkill >/dev/null 2>&1; then
        echo "ui-stress: pgrep/pkill (procps) are missing" >&2
        exit 2
    fi
    # The out dir is wiped: only under /work/ui-stress, after resolving any
    # `..` and symlinks.
    out=$(realpath -m -- "$out") || exit 2
    case $out in
        /work/ui-stress/?*) ;;
        *) echo "ui-stress: UI_STRESS_OUT must be under /work/ui-stress" >&2; exit 2 ;;
    esac
    export UI_STRESS_OUT=$out
    rm -rf -- "$root" "$out"
    mkdir -p -- "$base" "$out" "$root"/{config,data,cache,runtime} || exit 2
    chmod 700 "$root/runtime"
    # A session bus that can't start services (no portals, no daemons).
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
    exec dbus-run-session --config-file="$base/bus.conf" -- xvfb-run -a -s "-screen 0 1920x1080x24" "$0" --inner "$@"
fi
shift
# The teardown kills every atlas-settings and kcmshell6: only in the dev
# container, never on the desktop.
if [ ! -e /run/.containerenv ] || [ ! -d /work ]; then
    echo "ui-stress: runs only inside the dev container (scripts/dev.sh)" >&2
    exit 2
fi

export XDG_CONFIG_HOME=$root/config XDG_DATA_HOME=$root/data XDG_CACHE_HOME=$root/cache XDG_RUNTIME_DIR=$root/runtime
export LANG=C.UTF-8 LC_ALL=C.UTF-8
export QT_QPA_PLATFORM=xcb QT_FORCE_STDERR_LOGGING=1 QT_SCALE_FACTOR=1
unset WAYLAND_DISPLAY

summary=$out/summary.txt
: >"$summary"
fails=0
shotn=0
launchn=0
pid=
win=

pass() { echo "PASS: $*" | tee -a "$summary"; }
fail() {
    echo "FAIL: $*" | tee -a "$summary"
    fails=$((fails + 1))
}
check() { # check <description> <command...>: PASS when the command succeeds
    local desc=$1
    shift
    if "$@"; then pass "$desc"; else fail "$desc"; fi
}

alive() { [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }
count_app() { pgrep -x atlas-settings | wc -l; }
one_instance() { [ "$(count_app)" -eq 1 ]; }

find_window() { # prints the window id, waits up to ~10 s
    local i w
    for i in $(seq 1 50); do
        w=$(xdotool search --onlyvisible --name '^Settings' 2>/dev/null | head -1 || true)
        if [ -n "$w" ]; then
            echo "$w"
            return 0
        fi
        sleep 0.2
    done
    return 1
}

start_app() { # start_app [scale]: launches the first instance, sets pid and win
    local scale=${1:-1}
    launchn=$((launchn + 1))
    QT_SCALE_FACTOR=$scale "$bin" >"$out/app-$launchn.log" 2>&1 &
    pid=$!
    win=$(find_window) || win=
    if [ -z "$win" ]; then
        fail "launch $launchn: no Settings window within 10 s"
        return 1
    fi
    xdotool windowfocus "$win" 2>/dev/null || true
    sleep 0.8
    return 0
}

stop_app() {
    if [ -n "$pid" ]; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
    pid=
}

second_launch() { # second_launch <args...>: hands args to the running instance
    launchn=$((launchn + 1))
    timeout 10 "$bin" "$@" >"$out/app-$launchn.log" 2>&1
    local rc=$?
    sleep 0.8
    [ "$rc" -ne 124 ]
}

shot() { # shot <name>: numbered screenshot of the window
    shotn=$((shotn + 1))
    local f
    f=$(printf '%s/%02d-%s.png' "$out" "$shotn" "$1")
    LAST_SHOT=$f
    if [ -z "$win" ] || ! import -window "$win" "$f" 2>>"$out/import.log"; then
        fail "screenshot $1 failed"
        return 1
    fi
    return 0
}

differs() { ! cmp -s -- "$1" "$2"; }

click() { # click <x> <y>, window-relative
    xdotool mousemove --window "$win" "$1" "$2" click 1
    sleep 0.3
}

focus_win() {
    xdotool windowfocus "$win" 2>/dev/null || true
    xdotool mousemove --window "$win" 600 400
}

key() { xdotool key --delay 80 "$@"; sleep 0.4; }

settingsrc=$XDG_CONFIG_HOME/atlas-settingsrc
has_page_saved() { grep -q '^Page=' "$settingsrc" 2>/dev/null; }

cleanup() {
    stop_app
    pkill -x kcmshell6 2>/dev/null || true
    pkill -x atlas-settings 2>/dev/null || true
    sleep 0.5
    if [ "$(pgrep -x atlas-settings | wc -l)" -eq 0 ] && [ "$(pgrep -x kcmshell6 | wc -l)" -eq 0 ]; then
        pass "teardown: no atlas-settings or kcmshell6 left"
    else
        fail "teardown: atlas-settings or kcmshell6 still running"
    fi
    if grep -nE 'qrc:|\.qml:[0-9]+|TypeError|ReferenceError|is not defined|Cannot assign|Unable to assign' "$out"/app-*.log >"$out/qml-problems.txt" 2>&1; then
        fail "QML problems in the logs (see $out/qml-problems.txt)"
    else
        pass "no QML problems in the logs"
    fi
    echo "summary: $summary"
    echo "screenshots and logs: $out"
    echo "failures: $fails"
    if [ "$fails" -ne 0 ]; then exit 1; fi
    exit 0
}
trap cleanup EXIT
# A signal (run.sh's timeout, Ctrl+C) runs the EXIT trap too.
trap 'exit 143' TERM INT HUP

if [ ! -x "$bin" ]; then
    fail "binary $bin is missing or not executable"
    exit 1
fi

# 1. First run: home is Wi-Fi & Network.
if ! start_app; then exit 1; fi
shot first-run
check "1 app alive after first run" alive

# 2. Sidebar clicks.
click 70 135
shot sidebar-bluetooth
click 70 284
shot sidebar-sound
prev=$LAST_SHOT
xdotool mousemove --window "$win" 70 400
for _ in 1 2 3 4 5 6 7 8; do xdotool click 5; done
# Long enough for the flick to stop: a click while it moves only stops it.
sleep 1.5
shot sidebar-scrolled
# More Settings is the last entry; the exact y is not known, so click the
# bottom of the sidebar viewport.
click 70 690
shot sidebar-more
check "2 sidebar clicks changed the page" differs "$prev" "$LAST_SHOT"
check "2 app alive after sidebar clicks" alive

# 3. Search.
focus_win
key ctrl+f
# A query with several results, so Down has somewhere to go.
xdotool type --delay 60 "sound"
sleep 0.6
shot search-sound
before=$LAST_SHOT
key Down
key Down
shot search-down2
check "3 Down moved the search selection" differs "$before" "$LAST_SHOT"
before=$LAST_SHOT
key Return
sleep 0.5
shot search-opened
check "3 Enter opened a page" differs "$before" "$LAST_SHOT"
focus_win
key ctrl+f
xdotool type --delay 60 "zzzzqq"
sleep 0.6
shot search-nothing
before=$LAST_SHOT
key Escape
shot search-cleared
check "3 Escape cleared the search" differs "$before" "$LAST_SHOT"
check "3 app alive after search" alive

# 4. Second launches hand their arguments to the running instance.
for args in "bluetooth" "displays night-light" "--search sound" "--kcm kcm_nightlight" "--page more" "--bogus"; do
    prev=$LAST_SHOT
    # shellcheck disable=SC2086 # args is a fixed list of plain words
    if second_launch $args; then
        pass "4 second launch '$args' returned"
    else
        fail "4 second launch '$args' hung"
    fi
    shot "deeplink-$(echo "$args" | tr -c 'a-z0-9\n' '-')"
    check "4 exactly one atlas-settings after '$args'" one_instance
done
# --bogus changes no page: only the banner saying it was ignored.
check "4 --bogus shows a banner" differs "$prev" "$LAST_SHOT"
check "4 --bogus was refused by the running window" grep -qF 'ignored argument "--bogus": unknown option' "$out/app-1.log"
check "4 app alive after deep links" alive
# KCM arguments are data: shell syntax in them must reach kcmshell6 as one
# word, never run. kcm_proxy is installed in the dev image.
rm -f -- "$base/pwn1" "$base/pwn2"
# Only this launch's kcmshell6 counts below.
pkill -x kcmshell6 2>/dev/null || true
sleep 0.3
# shellcheck disable=SC2016 # the $(...) and backticks are meant literally
second_launch --kcm kcm_proxy --args "\$(touch $base/pwn1) \`touch $base/pwn2\` ';touch $base/pwn1'" || true
sleep 1
check "4 --kcm with arguments started kcmshell6" pgrep -f 'kcmshell6 kcm_proxy'
check "4 shell syntax in --args ran nothing" test ! -e "$base/pwn1" -a ! -e "$base/pwn2"
pkill -x kcmshell6 2>/dev/null || true
sleep 0.3

# 5. Narrow window.
xdotool windowsize "$win" 520 700
sleep 0.8
shot narrow
check "5 app alive when narrow" alive
# The folded sidebar has no room for the search field: it moves above the page.
before=$LAST_SHOT
focus_win
key ctrl+f
xdotool type --delay 60 "sound"
sleep 0.6
shot narrow-search
check "5 search works when narrow" differs "$before" "$LAST_SHOT"
key ctrl+a BackSpace
xdotool windowsize "$win" 1008 720
sleep 0.8
shot narrow-restored

# 6. Stress: rapid sidebar clicks and rapid search typing.
for i in $(seq 1 20); do
    case $((i % 6)) in
        0) y=95 ;;
        1) y=135 ;;
        2) y=175 ;;
        3) y=244 ;;
        4) y=284 ;;
        *) y=324 ;;
    esac
    xdotool mousemove --window "$win" 70 "$y" click 1
    sleep 0.05
done
sleep 0.5
shot stress-clicks
check "6 app alive after 20 rapid clicks" alive
check "6 rapid clicks started kcmshell6 at most once" test "$(pgrep -x kcmshell6 | wc -l)" -le 1
# A kcmshell6 window would cover the later screenshots.
pkill -x kcmshell6 2>/dev/null || true
focus_win
key ctrl+f
xdotool type --delay 5 "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz"
sleep 0.5
shot stress-typed
check "6 app alive after 60 typed characters" alive
for _ in $(seq 1 60); do xdotool key --delay 5 BackSpace; done
sleep 0.5
shot stress-backspaced
check "6 app alive after 60 BackSpaces" alive
key ctrl+a BackSpace

# 7. Restart: the last page comes back.
click 70 135
stop_app
check "7 atlas-settingsrc has Page=" has_page_saved
if start_app; then
    shot restart
    check "7 app alive after restart" alive
fi

# 8. 1.5x scale, like the real desktop.
stop_app
if start_app 1.5; then
    shot scale-1_5
    check "8 app alive at 1.5x" alive
fi
