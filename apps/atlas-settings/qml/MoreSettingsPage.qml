pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Atlas.Ui

// The KCMs Settings has no page for, grouped as System Settings groups them.
// Each opens in kcmshell6.
AtlasPage {
    id: page

    // cpp/kcmcatalog.h and src/backend.rs.
    required property var kcmCatalog
    required property var backend

    signal openKcm(string name)

    // System Settings' category IDs (X-KDE-System-Settings-Category in
    // /usr/share/systemsettings/categories, Plasma 6.7), by the name it
    // shows. An unknown one is shown as its ID made readable.
    readonly property var categoryTitles: ({
            "appearance": qsTr("Appearance & Style"),
            "font": qsTr("Text & Fonts"),
            "themes": qsTr("Colors & Themes"),
            "applications": qsTr("Apps & Windows"),
            "applications-defaults": qsTr("Default Applications"),
            "applications-permissions": qsTr("Application Permissions"),
            "windowmanagement": qsTr("Window Management"),
            "hardware": qsTr("Connected Devices"),
            "removable-storage": qsTr("Disks & Cameras"),
            "input-devices": qsTr("Input & Output"),
            "display": qsTr("Display & Monitor"),
            "pointing-devices": qsTr("Mouse & Touchpad"),
            "hardware-input-touchscreen": qsTr("Touchscreen"),
            "keyboard": qsTr("Keyboard"),
            "audio-and-video": qsTr("Multimedia"),
            "network": qsTr("Networking"),
            "networksettings": qsTr("Wi-Fi & Internet"),
            "personalization": qsTr("Personalization"),
            "regionalsettings": qsTr("Language & Time"),
            "security-privacy": qsTr("Security & Privacy"),
            "system-administration": qsTr("System"),
            "session": qsTr("Session"),
            "workspace": qsTr("Workspace"),
            "search": qsTr("Search")
        })

    function categoryTitle(id: string): string {
        if (id === "")
            return qsTr("Other");
        // Own keys only: "constructor" is no category.
        if (Object.prototype.hasOwnProperty.call(categoryTitles, id))
            return categoryTitles[id];
        const words = id.replace(/[_-]+/g, " ").trim();
        if (words === "")
            return qsTr("Other");
        return words.charAt(0).toUpperCase() + words.slice(1);
    }

    // [{title, kcms: [...]}], groups by title with Other last, the KCMs in
    // name order.
    readonly property var groups: {
        const byTitle = {};
        const other = categoryTitle("");
        for (const k of kcmCatalog.list()) {
            if (backend.hasPage(k.id))
                continue;
            const t = categoryTitle(k.category);
            (byTitle[t] = byTitle[t] ?? []).push(k);
        }
        return Object.keys(byTitle).sort((a, b) => (a === other) - (b === other) || a.localeCompare(b)).map(t => ({
                    title: t,
                    kcms: byTitle[t]
                }));
    }

    title: qsTr("Other Plasma Settings")

    AtlasLabel {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: qsTr("Settings that have no page here yet. Each opens Plasma's own settings in a window.")
    }

    AtlasEmptyState {
        Layout.fillWidth: true
        visible: page.groups.length === 0
        symbol: Symbols.Tune
        title: qsTr("Nothing Else to Set")
        text: qsTr("Every installed settings module has a page in Settings.")
    }

    Repeater {
        model: page.groups

        Section {
            id: section
            required property var modelData
            Layout.fillWidth: true
            title: modelData.title

            Repeater {
                model: section.modelData.kcms

                SectionRow {
                    required property var modelData
                    Layout.fillWidth: true
                    title: modelData.name
                    subtitle: modelData.description
                    iconName: modelData.icon
                    chevron: true
                    onClicked: page.openKcm(modelData.id)
                }
            }
        }
    }
}
