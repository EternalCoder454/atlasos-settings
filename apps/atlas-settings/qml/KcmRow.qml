import QtQuick
import QtQuick.Layouts
import Atlas.Ui

// A setting Plasma's own settings hold: says so, and opens them.
SectionRow {
    id: row

    // The KCM to open (the registry item's `kcm`).
    required property string kcm

    signal openKcm(string name)

    Layout.fillWidth: true
    subtitle: qsTr("Opens in Plasma's settings")
    clickable: true
    onClicked: row.openKcm(row.kcm)

    Symbol {
        icon: Symbols.OpenInNew
        opacity: 0.6
    }
}
