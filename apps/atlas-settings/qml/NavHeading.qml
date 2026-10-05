import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// A group's name above its pages in the sidebar, as Atlas Monitor has them.
// Hidden when the sidebar folds to icons. AtlasSidebar skips it: it is no
// entry, so selection and keyboard moves pass over it.
AtlasLabel {
    property bool compact: false

    Layout.fillWidth: true
    Layout.topMargin: Kirigami.Units.largeSpacing
    Layout.bottomMargin: Kirigami.Units.smallSpacing
    Layout.leftMargin: Kirigami.Units.largeSpacing
    visible: !compact
    textStyle: AtlasLabel.Caption
    opacity: 0.7
    elide: Text.ElideRight
    Accessible.role: Accessible.Heading
}
