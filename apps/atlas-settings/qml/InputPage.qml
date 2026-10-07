pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Keyboard & Mouse: the input sources (keyboard layouts), the pointer's
// speed, scrolling and tapping, and the keyboard shortcuts; under Advanced
// the primary button, key repeat, Num Lock and the on-screen keyboard.
// Layouts are Plasma's kxkbrc, key repeat and Num Lock its kcminputrc, and
// the pointers are KWin's own input devices (cpp/inputconfig.h).
SettingsPage {
    id: page

    // cpp/inputconfig.h; it goes with the page.
    readonly property var cfg: pageBackends ? pageBackends.create("input", page) : null
    // The layouts in use, [{id, layout, variant}], and the keyboard's own
    // settings.
    property var layouts: cfg ? cfg.layouts() : []
    property var keyboard: cfg ? cfg.keyboard() : ({})
    property string notice: ""
    // KWin has had a moment to answer.
    property bool waited: false

    readonly property var devices: cfg ? cfg.devices : ({})
    readonly property bool havePointers: (devices.known ?? false) && (devices.pointers ?? 0) > 0

    // Names of the layouts, from xkeyboard-config (read when first needed).
    property var catalog: ({})

    function loadCatalog() {
        if (!cfg || Object.keys(catalog).length > 0)
            return;
        const map = {};
        for (const l of cfg.availableLayouts())
            map[l.id] = l.name;
        catalog = map;
    }

    function layoutName(id: string): string {
        return catalog[id] ?? id;
    }

    function layoutSummary(): string {
        if (page.layouts.length === 0)
            return qsTr("Default");
        const first = page.layoutName(page.layouts[0].id);
        return page.layouts.length > 1 ? qsTr("%1 and %2 more").arg(first).arg(page.layouts.length - 1) : first;
    }

    // "Delay 600 ms, 25 per second", or "Default".
    function repeatSummary(): string {
        const d = page.keyboard.delay ?? 600;
        const r = page.keyboard.rate ?? 25;
        if (d === 600 && Math.round(r) === 25)
            return qsTr("Default");
        return qsTr("%1 ms, %2 per second").arg(d).arg(Math.round(r));
    }

    // Changes the layout list and reads it back.
    function setLayouts(ids: var) {
        if (!page.cfg.setLayouts(ids))
            page.notice = qsTr("Settings couldn't save the input sources.");
        page.layouts = page.cfg.layouts();
    }

    function moveLayout(index: int, by: int) {
        const ids = page.layouts.map(l => l.id);
        const to = index + by;
        if (to < 0 || to >= ids.length)
            return;
        ids.splice(to, 0, ids.splice(index, 1)[0]);
        page.setLayouts(ids);
    }

    Component.onCompleted: {
        if (!cfg)
            return;
        cfg.refresh();
        loadCatalog();
    }

    Timer {
        interval: 800
        running: true
        onTriggered: page.waited = true
    }

    // The pointer's speed, sent a moment after the slider stops moving.
    Timer {
        id: speedTimer
        property real pending: 0
        interval: 120
        onTriggered: page.cfg.setSpeed(pending)
    }

    Connections {
        target: page.cfg
        function onFailed(message) {
            page.notice = message;
        }
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
        title: qsTr("Keyboard")

        SectionRow {
            objectName: "layouts"
            title: qsTr("Input Sources")
            subtitle: qsTr("Keyboard layouts and languages you type in")
            value: page.layoutSummary()
            chevron: true
            enabled: page.cfg !== null
            onClicked: layoutSheet.open()
        }
        KcmRow {
            objectName: "shortcuts"
            title: qsTr("Keyboard Shortcuts")
            kcm: "kcm_keys"
            onOpenKcm: name => page.openKcm(name)
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Mouse and Touchpad")
        visible: page.havePointers || page.waited

        SectionRow {
            visible: !page.havePointers
            title: qsTr("No mouse or touchpad found")
            subtitle: qsTr("Connect one and its settings appear here")
        }
        SectionRow {
            objectName: "speed"
            visible: page.havePointers && (page.devices.canSpeed ?? false)
            title: qsTr("Pointer Speed")

            AtlasSlider {
                from: -1
                to: 1
                stepSize: 0.1
                implicitWidth: Kirigami.Units.gridUnit * 12
                value: page.devices.speed ?? 0
                onMoved: {
                    speedTimer.pending = value;
                    speedTimer.restart();
                }
                Accessible.name: qsTr("Pointer Speed")
            }
        }
        SectionRow {
            objectName: "scrolling"
            visible: page.havePointers && (page.devices.canNatural ?? false)
            title: qsTr("Natural Scrolling")
            subtitle: qsTr("Content follows your fingers, as on a phone")
            showSwitch: true
            switchChecked: page.devices.naturalScroll ?? false
            onSwitchToggled: checked => page.cfg.setNaturalScroll(checked)
        }
        SectionRow {
            objectName: "tap"
            visible: page.havePointers && (page.devices.canTap ?? false)
            title: qsTr("Tap to Click")
            subtitle: qsTr("Tap the touchpad instead of pressing it")
            showSwitch: true
            switchChecked: page.devices.tap ?? false
            onSwitchToggled: checked => page.cfg.setTapToClick(checked)
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "primary"
            visible: page.havePointers && (page.devices.canLeftHanded ?? false)
            title: qsTr("Primary Button")
            subtitle: qsTr("Which mouse button clicks")

            AtlasSegmentedControl {
                model: [qsTr("Left"), qsTr("Right")]
                currentIndex: (page.devices.leftHanded ?? false) ? 1 : 0
                onActivated: index => page.cfg.setLeftHanded(index === 1)
                Accessible.name: qsTr("Primary Button")
            }
        }
        SectionRow {
            objectName: "repeat"
            title: qsTr("Key Repeat")
            subtitle: qsTr("How soon and how fast a held key repeats")
            value: page.repeatSummary()
            chevron: true
            enabled: page.cfg !== null
            onClicked: repeatSheet.open()
        }
        SectionRow {
            objectName: "numlock"
            title: qsTr("Num Lock on Startup")
            subtitle: qsTr("When you sign in")

            AtlasComboBox {
                readonly property var stateIds: ["on", "off", "keep"]
                width: Kirigami.Units.gridUnit * 10
                model: [qsTr("On"), qsTr("Off"), qsTr("Don't Change")]
                currentIndex: Math.max(0, stateIds.indexOf(page.keyboard.numLock ?? "keep"))
                onActivated: index => {
                    if (!page.cfg.setNumLock(stateIds[index]))
                        page.notice = qsTr("Settings couldn't save Num Lock.");
                    page.keyboard = page.cfg.keyboard();
                }
                Accessible.name: qsTr("Num Lock on Startup")
            }
        }
    }

    RelatedLinks {
        page: page
    }

    // The layouts in use, in the order they switch, and the way to add one.
    AtlasDialog {
        id: layoutSheet
        title: qsTr("Input Sources")
        preferredWidth: Kirigami.Units.gridUnit * 28
        footerContent: [
            SecondaryButton {
                text: qsTr("Done")
                onClicked: layoutSheet.close()
            },
            PrimaryButton {
                text: qsTr("Add…")
                enabled: page.layouts.length < 4
                onClicked: addSheet.open()
            }
        ]

        AtlasLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            textStyle: AtlasLabel.Caption
            text: page.layouts.length > 1 ? qsTr("Switch between them with Meta+Space. The first one is used at sign-in.") : qsTr("Add a second layout to switch between them.")
        }
        Section {
            Layout.fillWidth: true

            Repeater {
                model: page.layouts

                SectionRow {
                    id: layoutRow
                    required property var modelData
                    required property int index
                    title: page.layoutName(modelData.id)
                    subtitle: modelData.id

                    SecondaryButton {
                        symbol: Symbols.ArrowUpward
                        enabled: layoutRow.index > 0
                        onClicked: page.moveLayout(layoutRow.index, -1)
                        Accessible.name: qsTr("Move Up")
                    }
                    SecondaryButton {
                        symbol: Symbols.ArrowDownward
                        enabled: layoutRow.index < page.layouts.length - 1
                        onClicked: page.moveLayout(layoutRow.index, 1)
                        Accessible.name: qsTr("Move Down")
                    }
                    SecondaryButton {
                        symbol: Symbols.Delete
                        enabled: page.layouts.length > 1
                        onClicked: page.setLayouts(page.layouts.map(l => l.id).filter((id, i) => i !== layoutRow.index))
                        Accessible.name: qsTr("Remove")
                    }
                }
            }
            SectionRow {
                visible: page.layouts.length === 0
                title: qsTr("Default")
                subtitle: qsTr("The layout the system starts with")
            }
        }
    }

    PickerSheet {
        id: addSheet
        title: qsTr("Add Input Source")
        placeholderText: qsTr("No Layout Found")
        choices: {
            const used = page.layouts.map(l => l.id);
            return page.cfg ? page.cfg.availableLayouts().filter(l => !used.includes(l.id)).map(l => ({
                        title: l.name,
                        subtitle: l.id,
                        value: l.id,
                        keys: l.keys
                    })) : [];
        }
        onChosen: value => {
            // The first layout added to the default one keeps the default
            // first, as the module does.
            const ids = page.layouts.length === 0 ? ["us"] : page.layouts.map(l => l.id);
            ids.push(value);
            page.setLayouts(ids);
            page.loadCatalog();
        }
    }

    // How soon and how fast a held key repeats, with a field to try it in.
    AtlasDialog {
        id: repeatSheet
        title: qsTr("Key Repeat")
        preferredWidth: Kirigami.Units.gridUnit * 28
        onOpened: tryField.forceActiveFocus()

        function save() {
            if (!page.cfg.setRepeat(Math.round(delay.value), rate.value))
                page.notice = qsTr("Settings couldn't save the key repeat.");
            page.keyboard = page.cfg.keyboard();
        }

        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Delay")
                subtitle: qsTr("Before a held key starts to repeat")
                value: qsTr("%1 ms").arg(Math.round(delay.value))

                AtlasSlider {
                    id: delay
                    from: 150
                    to: 1000
                    stepSize: 50
                        implicitWidth: Kirigami.Units.gridUnit * 10
                    value: page.keyboard.delay ?? 600
                    onMoved: repeatSheet.save()
                    Accessible.name: qsTr("Delay")
                }
            }
            SectionRow {
                title: qsTr("Speed")
                subtitle: qsTr("How often it repeats once it starts")
                value: qsTr("%1 per second").arg(Math.round(rate.value))

                AtlasSlider {
                    id: rate
                    from: 2
                    to: 60
                    stepSize: 1
                        implicitWidth: Kirigami.Units.gridUnit * 10
                    value: page.keyboard.rate ?? 25
                    onMoved: repeatSheet.save()
                    Accessible.name: qsTr("Speed")
                }
            }
        }
        AtlasTextField {
            id: tryField
            Layout.fillWidth: true
            placeholderText: qsTr("Hold a key here to try it")
        }
    }
}
