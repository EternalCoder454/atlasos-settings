import QtQuick
import QtQuick.Layouts
import Atlas.Ui

// A page whose settings are in their own window: Printers (Plasma's KCM) and
// Updates (Atlas Updater). Opening the page opens that window; this button
// opens it again.
AtlasPage {
    id: page

    // The page's entry from the registry (backend.pagesJson()).
    required property var entry

    signal open()

    title: entry.title

    AtlasEmptyState {
        Layout.fillWidth: true
        symbol: Symbols.codepoint(page.entry.symbol)
        title: page.entry.title
        text: page.entry.kind === "app"
            ? qsTr("Updates are in Atlas Updater, in its own window.")
            : qsTr("These settings open in their own window.")
        actionText: page.entry.kind === "app" ? qsTr("Open Atlas Updater") : qsTr("Open %1").arg(page.entry.title)
        actionSymbol: Symbols.OpenInNew
        onTriggered: page.open()
    }
}
