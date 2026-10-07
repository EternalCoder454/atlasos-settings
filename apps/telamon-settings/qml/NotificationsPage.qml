pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Notifications: Do Not Disturb, and which apps may show banners; under
// Advanced where popups appear and how long they stay, and the lock screen.
// All of it is Plasma's plasmanotifyrc, which its notification server
// watches (cpp/notificationsconfig.h).
SettingsPage {
    id: page

    // cpp/notificationsconfig.h; it goes with the page.
    readonly property var cfg: pageBackends ? pageBackends.create("notifications", page) : null
    // {active, until, forever}
    property var dnd: cfg ? cfg.doNotDisturb() : ({})
    property var apps: cfg ? cfg.apps() : []
    property var popups: cfg ? cfg.popups() : ({})
    // How long Do Not Disturb lasts when it is turned on, as a choice of
    // `durations`.
    property int duration: 4
    property string notice: ""

    readonly property var durations: ["30m", "1h", "4h", "tomorrow", "forever"]
    readonly property var durationNames: [qsTr("For 30 minutes"), qsTr("For 1 hour"), qsTr("For 4 hours"), qsTr("Until tomorrow morning"), qsTr("Until I turn it off")]
    readonly property var positionNames: [qsTr("Near the Tray"), qsTr("Top Left"), qsTr("Top Center"), qsTr("Top Right"), qsTr("Bottom Left"), qsTr("Bottom Center"), qsTr("Bottom Right")]
    readonly property var timeouts: [3, 5, 10, 30, 0]
    readonly property var timeoutNames: [qsTr("3 seconds"), qsTr("5 seconds"), qsTr("10 seconds"), qsTr("30 seconds"), qsTr("Never, until dismissed")]

    function dndSubtitle(): string {
        if (!page.dnd.active)
            return qsTr("Banners and sounds stay quiet while it is on");
        if (page.dnd.forever)
            return qsTr("On until you turn it off");
        const until = new Date(page.dnd.until * 1000);
        const today = new Date();
        const time = until.toLocaleTimeString(Qt.locale(), Locale.ShortFormat);
        return until.toDateString() === today.toDateString() ? qsTr("On until %1").arg(time) : qsTr("On until %1 %2").arg(until.toLocaleDateString(Qt.locale(), Locale.ShortFormat)).arg(time);
    }

    function appsSummary(): string {
        const off = page.apps.filter(a => !a.allowed).length;
        return page.apps.length === 0 ? "" : (off === 0 ? qsTr("%1 apps").arg(page.apps.length) : qsTr("%1 apps, %2 off").arg(page.apps.length).arg(off));
    }

    function popupsSummary(): string {
        const timeout = page.popups.timeout ?? 5;
        const names = page.positionNames[page.popups.position ?? 0];
        return timeout === 0 ? qsTr("%1, until dismissed").arg(names) : qsTr("%1, %2 s").arg(names).arg(timeout);
    }

    // Writes, then reads the file again so the page shows what is saved.
    function saved(ok: bool) {
        if (!ok)
            page.notice = qsTr("Settings couldn't save the notification settings.");
        page.dnd = page.cfg.doNotDisturb();
        page.apps = page.cfg.apps();
        page.popups = page.cfg.popups();
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.notice
        shown: text !== ""
        closable: true
        onClosed: page.notice = ""
    }

    // Do Not Disturb ends by itself: read the file once more when it does.
    Timer {
        running: page.dnd.active ?? false
        interval: Math.min(2147483000, Math.max(1000, (page.dnd.until ?? 0) * 1000 - Date.now() + 500))
        onTriggered: page.dnd = page.cfg.doNotDisturb()
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "dnd"
            title: qsTr("Do Not Disturb")
            subtitle: page.dndSubtitle()
            showSwitch: true
            switchChecked: page.dnd.active ?? false
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setDoNotDisturb(checked ? page.durations[page.duration] : "off"))
        }
        SectionRow {
            title: qsTr("Duration")
            subtitle: qsTr("How long it stays on once you turn it on")

            TelamonComboBox {
                width: Kirigami.Units.gridUnit * 13
                model: page.durationNames
                currentIndex: page.duration
                onActivated: index => {
                    page.duration = index;
                    // While it is on, the choice moves its end.
                    if (page.dnd.active)
                        page.saved(page.cfg.setDoNotDisturb(page.durations[index]));
                }
                Accessible.name: qsTr("How long Do Not Disturb lasts")
            }
        }
        SectionRow {
            objectName: "apps"
            title: qsTr("App Notifications")
            subtitle: qsTr("Choose which apps may show notifications")
            value: page.appsSummary()
            chevron: true
            enabled: page.cfg !== null
            onClicked: {
                page.apps = page.cfg.apps();
                appSheet.open();
            }
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "popups"
            title: qsTr("Popups")
            subtitle: qsTr("Where banners appear and how long they stay")
            value: page.popupsSummary()
            chevron: true
            enabled: page.cfg !== null
            onClicked: popupSheet.open()
        }
        KcmRow {
            objectName: "lock-screen"
            title: qsTr("On the Lock Screen")
            kcm: "kcm_screenlocker"
            onOpenKcm: name => page.openKcm(name)
        }
    }

    RelatedLinks {
        page: page
    }

    // Each app that has sent a notification: whether it may show any, and
    // whether as banners.
    TelamonDialog {
        id: appSheet
        title: qsTr("App Notifications")
        preferredWidth: Kirigami.Units.gridUnit * 30
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: appSheet.close()
            }
        ]

        TelamonLabel {
            Layout.fillWidth: true
            visible: page.apps.length === 0
            wrapMode: Text.Wrap
            textStyle: TelamonLabel.Caption
            text: qsTr("No app has sent a notification yet. Apps appear here after their first one.")
        }
        Section {
            Layout.fillWidth: true
            visible: page.apps.length > 0

            Repeater {
                model: page.apps

                SectionRow {
                    id: appRow
                    required property var modelData
                    title: modelData.name
                    iconName: modelData.icon
                    subtitle: !modelData.allowed ? qsTr("Off") : (modelData.banners ? qsTr("Banners and history") : qsTr("History only"))
                    showSwitch: true
                    switchChecked: modelData.allowed
                    onSwitchToggled: checked => page.saved(page.cfg.setAppAllowed(modelData.id, checked))

                    TelamonCheckBox {
                        text: qsTr("Banners")
                        enabled: appRow.modelData.allowed
                        checked: appRow.modelData.banners
                        onToggled: page.saved(page.cfg.setAppBanners(appRow.modelData.id, checked))
                    }
                }
            }
        }
    }

    // Where banners appear and how long they stay.
    TelamonDialog {
        id: popupSheet
        title: qsTr("Popups")
        preferredWidth: Kirigami.Units.gridUnit * 28
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: popupSheet.close()
            }
        ]

        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Position")

                TelamonComboBox {
                    width: Kirigami.Units.gridUnit * 12
                    model: page.positionNames
                    currentIndex: page.popups.position ?? 0
                    onActivated: index => page.saved(page.cfg.setPopupPosition(index))
                    Accessible.name: qsTr("Popup position")
                }
            }
            SectionRow {
                title: qsTr("Hide After")

                TelamonComboBox {
                    width: Kirigami.Units.gridUnit * 12
                    model: page.timeoutNames
                    currentIndex: Math.max(0, page.timeouts.indexOf(page.popups.timeout ?? 5))
                    onActivated: index => page.saved(page.cfg.setPopupTimeout(page.timeouts[index]))
                    Accessible.name: qsTr("Hide popups after")
                }
            }
        }
    }
}
