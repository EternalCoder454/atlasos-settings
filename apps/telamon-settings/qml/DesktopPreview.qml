pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Effects
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import Telamon.Ui

// A picture of the Telamon OS desktop that follows the choices made for it:
// the wallpaper, the menu bar's islands, a Telamon.Ui window in Light or Dark
// and the accent colour, the dock, and the window, bar and dock either
// translucent over a blurred wallpaper or opaque. It is drawn with shapes (so
// it is sharp at any scale) over the wallpaper's image; it shows nothing of
// the user's: the window is a made-up settings window, the dock holds generic
// theme icons, and nothing in it reacts to the pointer.
//
// The colours are the ones TelamonStyle derives from the TelamonLight and
// TelamonDark schemes (the window colour, tinted toward violet, and its tonal
// steps), whichever scheme the app itself is in, and `accent` is the accent
// colour. The picture is 16:10, laid out in 160 x 100 units, every edge
// snapped to a device pixel. It started as Telamon Setup's WizardThemePreview;
// a candidate for Telamon.Ui, with the wallpaper, the dock and the blur as
// options of it.
Item {
    id: scene

    property bool dark: false
    property color accent: "#6858E2" // telamon-lint: allow-raw
    // The wallpaper's image (a file URL); "" draws Telamon OS's sky in colours
    // instead, which also shows while the image loads.
    property url wallpaper
    // The window, the bar and the dock are see-through over the wallpaper
    // (blurred where the graphics allow it), or opaque.
    property bool translucent: true
    // Blur behind the see-through parts. Off for the small pictures.
    property bool blur: true
    // The top bar.
    property bool showBar: true
    // The dock: where ("bottom", "left", "right" or "top"), whether it hides
    // until the pointer reaches its edge, and its thickness in pixels as
    // Plasma's panel has it (48, 60 or 72).
    property bool showDock: true
    property string dockPosition: "bottom"
    property bool dockAutoHide: false
    property real dockSize: 60

    // TelamonLight.colors and TelamonDark.colors: Window background and text.
    // They change with a short fade (none under reduced motion), and the
    // steps below follow.
    property color _window: scene.dark ? "#211E38" : "#F3F2FA" // telamon-lint: allow-raw
    property color _fg: scene.dark ? "#EEECFA" : "#1B1748" // telamon-lint: allow-raw
    property color _tint: scene.dark ? "#8A7AF4" : "#6858E2" // telamon-lint: allow-raw
    property color _accent: scene.accent
    Behavior on _window {
        ColorAnimation {
            duration: TelamonStyle.durationShort
        }
    }
    Behavior on _fg {
        ColorAnimation {
            duration: TelamonStyle.durationShort
        }
    }
    Behavior on _tint {
        ColorAnimation {
            duration: TelamonStyle.durationShort
        }
    }
    Behavior on _accent {
        ColorAnimation {
            duration: TelamonStyle.durationShort
        }
    }
    readonly property color _white: "#FFFFFF" // telamon-lint: allow-raw
    // The same steps as TelamonStyle's, from those two colours.
    readonly property color _base: TelamonStyle.mix(scene._window, scene._tint, scene.dark ? 0.06 : 0.045)
    readonly property color _surface: scene.dark ? TelamonStyle.mix(scene._base, scene._white, 0.045) : TelamonStyle.mix(scene._base, scene._white, 0.6)
    readonly property color _raised: scene.dark ? TelamonStyle.mix(scene._base, scene._white, 0.085) : TelamonStyle.mix(scene._base, scene._white, 0.85)
    readonly property color _control: scene.dark ? TelamonStyle.mix(scene._base, scene._white, 0.065) : TelamonStyle.mix(scene._base, scene._fg, 0.055)
    readonly property color _line: TelamonStyle.alpha(scene._fg, 0.08)
    readonly property color _edge: TelamonStyle.alpha(scene._fg, 0.22)
    readonly property color _muted: TelamonStyle.alpha(scene._fg, 0.55)
    readonly property color _onAccent: {
        const c = scene._accent;
        return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b > 0.6 ? "#14121F" : "#FFFFFF"; // telamon-lint: allow-raw
    }

    // How much of the wallpaper shows through the window, the bar and the dock.
    readonly property real _winAlpha: scene.translucent ? (scene.dark ? 0.7 : 0.74) : 1
    readonly property real _barAlpha: scene.translucent ? 0.66 : 1
    readonly property real _dockAlpha: scene.translucent ? (scene.dark ? 0.5 : 0.58) : 1
    // The software renderer draws no MultiEffect: the see-through parts are
    // tinted and the wallpaper has square corners, as in Telamon.Ui.
    readonly property bool _effects: GraphicsInfo.api !== GraphicsInfo.Software && !TelamonStyle.softwareRendering
    readonly property bool _blurred: scene.translucent && scene.blur && scene._effects

    // Units to pixels, kept on the device's pixel grid.
    readonly property real _u: width / 160
    readonly property real _dpr: Screen.devicePixelRatio > 0 ? Screen.devicePixelRatio : 1
    // A hairline: one device pixel (two from 1.5x up, where one is too faint).
    readonly property real _hair: Math.max(1, Math.round(scene._dpr)) / scene._dpr
    function px(v: real): real {
        return Math.round(v * scene._u * scene._dpr) / scene._dpr;
    }

    // The dock's size in units and where it sits.
    readonly property bool _dockVertical: scene.dockPosition === "left" || scene.dockPosition === "right"
    readonly property real _dockT: Math.max(8.5, Math.min(15, 11.5 * scene.dockSize / 60))
    readonly property real _dockIcon: scene._dockT - 4.2
    readonly property real _dockPitch: scene._dockIcon + 2.2
    readonly property int _dockCount: 6
    readonly property real _dockLength: scene._dockCount * scene._dockPitch + 2
    // A dock above the window pushes the window down.
    readonly property real _winDy: scene.showDock && scene.dockPosition === "top" ? Math.max(0, 12.5 + scene._dockT + 3.5 - 17) : 0

    // The wallpaper, which the blur behind the see-through parts copies.
    readonly property Item _wall: wall

    implicitWidth: 160
    implicitHeight: 100
    Accessible.ignored: true

    // A soft round glow: a radial gradient that fades out before its edge.
    component Glow: Shape {
        id: glow
        property color color: scene._white
        property real cx
        property real cy
        property real reach
        x: scene.px(cx - reach)
        y: scene.px(cy - reach)
        width: scene.px(reach * 2)
        height: width
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: -1
            fillGradient: RadialGradient {
                centerX: glow.width / 2
                centerY: glow.height / 2
                centerRadius: glow.width / 2
                focalX: centerX
                focalY: centerY
                GradientStop {
                    position: 0
                    color: glow.color
                }
                GradientStop {
                    position: 1
                    color: TelamonStyle.alpha(glow.color, 0)
                }
            }
            startX: 0
            startY: 0
            PathLine {
                x: glow.width
                y: 0
            }
            PathLine {
                x: glow.width
                y: glow.height
            }
            PathLine {
                x: 0
                y: glow.height
            }
        }
    }
    // A rounded bar or block in scene units.
    component Block: Rectangle {
        property real ux
        property real uy
        property real uw
        property real uh
        property real ur: 0
        x: scene.px(ux)
        y: scene.px(uy)
        width: scene.px(uw)
        height: scene.px(uh)
        radius: ur < 0 ? height / 2 : Math.min(scene.px(ur), height / 2)
        antialiasing: true
    }
    // The wallpaper behind a see-through block, blurred: a copy of the part of
    // the wallpaper under it. It goes under the block's own tint.
    component Glass: Loader {
        id: glass
        // The block's place, in pixels (the scene's).
        required property real gx
        required property real gy
        required property real gw
        required property real gh
        required property real gr
        x: gx
        y: gy
        width: gw
        height: gh
        active: scene._blurred && glass.visible && gw > 0 && gh > 0
        sourceComponent: Item {
            Rectangle {
                id: shape
                anchors.fill: parent
                radius: glass.gr
                layer.enabled: true
                visible: false
            }
            MultiEffect {
                anchors.fill: parent
                source: ShaderEffectSource {
                    sourceItem: scene._wall
                    sourceRect: Qt.rect(glass.gx, glass.gy, glass.gw, glass.gh)
                }
                blurEnabled: true
                blurMax: 32
                blur: Math.min(1, Math.max(0.05, scene.width / 2000))
                maskEnabled: true
                maskSource: shape
            }
        }
    }

    // The wallpaper: the sky in colours (Telamon OS's cherry tree, as soft
    // glows) with the image over it once it has loaded.
    Item {
        id: wall
        anchors.fill: parent
        layer.enabled: scene._effects
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: wallMask
        }
        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop {
                    position: 0
                    color: scene.dark ? "#171548" : "#86AECF" // telamon-lint: allow-raw
                }
                GradientStop {
                    position: 0.7
                    color: scene.dark ? "#4A2263" : "#DDB0C4" // telamon-lint: allow-raw
                }
                GradientStop {
                    position: 1
                    color: scene.dark ? "#2A1038" : "#C98AA6" // telamon-lint: allow-raw
                }
            }
            Glow {
                color: scene.dark ? "#E8559A" : "#F2658F" // telamon-lint: allow-raw
                cx: 52
                cy: 42
                reach: 42
            }
            Glow {
                color: scene.dark ? "#F7A57C" : "#FBC9A0" // telamon-lint: allow-raw
                cx: 126
                cy: 32
                reach: 32
            }
            Glow {
                color: scene.dark ? "#B24CC4" : "#F58FB4" // telamon-lint: allow-raw
                cx: 96
                cy: 66
                reach: 34
            }
        }
        Image {
            anchors.fill: parent
            // Decoded at about the size shown (in steps, so resizing does not
            // decode again at every pixel), not the file's.
            readonly property real step: 128
            source: scene.width > 0 ? scene.wallpaper : ""
            sourceSize: Qt.size(Math.ceil(scene.width * scene._dpr / step) * step, Math.ceil(scene.height * scene._dpr / step) * step)
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            retainWhileLoading: true
            cache: false
            Accessible.ignored: true
        }
    }
    Rectangle {
        id: wallMask
        anchors.fill: parent
        radius: TelamonStyle.radiusLarge
        layer.enabled: true
        visible: false
    }

    // The menu bar: three islands, each as wide as what it holds.
    Repeater {
        model: scene.showBar ? [
            {
                x: 4,
                w: 33
            },
            {
                x: 66,
                w: 28
            },
            {
                x: 123,
                w: 33
            }
        ] : []
        Item {
            id: island
            required property var modelData
            required property int index
            Glass {
                gx: scene.px(island.modelData.x)
                gy: scene.px(3)
                gw: scene.px(island.modelData.w)
                gh: scene.px(7)
                gr: gh / 2
            }
            Block {
                ux: island.modelData.x
                uy: 3
                uw: island.modelData.w
                uh: 7
                ur: -1
                color: TelamonStyle.alpha(scene._raised, scene._barAlpha)
                border.width: scene.translucent ? 0 : scene._hair
                border.color: scene._line
            }
            // The Telamon OS menu, then the open app's menus.
            Block {
                visible: island.index === 0
                ux: island.modelData.x + 3
                uy: 4.5
                uw: 4
                uh: 4
                ur: -1
                color: scene._accent
            }
            Repeater {
                model: island.index === 0 ? 3 : (island.index === 1 ? 1 : 4)
                Block {
                    id: tick
                    required property int index
                    ux: island.index === 0 ? island.modelData.x + 10 + tick.index * 7.5 : (island.index === 1 ? island.modelData.x + 7 : island.modelData.x + 6 + tick.index * 6.5)
                    uy: island.index === 2 ? 5.0 : 5.6
                    uw: island.index === 0 ? 5.5 : (island.index === 1 ? 14 : 3.2)
                    uh: island.index === 2 ? 3.2 : 1.8
                    ur: -1
                    color: scene._muted
                }
            }
        }
    }

    // The window: its colour, the header bar and the sidebar (their corners
    // follow the window's), the page, then the outline on top.
    Glass {
        gx: scene.px(24)
        gy: scene.px(17 + scene._winDy)
        gw: scene.px(100)
        gh: scene.px(60)
        gr: scene.px(2.6)
    }
    Item {
        id: win
        y: scene.px(scene._winDy)

        Block {
            ux: 24
            uy: 17
            uw: 100
            uh: 60
            ur: 2.6
            color: TelamonStyle.alpha(scene._base, scene._winAlpha)
        }
        Block {
            ux: 24
            uy: 17
            uw: 100
            uh: 7
            ur: 2.6
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.55 : 1)
        }
        Block {
            ux: 24
            uy: 20.5
            uw: 100
            uh: 3.5
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.55 : 1)
        }
        Block {
            ux: 24
            uy: 24
            uw: 27
            uh: 53
            ur: 2.6
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.55 : 1)
        }
        Block {
            ux: 24
            uy: 24
            uw: 27
            uh: 4
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.55 : 1)
        }
        Block {
            ux: 47
            uy: 24
            uw: 4
            uh: 53
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.55 : 1)
        }
        Rectangle {
            x: scene.px(24)
            y: scene.px(24)
            width: scene.px(100)
            height: scene._hair
            color: scene._line
        }
        Rectangle {
            x: scene.px(51)
            y: scene.px(24)
            width: scene._hair
            height: scene.px(53)
            color: scene._line
        }
        // Header: title and window buttons.
        Block {
            ux: 66
            uy: 19.1
            uw: 18
            uh: 1.8
            ur: -1
            color: scene._fg
            opacity: 0.8
        }
        Repeater {
            model: 3
            Block {
                required property int index
                ux: 107 + index * 4.4
                uy: 18.7
                uw: 2.6
                uh: 2.6
                ur: -1
                color: index === 2 ? TelamonStyle.alpha(scene._fg, 0.28) : scene._muted
            }
        }
        // Sidebar: the chosen row, then four more.
        Block {
            ux: 27
            uy: 27
            uw: 21
            uh: 5.6
            ur: 1.2
            color: TelamonStyle.alpha(scene._accent, scene.dark ? 0.24 : 0.16)
        }
        Block {
            ux: 30
            uy: 29
            uw: 4
            uh: 1.8
            ur: -1
            color: scene._accent
        }
        Block {
            ux: 36
            uy: 29
            uw: 9
            uh: 1.8
            ur: -1
            color: scene._fg
        }
        Repeater {
            model: 4
            Item {
                id: navRow
                required property int index
                Block {
                    ux: 30
                    uy: 36.6 + navRow.index * 5.8
                    uw: 4
                    uh: 1.8
                    ur: -1
                    color: scene._muted
                }
                Block {
                    ux: 36
                    uy: 36.6 + navRow.index * 5.8
                    uw: 7 + (navRow.index % 2) * 2.5
                    uh: 1.8
                    ur: -1
                    color: scene._muted
                }
            }
        }
        // Page title and a section of three rows.
        Block {
            ux: 56
            uy: 28
            uw: 30
            uh: 2.6
            ur: -1
            color: scene._fg
            opacity: 0.9
        }
        Rectangle {
            id: card
            x: scene.px(56)
            y: scene.px(34)
            width: scene.px(64)
            height: scene.px(22.5)
            radius: scene.px(1.6)
            color: TelamonStyle.alpha(scene._surface, scene.translucent ? 0.7 : 1)
            border.width: scene._hair
            border.color: scene._line
            Repeater {
                model: 3
                Item {
                    id: row
                    required property int index
                    Rectangle {
                        visible: row.index > 0
                        x: scene.px(3)
                        y: scene.px(row.index * 7.5)
                        width: card.width - 2 * scene.px(3)
                        height: scene._hair
                        color: scene._line
                    }
                    Block {
                        x: scene.px(3)
                        y: scene.px(row.index * 7.5 + 2.8)
                        uw: 20 + row.index * 4
                        uh: 1.8
                        ur: -1
                        color: scene._fg
                        opacity: 0.8
                    }
                    // A switch: on for the first row.
                    Block {
                        x: card.width - scene.px(3 + 7)
                        y: scene.px(row.index * 7.5 + 1.9)
                        uw: 7
                        uh: 3.6
                        ur: -1
                        color: row.index === 0 ? scene._accent : scene._control
                        border.width: row.index === 0 ? 0 : scene._hair
                        border.color: scene._edge
                        Rectangle {
                            width: parent.height - scene.px(1)
                            height: width
                            radius: width / 2
                            y: (parent.height - height) / 2
                            x: row.index === 0 ? parent.width - width - scene.px(0.5) : scene.px(0.5)
                            color: row.index === 0 ? scene._onAccent : scene._muted
                        }
                    }
                }
            }
        }
        // A primary and a default button.
        Block {
            ux: 56
            uy: 61
            uw: 17
            uh: 5.6
            ur: 1.2
            color: scene._accent
            Block {
                x: (parent.width - width) / 2
                y: (parent.height - height) / 2
                uw: 8.5
                uh: 1.6
                ur: -1
                color: scene._onAccent
            }
        }
        Block {
            ux: 75
            uy: 61
            uw: 14
            uh: 5.6
            ur: 1.2
            color: scene._control
            border.width: scene._hair
            border.color: scene._edge
            Block {
                x: (parent.width - width) / 2
                y: (parent.height - height) / 2
                uw: 7
                uh: 1.6
                ur: -1
                color: scene._fg
                opacity: 0.8
            }
        }
        // The outline.
        Block {
            ux: 24
            uy: 17
            uw: 100
            uh: 60
            ur: 2.6
            color: "transparent"
            border.width: scene._hair
            border.color: scene._edge
        }
    }

    // The dock: generic icons of the icon theme. Hidden (autohide), only a
    // slim handle shows at its edge.
    Item {
        id: dock
        readonly property real len: scene._dockLength
        readonly property real thick: scene._dockT
        // The dock's box, in units.
        readonly property real bx: scene.dockPosition === "left" ? 3.5 : (scene.dockPosition === "right" ? 160 - 3.5 - thick : (160 - len) / 2)
        readonly property real by: scene._dockVertical ? (100 - len) / 2 : (scene.dockPosition === "top" ? 12.5 : 100 - 4.5 - thick)
        readonly property real bw: scene._dockVertical ? thick : len
        readonly property real bh: scene._dockVertical ? len : thick
        visible: scene.showDock

        // Fades between shown and hidden (not at all under reduced motion).
        Item {
            id: shown
            opacity: scene.dockAutoHide ? 0 : 1
            visible: opacity > 0
            Behavior on opacity {
                NumberAnimation {
                    duration: TelamonStyle.durationShort
                }
            }
            Glass {
                gx: scene.px(dock.bx)
                gy: scene.px(dock.by)
                gw: scene.px(dock.bw)
                gh: scene.px(dock.bh)
                gr: scene.px(4)
            }
            Block {
                ux: dock.bx
                uy: dock.by
                uw: dock.bw
                uh: dock.bh
                ur: 4
                color: TelamonStyle.alpha(scene._raised, scene._dockAlpha)
                border.width: scene._hair
                border.color: TelamonStyle.alpha(scene._fg, scene.dark ? 0.16 : 0.1)
            }
            Repeater {
                model: ["system-file-manager", "internet-web-browser", "utilities-terminal", "accessories-text-editor", "applications-multimedia", "preferences-system"]
                Rectangle {
                    id: app
                    required property string modelData
                    required property int index
                    readonly property real along: 1 + app.index * scene._dockPitch
                    x: scene.px(dock.bx + (scene._dockVertical ? (dock.thick - scene._dockIcon) / 2 : app.along))
                    y: scene.px(dock.by + (scene._dockVertical ? app.along : (dock.thick - scene._dockIcon) / 2))
                    width: scene.px(scene._dockIcon)
                    height: width
                    radius: scene.px(2)
                    // Until the icon theme answers (or when it has no such icon).
                    color: icon.valid ? "transparent" : scene._control
                    Kirigami.Icon {
                        id: icon
                        anchors.fill: parent
                        source: app.modelData
                        roundToIconSize: false
                        animated: false
                        Accessible.ignored: true
                    }
                }
            }
        }
        // The handle of a hidden dock.
        Block {
            opacity: 1 - shown.opacity
            visible: opacity > 0
            ux: scene._dockVertical ? (scene.dockPosition === "left" ? 1.6 : 160 - 1.6 - 1.2) : (160 - 16) / 2
            uy: scene._dockVertical ? (100 - 16) / 2 : (scene.dockPosition === "top" ? 12.5 : 100 - 1.6 - 1.2)
            uw: scene._dockVertical ? 1.2 : 16
            uh: scene._dockVertical ? 16 : 1.2
            ur: -1
            color: TelamonStyle.alpha(scene._white, 0.7)
        }
    }

    // The edge of the picture.
    Rectangle {
        anchors.fill: parent
        radius: TelamonStyle.radiusLarge
        color: "transparent"
        border.width: scene._hair
        border.color: TelamonStyle.alpha(scene._fg, scene.dark ? 0.3 : 0.2)
    }
}
