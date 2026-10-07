"""systemd-hostnamed mock template, for the settings-sys tests (python-dbusmock
has none). Parameters: PrettyHostname, StaticHostname, Hostname, Chassis.
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.freedesktop.hostname1"
MAIN_OBJ = "/org/freedesktop/hostname1"
MAIN_IFACE = "org.freedesktop.hostname1"
SYSTEM_BUS = True


def load(mock, parameters):
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("SetPrettyHostname", "sb", "", f'self.Set("{MAIN_IFACE}", "PrettyHostname", args[0])'),
            (
                "SetStaticHostname",
                "sb",
                "",
                f'self.Set("{MAIN_IFACE}", "StaticHostname", args[0]); '
                f'self.Set("{MAIN_IFACE}", "Hostname", args[0])',
            ),
        ],
    )
    mock.AddProperties(
        MAIN_IFACE,
        dbus.Dictionary(
            {
                "Hostname": parameters.get("Hostname", "localhost"),
                "StaticHostname": parameters.get("StaticHostname", ""),
                "PrettyHostname": parameters.get("PrettyHostname", ""),
                "Chassis": parameters.get("Chassis", "laptop"),
            },
            signature="sv",
        ),
    )
