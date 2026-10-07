"""firewalld mock template, for the settings-sys tests (python-dbusmock has
none). Parameters: DefaultZone, Zones ({zone: {"Services": [...],
"Ports": [[port, protocol]...]}}), ActiveZones (the zone names in use;
default: the default zone), Available (the service names firewalld knows).

The calls it answers are the runtime zone API and `runtimeToPermanent`;
adding a service that isn't in Available is firewalld's INVALID_SERVICE.
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.fedoraproject.FirewallD1"
MAIN_OBJ = "/org/fedoraproject/FirewallD1"
MAIN_IFACE = "org.fedoraproject.FirewallD1"
ZONE_IFACE = "org.fedoraproject.FirewallD1.zone"
SYSTEM_BUS = True


def fail(code):
    return dbus.exceptions.DBusException(code, name="org.fedoraproject.FirewallD1.Exception")


def zone_of(mock, zone):
    if zone not in mock.zones:
        raise fail("INVALID_ZONE: " + zone)
    return mock.zones[zone]


def add_service(mock, zone, service):
    z = zone_of(mock, zone)
    if service not in mock.available:
        raise fail("INVALID_SERVICE: " + service)
    if service in z["Services"]:
        raise fail("ALREADY_ENABLED: " + service)
    z["Services"].append(service)
    return zone


def remove_service(mock, zone, service):
    z = zone_of(mock, zone)
    if service not in z["Services"]:
        raise fail("NOT_ENABLED: " + service)
    z["Services"].remove(service)
    return zone


def add_port(mock, zone, port, protocol):
    z = zone_of(mock, zone)
    if [port, protocol] in z["Ports"]:
        raise fail("ALREADY_ENABLED: " + port)
    z["Ports"].append([port, protocol])
    return zone


def remove_port(mock, zone, port, protocol):
    z = zone_of(mock, zone)
    if [port, protocol] not in z["Ports"]:
        raise fail("NOT_ENABLED: " + port)
    z["Ports"].remove([port, protocol])
    return zone


def load(mock, parameters):
    mock.default_zone = parameters.get("DefaultZone", "public")
    zones = parameters.get("Zones", {mock.default_zone: {}})
    mock.zones = {
        name: {"Services": list(z.get("Services", [])), "Ports": [list(p) for p in z.get("Ports", [])]}
        for name, z in zones.items()
    }
    mock.active = parameters.get("ActiveZones", [mock.default_zone])
    mock.available = parameters.get("Available", ["ssh", "mdns", "samba-client", "kdeconnect", "http"])
    mock.add_service = add_service
    mock.remove_service = remove_service
    mock.add_port = add_port
    mock.remove_port = remove_port
    mock.zone_of = zone_of

    mock.AddMethods(
        MAIN_IFACE,
        [
            ("getDefaultZone", "", "s", "ret = self.default_zone"),
            ("listServices", "", "as", "ret = list(self.available)"),
            ("runtimeToPermanent", "", "", "pass"),
        ],
    )
    mock.AddMethods(
        ZONE_IFACE,
        [
            (
                "getActiveZones",
                "",
                "a{sa{sas}}",
                "ret = {z: {'interfaces': ['eth0'], 'sources': []} for z in self.active}",
            ),
            ("getServices", "s", "as", "ret = list(self.zone_of(self, args[0])['Services'])"),
            ("getPorts", "s", "aas", "ret = [list(p) for p in self.zone_of(self, args[0])['Ports']]"),
            ("addService", "ssi", "s", "ret = self.add_service(self, args[0], args[1])"),
            ("removeService", "ss", "s", "ret = self.remove_service(self, args[0], args[1])"),
            ("addPort", "sssi", "s", "ret = self.add_port(self, args[0], args[1], args[2])"),
            ("removePort", "sss", "s", "ret = self.remove_port(self, args[0], args[1], args[2])"),
        ],
    )
