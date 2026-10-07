pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Network: Wi-Fi and the networks in range, wired status, VPN, the hotspot
// and airplane mode (NetworkManager), and under Advanced the proxy and
// mobile broadband (Plasma's). A network opens in a sheet: join it with its
// password, leave it, or forget it.
SettingsPage {
    id: page

    // src/network.rs; it goes with the page.
    readonly property var sys: pageBackends ? pageBackends.create("network", page) : null

    // The window is on screen (not minimized or hidden): polling and
    // looking for devices stop otherwise.
    readonly property bool onScreen: Window.window !== null && Window.window.visible && Window.window.visibility !== Window.Minimized

    // From the backend's JSON: [{ssid, signal, security, connected,
    // connecting, known}] and [{id, uuid, active, connecting}].
    readonly property var wifiList: sys ? JSON.parse(sys.networks || "[]") : []
    readonly property var vpnList: sys ? JSON.parse(sys.vpns || "[]") : []
    readonly property int shownNetworks: 8
    property bool showAllNetworks: false
    readonly property var visibleNetworks: showAllNetworks ? wifiList : wifiList.slice(0, shownNetworks)

    // The network in the sheet, by name (the list changes under it).
    property string sheetSsid: ""
    property var sheetNetwork: null
    // The sheet closes when the change it started has answered well.
    property bool pending: false
    property int ticks: 0

    // The row's picture: the signal in bars, with a lock when secured.
    function wifiSymbol(n: var): int {
        const bars = n.signal >= 75 ? "" : n.signal >= 50 ? "3Bar" : n.signal >= 25 ? "2Bar" : "1Bar";
        return Symbols.codepoint("NetworkWifi" + bars + (n.security !== "open" ? "Locked" : ""));
    }

    function signalName(signal: int): string {
        return signal >= 75 ? qsTr("Strong") : signal >= 50 ? qsTr("Good") : signal >= 25 ? qsTr("Fair") : qsTr("Weak");
    }

    function securityName(security: string): string {
        switch (security) {
        case "personal":
            return qsTr("Password (WPA)");
        case "sae":
            return qsTr("Password (WPA3)");
        case "wep":
            return qsTr("Password (WEP, weak)");
        case "enterprise":
            return qsTr("Work or school login");
        default:
            return qsTr("None");
        }
    }

    function networkSubtitle(n: var): string {
        if (n.connecting)
            return qsTr("Connecting…");
        if (n.connected)
            return qsTr("Connected");
        if (n.known)
            return qsTr("Saved");
        if (n.security === "enterprise")
            return qsTr("Work or school login");
        return n.security === "open" ? qsTr("Open") : "";
    }

    function openNetwork(n: var) {
        page.sheetSsid = n.ssid;
        page.sheetNetwork = n;
        page.sys.refresh();
        networkSheet.open();
    }

    readonly property string wifiSubtitle: {
        if (!page.sys || !page.sys.loaded)
            return "";
        if (!page.sys.wifiPresent)
            return qsTr("No Wi-Fi adapter found");
        if (page.sys.wifiBlocked)
            return qsTr("Turned off by a switch on this computer");
        if (!page.sys.wifiEnabled)
            return qsTr("Off");
        if (page.sys.wifiConnecting)
            return qsTr("Connecting to %1…").arg(page.sys.wifiSsid);
        if (page.sys.wifiSsid !== "")
            return qsTr("Connected to %1").arg(page.sys.wifiSsid);
        return qsTr("Not connected");
    }

    readonly property string wiredSubtitle: {
        switch (page.sys ? page.sys.wired : "") {
        case "connected":
            return qsTr("Connected");
        case "connecting":
            return qsTr("Connecting…");
        case "unplugged":
            return qsTr("Cable unplugged");
        default:
            return qsTr("Not connected");
        }
    }

    Component.onCompleted: {
        if (sys) {
            sys.refresh();
            sys.scan();
        }
    }

    // The networks in range change under us; look again while the page is
    // on screen, and ask the adapter to scan now and then.
    Timer {
        interval: 6000
        repeat: true
        running: page.sys !== null && page.onScreen
        onTriggered: {
            page.sys.refresh();
            page.ticks++;
            if (page.ticks % 3 === 0)
                page.sys.scan();
        }
    }
    // The first scan's answer.
    Timer {
        interval: 2500
        running: page.sys !== null
        onTriggered: page.sys.refresh()
    }

    Connections {
        target: page.sys
        function onBusyChanged() {
            if (!page.sys.busy && page.pending) {
                page.pending = false;
                if (page.sys.error === "") {
                    networkSheet.close();
                    hotspotSheet.close();
                }
            }
        }
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.sys ? page.sys.error : ""
        shown: text !== "" && !networkSheet.visible && !hotspotSheet.visible
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "wifi"
            title: qsTr("Wi-Fi")
            subtitle: page.wifiSubtitle
            showSwitch: true
            switchChecked: page.sys ? page.sys.wifiEnabled : false
            enabled: page.sys !== null && page.sys.wifiPresent && !page.sys.wifiBlocked && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeWifi(checked)
            leading: Symbol {
                icon: Symbols.codepoint(page.sys && page.sys.wifiEnabled ? "Wifi" : "WifiOff")
            }
        }
        Repeater {
            model: page.sys && page.sys.wifiEnabled ? page.visibleNetworks : []

            SectionRow {
                id: net
                required property var modelData
                title: net.modelData.ssid
                subtitle: page.networkSubtitle(net.modelData)
                clickable: true
                checkmark: net.modelData.connected
                busy: net.modelData.connecting || (page.sys !== null && page.sys.busy && page.sys.joining === net.modelData.ssid)
                onClicked: page.openNetwork(net.modelData)
                leading: Symbol {
                    icon: page.wifiSymbol(net.modelData)
                }
            }
        }
        SectionRow {
            visible: page.sys !== null && page.sys.wifiEnabled && page.wifiList.length > page.shownNetworks
            title: page.showAllNetworks ? qsTr("Show Fewer Networks") : qsTr("Show All Networks (%1)").arg(page.wifiList.length)
            chevron: true
            disclosure: true
            expanded: page.showAllNetworks
            onClicked: page.showAllNetworks = !page.showAllNetworks
        }
        SectionRow {
            visible: page.sys !== null && page.sys.wifiEnabled && page.wifiList.length === 0
            title: page.sys && page.sys.loaded ? qsTr("No Networks Found") : qsTr("Looking for Networks…")
            subtitle: page.sys && page.sys.loaded ? qsTr("Move closer to a router, or try again in a moment") : ""
            busy: page.sys === null || !page.sys.loaded
        }
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "wired"
            visible: page.sys !== null && page.sys.wired !== ""
            title: qsTr("Wired")
            subtitle: page.wiredSubtitle
            leading: Symbol {
                icon: Symbols.Lan
            }
        }
        SectionRow {
            objectName: "hotspot"
            title: qsTr("Hotspot")
            subtitle: {
                if (!page.sys || !page.sys.loaded)
                    return "";
                if (page.sys.hotspotActive)
                    return qsTr("Sharing as %1").arg(page.sys.hotspotName);
                return qsTr("Share this computer's internet over Wi-Fi");
            }
            showSwitch: true
            switchChecked: page.sys ? page.sys.hotspotActive : false
            enabled: page.sys !== null && page.sys.wifiPresent && !page.sys.busy
            onSwitchToggled: checked => {
                if (!checked)
                    page.sys.stopHotspot();
                else if (page.sys.hotspotSaved)
                    page.sys.startHotspot("", "");
                else
                    hotspotSheet.openNew();
            }
            leading: Symbol {
                icon: Symbols.WifiTethering
            }

            SecondaryButton {
                visible: page.sys !== null && page.sys.hotspotSaved
                text: qsTr("Change…")
                enabled: page.sys !== null && !page.sys.busy
                onClicked: hotspotSheet.openNew()
            }
        }
        SectionRow {
            objectName: "airplane"
            title: qsTr("Airplane Mode")
            subtitle: qsTr("Turns off Wi-Fi, Bluetooth and mobile broadband")
            showSwitch: true
            switchChecked: page.sys ? page.sys.airplane : false
            enabled: page.sys !== null && page.sys.available && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeAirplane(checked)
            leading: Symbol {
                icon: Symbols.AirplanemodeActive
            }
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("VPN")

        SectionRow {
            objectName: "vpn"
            visible: page.vpnList.length === 0
            title: qsTr("No VPN Set Up")
            leading: Symbol {
                icon: Symbols.VpnKey
            }
        }
        Repeater {
            model: page.vpnList

            SectionRow {
                id: vpn
                required property var modelData
                required property int index
                // The first row is where a link to VPN lands.
                objectName: vpn.index === 0 ? "vpn" : ""
                title: vpn.modelData.id
                subtitle: vpn.modelData.connecting ? qsTr("Connecting…") : vpn.modelData.active ? qsTr("Connected") : ""
                showSwitch: true
                switchChecked: vpn.modelData.active || vpn.modelData.connecting
                enabled: page.sys !== null && !page.sys.busy
                busy: vpn.modelData.connecting
                onSwitchToggled: checked => page.sys.changeVpn(vpn.modelData.uuid, checked)
                leading: Symbol {
                    icon: Symbols.codepoint(vpn.modelData.active ? "VpnLock" : "VpnKey")
                }
            }
        }
        KcmRow {
            title: qsTr("Add a VPN")
            kcm: "kcm_networkmanagement"
            onOpenKcm: name => page.openKcm(name)
        }
    }

    AdvancedSection {
        page: page
    }

    RelatedLinks {
        page: page
    }

    // A network: how it is, and what can be done with it.
    AtlasDialog {
        id: networkSheet
        title: page.sheetNetwork ? page.sheetNetwork.ssid : ""
        preferredWidth: Kirigami.Units.gridUnit * 24

        // The list's newest entry for the network, else the one opened.
        readonly property var net: page.wifiList.find(n => n.ssid === page.sheetSsid) ?? page.sheetNetwork
        readonly property bool needsPassword: net !== null && !net.known && !net.connected && net.security !== "open" && net.security !== "enterprise"
        readonly property bool busy: page.sys !== null && page.sys.busy

        function join() {
            if (networkSheet.needsPassword && !page.sys.validPassword(networkSheet.net.security, passwordField.text))
                return;
            page.pending = true;
            page.sys.connectTo(networkSheet.net.ssid, networkSheet.needsPassword ? passwordField.text : "");
        }

        onNetChanged: if (net !== null && visible)
            page.sheetNetwork = net
        onOpened: passwordField.text = ""
        onClosed: {
            passwordField.text = "";
            page.pending = false;
        }
        footerContent: [
            SecondaryButton {
                visible: networkSheet.net !== null && networkSheet.net.known
                text: qsTr("Forget")
                enabled: !networkSheet.busy
                onClicked: {
                    page.pending = true;
                    page.sys.forget(networkSheet.net.ssid);
                }
            },
            PrimaryButton {
                visible: networkSheet.net !== null && networkSheet.net.security !== "enterprise"
                text: networkSheet.net !== null && networkSheet.net.connected ? qsTr("Disconnect") : networkSheet.needsPassword ? qsTr("Join") : qsTr("Connect")
                enabled: !networkSheet.busy && networkSheet.net !== null && (networkSheet.net.connected || !networkSheet.needsPassword || (page.sys !== null && page.sys.validPassword(networkSheet.net.security, passwordField.text)))
                onClicked: {
                    if (networkSheet.net.connected) {
                        page.pending = true;
                        page.sys.disconnect();
                    } else {
                        networkSheet.join();
                    }
                }
            },
            PrimaryButton {
                visible: networkSheet.net !== null && networkSheet.net.security === "enterprise"
                text: qsTr("Open Network Settings")
                onClicked: {
                    networkSheet.close();
                    page.openKcm("kcm_networkmanagement");
                }
            }
        ]

        InfoBanner {
            Layout.fillWidth: true
            type: "warning"
            text: page.sys ? page.sys.error : ""
            shown: text !== "" && networkSheet.visible
        }
        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Status")
                value: networkSheet.net === null ? "" : networkSheet.net.connecting ? qsTr("Connecting…") : networkSheet.net.connected ? qsTr("Connected") : networkSheet.net.known ? qsTr("Saved") : qsTr("Not connected")
                busy: networkSheet.busy && networkSheet.net !== null && page.sys.joining === networkSheet.net.ssid
            }
            SectionRow {
                title: qsTr("Signal")
                value: networkSheet.net === null ? "" : page.signalName(networkSheet.net.signal)
            }
            SectionRow {
                title: qsTr("Security")
                value: networkSheet.net === null ? "" : page.securityName(networkSheet.net.security)
            }
        }
        AtlasLabel {
            Layout.fillWidth: true
            visible: networkSheet.net !== null && networkSheet.net.security === "enterprise"
            wrapMode: Text.Wrap
            text: qsTr("This network needs a work or school login. Set it up in Plasma's network settings.")
        }
        AtlasLabel {
            Layout.fillWidth: true
            visible: networkSheet.net !== null && networkSheet.net.security === "wep"
            wrapMode: Text.Wrap
            text: qsTr("WEP is an old, weak kind of protection. Anyone nearby can learn the password.")
        }
        AtlasPasswordField {
            id: passwordField
            Layout.fillWidth: true
            visible: networkSheet.needsPassword
            enabled: !networkSheet.busy
            placeholderText: qsTr("Password")
            Accessible.name: qsTr("Password")
            onAccepted: networkSheet.join()
        }
    }

    // A new hotspot: its name and password, which NetworkManager keeps.
    AtlasDialog {
        id: hotspotSheet
        title: qsTr("Hotspot")
        preferredWidth: Kirigami.Units.gridUnit * 24

        readonly property bool valid: page.sys !== null && page.sys.validHotspotName(nameField.text) && page.sys.validPassword("personal", hotspotPassword.text)

        function openNew() {
            nameField.text = page.sys && page.sys.hotspotName !== "" ? page.sys.hotspotName : qsTr("Telamon Hotspot");
            hotspotPassword.text = page.sys ? page.sys.randomPassword() : "";
            hotspotSheet.open();
        }
        function start() {
            if (!hotspotSheet.valid)
                return;
            page.pending = true;
            page.sys.startHotspot(nameField.text.trim(), hotspotPassword.text);
        }

        // The password is made up for the person, so it is shown to be
        // written down or read out.
        onOpened: {
            hotspotPassword.forceActiveFocus();
            hotspotPassword.reveal();
        }
        onClosed: {
            hotspotPassword.text = "";
            page.pending = false;
        }
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: hotspotSheet.close()
            },
            PrimaryButton {
                text: qsTr("Start Hotspot")
                enabled: hotspotSheet.valid && page.sys !== null && !page.sys.busy
                onClicked: hotspotSheet.start()
            }
        ]

        InfoBanner {
            Layout.fillWidth: true
            type: "warning"
            text: page.sys ? page.sys.error : ""
            shown: text !== "" && hotspotSheet.visible
        }
        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Other devices join this network with the password. It is shared over Wi-Fi, so this computer's own Wi-Fi connection is turned off while it runs.")
        }
        AtlasTextField {
            id: nameField
            Layout.fillWidth: true
            maximumLength: 32
            placeholderText: qsTr("Network name")
            Accessible.name: qsTr("Network name")
        }
        AtlasPasswordField {
            id: hotspotPassword
            Layout.fillWidth: true
            placeholderText: qsTr("Password (8 or more characters)")
            Accessible.name: qsTr("Password")
            onAccepted: hotspotSheet.start()
        }
    }
}
