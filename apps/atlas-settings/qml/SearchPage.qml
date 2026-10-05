import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Search results over pages and the settings on them (settings-registry's
// search.rs), shown in place of the page while the search field has text.
// Not an AtlasPage: the results list scrolls itself.
Item {
    id: page

    required property var backend
    required property string query

    // A result was chosen: its page and setting, or the KCM that holds it.
    signal chosen(string pageId, string itemId, string kcm)

    readonly property var hits: query === "" ? [] : JSON.parse(backend.search(query)).map(h => Object.assign(h, {
                symbolValue: Symbols.codepoint(h.symbol)
            }))

    // Keys from the search field: Up, Down and Enter move and choose here.
    function handleKey(event): bool {
        return results.handleKey(event);
    }

    // AtlasPage's geometry, so the title doesn't jump when results replace
    // a page.
    ColumnLayout {
        y: Kirigami.Units.gridUnit * 1.5
        width: Math.min(parent.width - Kirigami.Units.gridUnit * 3, Kirigami.Units.gridUnit * 38)
        height: parent.height - Kirigami.Units.gridUnit * 3
        x: Math.round((parent.width - width) / 2)
        spacing: Kirigami.Units.gridUnit * 1.2

        AtlasLabel {
            Layout.fillWidth: true
            textStyle: AtlasLabel.Title
            text: qsTr("Search")
            Accessible.role: Accessible.Heading
        }

        AtlasSearchResults {
            id: results
            Layout.fillWidth: true
            Layout.fillHeight: true
            model: page.hits
            textRole: "title"
            subtitleRole: "subtitle"
            symbolRole: "symbolValue"
            placeholderText: qsTr("No Settings Found")
            onActivated: index => {
                const h = page.hits[index];
                if (h)
                    page.chosen(h.page, h.item, h.kcm);
            }
        }
    }
}
