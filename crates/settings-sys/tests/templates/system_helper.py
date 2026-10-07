"""Telamon Updater's system helper (`telamon-system-helper`) mock template for
the smoke runs of the Updates page (scripts/smoke-mock.sh updates): the
system bus name, the six methods and the Progress property of the helper's
D-Bus API (the Updater repo's docs/DESIGN.md), with `bootc status --json`
documents as the answers.

Parameters:
  Scenario  "uptodate" (default), "available" (an update is offered) or
            "staged" (an update is downloaded and waits for a restart)
  Status    the JSON text of the status the helper reports (instead of a
            Scenario's)
  Staged    the JSON text Upgrade answers with (default: Status)
  Hold      seconds Upgrade takes, while Progress shows `installing`
            (default 0: it answers at once)
  Name, Path, Interface   the identity (default: the new one)
"""

# SPDX-License-Identifier: MIT

import json

BUS_NAME = "net.eterneon.telamon.SystemHelper"
MAIN_OBJ = "/net/eterneon/telamon/SystemHelper"
MAIN_IFACE = "net.eterneon.telamon.SystemHelper1"
SYSTEM_BUS = True

IMAGE = {"image": "ghcr.io/eternalcoder454/atlasos:stable", "transport": "registry"}


def entry(version, digest, timestamp, cached=None):
    return {
        "image": {
            "image": IMAGE,
            "architecture": "amd64",
            "version": version,
            "timestamp": timestamp,
            "imageDigest": digest,
        },
        "cachedUpdate": cached,
        "incompatible": False,
        "pinned": False,
        "store": "ostreeContainer",
        "ostree": {"checksum": "a" * 64, "deploySerial": 0, "stateroot": "default"},
    }


def status(booted, staged=None, rollback=None):
    return json.dumps(
        {
            "apiVersion": "org.containers.bootc/v1",
            "kind": "BootcHost",
            "metadata": {"name": "host"},
            "spec": {"image": IMAGE, "bootOrder": "default"},
            "status": {"staged": staged, "booted": booted, "rollback": rollback},
        }
    )


def scenario_status(name):
    new = entry("44.20261002", "sha256:" + "2" * 64, "2026-10-02T04:10:41Z")["image"]
    old = entry("44.20260924", "sha256:" + "0" * 64, "2026-09-24T04:09:30Z")
    if name == "available":
        return status(entry("44.20261001", "sha256:" + "1" * 64, "2026-10-01T04:12:09Z", new), None, old)
    if name == "staged":
        return status(
            entry("44.20261001", "sha256:" + "1" * 64, "2026-10-01T04:12:09Z"),
            entry("44.20261002", "sha256:" + "2" * 64, "2026-10-02T04:10:41Z"),
            old,
        )
    return status(entry("44.20261001", "sha256:" + "1" * 64, "2026-10-01T04:12:09Z"), None, old)


def load(mock, parameters):
    mock.status_text = parameters.get("Status") or scenario_status(parameters.get("Scenario", "uptodate"))
    # What Upgrade leaves: the update staged.
    mock.staged_text = parameters.get("Staged") or scenario_status("staged")
    mock.hold = float(parameters.get("Hold", 0))
    mock.iface = MAIN_IFACE
    mock.AddProperty(MAIN_IFACE, "Progress", "")
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("Status", "", "s", "ret = self.status_text"),
            ("CheckForUpdate", "", "s", "ret = self.status_text"),
            (
                "Upgrade",
                "",
                "s",
                """
import json, time
if self.hold > 0:
    self.UpdateProperties(self.iface, {'Progress': json.dumps({'op': 'upgrade', 'stage': 'installing', 'done': 1, 'total': 4, 'detail': 'Importing Image'})})
    time.sleep(self.hold)
    self.UpdateProperties(self.iface, {'Progress': ''})
self.status_text = self.staged_text
ret = self.status_text
""",
            ),
            ("Rollback", "", "s", "ret = self.status_text"),
            ("CancelRollback", "", "s", "ret = self.status_text"),
            ("SwitchChannel", "s", "s", "ret = self.status_text"),
        ],
    )
