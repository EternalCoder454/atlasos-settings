pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Power & Battery: the power mode (power-profiles-daemon), the battery
// (UPower; no rows on a desktop without one), when the screen turns off and
// the computer sleeps (PowerDevil's powerdevilrc, as its settings page
// writes it), and under Advanced the lid, the power button and the charge
// limit.
SettingsPage {
    id: page

    // src/power_page.rs and cpp/powerconfig.h; they go with the page.
    readonly property var sys: pageBackends ? pageBackends.create("power", page) : null
    readonly property var config: pageBackends ? pageBackends.create("power-config", page) : null
    // powerdevilrc as read for each profile: {screenOff, sleep, lid, powerButton}.
    property var ac: config ? config.read("AC") : ({})
    property var battery: config ? config.read("Battery") : ({})
    // A write to powerdevilrc failed.
    property string notice: ""

    readonly property bool hasBattery: sys !== null && sys.hasBattery

    readonly property var modeNames: ({
            "power-saver": qsTr("Power Saver"),
            "balanced": qsTr("Balanced"),
            "performance": qsTr("Performance")
        })
    readonly property var modeHints: ({
            "power-saver": qsTr("Quieter and cooler, and lasts longer on battery"),
            "balanced": qsTr("A good mix of speed and power use"),
            "performance": qsTr("As fast as it goes, and uses more power")
        })

    // What a lid close or power button press can do, as PowerDevil numbers.
    readonly property var lidActions: [
        {
            action: 1,
            text: qsTr("Sleep")
        },
        {
            action: 2,
            text: qsTr("Hibernate")
        },
        {
            action: 0,
            text: qsTr("Do Nothing")
        },
        {
            action: 8,
            text: qsTr("Shut Down")
        },
        {
            action: 32,
            text: qsTr("Lock the Screen")
        },
        {
            action: 64,
            text: qsTr("Turn Off the Screen")
        }
    ]
    readonly property var buttonActions: [
        {
            action: 16,
            text: qsTr("Ask What to Do")
        },
        {
            action: 1,
            text: qsTr("Sleep")
        },
        {
            action: 8,
            text: qsTr("Shut Down")
        },
        {
            action: 64,
            text: qsTr("Turn Off the Screen")
        },
        {
            action: 0,
            text: qsTr("Do Nothing")
        }
    ]

    function actionIndex(list: var, action: int): int {
        const i = list.findIndex(a => a.action === action);
        return i < 0 ? 0 : i;
    }

    function reload() {
        page.ac = page.config.read("AC");
        page.battery = page.config.read("Battery");
    }

    // After PowerDevil's file was written: say so when it wasn't, and have
    // PowerDevil read it again.
    function wrote(ok: bool) {
        if (!ok)
            page.notice = qsTr("Settings couldn't save the power settings.");
        else
            page.sys.reconfigure();
        page.reload();
    }

    // Both profiles on a laptop, the plugged-in one on a desktop.
    function profiles(): var {
        return page.hasBattery ? ["AC", "Battery"] : ["AC"];
    }

    function timeoutWords(seconds: int): string {
        if (seconds <= 0)
            return qsTr("Never");
        const minutes = Math.round(seconds / 60);
        if (minutes < 60)
            return minutes === 1 ? qsTr("After 1 minute") : qsTr("After %1 minutes").arg(minutes);
        const hours = minutes / 60;
        if (Number.isInteger(hours))
            return hours === 1 ? qsTr("After 1 hour") : qsTr("After %1 hours").arg(hours);
        return qsTr("After %1 minutes").arg(minutes);
    }

    function batteryStatus(): string {
        if (!page.sys)
            return "";
        const left = page.sys.timeLeft;
        switch (page.sys.chargeState) {
        case "charging":
            return left !== "" ? qsTr("Charging · %1 until full").arg(left) : qsTr("Charging");
        case "discharging":
            return left !== "" ? qsTr("On battery · %1 left").arg(left) : qsTr("On battery");
        case "full":
            return qsTr("Fully charged");
        case "holding":
            return qsTr("Plugged in, not charging");
        case "empty":
            return qsTr("Empty");
        default:
            return "";
        }
    }

    Component.onCompleted: if (sys) sys.refresh()

    // A laptop's charge changes while the page is open.
    Timer {
        interval: 30000
        repeat: true
        running: page.hasBattery && Application.state === Qt.ApplicationActive
        onTriggered: page.sys.refresh()
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.sys ? page.sys.error : ""
        shown: text !== ""
    }
    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.notice
        shown: text !== ""
        closable: true
        onClosed: page.notice = ""
    }

    Section {
        Layout.fillWidth: true
        footer: page.sys && page.sys.degraded !== "" ? page.sys.degraded : ""

        SectionRow {
            objectName: "profile"
            title: qsTr("Power Mode")
            subtitle: {
                if (!page.sys || !page.sys.loaded)
                    return "";
                if (!page.sys.hasProfiles)
                    return qsTr("Power modes aren't available on this computer");
                return page.modeHints[page.sys.profile] ?? "";
            }
            busy: page.sys !== null && page.sys.busy
            enabled: page.sys !== null && page.sys.hasProfiles

            AtlasSegmentedControl {
                readonly property var modes: page.sys ? page.sys.profiles : []
                visible: modes.length > 1
                model: modes.map(m => page.modeNames[m] ?? m)
                currentIndex: Math.max(0, modes.indexOf(page.sys ? page.sys.profile : ""))
                onActivated: index => page.sys.changeProfile(modes[index])
                Accessible.name: qsTr("Power mode")
            }
        }
    }

    Section {
        Layout.fillWidth: true
        visible: page.hasBattery

        SectionRow {
            objectName: "battery"
            title: qsTr("Battery")
            subtitle: page.batteryStatus()
            value: page.sys ? qsTr("%1%").arg(page.sys.percentage) : ""

            AtlasProgressBar {
                implicitWidth: Kirigami.Units.gridUnit * 6
                implicitHeight: Kirigami.Units.gridUnit * 0.5
                value: page.sys ? page.sys.percentage / 100 : 0
                status: page.sys && page.sys.chargeState === "discharging" && page.sys.percentage <= 15 ? "error" : "normal"
                animated: false
            }
        }
        SectionRow {
            objectName: "battery-health"
            visible: page.sys !== null && page.sys.health >= 0
            title: qsTr("Battery Health")
            subtitle: qsTr("How much of its original charge it still holds")
            value: page.sys ? qsTr("%1%").arg(page.sys.health) : ""
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Screen and Sleep")

        SectionRow {
            objectName: "screen-off"
            title: qsTr("Turn Off the Screen")
            subtitle: page.hasBattery ? qsTr("On battery: %1").arg(page.timeoutWords(page.battery.screenOff ?? 0)) : ""
            value: page.timeoutWords(page.ac.screenOff ?? 0)
            chevron: true
            enabled: page.config !== null
            onClicked: screenSheet.open()
        }
        SectionRow {
            objectName: "sleep"
            title: qsTr("Sleep")
            subtitle: page.hasBattery ? qsTr("On battery: %1").arg(page.timeoutWords(page.battery.sleep ?? 0)) : ""
            value: page.timeoutWords(page.ac.sleep ?? 0)
            chevron: true
            enabled: page.config !== null
            onClicked: sleepSheet.open()
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "lid"
            visible: page.sys !== null && page.sys.hasLid
            title: qsTr("When the Lid Is Closed")
            enabled: page.config !== null

            AtlasComboBox {
                width: Kirigami.Units.gridUnit * 12
                model: page.lidActions.map(a => a.text)
                currentIndex: page.actionIndex(page.lidActions, page.ac.lid ?? 1)
                onActivated: index => {
                    let ok = true;
                    for (const p of page.profiles())
                        ok = page.config.setLid(p, page.lidActions[index].action) && ok;
                    page.wrote(ok);
                }
                Accessible.name: qsTr("When the lid is closed")
            }
        }
        SectionRow {
            objectName: "power-button"
            title: qsTr("Power Button")
            subtitle: qsTr("What pressing it does")
            enabled: page.config !== null

            AtlasComboBox {
                width: Kirigami.Units.gridUnit * 12
                model: page.buttonActions.map(a => a.text)
                currentIndex: page.actionIndex(page.buttonActions, page.ac.powerButton ?? 16)
                onActivated: index => {
                    let ok = true;
                    for (const p of page.profiles())
                        ok = page.config.setPowerButton(p, page.buttonActions[index].action) && ok;
                    page.wrote(ok);
                }
                Accessible.name: qsTr("Power button")
            }
        }
        SectionRow {
            objectName: "charge-limit"
            visible: page.sys !== null && page.sys.limitSupported
            title: qsTr("Charge Limit")
            subtitle: qsTr("Stop charging at 80% to make the battery last longer")
            showSwitch: true
            switchChecked: page.sys ? page.sys.limitEnabled : false
            enabled: page.sys !== null && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeChargeLimit(checked)
        }
    }

    RelatedLinks {
        page: page
    }

    TimeoutSheet {
        id: screenSheet
        title: qsTr("Turn Off the Screen")
        description: qsTr("When you haven't used the computer for a while.")
        hasBattery: page.hasBattery
        acSeconds: page.ac.screenOff ?? 0
        batterySeconds: page.battery.screenOff ?? 0
        onPicked: (profile, seconds) => page.wrote(page.config.setScreenOff(profile, seconds))
    }

    TimeoutSheet {
        id: sleepSheet
        title: qsTr("Sleep")
        description: qsTr("When you haven't used the computer for a while. Sleep saves your session and uses very little power.")
        hasBattery: page.hasBattery
        acSeconds: page.ac.sleep ?? 0
        batterySeconds: page.battery.sleep ?? 0
        onPicked: (profile, seconds) => page.wrote(page.config.setSleep(profile, seconds))
    }
}
