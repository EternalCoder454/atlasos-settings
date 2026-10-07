pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import Atlas.Ui

// The screens drawn as they sit on the desk, each a rectangle in proportion
// to its size on the desktop (logical pixels). A click picks one; with
// several, a screen is dragged to where it sits: a dashed outline shows where
// it will land (touching another screen, as ScreenConfig::snapped says), and
// dropping it there sets that.
Item {
    id: view

    // The enabled screens, as ScreenConfig.outputs gives them.
    property var screens: []
    // The ID of the screen picked.
    property int selectedId: -1
    // The ScreenConfig: for the snapped position while dragging and the drop.
    property var backend: null
    // Nothing can be dragged while a change is being applied.
    readonly property bool draggable: screens.length > 1 && backend !== null && !backend.busy

    signal picked(int id)

    implicitHeight: Kirigami.Units.gridUnit * (screens.length > 1 ? 15 : 10)

    // What the screens cover, in desktop pixels, and room around it to drop
    // a screen on any side.
    readonly property var bounds: {
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const s of view.screens) {
            x0 = Math.min(x0, s.x);
            y0 = Math.min(y0, s.y);
            x1 = Math.max(x1, s.x + s.width);
            y1 = Math.max(y1, s.y + s.height);
        }
        return view.screens.length > 0 ? {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0
        } : {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080
        };
    }
    readonly property real room: view.screens.length > 1 ? 0.18 : 0
    readonly property real worldW: bounds.w * (1 + 2 * room)
    readonly property real worldH: bounds.h * (1 + 2 * room)
    readonly property real pad: Kirigami.Units.gridUnit
    // Pixels on the page per desktop pixel.
    readonly property real zoom: Math.min((width - 2 * pad) / worldW, (height - 2 * pad) / worldH, 0.2)
    readonly property real originX: (width - worldW * zoom) / 2 + bounds.w * room * zoom - bounds.x * zoom
    readonly property real originY: (height - worldH * zoom) / 2 + bounds.h * room * zoom - bounds.y * zoom

    function toDesktopX(px: real): int {
        return Math.round((px - originX) / zoom);
    }
    function toDesktopY(py: real): int {
        return Math.round((py - originY) / zoom);
    }

    // Where the dragged screen would land.
    Rectangle {
        id: landing
        property bool shown: false
        visible: shown
        radius: AtlasStyle.radius
        color: "transparent"
        border.width: 2
        border.color: AtlasStyle.accent
        opacity: 0.8
        z: 0
    }

    Repeater {
        model: view.screens

        Rectangle {
            id: screen

            required property var modelData
            required property int index
            readonly property bool picked: modelData.id === view.selectedId
            readonly property real homeX: view.originX + modelData.x * view.zoom
            readonly property real homeY: view.originY + modelData.y * view.zoom

            width: Math.max(Kirigami.Units.gridUnit * 2, modelData.width * view.zoom)
            height: Math.max(Kirigami.Units.gridUnit * 2, modelData.height * view.zoom)
            radius: AtlasStyle.radius
            color: picked ? AtlasStyle.selection : AtlasStyle.control
            border.width: picked ? 2 : 1
            border.color: picked ? AtlasStyle.accent : AtlasStyle.controlBorder
            z: drag.active ? 2 : 1
            activeFocusOnTab: true

            Accessible.role: Accessible.Button
            Accessible.name: qsTr("%1, screen %2").arg(modelData.label).arg(index + 1)
            Accessible.description: qsTr("%1, %2").arg(modelData.resolutionText).arg(modelData.primary ? qsTr("main display") : "")
            Accessible.onPressAction: view.picked(modelData.id)
            Keys.onReturnPressed: view.picked(modelData.id)
            Keys.onEnterPressed: view.picked(modelData.id)
            Keys.onSpacePressed: view.picked(modelData.id)

            // Home unless being dragged; it glides to a new place after a drop.
            Binding {
                target: screen
                property: "x"
                value: screen.homeX
                when: !drag.active
                restoreMode: Binding.RestoreNone
            }
            Binding {
                target: screen
                property: "y"
                value: screen.homeY
                when: !drag.active
                restoreMode: Binding.RestoreNone
            }
            Behavior on x {
                enabled: !drag.active && !AtlasStyle.reducedMotion
                NumberAnimation {
                    duration: AtlasStyle.duration
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on y {
                enabled: !drag.active && !AtlasStyle.reducedMotion
                NumberAnimation {
                    duration: AtlasStyle.duration
                    easing.type: Easing.OutCubic
                }
            }

            // The number the screen has in this list, as on the identify
            // badges: the dock of the first screen is "1".
            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.margins: AtlasStyle.spacing
                width: Kirigami.Units.gridUnit * 1.2
                height: width
                radius: width / 2
                color: screen.picked ? AtlasStyle.accent : AtlasStyle.alpha(Kirigami.Theme.textColor, 0.14)
                QQC2.Label {
                    anchors.centerIn: parent
                    text: String(screen.index + 1)
                    font.bold: true
                    color: screen.picked ? AtlasStyle.accentText : Kirigami.Theme.textColor
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
            }
            ColumnLayout {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: AtlasStyle.spacingLarge
                width: parent.width - AtlasStyle.spacingLarge * 2
                spacing: 0
                visible: screen.width > Kirigami.Units.gridUnit * 6 && screen.height > Kirigami.Units.gridUnit * 4.5
                QQC2.Label {
                    Layout.fillWidth: true
                    text: screen.modelData.label
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: screen.modelData.resolutionText
                    font.pointSize: AtlasStyle.fontSizeCaption
                    opacity: 0.65
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: screen.modelData.primary && view.screens.length > 1
                    text: qsTr("Main Display")
                    font.pointSize: AtlasStyle.fontSizeCaption
                    color: AtlasStyle.accent
                    horizontalAlignment: Text.AlignHCenter
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
            }

            HoverHandler {
                cursorShape: view.draggable ? (drag.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.PointingHandCursor
            }
            TapHandler {
                onTapped: {
                    screen.forceActiveFocus();
                    view.picked(screen.modelData.id);
                }
            }
            DragHandler {
                id: drag
                enabled: view.draggable
                // Stays inside the area.
                xAxis.minimum: 0
                xAxis.maximum: view.width - screen.width
                yAxis.minimum: 0
                yAxis.maximum: view.height - screen.height
                onActiveChanged: {
                    if (active) {
                        view.picked(screen.modelData.id);
                        return;
                    }
                    landing.shown = false;
                    if (view.backend)
                        view.backend.moveOutput(screen.modelData.id, view.toDesktopX(screen.x), view.toDesktopY(screen.y));
                }
                onTranslationChanged: {
                    if (!active || !view.backend)
                        return;
                    const to = view.backend.snapped(screen.modelData.id, view.toDesktopX(screen.x), view.toDesktopY(screen.y));
                    landing.x = view.originX + to.x * view.zoom;
                    landing.y = view.originY + to.y * view.zoom;
                    landing.width = screen.width;
                    landing.height = screen.height;
                    landing.shown = true;
                }
            }
        }
    }
}
