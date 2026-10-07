pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Accessibility: text size, high contrast, reduced motion, the screen reader
// and zoom; under Advanced sticky keys and the other keyboard and pointer
// helps. Plasma's own settings (cpp/accessibilityconfig.h); high contrast is
// a colour scheme, which the Appearance backend applies.
SettingsPage {
    id: page

    // cpp/accessibilityconfig.h and cpp/appearanceconfig.h; they go with the
    // page.
    readonly property var cfg: pageBackends ? pageBackends.create("accessibility", page) : null
    readonly property var look: pageBackends ? pageBackends.create("appearance", page) : null
    property var prefs: cfg ? cfg.read() : ({})
    property var lookPrefs: look ? look.read() : ({})
    property bool highContrast: lookPrefs.highContrast ?? false
    property string notice: ""

    // The size shown while the slider moves; the one saved is `prefs`.
    property real wanted: prefs.textScale ?? 1.0

    function reread() {
        page.prefs = page.cfg.read();
    }

    function saved(ok: bool) {
        if (!ok)
            page.notice = qsTr("Settings couldn't save the accessibility settings.");
        page.reread();
    }

    Connections {
        target: page.cfg
        function onRun(argv) {
            page.run(argv);
        }
    }
    Connections {
        target: page.look
        function onRun(argv) {
            page.run(argv);
        }
        function onChanged() {
            page.lookPrefs = page.look.read();
            page.highContrast = page.lookPrefs.highContrast ?? false;
        }
        function onFailed(message) {
            page.notice = message;
        }
    }

    // The slider saves a moment after it stops moving: every size change
    // redraws every app.
    Timer {
        id: sizeTimer
        interval: 350
        onTriggered: page.saved(page.cfg.setTextScale(page.wanted))
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.notice
        shown: text !== ""
        closable: true
        onClosed: page.notice = ""
    }

    Section {
        Layout.fillWidth: true

        SectionRow {
            objectName: "text-size"
            title: qsTr("Text Size")
            subtitle: qsTr("Makes text in apps and the desktop larger or smaller")
            value: qsTr("%1%").arg(Math.round(page.wanted * 100))
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing * 1.5
            spacing: Kirigami.Units.largeSpacing

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.largeSpacing

                TelamonLabel {
                    text: qsTr("A")
                    font.pointSize: TelamonStyle.fontSizeBody * 0.85
                    textStyle: TelamonLabel.Caption
                }
                TelamonSlider {
                    Layout.fillWidth: true
                    from: 0.8
                    to: 2.0
                    stepSize: 0.05
                    value: page.prefs.textScale ?? 1.0
                    onMoved: {
                        page.wanted = Math.round(value * 20) / 20;
                        sizeTimer.restart();
                    }
                    Accessible.name: qsTr("Text Size")
                }
                TelamonLabel {
                    text: qsTr("A")
                    font.pointSize: TelamonStyle.fontSizeBody * 1.6
                }
            }
            // The chosen size next to the one in use now.
            TelamonLabel {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("The quick brown fox jumps over the lazy dog.")
                font.pointSize: TelamonStyle.fontSizeBody * page.wanted / Math.max(0.5, page.prefs.textScale ?? 1.0)
            }
        }
        SectionRow {
            objectName: "contrast"
            title: qsTr("High Contrast")
            subtitle: qsTr("Stronger colors that are easier to tell apart")
            showSwitch: true
            switchChecked: page.highContrast
            enabled: page.look !== null
            onSwitchToggled: checked => {
                page.highContrast = checked;
                page.look.setHighContrast(checked);
            }
        }
        SectionRow {
            objectName: "reduce-motion"
            title: qsTr("Reduce Motion")
            subtitle: qsTr("Turns off animations on the desktop and in apps")
            showSwitch: true
            switchChecked: page.prefs.reduceMotion ?? false
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setReduceMotion(checked))
        }
        SectionRow {
            objectName: "screen-reader"
            title: qsTr("Screen Reader")
            subtitle: (page.prefs.orcaInstalled ?? true) ? qsTr("Reads what is on the screen aloud, with Orca") : qsTr("Orca, the screen reader, isn't installed")
            showSwitch: true
            switchChecked: page.prefs.screenReader ?? false
            enabled: page.cfg !== null && (page.prefs.orcaInstalled ?? false)
            onSwitchToggled: checked => page.saved(page.cfg.setScreenReader(checked))
        }
        SectionRow {
            objectName: "zoom"
            title: qsTr("Zoom")
            subtitle: qsTr("Magnify the screen with Meta and the plus and minus keys")
            showSwitch: true
            switchChecked: page.prefs.zoom ?? true
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setZoom(checked))
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "sticky-keys"
            title: qsTr("Sticky Keys")
            subtitle: qsTr("Press Shift, Ctrl, Alt and Meta one at a time instead of together")
            showSwitch: true
            switchChecked: page.prefs.sticky ?? false
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("sticky", checked))
        }
        SectionRow {
            title: qsTr("Lock with a Double Press")
            subtitle: qsTr("Press a modifier key twice to keep it held")
            showSwitch: true
            switchChecked: page.prefs.stickyLock ?? true
            enabled: page.cfg !== null && (page.prefs.sticky ?? false)
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("stickyLock", checked))
        }
        SectionRow {
            title: qsTr("Turn Off with Two Keys")
            subtitle: qsTr("Pressing two keys at once turns Sticky Keys off")
            showSwitch: true
            switchChecked: page.prefs.stickyOff ?? false
            enabled: page.cfg !== null && (page.prefs.sticky ?? false)
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("stickyOff", checked))
        }
        SectionRow {
            title: qsTr("Beep for Modifier Keys")
            subtitle: qsTr("A sound when a modifier key is held or let go")
            showSwitch: true
            switchChecked: page.prefs.stickyBeep ?? false
            enabled: page.cfg !== null && (page.prefs.sticky ?? false)
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("stickyBeep", checked))
        }
        SectionRow {
            title: qsTr("Mouse Keys")
            subtitle: qsTr("Move the pointer with the number pad")
            showSwitch: true
            switchChecked: page.prefs.mouseKeys ?? false
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("mouseKeys", checked))
        }
        SectionRow {
            title: qsTr("Shake to Find the Pointer")
            subtitle: qsTr("Shake the mouse to make the pointer bigger for a moment")
            showSwitch: true
            switchChecked: page.prefs.shake ?? true
            enabled: page.cfg !== null
            onSwitchToggled: checked => page.saved(page.cfg.setFlag("shake", checked))
        }
    }

    RelatedLinks {
        page: page
    }
}
