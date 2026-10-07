pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// A sheet to choose how long until something happens (the screen goes off,
// the computer sleeps), plugged in and, on a laptop, on battery. Each choice
// applies at once: `picked(profile, seconds)`, profile "AC" or "Battery".
AtlasDialog {
    id: sheet

    // What each profile does now, in seconds (0 for never).
    property int acSeconds: 0
    property int batterySeconds: 0
    property bool hasBattery: false
    property string description: ""

    signal picked(string profile, int seconds)

    // Seconds, in picker order; a value the file has that isn't here is
    // added, so it is shown and not lost.
    function secondsChoices(current: int): var {
        const list = [0, 60, 120, 180, 300, 600, 900, 1800, 3600, 7200];
        if (!list.includes(current))
            list.push(current);
        return list.sort((a, b) => a - b);
    }

    function words(seconds: int): string {
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

    preferredWidth: Kirigami.Units.gridUnit * 26
    footerContent: [
        PrimaryButton {
            text: qsTr("Done")
            onClicked: sheet.close()
        }
    ]

    AtlasLabel {
        Layout.fillWidth: true
        visible: sheet.description !== ""
        wrapMode: Text.Wrap
        text: sheet.description
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            title: sheet.hasBattery ? qsTr("Plugged In") : qsTr("When Idle")

            AtlasComboBox {
                id: acBox
                readonly property var values: sheet.secondsChoices(sheet.acSeconds)
                width: Kirigami.Units.gridUnit * 11
                model: acBox.values.map(s => sheet.words(s))
                currentIndex: Math.max(0, acBox.values.indexOf(sheet.acSeconds))
                onActivated: index => sheet.picked("AC", acBox.values[index])
                Accessible.name: sheet.hasBattery ? qsTr("Plugged in") : qsTr("When idle")
            }
        }
        SectionRow {
            visible: sheet.hasBattery
            title: qsTr("On Battery")

            AtlasComboBox {
                id: batteryBox
                readonly property var values: sheet.secondsChoices(sheet.batterySeconds)
                width: Kirigami.Units.gridUnit * 11
                model: batteryBox.values.map(s => sheet.words(s))
                currentIndex: Math.max(0, batteryBox.values.indexOf(sheet.batterySeconds))
                onActivated: index => sheet.picked("Battery", batteryBox.values[index])
                Accessible.name: qsTr("On battery")
            }
        }
    }
}
