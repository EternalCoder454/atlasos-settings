pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Privacy & Security: when the screen locks (kscreenlockerrc, as Plasma's
// screen locking page writes it), Location Services, the Firewall
// (firewalld, switched by systemd), and Crash Reports (the setting Telamon
// Updater also writes). Under Advanced: what the firewall allows, and
// Recent Files and File Search, which are Plasma's.
SettingsPage {
    id: page

    // src/privacy_page.rs, cpp/screenlockconfig.h and cpp/autostartconfig.h;
    // they go with the page.
    readonly property var sys: pageBackends ? pageBackends.create("privacy", page) : null
    readonly property var lockConfig: pageBackends ? pageBackends.create("screenlock-config", page) : null
    readonly property var autostart: pageBackends ? pageBackends.create("autostart-config", page) : null
    // kscreenlockerrc as read: {autoLock, minutes, onWake, grace}.
    property var lock: lockConfig ? lockConfig.read() : ({})
    // Location Services is geoclue's agent, which starts when you sign in.
    readonly property string locationAgent: "geoclue-demo-agent.desktop"
    property bool locationKnown: autostart ? autostart.known(locationAgent) : false
    property bool locationOn: autostart ? autostart.isEnabled(locationAgent) : false
    property bool locationChanged: false
    // A write to a file failed.
    property string notice: ""

    readonly property var serviceNames: ({
            "ssh": qsTr("Remote Login (SSH)"),
            "mdns": qsTr("Local Network Discovery"),
            "samba-client": qsTr("Windows File Sharing"),
            "samba": qsTr("File Sharing Server"),
            "kdeconnect": qsTr("KDE Connect"),
            "http": qsTr("Web Server"),
            "https": qsTr("Secure Web Server"),
            "ftp": qsTr("FTP Server"),
            "dhcpv6-client": qsTr("Network Setup (DHCPv6)"),
            "steam-streaming": qsTr("Steam Remote Play"),
            "steam-lan-transfer": qsTr("Steam Local Transfers"),
            "ipp": qsTr("Printer Sharing"),
            "ipp-client": qsTr("Network Printers"),
            "vnc-server": qsTr("Screen Sharing (VNC)"),
            "rdp": qsTr("Remote Desktop"),
            "syncthing": qsTr("Syncthing"),
            "cockpit": qsTr("Web Console")
        })

    function serviceName(id: string): string {
        if (page.serviceNames[id] !== undefined)
            return page.serviceNames[id];
        const words = id.replace(/[-_.]+/g, " ");
        return words.charAt(0).toUpperCase() + words.slice(1);
    }

    function lockWords(minutes: int): string {
        if (minutes <= 0)
            return qsTr("Never");
        if (minutes < 60)
            return minutes === 1 ? qsTr("After 1 minute") : qsTr("After %1 minutes").arg(minutes);
        const hours = minutes / 60;
        if (Number.isInteger(hours))
            return hours === 1 ? qsTr("After 1 hour") : qsTr("After %1 hours").arg(hours);
        return qsTr("After %1 minutes").arg(minutes);
    }

    function lockChoices(current: int): var {
        const list = [0, 1, 2, 5, 10, 15, 30, 60];
        if (!list.includes(current))
            list.push(current);
        return list.sort((a, b) => a - b);
    }

    function wroteLock(ok: bool) {
        if (!ok)
            page.notice = qsTr("Settings couldn't save the screen lock settings.");
        page.lock = page.lockConfig.read();
    }

    function setLocation(on: bool) {
        if (!page.autostart.setEnabled(page.locationAgent, on))
            page.notice = qsTr("Settings couldn't change Location Services.");
        else
            page.locationChanged = true;
        page.locationOn = page.autostart.isEnabled(page.locationAgent);
    }

    function firewallSubtitle(): string {
        if (!page.sys || !page.sys.loaded)
            return "";
        if (!page.sys.fwInstalled)
            return qsTr("The firewall isn't installed");
        return page.sys.fwRunning ? qsTr("Stops other computers on the network from reaching apps here") : qsTr("Off: any app that listens can be reached from the network");
    }

    Component.onCompleted: if (sys) sys.refresh()

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
    InfoBanner {
        Layout.fillWidth: true
        type: "information"
        text: qsTr("Location Services changes when you sign out and back in.")
        shown: page.locationChanged
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "screen-lock"
            title: qsTr("Screen Lock")
            subtitle: page.lock.onWake === false ? qsTr("No password is asked when the computer wakes up") : ""
            value: page.lockWords(page.lock.minutes ?? 0)
            chevron: true
            enabled: page.lockConfig !== null
            onClicked: lockSheet.open()
        }
        SectionRow {
            objectName: "location"
            title: qsTr("Location Services")
            subtitle: page.locationKnown ? qsTr("Let apps you allow find where you are") : qsTr("Location services aren't installed")
            showSwitch: true
            switchChecked: page.locationOn
            enabled: page.autostart !== null && page.locationKnown
            onSwitchToggled: checked => page.setLocation(checked)
        }
        SectionRow {
            objectName: "firewall"
            title: qsTr("Firewall")
            subtitle: page.firewallSubtitle()
            showSwitch: true
            switchChecked: page.sys !== null && page.sys.fwRunning
            busy: page.sys !== null && page.sys.busy
            enabled: page.sys !== null && page.sys.loaded && page.sys.fwInstalled && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeFirewall(checked)
        }
    }

    Section {
        Layout.fillWidth: true
        footer: page.sys && page.sys.crashHasServer ? qsTr("When an app crashes, a report is saved that you can read. Nothing is sent unless you choose to send it in Telamon Updater, and sent reports are public on the Telamon OS GitHub project.") : qsTr("When an app crashes, a report is saved that you can read. No crash report server is set up on this computer, so reports can't be sent.")

        SectionRow {
            objectName: "crash-reports"
            title: qsTr("Crash Reports")
            subtitle: qsTr("Save a report you can look at when an app crashes")
            showSwitch: true
            switchChecked: page.sys !== null && page.sys.crashEnabled
            enabled: page.sys !== null && page.sys.loaded && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeCrashReports(checked)
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "firewall-rules"
            title: qsTr("Allowed Apps and Ports")
            subtitle: {
                if (!page.sys || !page.sys.fwRunning)
                    return qsTr("Turn the firewall on to change this");
                const n = page.sys.fwServices.length + page.sys.fwPorts.length;
                return n === 1 ? qsTr("1 way in") : qsTr("%1 ways in").arg(n);
            }
            chevron: true
            enabled: page.sys !== null && page.sys.fwRunning
            onClicked: rulesSheet.open()
        }
    }

    RelatedLinks {
        page: page
    }

    // Screen Lock.
    AtlasDialog {
        id: lockSheet
        title: qsTr("Screen Lock")
        preferredWidth: Kirigami.Units.gridUnit * 26
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: lockSheet.close()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Locking hides what is on the screen until you sign in again.")
        }
        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Lock When Idle")
                subtitle: qsTr("After you haven't used the computer for a while")

                AtlasComboBox {
                    id: lockBox
                    readonly property var values: page.lockChoices(page.lock.minutes ?? 0)
                    width: Kirigami.Units.gridUnit * 11
                    model: lockBox.values.map(m => page.lockWords(m))
                    currentIndex: Math.max(0, lockBox.values.indexOf(page.lock.minutes ?? 0))
                    onActivated: index => page.wroteLock(page.lockConfig.setLockAfter(lockBox.values[index]))
                    Accessible.name: qsTr("Lock when idle")
                }
            }
            SectionRow {
                title: qsTr("Ask for a Password When Waking Up")
                subtitle: qsTr("After the computer has been asleep")
                showSwitch: true
                switchChecked: page.lock.onWake ?? true
                onSwitchToggled: checked => page.wroteLock(page.lockConfig.setLockOnWake(checked))
            }
        }
    }

    // What the firewall lets through.
    AtlasDialog {
        id: rulesSheet
        title: qsTr("Allowed Apps and Ports")
        preferredWidth: Kirigami.Units.gridUnit * 28
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: rulesSheet.close()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Other computers on the network can reach these on this computer. Anything not listed is blocked.")
        }

        Section {
            Layout.fillWidth: true
            title: qsTr("Apps")

            Repeater {
                model: page.sys ? page.sys.fwServices : []

                SectionRow {
                    id: serviceRow
                    required property string modelData
                    Layout.fillWidth: true
                    title: page.serviceName(serviceRow.modelData)
                    subtitle: serviceRow.modelData
                    showSwitch: true
                    switchChecked: true
                    enabled: page.sys !== null && !page.sys.busy
                    onSwitchToggled: checked => {
                        if (!checked)
                            page.sys.removeService(serviceRow.modelData);
                    }
                }
            }
            SectionRow {
                title: qsTr("Allow Another App")
                clickable: true
                enabled: page.sys !== null && !page.sys.busy
                leading: Symbol {
                    icon: Symbols.Add
                }
                onClicked: servicePicker.open()
            }
        }

        Section {
            Layout.fillWidth: true
            title: qsTr("Ports")

            Repeater {
                model: page.sys ? page.sys.fwPorts : []

                SectionRow {
                    id: portRow
                    required property string modelData
                    Layout.fillWidth: true
                    title: portRow.modelData.split("/")[0]
                    subtitle: portRow.modelData.split("/")[1].toUpperCase()
                    showSwitch: true
                    switchChecked: true
                    enabled: page.sys !== null && !page.sys.busy
                    onSwitchToggled: checked => {
                        if (!checked)
                            page.sys.removePort(portRow.modelData);
                    }
                }
            }
            SectionRow {
                title: qsTr("Open a Port")
                subtitle: qsTr("A number such as 8080, or a range such as 8000-8100")

                AtlasTextField {
                    id: portField
                    width: Kirigami.Units.gridUnit * 7
                    placeholderText: qsTr("Port")
                    maximumLength: 11
                    errorText: portField.text.length > 0 && page.sys && !page.sys.validPort(portField.text) ? qsTr("Not a port") : ""
                    Accessible.name: qsTr("Port")
                }
                AtlasComboBox {
                    id: protocolBox
                    width: Kirigami.Units.gridUnit * 5
                    model: ["TCP", "UDP"]
                    Accessible.name: qsTr("Protocol")
                }
                SecondaryButton {
                    text: qsTr("Open")
                    enabled: page.sys !== null && !page.sys.busy && page.sys.validPort(portField.text)
                    onClicked: {
                        page.sys.addPort(portField.text + "/" + protocolBox.currentText.toLowerCase());
                        portField.text = "";
                    }
                }
            }
        }
    }

    PickerSheet {
        id: servicePicker
        title: qsTr("Allow Another App")
        placeholderText: qsTr("Nothing Left to Allow")
        choices: (page.sys ? page.sys.fwAvailable : []).filter(s => !(page.sys ? page.sys.fwServices : []).includes(s)).map(s => ({
                    title: page.serviceName(s),
                    subtitle: s,
                    value: s,
                    keys: s.toLowerCase()
                })).sort((a, b) => a.title.localeCompare(b.title))
        onChosen: value => page.sys.addService(value)
    }
}
