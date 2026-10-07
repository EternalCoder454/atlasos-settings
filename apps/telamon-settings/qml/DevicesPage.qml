pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Bluetooth & Devices: Bluetooth on or off and the devices (paired ones,
// and the ones nearby while the page is open: BlueZ), Printers (Plasma's),
// and under Advanced whether other devices can find this computer, the
// tablet, touchscreen and game controller settings (Plasma's). A paired
// device opens in a sheet; one nearby is paired with a click.
SettingsPage {
    id: page

    // src/bluetooth.rs; it goes with the page.
    readonly property var sys: pageBackends ? pageBackends.create("bluetooth", page) : null

    // The window is on screen (not minimized or hidden): polling and
    // looking for devices stop otherwise.
    readonly property bool onScreen: Window.window !== null && Window.window.visible && Window.window.visibility !== Window.Minimized

    // [{address, name, kind, paired, connected, battery}] from the backend.
    readonly property var deviceList: sys ? JSON.parse(sys.devices || "[]") : []
    readonly property var printers: entry.items.find(i => i.id === "printers")

    // Looking for devices runs while the page is on screen and Bluetooth
    // is on; it stops with the page (the backend ends it).
    readonly property bool searching: sys !== null && sys.powered && page.onScreen
    onSearchingChanged: {
        if (!sys)
            return;
        if (searching)
            sys.startDiscovery();
        else
            sys.stopDiscovery();
    }

    // The device in the sheet, by address (the list changes under it).
    property string sheetAddress: ""
    property var sheetDevice: null
    // The sheet closes when the change it started has answered well.
    property bool pending: false

    function kindSymbol(kind: string): int {
        const names = {
            "headphones": "Headphones",
            "speaker": "Speaker",
            "keyboard": "Keyboard",
            "mouse": "Mouse",
            "gamepad": "Gamepad",
            "phone": "Mobile",
            "computer": "Computer",
            "watch": "Watch"
        };
        return Symbols.codepoint(names[kind] ?? "DevicesOther");
    }

    function deviceSubtitle(d: var): string {
        if (d.connected)
            return d.battery >= 0 ? qsTr("Connected · %1% battery").arg(d.battery) : qsTr("Connected");
        if (d.paired)
            return d.battery >= 0 ? qsTr("Paired · %1% battery").arg(d.battery) : qsTr("Paired");
        return qsTr("Ready to pair");
    }

    function openDevice(d: var) {
        if (d.paired) {
            page.sheetAddress = d.address;
            page.sheetDevice = d;
            deviceSheet.open();
        } else {
            page.sys.pair(d.address, d.name);
        }
    }

    readonly property string bluetoothSubtitle: {
        if (!page.sys || !page.sys.loaded)
            return "";
        if (!page.sys.available)
            return qsTr("Not available");
        if (!page.sys.adapter)
            return qsTr("No Bluetooth adapter found");
        if (!page.sys.powered)
            return qsTr("Off");
        return page.sys.connectedCount > 0 ? qsTr("%1 connected").arg(page.sys.connectedCount) : qsTr("On");
    }

    Component.onCompleted: {
        if (sys) {
            sys.refresh();
            if (searching)
                sys.startDiscovery();
        }
    }

    // Devices come and go while it looks.
    Timer {
        interval: 3000
        repeat: true
        running: page.sys !== null && page.sys.powered && page.onScreen
        onTriggered: page.sys.refresh()
    }

    Connections {
        target: page.sys
        function onBusyChanged() {
            if (!page.sys.busy && page.pending) {
                page.pending = false;
                if (page.sys.error === "")
                    deviceSheet.close();
            }
        }
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.sys ? page.sys.error : ""
        shown: text !== "" && !deviceSheet.visible
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "bluetooth"
            title: qsTr("Bluetooth")
            subtitle: page.bluetoothSubtitle
            showSwitch: true
            switchChecked: page.sys ? page.sys.powered : false
            enabled: page.sys !== null && page.sys.available && page.sys.adapter && !page.sys.busy
            onSwitchToggled: checked => page.sys.changePowered(checked)
            leading: Symbol {
                icon: Symbols.codepoint(page.sys && page.sys.powered ? "Bluetooth" : "BluetoothDisabled")
            }
        }
    }

    Section {
        id: devicesSection
        objectName: "devices"
        Layout.fillWidth: true
        visible: page.sys !== null && page.sys.powered
        title: qsTr("Devices")
        footer: qsTr("To add a device, put it in pairing mode, then choose it here.")

        Repeater {
            model: page.deviceList

            SectionRow {
                id: dev
                required property var modelData
                title: dev.modelData.name
                subtitle: page.deviceSubtitle(dev.modelData)
                clickable: true
                chevron: dev.modelData.paired
                busy: page.sys !== null && page.sys.working === dev.modelData.address
                onClicked: page.openDevice(dev.modelData)
                leading: Symbol {
                    icon: page.kindSymbol(dev.modelData.kind)
                }
            }
        }
        SectionRow {
            visible: page.deviceList.length === 0
            title: qsTr("Looking for Devices…")
            subtitle: qsTr("Nothing paired or nearby yet")
            busy: page.sys !== null && page.sys.discovering
        }
    }

    Section {
        Layout.fillWidth: true

        KcmRow {
            objectName: "printers"
            title: page.printers ? page.printers.title : qsTr("Printers")
            kcm: page.printers ? page.printers.kcm : "kcm_printer_manager"
            onOpenKcm: name => page.openKcm(name)
            leading: Symbol {
                icon: Symbols.Print
            }
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "visibility"
            title: qsTr("Visible to Other Devices")
            subtitle: page.sys && page.sys.adapterName !== "" ? qsTr("Other devices can find this computer as %1").arg(page.sys.adapterName) : qsTr("Other devices can find this computer")
            showSwitch: true
            switchChecked: page.sys ? page.sys.discoverable : false
            enabled: page.sys !== null && page.sys.powered && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeDiscoverable(checked)
        }
    }

    RelatedLinks {
        page: page
    }

    // A paired device: connect, disconnect or remove it.
    TelamonDialog {
        id: deviceSheet
        title: page.sheetDevice ? page.sheetDevice.name : ""
        preferredWidth: Kirigami.Units.gridUnit * 24

        // The list's newest entry for the device, else the one opened.
        readonly property var device: page.deviceList.find(d => d.address === page.sheetAddress) ?? page.sheetDevice
        readonly property bool busy: page.sys !== null && page.sys.busy

        onDeviceChanged: if (device !== null && visible)
            page.sheetDevice = device
        onClosed: page.pending = false
        footerContent: [
            SecondaryButton {
                text: qsTr("Forget")
                enabled: !deviceSheet.busy
                onClicked: {
                    page.pending = true;
                    page.sys.forget(deviceSheet.device.address);
                }
            },
            PrimaryButton {
                text: deviceSheet.device !== null && deviceSheet.device.connected ? qsTr("Disconnect") : qsTr("Connect")
                enabled: !deviceSheet.busy
                onClicked: {
                    page.pending = true;
                    if (deviceSheet.device.connected)
                        page.sys.disconnectDevice(deviceSheet.device.address);
                    else
                        page.sys.connectDevice(deviceSheet.device.address, deviceSheet.device.name);
                }
            }
        ]

        InfoBanner {
            Layout.fillWidth: true
            type: "warning"
            text: page.sys ? page.sys.error : ""
            shown: text !== "" && deviceSheet.visible
        }
        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Status")
                value: deviceSheet.device === null ? "" : deviceSheet.device.connected ? qsTr("Connected") : qsTr("Not connected")
                busy: deviceSheet.busy
            }
            SectionRow {
                visible: deviceSheet.device !== null && deviceSheet.device.battery >= 0
                title: qsTr("Battery")
                value: deviceSheet.device !== null && deviceSheet.device.battery >= 0 ? qsTr("%1%").arg(deviceSheet.device.battery) : ""
            }
            SectionRow {
                title: qsTr("Address")
                value: deviceSheet.device === null ? "" : deviceSheet.device.address
            }
        }
    }
}
