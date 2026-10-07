pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// System: the device's name (hostnamed) and what
// the device is. Other Plasma Settings is a link under Advanced.
SettingsPage {
    id: page

    // src/system_info.rs; it goes with the page.
    readonly property var sys: pageBackends ? pageBackends.create("system", page) : null

    function known(text: string): string {
        if (!page.sys || !page.sys.loaded)
            return "";
        return text !== "" ? text : qsTr("Unknown");
    }

    Component.onCompleted: if (sys) sys.refresh()

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.sys ? page.sys.error : ""
        shown: text !== ""
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "device-name"
            title: qsTr("Device Name")
            subtitle: page.sys && page.sys.hostname !== "" && page.sys.hostname !== page.sys.deviceName ? qsTr("On the network: %1").arg(page.sys.hostname) : ""
            value: page.known(page.sys ? page.sys.deviceName : "")
            chevron: true
            busy: page.sys !== null && page.sys.busy
            enabled: page.sys !== null && page.sys.loaded
            onClicked: renameSheet.open()
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("About This Device")

        SectionRow {
            objectName: "version"
            title: qsTr("Telamon OS Version")
            value: page.known(page.sys ? page.sys.os : "")
            subtitle: page.sys && page.sys.osBuild !== "" ? qsTr("Build %1").arg(page.sys.osBuild) : ""
        }
        SectionRow {
            objectName: "hardware"
            title: qsTr("Processor")
            value: page.known(page.sys ? page.sys.cpu : "")
        }
        SectionRow {
            title: qsTr("Memory")
            value: page.known(page.sys && page.sys.memory > 0 ? TelamonFormat.bytes(page.sys.memory, 1) : "")
        }
        SectionRow {
            title: qsTr("Graphics")
            value: page.known(page.sys ? page.sys.graphics : "")
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "other"
            title: qsTr("Other Plasma Settings")
            subtitle: qsTr("Settings that have no page here yet")
            chevron: true
            onClicked: page.openPage("more", "")
        }
    }

    RelatedLinks {
        page: page
    }

    TelamonDialog {
        id: renameSheet
        title: qsTr("Device Name")
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            nameField.text = page.sys ? page.sys.deviceName : "";
            nameField.selectAll();
            nameField.forceActiveFocus();
        }
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: renameSheet.close()
            },
            PrimaryButton {
                text: qsTr("Rename")
                enabled: page.sys !== null && page.sys.validName(nameField.text) && nameField.text.trim() !== page.sys.deviceName
                onClicked: {
                    page.sys.renameDevice(nameField.text.trim());
                    renameSheet.close();
                }
            }
        ]

        TelamonLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Other devices see this name on the network and over Bluetooth.")
        }
        TelamonTextField {
            id: nameField
            Layout.fillWidth: true
            maximumLength: 64
            placeholderText: qsTr("Name")
            onAccepted: if (page.sys && page.sys.validName(text)) {
                page.sys.renameDevice(text.trim());
                renameSheet.close();
            }
        }
    }
}
