#!/usr/bin/env python3
"""Runs a command with python-dbusmock's NetworkManager and BlueZ on a
private bus, filled with a few networks and devices, and the address in
TELAMON_SETTINGS_TEST_BUS, which debug builds of Settings use instead of the
system bus (apps/telamon-settings/src/support.rs). For smoke runs that show
Network and Bluetooth & Devices working, in the dev container:

    scripts/dev.sh scripts/with-mock-services.py scripts/smoke.sh network

Never touches the machine's own buses or services.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

import dbus

CONFIG = """<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path={dir}/bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"""

NM = "org.freedesktop.NetworkManager"
BLUEZ = "org.bluez"
MOCK = "org.freedesktop.DBus.Mock"


def wait_for(bus, name):
    for _ in range(300):
        if bus.name_has_owner(name):
            return
        time.sleep(0.05)
    raise SystemExit(f"{name} never appeared")


def fill_network(bus):
    main = dbus.Interface(bus.get_object(NM, "/org/freedesktop"), MOCK)
    wifi = main.AddWiFiDevice("wlan0", "wlan0", 100)
    main.AddEthernetDevice("eth0", "enp3s0", 20)
    aps = [
        ("ap1", "Home", 82, 0x100),
        ("ap2", "Cafe Guest", 64, 0),
        ("ap3", "Neighbors 5G", 47, 0x100),
        ("ap4", "Office", 71, 0x200),
        ("ap5", "Hotspot of Sam", 33, 0x100),
        ("ap6", "xfinitywifi", 21, 0),
    ]
    paths = {}
    for name, ssid, strength, sec in aps:
        paths[ssid] = main.AddAccessPoint(
            wifi, name, ssid, "00:11:22:33:44:%02x" % strength, dbus.UInt32(2),
            dbus.UInt32(2412), dbus.UInt32(54000), dbus.Byte(strength), dbus.UInt32(sec),
        )
    conn = main.AddWiFiConnection(wifi, "home", "Home", "")
    main.AddWiFiConnection(wifi, "neighbors", "Neighbors 5G", "")
    main.AddActiveConnection([wifi], conn, paths["Home"], "home", dbus.UInt32(2))
    settings = dbus.Interface(
        bus.get_object(NM, "/org/freedesktop/NetworkManager/Settings"),
        "org.freedesktop.NetworkManager.Settings",
    )
    settings.AddConnection(
        {
            "connection": {"type": "vpn", "id": "Work VPN", "uuid": "11111111-2222-3333-4444-555555555555"},
            "vpn": {"service-type": "org.freedesktop.NetworkManager.openvpn"},
        }
    )


def fill_bluetooth(bus):
    main = dbus.Interface(bus.get_object(BLUEZ, "/"), "org.bluez.Mock")
    main.AddAdapter("hci0", "Zach's PC")
    devices = [
        ("AA:BB:CC:00:00:01", "WH-1000XM5", "audio-headset", True, True, 87),
        ("AA:BB:CC:00:00:02", "Keychron K3", "input-keyboard", True, False, 64),
        ("AA:BB:CC:00:00:03", "MX Master 3", "input-mouse", False, False, None),
        ("AA:BB:CC:00:00:04", "Pixel 9", "phone", False, False, None),
    ]
    for address, name, icon, paired, connected, battery in devices:
        path = main.AddDevice("hci0", address, name)
        dev = dbus.Interface(bus.get_object(BLUEZ, path), "org.freedesktop.DBus.Properties")
        dev.Set("org.bluez.Device1", "Icon", icon)
        if paired:
            main.PairDevice("hci0", address)
        if connected:
            main.ConnectDevice("hci0", address)
        if battery is not None:
            mock = dbus.Interface(bus.get_object(BLUEZ, path), MOCK)
            mock.AddProperties("org.bluez.Battery1", {"Percentage": dbus.Byte(battery)})


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    tmp = tempfile.mkdtemp(prefix="settings-mock-")
    os.chmod(tmp, 0o700)
    conf = os.path.join(tmp, "bus.conf")
    with open(conf, "w") as f:
        f.write(CONFIG.format(dir=tmp))
    daemon = subprocess.Popen(
        ["dbus-daemon", f"--config-file={conf}", "--nofork", "--nopidfile", "--print-address=1"],
        stdout=subprocess.PIPE, text=True,
    )
    children = [daemon]
    try:
        address = daemon.stdout.readline().strip()
        env = dict(os.environ, DBUS_SYSTEM_BUS_ADDRESS=address)
        env.pop("DBUS_SESSION_BUS_ADDRESS", None)
        for template in ("networkmanager", "bluez5"):
            children.append(subprocess.Popen(
                [sys.executable, "-m", "dbusmock", "--system", "--template", template],
                env=env, stdout=subprocess.DEVNULL,
            ))
        bus = dbus.bus.BusConnection(address)
        wait_for(bus, NM)
        wait_for(bus, BLUEZ)
        fill_network(bus)
        fill_bluetooth(bus)
        bus.close()
        code = subprocess.call(sys.argv[1:], env=dict(os.environ, TELAMON_SETTINGS_TEST_BUS=address))
    finally:
        for c in reversed(children):
            c.terminate()
        for c in children:
            c.wait()
        shutil.rmtree(tmp, ignore_errors=True)
    sys.exit(code)


main()
