#!/bin/bash
# Headless smoke run (scripts/smoke.sh) with the system services mocked by
# python-dbusmock on a private system bus, so pages show real-looking data
# without touching the machine's own services. Inside the dev container:
#   SMOKE_OUT=/work/smoke/<name> SMOKE_SCENARIO=laptop scripts/dev.sh scripts/smoke-mock.sh [app arguments]
# Scenarios: desktop (default; no battery, no fingerprint reader) and laptop.
# SMOKE_DARK=1 starts the app with the AtlasOS dark colour scheme.
# SMOKE_SERVICES (default all): power accounts fprintd firewalld permissions.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scenario=${SMOKE_SCENARIO:-desktop}
services=${SMOKE_SERVICES:-power accounts fprintd firewalld permissions}
templates=$here/../crates/settings-sys/tests/templates
root=/work/smoke/mock
rm -rf "$root"
mkdir -p "$root"
chmod 700 "$root"

cat >"$root/system.conf" <<EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path=$root/system-bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF
dbus-daemon --config-file="$root/system.conf" --nofork --nopidfile &
pids=($!)
cleanup() {
    for p in "${pids[@]}"; do
        kill "$p" 2>/dev/null || true
        wait "$p" 2>/dev/null || true
    done
}
trap cleanup EXIT
trap 'exit 143' TERM INT HUP
export DBUS_SYSTEM_BUS_ADDRESS=unix:path=$root/system-bus
unset DBUS_SESSION_BUS_ADDRESS
for _ in $(seq 100); do [ -S "$root/system-bus" ] && break; sleep 0.05; done

# Starts a dbusmock template (a name of python-dbusmock's, or a path) and
# waits for its bus name.
mock() {
    local template=$1 name=$2 params=${3:-}
    if [ -n "$params" ]; then
        python3 -m dbusmock --system --template "$template" --parameters "$params" >/dev/null &
    else
        python3 -m dbusmock --system --template "$template" >/dev/null &
    fi
    pids+=($!)
    for _ in $(seq 200); do
        python3 - "$name" <<'PY' && return 0 || sleep 0.05
import sys, dbus
bus = dbus.SystemBus()
sys.exit(0 if bus.name_has_owner(sys.argv[1]) else 1)
PY
    done
    echo "smoke-mock: $name never appeared" >&2
    exit 1
}

# python snippet runner with the system bus and a helper to call a mock.
py() { python3 -c "import dbus
bus = dbus.SystemBus()
def call(name, path, iface, method, *args):
    return dbus.Interface(bus.get_object(name, path), iface).__getattr__(method)(*args)
$1"; }

for s in $services; do
    case $s in
    power)
        mock power_profiles_daemon net.hadess.PowerProfiles '{"ActiveProfile": "balanced"}'
        mock upower org.freedesktop.UPower
        if [ "$scenario" = laptop ]; then
            py '
call("org.freedesktop.UPower", "/org/freedesktop/UPower", "org.freedesktop.DBus.Mock", "AddDischargingBattery", "BAT0", "Smoke Battery", 64.0, 9000)
dev = "/org/freedesktop/UPower/devices/BAT0"
m = dbus.Interface(bus.get_object("org.freedesktop.UPower", dev), "org.freedesktop.DBus.Mock")
m.AddProperty("org.freedesktop.UPower.Device", "Capacity", 91.0)
m.AddProperty("org.freedesktop.UPower.Device", "ChargeThresholdSupported", True)
m.AddProperty("org.freedesktop.UPower.Device", "ChargeThresholdEnabled", False)
'
        fi
        ;;
    accounts)
        # The smoke run is root in the container: that is "you".
        mock "$templates/accounts_service.py" org.freedesktop.Accounts '{"Users": [
            {"Uid": 0, "UserName": "ada", "RealName": "Ada Lovelace", "AccountType": 1},
            {"Uid": 1001, "UserName": "grace", "RealName": "Grace Hopper", "AccountType": 1},
            {"Uid": 1002, "UserName": "kit", "RealName": "Kit Marlowe"}]}'
        ;;
    fprintd)
        if [ "$scenario" = laptop ]; then
            mock "$templates/fprintd.py" net.reactivated.Fprint '{"Enrolled": ["right-index-finger", "left-thumb"]}'
        fi
        ;;
    esac
done

if [ "${SMOKE_DARK:-0}" = 1 ]; then
    mkdir -p /work/smoke/xdg/config /work/smoke/xdg/data/color-schemes
    dark=${SMOKE_DARK_SCHEME:-/work/smoke/AtlasOSDark.colors}
    cp "$dark" /work/smoke/xdg/data/color-schemes/AtlasOSDark.colors
    { printf '[General]\nColorScheme=AtlasOSDark\n\n'; cat "$dark"; } >/work/smoke/xdg/config/kdeglobals
else
    rm -f /work/smoke/xdg/config/kdeglobals
fi

"$here/smoke.sh" "$@"
