pragma ComponentBehavior: Bound

import QtQuick
import Atlas.Ui

// A page's folded settings, last on the page and closed on every visit
// (docs/DESIGN.md, "Pages: simple first"). The page's own rows go inside;
// a row for each folded KCM setting of the registry is added after them.
Section {
    id: section

    required property SettingsPage page

    title: qsTr("Advanced")
    foldable: true
    folded: !page.advancedOpen
    onFoldRequested: fold => section.page.advancedOpen = !fold

    Repeater {
        model: section.page.advancedItems.filter(i => i.kcm !== "")

        KcmRow {
            required property var modelData
            objectName: modelData.id
            title: modelData.title
            kcm: modelData.kcm
            onOpenKcm: name => section.page.openKcm(name)
        }
    }
}
