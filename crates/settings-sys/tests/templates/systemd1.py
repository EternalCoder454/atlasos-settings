"""systemd mock template for the settings-sys tests: just the unit calls
Settings makes (python-dbusmock's own `systemd` has no unit files).
Parameters: Unit (default firewalld.service), LoadState, ActiveState,
UnitFileState. Start, stop, enable and disable change the states the way
systemd does.
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.freedesktop.systemd1"
MAIN_OBJ = "/org/freedesktop/systemd1"
MAIN_IFACE = "org.freedesktop.systemd1.Manager"
UNIT_IFACE = "org.freedesktop.systemd1.Unit"
SYSTEM_BUS = True


def unit_path(name):
    return "/org/freedesktop/systemd1/unit/" + name.replace(".", "_2e").replace("-", "_2d")


def check(mock, name):
    if name != mock.unit:
        raise dbus.exceptions.DBusException(
            f"no such unit {name}", name="org.freedesktop.systemd1.NoSuchUnit"
        )


def set_state(mock, prop, value):
    from dbusmock import get_object

    get_object(unit_path(mock.unit)).Set(UNIT_IFACE, prop, value)


def load(mock, parameters):
    mock.unit = parameters.get("Unit", "firewalld.service")
    mock.check = check
    mock.set_state = set_state
    mock.unit_path = unit_path
    mock.AddObject(
        unit_path(mock.unit),
        UNIT_IFACE,
        dbus.Dictionary(
            {
                "LoadState": parameters.get("LoadState", "loaded"),
                "ActiveState": parameters.get("ActiveState", "active"),
                "UnitFileState": parameters.get("UnitFileState", "enabled"),
            },
            signature="sv",
        ),
        [],
    )
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("LoadUnit", "s", "o", "self.check(self, args[0])\nret = dbus.ObjectPath(self.unit_path(args[0]))"),
            (
                "StartUnit",
                "ss",
                "o",
                "self.check(self, args[0])\nself.set_state(self, 'ActiveState', 'active')\nret = dbus.ObjectPath('/org/freedesktop/systemd1/job/1')",
            ),
            (
                "StopUnit",
                "ss",
                "o",
                "self.check(self, args[0])\nself.set_state(self, 'ActiveState', 'inactive')\nret = dbus.ObjectPath('/org/freedesktop/systemd1/job/2')",
            ),
            (
                "EnableUnitFiles",
                "asbb",
                "ba(sss)",
                "self.check(self, args[0][0])\nself.set_state(self, 'UnitFileState', 'enabled')\nret = (True, [])",
            ),
            (
                "DisableUnitFiles",
                "asb",
                "a(sss)",
                "self.check(self, args[0][0])\nself.set_state(self, 'UnitFileState', 'disabled')\nret = []",
            ),
        ],
    )
