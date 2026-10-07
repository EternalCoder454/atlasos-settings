pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Apps: which app opens what (mimeapps.list, as Plasma's default apps page
// writes it), which apps start when you sign in (~/.config/autostart), and
// what each Flatpak app may reach (its override file and the portals'
// permission store). File Associations, under Advanced, are Plasma's.
SettingsPage {
    id: page

    // src/apps_page.rs, cpp/defaultapps.h and cpp/autostartconfig.h; they
    // go with the page.
    readonly property var sys: pageBackends ? pageBackends.create("apps", page) : null
    readonly property var defaultApps: pageBackends ? pageBackends.create("default-apps", page) : null
    readonly property var autostart: pageBackends ? pageBackends.create("autostart-config", page) : null

    // The Flatpak apps.
    readonly property var apps: parse(page.sys ? page.sys.appsJson : "", [])
    // The app whose permissions are open, and its permissions.
    property string appId: ""
    property string appName: ""
    readonly property var perm: {
        const p = parse(page.sys ? page.sys.appJson : "", ({}));
        return p.id === page.appId ? p : ({});
    }
    // Default Apps: for each kind {id, title, current, choices}.
    property var kinds: []
    // Startup Apps.
    property var startup: autostart ? autostart.entries() : []
    // A write failed.
    property string notice: ""

    readonly property var fileLevels: [
        {
            value: "none",
            text: qsTr("Only Its Own Files")
        },
        {
            value: "downloads",
            text: qsTr("Downloads Folder")
        },
        {
            value: "home",
            text: qsTr("Home Folder")
        },
        {
            value: "everything",
            text: qsTr("All Files")
        }
    ]
    readonly property var answers: [
        {
            value: "ask",
            text: qsTr("Ask Each Time")
        },
        {
            value: "yes",
            text: qsTr("Allow")
        },
        {
            value: "no",
            text: qsTr("Don't Allow")
        }
    ]

    function parse(text: string, fallback: var): var {
        try {
            return text === "" ? fallback : JSON.parse(text);
        } catch (e) {
            return fallback;
        }
    }

    function indexOfValue(list: var, value: string): int {
        const i = list.findIndex(x => x.value === value);
        return i < 0 ? 0 : i;
    }

    // What each kind has now and can have.
    function loadKinds() {
        if (!page.defaultApps)
            return;
        page.kinds = page.defaultApps.kinds().map(k => {
            const choices = page.defaultApps.choices(k.id);
            const current = page.defaultApps.current(k.id);
            // The default may be an app the list doesn't offer: keep it
            // shown rather than lose it.
            if (current.id !== "" && !choices.some(c => c.id === current.id))
                choices.unshift(current);
            return {
                id: k.id,
                title: k.title,
                current: current.id,
                currentName: current.name,
                choices: choices
            };
        });
    }

    function reloadStartup() {
        page.startup = page.autostart.entries();
    }

    function startupDone(ok: bool) {
        if (!ok)
            page.notice = qsTr("Settings couldn't change the startup apps.");
        page.reloadStartup();
    }

    readonly property string browserName: {
        const k = page.kinds.find(x => x.id === "browser");
        return k ? k.currentName : "";
    }

    Component.onCompleted: {
        if (sys)
            sys.refresh();
        loadKinds();
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

        SectionRow {
            objectName: "defaults"
            title: qsTr("Default Apps")
            subtitle: qsTr("Web browser, email, files, music, video and more")
            value: page.browserName
            chevron: true
            enabled: page.defaultApps !== null
            onClicked: {
                page.loadKinds();
                defaultsSheet.open();
            }
        }
        SectionRow {
            objectName: "autostart"
            title: qsTr("Startup Apps")
            subtitle: qsTr("Apps that open when you sign in")
            value: {
                const n = page.startup.filter(e => e.enabled).length;
                return n === 0 ? qsTr("None") : n === 1 ? qsTr("1 App") : qsTr("%1 Apps").arg(n);
            }
            chevron: true
            enabled: page.autostart !== null
            onClicked: {
                page.reloadStartup();
                startupSheet.open();
            }
        }
        SectionRow {
            objectName: "permissions"
            title: qsTr("App Permissions")
            subtitle: page.apps.length > 0 ? qsTr("Files, devices, network and more for each app") : (page.sys && page.sys.loaded ? qsTr("No sandboxed apps are installed") : "")
            value: page.apps.length > 0 ? (page.apps.length === 1 ? qsTr("1 App") : qsTr("%1 Apps").arg(page.apps.length)) : ""
            chevron: true
            enabled: page.apps.length > 0
            onClicked: permissionsSheet.open()
        }
    }

    AdvancedSection {
        page: page
    }

    RelatedLinks {
        page: page
    }

    // Default Apps.
    AtlasDialog {
        id: defaultsSheet
        title: qsTr("Default Apps")
        preferredWidth: Kirigami.Units.gridUnit * 30
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: defaultsSheet.close()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("The app that opens each kind of link or file.")
        }
        Section {
            Layout.fillWidth: true

            Repeater {
                model: page.kinds

                SectionRow {
                    id: kindRow
                    required property var modelData
                    Layout.fillWidth: true
                    title: kindRow.modelData.title

                    AtlasComboBox {
                        width: Kirigami.Units.gridUnit * 14
                        filterable: kindRow.modelData.choices.length > 8
                        enabled: kindRow.modelData.choices.length > 0
                        model: kindRow.modelData.choices.length > 0 ? kindRow.modelData.choices.map(c => c.name) : [qsTr("No App Found")]
                        currentIndex: Math.max(0, kindRow.modelData.choices.findIndex(c => c.id === kindRow.modelData.current))
                        onActivated: index => {
                            const choice = kindRow.modelData.choices[index];
                            if (choice && !page.defaultApps.setDefault(kindRow.modelData.id, choice.id))
                                page.notice = qsTr("Settings couldn't change the default app.");
                            page.loadKinds();
                        }
                        Accessible.name: kindRow.modelData.title
                    }
                }
            }
        }
    }

    // Startup Apps.
    AtlasDialog {
        id: startupSheet
        title: qsTr("Startup Apps")
        preferredWidth: Kirigami.Units.gridUnit * 30
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: startupSheet.close()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("These open each time you sign in. Turn one off to stop it; apps the system starts can be turned off but not removed.")
        }
        Section {
            Layout.fillWidth: true

            Repeater {
                model: page.startup

                SectionRow {
                    id: startRow
                    required property var modelData
                    Layout.fillWidth: true
                    title: startRow.modelData.name
                    subtitle: startRow.modelData.comment
                    showSwitch: true
                    switchChecked: startRow.modelData.enabled
                    leading: Kirigami.Icon {
                        source: startRow.modelData.icon
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                    }
                    onSwitchToggled: checked => page.startupDone(page.autostart.setEnabled(startRow.modelData.id, checked))

                    SecondaryButton {
                        visible: !startRow.modelData.system
                        text: qsTr("Remove")
                        onClicked: page.startupDone(page.autostart.remove(startRow.modelData.id))
                    }
                }
            }
            SectionRow {
                title: qsTr("Add an App")
                clickable: true
                leading: Symbol {
                    icon: Symbols.Add
                }
                onClicked: {
                    startupPicker.choices = page.autostart.installedApps().map(a => ({
                                title: a.name,
                                subtitle: a.comment,
                                value: a.id,
                                keys: a.id.toLowerCase()
                            }));
                    startupPicker.open();
                }
            }
        }
    }

    PickerSheet {
        id: startupPicker
        title: qsTr("Add an App")
        placeholderText: qsTr("No App Found")
        onChosen: value => page.startupDone(page.autostart.add(value))
    }

    // The Flatpak apps.
    AtlasDialog {
        id: permissionsSheet
        title: qsTr("App Permissions")
        preferredWidth: Kirigami.Units.gridUnit * 30
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: permissionsSheet.close()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Sandboxed apps only reach what you allow here.")
        }
        Section {
            Layout.fillWidth: true

            Repeater {
                model: page.apps

                SectionRow {
                    id: appRow
                    required property var modelData
                    Layout.fillWidth: true
                    title: appRow.modelData.name
                    subtitle: appRow.modelData.id
                    chevron: true
                    leading: Kirigami.Icon {
                        source: appRow.modelData.icon
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                    }
                    onClicked: {
                        page.appId = appRow.modelData.id;
                        page.appName = appRow.modelData.name;
                        page.sys.loadApp(appRow.modelData.id);
                        appSheet.open();
                    }
                }
            }
        }
    }

    // One app's permissions.
    AtlasDialog {
        id: appSheet
        title: page.appName
        preferredWidth: Kirigami.Units.gridUnit * 30
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: appSheet.close()
            }
        ]

        Section {
            Layout.fillWidth: true
            visible: page.perm.id !== undefined
            title: qsTr("Access")
            footer: qsTr("Changes apply the next time the app starts.")

            SectionRow {
                title: qsTr("Files")
                enabled: page.sys !== null && !page.sys.busy

                AtlasComboBox {
                    width: Kirigami.Units.gridUnit * 13
                    model: page.fileLevels.map(l => l.text)
                    currentIndex: page.indexOfValue(page.fileLevels, page.perm.files ?? "none")
                    onActivated: index => page.sys.changeFiles(page.appId, page.fileLevels[index].value)
                    Accessible.name: qsTr("Files")
                }
            }
            SectionRow {
                title: qsTr("Devices")
                subtitle: qsTr("USB devices, game controllers and other hardware")
                showSwitch: true
                switchChecked: page.perm.devices ?? false
                enabled: page.sys !== null && !page.sys.busy
                onSwitchToggled: checked => page.sys.changeDevices(page.appId, checked)
            }
            SectionRow {
                title: qsTr("Network")
                subtitle: qsTr("Reach the internet and your local network")
                showSwitch: true
                switchChecked: page.perm.network ?? false
                enabled: page.sys !== null && !page.sys.busy
                onSwitchToggled: checked => page.sys.changeNetwork(page.appId, checked)
            }
        }

        Section {
            Layout.fillWidth: true
            visible: page.perm.id !== undefined && page.sys !== null && page.sys.portalAvailable
            title: qsTr("When the App Asks")

            Repeater {
                model: [
                    {
                        what: "background",
                        title: qsTr("Run in the Background"),
                        hint: qsTr("Keep working when its window is closed")
                    },
                    {
                        what: "camera",
                        title: qsTr("Camera"),
                        hint: ""
                    },
                    {
                        what: "microphone",
                        title: qsTr("Microphone"),
                        hint: ""
                    },
                    {
                        what: "screen",
                        title: qsTr("Screen Sharing"),
                        hint: qsTr("Share your screen or take screenshots")
                    }
                ]

                SectionRow {
                    id: portalRow
                    required property var modelData
                    Layout.fillWidth: true
                    title: portalRow.modelData.title
                    subtitle: portalRow.modelData.hint
                    enabled: page.sys !== null && !page.sys.busy

                    AtlasComboBox {
                        width: Kirigami.Units.gridUnit * 11
                        model: page.answers.map(a => a.text)
                        currentIndex: page.indexOfValue(page.answers, page.perm[portalRow.modelData.what] ?? "ask")
                        onActivated: index => page.sys.changePortal(page.appId, portalRow.modelData.what, page.answers[index].value)
                        Accessible.name: portalRow.modelData.title
                    }
                }
            }
        }
    }
}
