pragma ComponentBehavior: Bound

import QtQuick
import Telamon.Ui

// What every native page is built on: its registry entry, the setting a
// link or search asked for (scrolled to and briefly highlighted, its
// Advanced section opened when it is folded there), and the ways out of the
// page. A page puts its Sections in, then an AdvancedSection and
// RelatedLinks at its end:
//
//   SettingsPage {
//       id: page
//       Section { SectionRow { objectName: "volume"; title: qsTr("Volume") } }
//       AdvancedSection { page: page }
//       RelatedLinks { page: page }
//   }
//
// A row is found by its objectName, which is the registry item's ID.
TelamonPage {
    id: page

    // The page's entry from the registry (backend.pagesJson()).
    required property var entry
    // The setting to show; "" for the page's top.
    property string itemId: ""
    // Every page of the registry, for links to them.
    property var pages: []
    // Makes the page's backends (cpp/pagebackends.h).
    property var pageBackends: null
    // Whether the Advanced section is open. Closed on every visit, opened
    // by the reader or by a link to a folded setting.
    property bool advancedOpen: false

    readonly property var advancedItems: entry.items.filter(i => i.advanced)

    // Ways out, which Main.qml carries out.
    signal openPage(string id, string item)
    signal openKcm(string name)
    signal run(var argv)

    title: entry.title

    function isAdvanced(id: string): bool {
        return advancedItems.some(i => i.id === id);
    }

    // The row for registry item `id` among `item`'s descendants, or null.
    function findRow(item: Item, id: string): Item {
        for (let i = 0; i < item.children.length; ++i) {
            const c = item.children[i];
            if (c.objectName === id)
                return c;
            const found = findRow(c, id);
            if (found)
                return found;
        }
        return null;
    }

    function reveal() {
        if (itemId === "")
            return;
        if (isAdvanced(itemId))
            advancedOpen = true;
        // After the opened section is laid out.
        revealTimer.restart();
    }

    onItemIdChanged: reveal()
    Component.onCompleted: reveal()

    Timer {
        id: revealTimer
        interval: 50
        onTriggered: {
            const row = page.findRow(page, page.itemId);
            if (!row)
                return;
            page.ensureVisible(row);
            page.highlight.parent = row;
            fade.restart();
        }
    }

    // A short glow over the row a link pointed at.
    // Not in the page's column: it is put over the row when needed.
    readonly property Rectangle highlight: Rectangle {
        id: highlight
        visible: opacity > 0
        anchors.fill: parent
        anchors.margins: 3
        radius: TelamonStyle.radiusSmall
        color: Qt.alpha(TelamonStyle.accent, 0.18)
        border.width: 2
        border.color: Qt.alpha(TelamonStyle.accent, 0.6)
        opacity: 0
        z: 10

        SequentialAnimation {
            id: fade
            NumberAnimation {
                target: highlight
                property: "opacity"
                to: 1
                duration: TelamonStyle.durationShort
            }
            PauseAnimation {
                duration: 1200
            }
            NumberAnimation {
                target: highlight
                property: "opacity"
                to: 0
                duration: TelamonStyle.duration * 3
            }
        }
    }
}
