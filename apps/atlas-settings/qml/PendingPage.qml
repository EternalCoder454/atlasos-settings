import QtQuick
import QtQuick.Layouts
import Atlas.Ui

// A native page that isn't built yet: says so, and opens the Plasma KCM it
// will replace, so nothing is out of reach meanwhile.
AtlasPage {
    id: page

    // The page's entry from the registry (backend.pagesJson()).
    required property var entry

    // Asks to open KCM `name`.
    signal openKcm(string name)

    title: entry.title

    AtlasEmptyState {
        Layout.fillWidth: true
        symbol: Symbols.codepoint(page.entry.symbol)
        title: qsTr("Coming Soon")
        text: page.entry.kcm !== ""
            ? qsTr("This page isn't ready yet. Until then, these settings are in Plasma's settings.")
            : qsTr("This page isn't ready yet.")
        actionText: page.entry.kcm !== "" ? qsTr("Open in Plasma Settings") : ""
        actionSymbol: Symbols.OpenInNew
        onTriggered: page.openKcm(page.entry.kcm)
    }
}
