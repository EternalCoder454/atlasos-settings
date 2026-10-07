"""fprintd mock template, for the settings-sys tests (python-dbusmock has
none). Parameters: NoDevice (no reader), Enrolled (fingers already
enrolled), Stages (scans an enrolment takes, default 3), Script (the
EnrollStatus results an enrolment sends, in order, as [result, done]
pairs; default: Stages passes then a completion).
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "net.reactivated.Fprint"
MAIN_OBJ = "/net/reactivated/Fprint/Manager"
MAIN_IFACE = "net.reactivated.Fprint.Manager"
DEVICE_OBJ = "/net/reactivated/Fprint/Device/0"
DEVICE_IFACE = "net.reactivated.Fprint.Device"
SYSTEM_BUS = True


def error(name, text):
    return dbus.exceptions.DBusException(text, name=f"net.reactivated.Fprint.Error.{name}")


def enroll_start(mock, device, finger):
    if mock.claimed is None:
        raise error("ClaimDevice", "claim the device first")
    for result, done in mock.script:
        device.EmitSignal(DEVICE_IFACE, "EnrollStatus", "sb", [result, done])
        if result == "enroll-completed":
            if finger not in mock.enrolled:
                mock.enrolled.append(finger)


def load(mock, parameters):
    mock.claimed = None
    mock.enrolled = list(parameters.get("Enrolled", []))
    stages = parameters.get("Stages", 3)
    default = [["enroll-stage-passed", False]] * stages + [["enroll-completed", True]]
    mock.script = parameters.get("Script", default)
    mock.enroll_start = enroll_start

    if parameters.get("NoDevice", False):
        mock.AddMethods(
            MAIN_IFACE,
            [
                ("GetDefaultDevice", "", "o", "raise dbus.exceptions.DBusException('no device', name='net.reactivated.Fprint.Error.NoSuchDevice')"),
                ("GetDevices", "", "ao", "ret = []"),
            ],
        )
        return

    mock.AddMethods(
        MAIN_IFACE,
        [
            ("GetDefaultDevice", "", "o", f"ret = dbus.ObjectPath('{DEVICE_OBJ}')"),
            ("GetDevices", "", "ao", f"ret = [dbus.ObjectPath('{DEVICE_OBJ}')]"),
        ],
    )
    mock.AddObject(
        DEVICE_OBJ,
        DEVICE_IFACE,
        dbus.Dictionary(
            {
                "name": "Mock Fingerprint Reader",
                "num-enroll-stages": dbus.Int32(stages),
                "scan-type": parameters.get("ScanType", "press"),
            },
            signature="sv",
        ),
        [
            (
                "ListEnrolledFingers",
                "s",
                "as",
                "m = objects['" + MAIN_OBJ + "']\n"
                "if not m.enrolled:\n"
                "    raise dbus.exceptions.DBusException('none', name='net.reactivated.Fprint.Error.NoEnrolledPrints')\n"
                "ret = list(m.enrolled)",
            ),
            ("Claim", "s", "", "objects['" + MAIN_OBJ + "'].claimed = args[0]"),
            ("Release", "", "", "objects['" + MAIN_OBJ + "'].claimed = None"),
            ("EnrollStart", "s", "", "m = objects['" + MAIN_OBJ + "']\nm.enroll_start(m, self, args[0])"),
            ("EnrollStop", "", "", "pass"),
            (
                "DeleteEnrolledFingers2",
                "",
                "",
                "m = objects['" + MAIN_OBJ + "']\n"
                "if m.claimed is None:\n"
                "    raise dbus.exceptions.DBusException('claim first', name='net.reactivated.Fprint.Error.ClaimDevice')\n"
                "m.enrolled = []",
            ),
        ],
    )
