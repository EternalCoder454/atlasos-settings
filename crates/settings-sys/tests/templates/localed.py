"""systemd-localed mock template, for the settings-sys tests (python-dbusmock
has none). Parameters: Locale (a list of "KEY=value").
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.freedesktop.locale1"
MAIN_OBJ = "/org/freedesktop/locale1"
MAIN_IFACE = "org.freedesktop.locale1"
SYSTEM_BUS = True


def load(mock, parameters):
    mock.AddMethods(
        MAIN_IFACE,
        [
            (
                "SetLocale",
                "asb",
                "",
                f'self.Set("{MAIN_IFACE}", "Locale", dbus.Array(args[0], signature="s"))',
            ),
        ],
    )
    mock.AddProperties(
        MAIN_IFACE,
        dbus.Dictionary(
            {
                "Locale": dbus.Array(parameters.get("Locale", ["LANG=C.UTF-8"]), signature="s"),
                "VConsoleKeymap": "us",
                "X11Layout": "us",
            },
            signature="sv",
        ),
    )
