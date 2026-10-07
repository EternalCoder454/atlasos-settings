pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Home: the device's name, switches for Wi-Fi, Bluetooth and Light or Dark,
// and links to the pages people use most. Short, and no promotions. The
// switches are the Network and Bluetooth pages' backends and the Plasma
// colour scheme (docs/DESIGN.md, "Network, Bluetooth & Devices and Home").
SettingsPage {
    id: page

    // The AtlasOS schemes (the image's /usr/share/color-schemes); Light or
    // Dark switches between them, as the Appearance page does.
    readonly property string lightScheme: "AtlasOSLight"
    readonly property string darkScheme: "AtlasOSDark"

    // src/system_info.rs, src/network.rs, src/bluetooth.rs and
    // cpp/colorscheme.h; they go with the page.
    readonly property var device: pageBackends ? pageBackends.create("system", page) : null
    readonly property var net: pageBackends ? pageBackends.create("network", page) : null
    readonly property var bluetooth: pageBackends ? pageBackends.create("bluetooth", page) : null
    readonly property var colors: pageBackends ? pageBackends.create("color-scheme", page) : null

    // The window is on screen (not minimized or hidden): polling and
    // looking for devices stop otherwise.
    readonly property bool onScreen: Window.window !== null && Window.window.visible && Window.window.visibility !== Window.Minimized

    // Whether the colour scheme in use is a dark one: read from kdeglobals,
    // and set at once when the switch is used (Plasma writes the file a
    // moment after the program runs).
    property bool dark: colors ? isDark(colors.current()) : false

    function isDark(scheme: string): bool {
        return /dark/i.test(scheme);
    }

    function wifiSubtitle(): string {
        if (!net || !net.loaded)
            return "";
        if (!net.wifiPresent)
            return qsTr("No Wi-Fi adapter found");
        if (net.wifiBlocked)
            return qsTr("Turned off by a switch on this computer");
        if (!net.wifiEnabled)
            return qsTr("Off");
        if (net.wifiConnecting)
            return qsTr("Connecting to %1…").arg(net.wifiSsid);
        if (net.wifiSsid !== "")
            return qsTr("Connected to %1").arg(net.wifiSsid);
        return net.wired === "connected" ? qsTr("On, using a cable") : qsTr("On, not connected");
    }

    function bluetoothSubtitle(): string {
        if (!bluetooth || !bluetooth.loaded)
            return "";
        if (!bluetooth.available)
            return qsTr("Not available");
        if (!bluetooth.adapter)
            return qsTr("No Bluetooth adapter found");
        if (!bluetooth.powered)
            return qsTr("Off");
        return bluetooth.connectedCount > 0 ? qsTr("%1 connected").arg(bluetooth.connectedCount) : qsTr("On");
    }

    Component.onCompleted: {
        if (device)
            device.refresh();
        if (net)
            net.refreshStatus();
        if (bluetooth)
            bluetooth.refresh();
    }

    // Once the switch has been used, what Plasma wrote.
    Timer {
        id: reread
        interval: 1500
        onTriggered: page.dark = page.isDark(page.colors.current())
    }
    // Wi-Fi and Bluetooth change on their own now and then.
    Timer {
        interval: 8000
        repeat: true
        running: page.onScreen
        onTriggered: {
            if (page.net)
                page.net.refreshStatus();
            if (page.bluetooth)
                page.bluetooth.refresh();
        }
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.net && page.net.error !== "" ? page.net.error : (page.bluetooth ? page.bluetooth.error : "")
        shown: text !== ""
    }

    // The device, big.
    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        Layout.leftMargin: AtlasStyle.spacingLarge
        spacing: AtlasStyle.spacingLarge

        Symbol {
            icon: Symbols.Computer
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: Kirigami.Units.iconSizes.large
            Layout.preferredHeight: Kirigami.Units.iconSizes.large
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            QQC2.Label {
                Layout.fillWidth: true
                text: page.device && page.device.deviceName !== "" ? page.device.deviceName : qsTr("This Computer")
                font.bold: true
                font.pointSize: AtlasStyle.fontSizeHeading * 1.25
                elide: Text.ElideRight
                textFormat: Text.PlainText
                Accessible.role: Accessible.Heading
            }
            QQC2.Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: page.device ? page.device.os : ""
                opacity: 0.65
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Quick Settings")

        SectionRow {
            objectName: "wifi"
            title: qsTr("Wi-Fi")
            subtitle: page.wifiSubtitle()
            showSwitch: true
            switchChecked: page.net ? page.net.wifiEnabled : false
            enabled: page.net !== null && page.net.wifiPresent && !page.net.wifiBlocked && !page.net.busy
            onSwitchToggled: checked => page.net.changeWifi(checked)
            leading: Symbol {
                icon: Symbols.codepoint(page.net && page.net.wifiEnabled ? "Wifi" : "WifiOff")
            }
        }
        SectionRow {
            objectName: "bluetooth"
            title: qsTr("Bluetooth")
            subtitle: page.bluetoothSubtitle()
            showSwitch: true
            switchChecked: page.bluetooth ? page.bluetooth.powered : false
            enabled: page.bluetooth !== null && page.bluetooth.available && page.bluetooth.adapter && !page.bluetooth.busy
            onSwitchToggled: checked => page.bluetooth.changePowered(checked)
            leading: Symbol {
                icon: Symbols.codepoint(page.bluetooth && page.bluetooth.powered ? "Bluetooth" : "BluetoothDisabled")
            }
        }
        SectionRow {
            objectName: "dark-mode"
            title: qsTr("Dark Mode")
            subtitle: qsTr("Colors for windows and apps")
            showSwitch: true
            switchChecked: page.dark
            enabled: page.colors !== null
            onSwitchToggled: checked => {
                page.dark = checked;
                page.run(["plasma-apply-colorscheme", checked ? page.darkScheme : page.lightScheme]);
                reread.restart();
            }
            leading: Symbol {
                icon: Symbols.codepoint(page.dark ? "DarkMode" : "LightMode")
            }
        }
    }

    RelatedLinks {
        page: page
        title: qsTr("Settings")
    }
}
