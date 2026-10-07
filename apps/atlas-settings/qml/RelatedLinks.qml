pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Atlas.Ui

// Links to the pages people often want next (the registry's `related`), at
// a page's end. Not shown when there are none.
Section {
    id: section

    required property SettingsPage page

    readonly property var links: page.entry.related.map(id => page.pages.find(p => p.id === id)).filter(p => p !== undefined)

    visible: links.length > 0
    title: qsTr("Related")

    Repeater {
        model: section.links

        SectionRow {
            id: link
            required property var modelData
            Layout.fillWidth: true
            title: link.modelData.title
            chevron: true
            onClicked: section.page.openPage(link.modelData.id, "")
            leading: Symbol {
                icon: Symbols.codepoint(link.modelData.symbol)
            }
        }
    }
}
