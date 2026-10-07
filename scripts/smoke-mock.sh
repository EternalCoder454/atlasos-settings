#!/bin/bash
# Headless smoke run (scripts/smoke.sh) with the system services mocked by
# python-dbusmock on a private system bus, so pages show real-looking data
# without touching the machine's own services. Inside the dev container:
#   SMOKE_OUT=/work/smoke/<name> SMOKE_SCENARIO=laptop scripts/dev.sh scripts/smoke-mock.sh [app arguments]
# Scenarios: desktop (default; no battery, no fingerprint reader) and laptop.
# SMOKE_DARK=1 starts the app with the Telamon OS dark colour scheme.
# SMOKE_SERVICES (default all but updates): power accounts fprintd firewalld permissions apps;
# `updates` mocks the system helper (SMOKE_SERVICES=updates scripts/smoke-mock.sh updates).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scenario=${SMOKE_SCENARIO:-desktop}
services=${SMOKE_SERVICES:-power accounts fprintd firewalld permissions apps}
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
    permissions)
        # Flatpak apps (fixtures in the smoke run's XDG_DATA_HOME), and the
        # portals' permission store on the smoke run's session bus.
        data=/work/smoke/xdg/data
        rm -rf "$data/flatpak"
        flatpak_app() {
            local id=$1 name=$2 icon=$3 context=$4
            local dep=$data/flatpak/app/$id/current/active
            mkdir -p "$dep/export/share/applications"
            printf '[Application]\nname=%s\nruntime=org.freedesktop.Platform/x86_64/24.08\n\n[Context]\n%s' "$id" "$context" >"$dep/metadata"
            printf '[Desktop Entry]\nType=Application\nName=%s\nIcon=%s\n' "$name" "$icon" >"$dep/export/share/applications/$id.desktop"
        }
        flatpak_app com.discordapp.Discord Discord internet-chat 'shared=network;ipc;
sockets=x11;pulseaudio;
devices=dri;
filesystems=xdg-download;
'
        flatpak_app org.mozilla.firefox Firefox firefox 'shared=network;ipc;
filesystems=xdg-download;
'
        flatpak_app org.videolan.VLC "VLC media player" vlc 'shared=network;ipc;
devices=all;
filesystems=host;
'
        cat >"$root/pre.sh" <<EOF
#!/bin/bash
exec python3 -m dbusmock --template "$templates/permission_store.py" --parameters '{"Tables": {"background": {"background": {"com.discordapp.Discord": ["yes"], "org.mozilla.firefox": ["no"]}}, "devices": {"camera": {"com.discordapp.Discord": ["yes"]}}}}' >/dev/null 2>&1
EOF
        chmod +x "$root/pre.sh"
        export SMOKE_PRE=$root/pre.sh
        ;;
    apps)
        # Installed apps and startup entries for Default Apps and Startup Apps.
        data=/work/smoke/xdg/data
        mkdir -p "$data/applications" /work/smoke/xdg/config/autostart
        desktop() {
            printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s\nIcon=%s\nComment=%s\n%s\n' "$1" "$2" "$3" "$4" "$5" >"$data/applications/$6.desktop"
        }
        desktop Brave brave internet-web-browser "Browse the web" 'Categories=Network;WebBrowser;
MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;' brave
        desktop Firefox firefox firefox "Browse the web" 'Categories=Network;WebBrowser;
MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;' firefox
        desktop Thunderbird thunderbird mail-client "Read and write email" 'MimeType=x-scheme-handler/mailto;' thunderbird
        desktop Dolphin dolphin system-file-manager "Manage files" 'MimeType=inode/directory;' dolphin
        desktop Konsole konsole utilities-terminal "Terminal" 'Categories=System;TerminalEmulator;' konsole
        desktop "Elisa" elisa elisa "Play music" 'MimeType=audio/mpeg;audio/flac;' elisa
        desktop "VLC media player" vlc vlc "Play video" 'MimeType=audio/mpeg;video/mp4;video/x-matroska;' vlc
        desktop Gwenview gwenview gwenview "View images" 'MimeType=image/png;image/jpeg;' gwenview
        desktop Steam steam steam "Games" '' steam
        cp "$data/applications/steam.desktop" /work/smoke/xdg/config/autostart/steam.desktop
        cp "$data/applications/thunderbird.desktop" /work/smoke/xdg/config/autostart/thunderbird.desktop
        printf '[Desktop Entry]\nType=Application\nName=Telamon Updater\nExec=telamon-updater-tray\nIcon=system-software-update\nComment=Checks for updates\n' >/work/smoke/xdg/config/autostart/net.eterneon.telamon.updater-tray.desktop
        printf '[Default Applications]\nx-scheme-handler/https=brave.desktop;\nx-scheme-handler/http=brave.desktop;\ninode/directory=dolphin.desktop;\n' >/work/smoke/xdg/config/mimeapps.list
        ;;
    updates)
        # The system helper of Telamon Updater, for the Updates page:
        # SMOKE_UPDATES=uptodate|available|staged, SMOKE_UPDATES_HOLD=<seconds>
        # an Upgrade takes (the page shows the helper's progress meanwhile).
        mock "$templates/system_helper.py" net.eterneon.telamon.SystemHelper "{\"Scenario\": \"${SMOKE_UPDATES:-available}\", \"Hold\": ${SMOKE_UPDATES_HOLD:-0}}"
        ;;
    firewalld)
        mock "$templates/systemd1.py" org.freedesktop.systemd1
        mock "$templates/firewalld.py" org.fedoraproject.FirewallD1 '{"DefaultZone": "AtlasOS",
            "Zones": {"AtlasOS": {"Services": ["mdns", "samba-client", "steam-streaming", "ssh"], "Ports": [["8080", "tcp"]]}},
            "Available": ["ssh", "mdns", "samba-client", "kdeconnect", "http", "https", "steam-streaming", "ftp", "syncthing"]}'
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
