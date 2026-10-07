pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Appearance: a picture of the desktop that follows the choices below it (and
// previews a Light or Dark card or an accent swatch under the pointer), Light
// or Dark, the accent colour, the wallpaper, transparency and the dock; under
// Advanced the top bar, hot corners, virtual desktops
// and the Global Theme. The colour scheme, the wallpaper and the Global Theme
// are applied by Plasma's own tools (plasma-apply-*), the dock and the top
// bar through Plasma's panel scripting, everything else through the files
// and services Plasma's modules use (cpp/appearanceconfig.h).
SettingsPage {
    id: page

    // cpp/appearanceconfig.h; it goes with the page.
    readonly property var cfg: pageBackends ? pageBackends.create("appearance", page) : null
    // kdeglobals as read: {scheme, dark, highContrast, accent, ...}.
    property var prefs: cfg ? cfg.read() : ({})
    // A choice the tools haven't written yet, so the page answers at once.
    property var pendingDark: null
    property string pendingAccent: ""
    // The wallpaper chosen in the sheet, {id, name, preview, picture,
    // pictureDark}, until Plasma has saved it (it does so a while later).
    property var pendingWallpaper: null
    property string notice: ""
    // What the pointer (or the keyboard focus) is on: a Light or Dark card
    // (false, true), an accent swatch ("" is violet); null for neither. The
    // picture shows it before it is chosen.
    property var hoverDark: null
    property var hoverAccent: null

    readonly property bool dark: pendingDark !== null ? pendingDark : (prefs.dark ?? false)
    readonly property var shell: cfg ? cfg.shell : ({})
    readonly property var dock: shell.dock ?? ({})
    readonly property bool dockFound: (shell.known ?? false) && (dock.found ?? false)
    readonly property var barState: shell.topBar ?? ({})
    // Until the panels have answered, the picture has both.
    readonly property bool showDock: !(shell.known ?? false) || page.dockFound
    readonly property bool showBar: !(shell.known ?? false) || (page.barState.found ?? false)

    // The accent swatches. "" is Atlas violet, the scheme's own accent.
    readonly property var accents: [
        {
            value: "",
            name: qsTr("Violet")
        },
        {
            value: "#3584e4",
            name: qsTr("Blue")
        },
        {
            value: "#1fa5a0",
            name: qsTr("Teal")
        },
        {
            value: "#3fa34d",
            name: qsTr("Green")
        },
        {
            value: "#e5a50a",
            name: qsTr("Yellow")
        },
        {
            value: "#f06d1a",
            name: qsTr("Orange")
        },
        {
            value: "#e5484d",
            name: qsTr("Red")
        },
        {
            value: "#e5487a",
            name: qsTr("Pink")
        },
        {
            value: "#6b7280",
            name: qsTr("Graphite")
        }
    ]
    readonly property string accentValue: {
        if (page.pendingAccent !== "")
            return page.pendingAccent === "default" ? "" : page.pendingAccent;
        return page.prefs.accentIsDefault ? "" : (page.prefs.accent ?? "");
    }
    readonly property bool accentFromWallpaper: page.pendingAccent === "wallpaper" || (page.pendingAccent === "" && (page.prefs.accentFromWallpaper ?? false))
    readonly property string accentName: {
        if (page.prefs.highContrast)
            return qsTr("Set by High Contrast");
        if (page.accentFromWallpaper)
            return qsTr("From Wallpaper");
        const hit = page.accents.find(a => a.value === page.accentValue.toLowerCase());
        return hit ? hit.name : qsTr("Custom");
    }

    // Telamon.Ui's violet in each scheme: the accent when none is chosen.
    readonly property var violet: ({
            "light": "#6858e2", // telamon-lint: allow-raw
            "dark": "#8a7af4" // telamon-lint: allow-raw
        })

    // What the picture of the desktop shows.
    readonly property bool previewDark: page.hoverDark !== null ? page.hoverDark : page.dark
    readonly property string previewAccentValue: page.hoverAccent !== null ? page.hoverAccent : page.accentValue
    readonly property string previewAccent: page.previewAccentValue !== "" ? page.previewAccentValue : (page.previewDark ? page.violet.dark : page.violet.light)
    readonly property var wallpaperNow: page.pendingWallpaper ?? page.prefs.wallpaper ?? ({})
    // The image of the wallpaper for a Light or a Dark desktop (Plasma shows
    // a package's dark images with a dark scheme).
    function wallpaperPicture(dark: bool): string {
        const w = page.wallpaperNow;
        return (dark && w.pictureDark) ? w.pictureDark : (w.picture ?? "");
    }
    // The note under the picture: what it is, or what is being previewed.
    readonly property string previewNote: {
        if (page.hoverDark !== null && page.hoverDark !== page.dark)
            return page.hoverDark ? qsTr("Previewing Dark. Click to use it.") : qsTr("Previewing Light. Click to use it.");
        if (page.hoverAccent !== null && page.hoverAccent.toLowerCase() !== page.accentValue.toLowerCase())
            return qsTr("Previewing %1. Click to use it.").arg(page.accentLabel(page.hoverAccent));
        return qsTr("A preview of your desktop");
    }

    function accentLabel(value: string): string {
        const hit = page.accents.find(a => a.value === value.toLowerCase());
        return hit ? hit.name : qsTr("Custom");
    }

    function dockSummary(): string {
        if (!page.shell.known)
            return "";
        if (!page.dockFound)
            return qsTr("No dock");
        const place = {
            "bottom": qsTr("Bottom"),
            "left": qsTr("Left"),
            "right": qsTr("Right"),
            "top": qsTr("Top")
        }[page.dock.location] ?? "";
        return page.dock.autohide ? qsTr("%1, hides automatically").arg(place) : place;
    }

    function cornerSummary(): string {
        const c = page.cfg ? page.cfg.hotCorners() : ({});
        const set = Object.values(c).filter(a => a !== "none").length;
        return set === 0 ? qsTr("Off") : qsTr("%1 on").arg(set);
    }

    // The Global Theme installed, for the picker.
    function themeChoices(): var {
        return (page.cfg ? page.cfg.lookAndFeels() : []).map(t => ({
                    title: t.name,
                    subtitle: t.description,
                    value: t.id,
                    keys: t.id.toLowerCase()
                }));
    }

    function themeName(id: string): string {
        const hit = (page.cfg ? page.cfg.lookAndFeels() : []).find(t => t.id === id);
        return hit ? hit.name : "";
    }

    Component.onCompleted: if (cfg) cfg.refreshShell()

    Connections {
        target: page.cfg
        function onChanged() {
            const p = page.cfg.read();
            page.prefs = p;
            // A choice stays shown until the files say the same (or a few
            // seconds pass): Plasma's tools save kdeglobals in steps.
            if (page.pendingDark !== null && p.dark === page.pendingDark)
                page.pendingDark = null;
            const want = page.pendingAccent;
            if (page.pendingWallpaper !== null && (p.wallpaper.id ?? "") === page.pendingWallpaper.id)
                page.pendingWallpaper = null;
            if (want === "wallpaper" ? p.accentFromWallpaper : want === "default" ? p.accentIsDefault : want !== "" && !p.accentFromWallpaper && (p.accent ?? "").toLowerCase() === want.toLowerCase())
                page.pendingAccent = "";
        }
        function onRun(argv) {
            page.run(argv);
        }
        function onFailed(message) {
            page.notice = message;
        }
    }

    // A tool that never finishes must not leave a choice shown that isn't
    // in effect.
    Timer {
        id: settle
        interval: 4000
        onTriggered: {
            page.pendingDark = null;
            page.pendingAccent = "";
            if (page.cfg)
                page.prefs = page.cfg.read();
        }
    }

    // Plasma saves its choice of wallpaper some seconds after it is made: until
    // then the picture shows the choice, but not for ever.
    Timer {
        id: wallpaperSettle
        interval: 30000
        onTriggered: page.pendingWallpaper = null
    }

    function chooseWallpaper(w: var) {
        const pictures = page.cfg.wallpaperPictures(w.id);
        page.pendingWallpaper = {
            "id": w.id,
            "name": w.name,
            "preview": w.preview,
            "picture": pictures.picture ?? w.preview,
            "pictureDark": pictures.pictureDark ?? ""
        };
        wallpaperSettle.restart();
        page.cfg.setWallpaper(w.id);
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.notice
        shown: text !== ""
        closable: true
        onClosed: page.notice = ""
    }

    // The desktop, large: what the choices below do, as they are made.
    // Nothing in it is the user's, and it is not a control.
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        DesktopPreview {
            objectName: "desktop-preview"
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 34
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredHeight: Math.round(width * 0.625)
            dark: page.previewDark
            accent: page.previewAccent
            wallpaper: page.wallpaperPicture(page.previewDark)
            translucent: Appearance.effective
            showBar: page.showBar
            showDock: page.showDock
            dockPosition: page.dock.location ?? "bottom"
            dockAutoHide: page.dock.autohide ?? false
            dockSize: page.dock.height ?? 60
        }
        TelamonLabel {
            Layout.alignment: Qt.AlignHCenter
            textStyle: TelamonLabel.Caption
            text: page.previewNote
        }
    }

    // One of the two looks, drawn small: the desktop in that look, in the
    // chosen accent. The pointer or the keyboard focus on it previews it in
    // the large picture.
    component StyleCard: Item {
        id: card

        required property string label
        required property bool light
        required property bool selected
        // The accent shown in the picture.
        property color accent: light ? page.violet.light : page.violet.dark
        // The pointer or the keyboard is on the card.
        readonly property bool previewed: hover.hovered || card.activeFocus

        signal chosen

        Layout.fillWidth: true
        Layout.preferredWidth: 1
        Layout.maximumWidth: Kirigami.Units.gridUnit * 13
        implicitHeight: preview.height + labelRow.height + Kirigami.Units.smallSpacing * 3
        activeFocusOnTab: true
        Accessible.role: Accessible.RadioButton
        Accessible.name: card.label
        Accessible.checked: card.selected
        Accessible.focusable: true
        Accessible.onPressAction: card.chosen()
        Keys.onSpacePressed: card.chosen()
        Keys.onReturnPressed: card.chosen()

        Item {
            id: preview
            width: parent.width
            height: Math.round(width * 0.625)

            DesktopPreview {
                anchors.fill: parent
                dark: !card.light
                accent: card.accent
                wallpaper: page.wallpaperPicture(!card.light)
                translucent: Appearance.effective
                blur: false
                showBar: page.showBar
                showDock: page.showDock
                dockPosition: page.dock.location ?? "bottom"
                dockAutoHide: page.dock.autohide ?? false
                dockSize: page.dock.height ?? 60
            }
            // The ring: the accent when chosen, fainter on hover and focus.
            Rectangle {
                anchors.fill: parent
                anchors.margins: -Kirigami.Units.smallSpacing
                radius: TelamonStyle.radiusLarge + Kirigami.Units.smallSpacing
                color: "transparent"
                border.width: card.selected ? 3 : 2
                border.color: card.selected ? TelamonStyle.accent : (card.previewed ? (card.activeFocus ? TelamonStyle.focus : TelamonStyle.alpha(TelamonStyle.accent, 0.45)) : "transparent")
                Behavior on border.color {
                    ColorAnimation {
                        duration: TelamonStyle.durationShort
                    }
                }
            }
        }

        RowLayout {
            id: labelRow
            anchors.top: preview.bottom
            anchors.topMargin: Kirigami.Units.smallSpacing * 2
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Kirigami.Units.smallSpacing

            Rectangle {
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: implicitWidth
                radius: width / 2
                color: card.selected ? TelamonStyle.accent : "transparent"
                border.width: card.selected ? 0 : 1
                border.color: TelamonStyle.controlBorder
                Symbol {
                    anchors.centerIn: parent
                    visible: card.selected
                    icon: Symbols.Check
                    color: TelamonStyle.accentText
                    size: parent.width * 0.75
                }
            }
            TelamonLabel {
                text: card.label
            }
        }

        HoverHandler {
            id: hover
        }
        TapHandler {
            onTapped: card.chosen()
        }
    }

    // A round colour swatch; a check mark when it is chosen.
    component Swatch: Rectangle {
        id: swatch

        required property color swatchColor
        required property string swatchName
        required property bool selected
        // The pointer or the keyboard is on the swatch.
        readonly property bool previewed: hover.hovered || swatch.activeFocus

        signal chosen

        implicitWidth: Kirigami.Units.gridUnit * 1.8
        implicitHeight: implicitWidth
        radius: width / 2
        color: swatch.swatchColor
        border.width: swatch.selected || swatch.activeFocus ? 2 : 0
        border.color: swatch.activeFocus && !swatch.selected ? TelamonStyle.focus : TelamonStyle.text
        activeFocusOnTab: true
        Accessible.role: Accessible.RadioButton
        Accessible.name: swatch.swatchName
        Accessible.checked: swatch.selected
        Accessible.focusable: true
        Accessible.onPressAction: swatch.chosen()
        Keys.onSpacePressed: swatch.chosen()
        Keys.onReturnPressed: swatch.chosen()

        Symbol {
            anchors.centerIn: parent
            visible: swatch.selected
            icon: Symbols.Check
            color: "white"
            size: parent.width * 0.6
        }
        HoverHandler {
            id: hover
        }
        TelamonToolTip {
            text: swatch.swatchName
            shown: hover.hovered
        }
        TapHandler {
            onTapped: swatch.chosen()
        }
    }

    Section {
        Layout.fillWidth: true

        RowLayout {
            objectName: "style"
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing * 2

            Item {
                Layout.fillWidth: true
            }
            StyleCard {
                label: qsTr("Light")
                light: true
                accent: page.accentValue !== "" ? page.accentValue : page.violet.light
                selected: !page.dark
                onPreviewedChanged: {
                    if (previewed)
                        page.hoverDark = false;
                    else if (page.hoverDark === false)
                        page.hoverDark = null;
                }
                onChosen: {
                    page.pendingDark = false;
                    settle.restart();
                    page.cfg.setDark(false);
                }
            }
            StyleCard {
                label: qsTr("Dark")
                light: false
                accent: page.accentValue !== "" ? page.accentValue : page.violet.dark
                selected: page.dark
                onPreviewedChanged: {
                    if (previewed)
                        page.hoverDark = true;
                    else if (page.hoverDark === true)
                        page.hoverDark = null;
                }
                onChosen: {
                    page.pendingDark = true;
                    settle.restart();
                    page.cfg.setDark(true);
                }
            }
            Item {
                Layout.fillWidth: true
            }
        }
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "accent"
            title: qsTr("Accent Color")
            value: page.accentName
        }
        Flow {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing
            enabled: !(page.prefs.highContrast ?? false)
            opacity: enabled ? 1 : 0.4

            Repeater {
                model: page.accents

                Swatch {
                    required property var modelData
                    swatchColor: modelData.value !== "" ? modelData.value : (page.dark ? page.violet.dark : page.violet.light)
                    swatchName: modelData.name
                    selected: !page.accentFromWallpaper && page.accentValue.toLowerCase() === modelData.value
                    onPreviewedChanged: {
                        if (previewed)
                            page.hoverAccent = modelData.value;
                        else if (page.hoverAccent === modelData.value)
                            page.hoverAccent = null;
                    }
                    onChosen: {
                        page.pendingAccent = modelData.value === "" ? "default" : modelData.value;
                        settle.restart();
                        page.cfg.setAccent(modelData.value);
                    }
                }
            }
        }
        SectionRow {
            objectName: "accent-wallpaper"
            title: qsTr("Match the Wallpaper")
            subtitle: qsTr("Pick the accent from the wallpaper's colors")
            showSwitch: true
            switchChecked: page.accentFromWallpaper
            enabled: !(page.prefs.highContrast ?? false)
            onSwitchToggled: checked => {
                if (checked) {
                    page.pendingAccent = "wallpaper";
                    settle.restart();
                    page.cfg.setAccentFromWallpaper();
                } else {
                    // Back to the accent chosen last, or Atlas violet.
                    page.pendingAccent = "default";
                    settle.restart();
                    page.cfg.setAccent(page.prefs.accent ?? "");
                }
            }
        }
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "wallpaper"
            title: qsTr("Wallpaper")
            value: page.wallpaperNow.name ?? ""
            chevron: true
            enabled: page.cfg !== null
            onClicked: wallpaperSheet.open()

            Rectangle {
                visible: thumb.status === Image.Ready
                implicitWidth: Kirigami.Units.gridUnit * 4
                implicitHeight: Math.round(implicitWidth * 0.6)
                radius: TelamonStyle.radiusSmall
                color: TelamonStyle.control
                border.width: 1
                border.color: TelamonStyle.separator
                Image {
                    id: thumb
                    anchors.fill: parent
                    anchors.margins: 1
                    source: page.wallpaperNow.preview ?? ""
                    sourceSize: Qt.size(160, 100)
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }
            }
        }
        SectionRow {
            objectName: "transparency"
            title: qsTr("Transparency")
            subtitle: Appearance.blurAvailable ? qsTr("Windows and menus are see-through and blur what is behind them") : qsTr("Not available: the desktop's blur effect is off or not supported")
            showSwitch: true
            switchChecked: Appearance.transparency
            enabled: Appearance.blurAvailable
            onSwitchToggled: checked => Appearance.transparency = checked
            Component.onCompleted: Appearance.refresh()
        }
        SectionRow {
            objectName: "dock"
            title: qsTr("Dock")
            value: page.dockSummary()
            chevron: true
            enabled: page.dockFound
            onClicked: dockSheet.open()
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "top-bar"
            title: qsTr("Top Bar")
            subtitle: qsTr("Hide it when a window covers it")
            showSwitch: true
            switchChecked: page.barState.hides ?? false
            enabled: (page.shell.known ?? false) && (page.barState.found ?? false)
            onSwitchToggled: checked => page.cfg.setTopBarHides(checked)
        }
        SectionRow {
            objectName: "hot-corners"
            title: qsTr("Hot Corners")
            subtitle: qsTr("Do something when the pointer reaches a corner")
            value: cornerSheet.summary
            chevron: true
            enabled: page.cfg !== null
            onClicked: cornerSheet.open()
        }
        SectionRow {
            objectName: "desktops"
            title: qsTr("Virtual Desktops")
            subtitle: qsTr("Separate workspaces for your windows")
            enabled: page.cfg !== null && page.cfg.desktopCount > 0

            TelamonSpinBox {
                from: 1
                to: 20
                value: page.cfg && page.cfg.desktopCount > 0 ? page.cfg.desktopCount : 1
                onValueModified: page.cfg.setDesktopCount(value)
                Accessible.name: qsTr("Virtual Desktops")
            }
        }
        SectionRow {
            objectName: "theme"
            title: qsTr("Global Theme")
            subtitle: qsTr("Colors, icons and the look of windows")
            value: page.themeName(page.prefs.lookAndFeel ?? "")
            chevron: true
            enabled: page.cfg !== null
            onClicked: themeSheet.open()
        }
    }

    RelatedLinks {
        page: page
    }

    PickerSheet {
        id: themeSheet
        title: qsTr("Global Theme")
        current: page.prefs.lookAndFeel ?? ""
        choices: page.themeChoices()
        onChosen: value => {
            page.cfg.setLookAndFeel(value);
            settle.restart();
        }
    }

    // The wallpapers installed, and the pictures in the Pictures folder.
    TelamonDialog {
        id: wallpaperSheet
        title: qsTr("Wallpaper")
        preferredWidth: Kirigami.Units.gridUnit * 34
        property var items: []
        onAboutToShow: items = page.cfg ? page.cfg.wallpapers() : []

        TelamonLabel {
            Layout.fillWidth: true
            visible: wallpaperSheet.items.length === 0
            wrapMode: Text.Wrap
            textStyle: TelamonLabel.Caption
            text: qsTr("No wallpapers found. Put pictures in your Pictures folder to choose them here.")
        }
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Repeater {
                model: wallpaperSheet.items

                Item {
                    id: tile
                    required property var modelData
                    readonly property bool current: (page.prefs.wallpaper ? page.prefs.wallpaper.id : "") === modelData.id
                    width: Kirigami.Units.gridUnit * 9.5
                    height: image.height + caption.height + Kirigami.Units.smallSpacing * 2
                    activeFocusOnTab: true
                    Accessible.role: Accessible.RadioButton
                    Accessible.name: modelData.name
                    Accessible.checked: tile.current
                    Accessible.focusable: true
                    Accessible.onPressAction: tile.choose()
                    Keys.onSpacePressed: tile.choose()
                    Keys.onReturnPressed: tile.choose()

                    function choose() {
                        page.chooseWallpaper(modelData);
                        wallpaperSheet.close();
                    }

                    Rectangle {
                        id: image
                        width: parent.width
                        height: Math.round(width * 0.6)
                        radius: TelamonStyle.radius
                        color: TelamonStyle.control
                        border.width: tile.current || tile.activeFocus ? 2 : 1
                        border.color: tile.current ? TelamonStyle.accent : (tile.activeFocus ? TelamonStyle.focus : TelamonStyle.separator)
                        Image {
                            anchors.fill: parent
                            anchors.margins: 2
                            source: tile.modelData.preview
                            sourceSize: Qt.size(320, 200)
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                        }
                    }
                    TelamonLabel {
                        id: caption
                        anchors.top: image.bottom
                        anchors.topMargin: Kirigami.Units.smallSpacing
                        width: parent.width
                        elide: Text.ElideRight
                        horizontalAlignment: Text.AlignHCenter
                        textStyle: TelamonLabel.Caption
                        text: tile.modelData.name
                    }
                    TapHandler {
                        onTapped: tile.choose()
                    }
                }
            }
        }
    }

    // The dock's own choices, a few of Plasma's panel settings.
    TelamonDialog {
        id: dockSheet
        title: qsTr("Dock")
        preferredWidth: Kirigami.Units.gridUnit * 28

        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Hide Automatically")
                subtitle: qsTr("The dock slides away until the pointer reaches its edge")
                showSwitch: true
                switchChecked: page.dock.autohide ?? false
                onSwitchToggled: checked => page.cfg.setDockAutoHide(checked)
            }
            SectionRow {
                title: qsTr("Size")

                TelamonSegmentedControl {
                    model: [qsTr("Small"), qsTr("Medium"), qsTr("Large")]
                    currentIndex: {
                        const h = page.dock.height ?? 60;
                        return h <= 54 ? 0 : (h <= 66 ? 1 : 2);
                    }
                    onActivated: index => page.cfg.setDockSize([48, 60, 72][index])
                    Accessible.name: qsTr("Dock size")
                }
            }
            SectionRow {
                title: qsTr("Position")

                TelamonSegmentedControl {
                    readonly property var places: ["bottom", "left", "right"]
                    model: [qsTr("Bottom"), qsTr("Left"), qsTr("Right")]
                    currentIndex: Math.max(0, places.indexOf(page.dock.location ?? "bottom"))
                    onActivated: index => page.cfg.setDockPosition(places[index])
                    Accessible.name: qsTr("Dock position")
                }
            }
        }
    }

    // What each corner of the screen does.
    TelamonDialog {
        id: cornerSheet
        title: qsTr("Hot Corners")
        preferredWidth: Kirigami.Units.gridUnit * 28

        property var corners: ({})
        readonly property string summary: {
            const set = Object.values(corners).filter(a => a !== "none").length;
            return set === 0 ? qsTr("Off") : qsTr("%1 on").arg(set);
        }
        readonly property var actions: ["none", "overview", "desktop", "lock"]
        readonly property var actionNames: [qsTr("Nothing"), qsTr("Overview"), qsTr("Show Desktop"), qsTr("Lock Screen")]

        function load() {
            corners = page.cfg ? page.cfg.hotCorners() : ({});
        }
        Component.onCompleted: load()
        onAboutToShow: load()

        Section {
            Layout.fillWidth: true

            Repeater {
                model: [
                    {
                        corner: "topLeft",
                        name: qsTr("Top Left")
                    },
                    {
                        corner: "topRight",
                        name: qsTr("Top Right")
                    },
                    {
                        corner: "bottomLeft",
                        name: qsTr("Bottom Left")
                    },
                    {
                        corner: "bottomRight",
                        name: qsTr("Bottom Right")
                    }
                ]

                SectionRow {
                    id: cornerRow
                    required property var modelData
                    title: modelData.name

                    TelamonComboBox {
                        width: Kirigami.Units.gridUnit * 11
                        model: cornerSheet.actionNames
                        currentIndex: Math.max(0, cornerSheet.actions.indexOf(cornerSheet.corners[cornerRow.modelData.corner] ?? "none"))
                        onActivated: index => {
                            page.cfg.setHotCorner(cornerRow.modelData.corner, cornerSheet.actions[index]);
                            cornerSheet.load();
                        }
                        Accessible.name: cornerRow.modelData.name
                    }
                }
            }
        }
    }
}
