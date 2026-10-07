pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// A sheet to pick one of many (a time zone, a language): a search field
// over a list. Up, Down and Enter work from the field.
TelamonDialog {
    id: sheet

    // [{title, subtitle, value, keys}]: `keys` is extra text the search
    // matches (lower case).
    property var choices: []
    // The value chosen now, marked in the list.
    property string current: ""
    property string placeholderText: qsTr("Nothing Found")

    signal chosen(string value)

    readonly property var shown: {
        const q = search.query.trim().toLowerCase();
        const all = sheet.choices.map(c => Object.assign({}, c, {
                    subtitle: c.value === sheet.current ? (c.subtitle !== "" ? qsTr("%1 · Current").arg(c.subtitle) : qsTr("Current")) : c.subtitle
                }));
        if (q === "")
            return all;
        const words = q.split(/\s+/);
        return all.filter(c => {
            const text = (c.title + " " + c.subtitle + " " + (c.keys ?? "")).toLowerCase();
            return words.every(w => text.includes(w));
        });
    }

    preferredWidth: Kirigami.Units.gridUnit * 26
    onOpened: {
        search.text = "";
        search.forceActiveFocus();
    }

    SearchField {
        id: search
        Layout.fillWidth: true
        placeholderText: qsTr("Search")
        Keys.onPressed: event => event.accepted = results.handleKey(event)
    }

    TelamonSearchResults {
        id: results
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 18
        model: sheet.shown
        textRole: "title"
        subtitleRole: "subtitle"
        placeholderText: sheet.placeholderText
        onActivated: index => {
            const c = sheet.shown[index];
            if (c) {
                sheet.chosen(c.value);
                sheet.close();
            }
        }
    }
}
