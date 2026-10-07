pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Displays: the screens as they sit on the desk (drag them to rearrange),
// and for the one picked its resolution and scale; Night Light; and under
// Advanced its refresh rate, orientation and HDR. The screens are libkscreen's
// (cpp/screenconfig.h), Night Light is kwinrc's (cpp/nightlight.h). Every
// change is applied at once; one that can leave a screen unusable asks
// "Keep these settings?" and goes back after 15 seconds without an answer.
SettingsPage {
    id: page

    // They go with the page.
    readonly property var screens: pageBackends ? pageBackends.create("displays", page) : null
    readonly property var night: pageBackends ? pageBackends.create("night-light", page) : null

    readonly property var enabledScreens: (screens ? screens.outputs : []).filter(o => o.enabled)
    readonly property bool multiple: enabledScreens.length > 1
    // The screen picked in the arrangement; the main one at first.
    property int pickedId: -1
    readonly property var current: enabledScreens.find(o => o.id === pickedId) ?? enabledScreens.find(o => o.primary) ?? enabledScreens[0] ?? null
    readonly property bool ready: screens !== null && screens.loaded && !screens.busy && current !== null

    // The four turns, as Plasma's page offers them.
    readonly property var turns: [
        {
            text: qsTr("Landscape"),
            degrees: 0
        },
        {
            text: qsTr("Portrait (90° Clockwise)"),
            degrees: 90
        },
        {
            text: qsTr("Landscape (Upside Down)"),
            degrees: 180
        },
        {
            text: qsTr("Portrait (90° Counterclockwise)"),
            degrees: 270
        }
    ]

    function scaleValue(): int {
        return current ? current.scalePercent : 100;
    }

    Component.onCompleted: {
        if (screens)
            screens.refresh();
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.screens ? page.screens.error : ""
        shown: text !== ""
    }
    InfoBanner {
        Layout.fillWidth: true
        type: "information"
        text: page.screens ? page.screens.notice : ""
        shown: text !== ""
    }

    // The screens.
    Section {
        Layout.fillWidth: true
        visible: page.enabledScreens.length > 0
        footer: page.multiple ? qsTr("Drag a screen to where it sits on your desk.") : ""

        ArrangementView {
            objectName: "arrangement"
            Layout.fillWidth: true
            Layout.margins: AtlasStyle.spacingLarge
            screens: page.enabledScreens
            selectedId: page.current ? page.current.id : -1
            backend: page.screens
            onPicked: id => page.pickedId = id
        }
    }

    // The one picked.
    Section {
        Layout.fillWidth: true
        visible: page.current !== null
        title: page.current ? (page.multiple ? page.current.label : qsTr("Display")) : ""

        SectionRow {
            objectName: "resolution"
            title: qsTr("Resolution")
            value: page.current ? page.current.resolutionText : ""
            subtitle: {
                if (!page.current)
                    return "";
                const now = page.current.resolutions.find(r => r.current);
                return now && now.recommended ? qsTr("Recommended") : "";
            }
            chevron: page.current !== null && page.current.resolutions.length > 1
            enabled: page.ready
            onClicked: resolutionSheet.open()
        }
        SectionRow {
            objectName: "scale"
            title: qsTr("Scale")
            visible: page.screens !== null && page.screens.perOutputScaling
            enabled: page.ready
            content: [
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    text: qsTr("Scale")
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                },
                AtlasSlider {
                    id: scaleSlider
                    Layout.fillWidth: true
                    from: 50
                    to: 300
                    stepSize: 5
                    value: page.scaleValue()
                    Accessible.name: qsTr("Scale")
                    // Applied when let go (a drag would ask "Keep these settings?"
                    // at every step), or a moment after the last key press.
                    onMoved: if (!pressed)
                        scaleTimer.restart()
                    onPressedChanged: {
                        if (!pressed)
                            page.applyScale()
                    }
                },
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 3.5
                    horizontalAlignment: Text.AlignRight
                    text: qsTr("%1%").arg(Math.round(scaleSlider.value))
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
            ]
        }
        SectionRow {
            title: qsTr("Main Display")
            subtitle: qsTr("Where panels and pop-ups go")
            visible: page.multiple
            showSwitch: true
            switchChecked: page.current !== null && page.current.primary
            enabled: page.ready && page.screens.primarySupported && page.current !== null && !page.current.primary
            onSwitchToggled: checked => {
                if (checked)
                    page.screens.setPrimary(page.current.id);
            }
        }
    }

    // Night Light.
    Section {
        Layout.fillWidth: true
        title: qsTr("Night Light")

        SectionRow {
            objectName: "night-light"
            title: qsTr("Night Light")
            subtitle: {
                if (!page.night)
                    return "";
                if (!page.night.available)
                    return qsTr("Not available on this system");
                if (!page.night.active)
                    return qsTr("Makes colors warmer, which is easier on the eyes at night");
                if (page.night.schedule === "always")
                    return qsTr("On all day");
                return page.night.running ? qsTr("Warming colors until sunrise") : qsTr("Warms colors from sunset to sunrise");
            }
            showSwitch: true
            switchChecked: page.night !== null && page.night.active
            enabled: page.night !== null && page.night.available
            onSwitchToggled: checked => page.night.setActive(checked)
        }
        SectionRow {
            title: qsTr("Schedule")
            visible: page.night !== null && page.night.active
            AtlasSegmentedControl {
                model: [qsTr("Sunrise to Sunset"), qsTr("All Day")]
                currentIndex: page.night && page.night.schedule === "always" ? 1 : 0
                onActivated: index => page.night.setSchedule(index === 1 ? "always" : "sun")
                Accessible.name: qsTr("Schedule")
            }
        }
        SectionRow {
            title: qsTr("Warmth")
            visible: page.night !== null && page.night.active
            content: [
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    text: qsTr("Warmth")
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                },
                AtlasSlider {
                    id: warmthSlider
                    Layout.fillWidth: true
                    // Warmer to the right: 6500 K is no filter, 1000 K the warmest.
                    from: 0
                    to: 55
                    stepSize: 1
                    value: page.night ? (6500 - page.night.temperature) / 100 : 20
                    Accessible.name: qsTr("Warmth")
                    onMoved: {
                        page.night.preview(6500 - Math.round(value) * 100);
                        if (!pressed)
                            warmthTimer.restart();
                    }
                    onPressedChanged: {
                        if (!pressed)
                            page.applyWarmth()
                    }
                },
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 3.5
                    horizontalAlignment: Text.AlignRight
                    text: qsTr("%1 K").arg(6500 - Math.round(warmthSlider.value) * 100)
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                }
            ]
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "refresh-rate"
            title: qsTr("Refresh Rate")
            subtitle: page.multiple && page.current ? page.current.label : ""
            value: page.current && page.current.rates.length <= 1 ? page.current.rateText : ""
            enabled: page.ready && page.current !== null && page.current.rates.length > 1

            AtlasComboBox {
                visible: page.current !== null && page.current.rates.length > 1
                width: Kirigami.Units.gridUnit * 9
                model: page.current ? page.current.rates.map(r => r.text) : []
                currentIndex: page.current ? Math.max(0, page.current.rates.findIndex(r => r.current)) : 0
                onActivated: index => page.screens.setRefreshRate(page.current.id, page.current.rates[index].hertz)
                Accessible.name: qsTr("Refresh Rate")
            }
        }
        SectionRow {
            objectName: "orientation"
            title: qsTr("Orientation")
            subtitle: page.multiple && page.current ? page.current.label : ""
            enabled: page.ready

            AtlasComboBox {
                width: Kirigami.Units.gridUnit * 14
                model: page.turns.map(t => t.text)
                currentIndex: page.current ? Math.max(0, page.turns.findIndex(t => t.degrees === page.current.rotation)) : 0
                onActivated: index => page.screens.setRotation(page.current.id, page.turns[index].degrees)
                Accessible.name: qsTr("Orientation")
            }
        }
        SectionRow {
            objectName: "hdr"
            title: qsTr("HDR")
            subtitle: {
                if (!page.current)
                    return "";
                if (!page.current.hdrSupported)
                    return qsTr("This screen doesn't support HDR");
                return page.multiple ? page.current.label : qsTr("Brighter, more vivid colors in apps that support it");
            }
            showSwitch: true
            switchChecked: page.current !== null && page.current.hdr
            enabled: page.ready && page.current !== null && page.current.hdrSupported
            onSwitchToggled: checked => page.screens.setHdr(page.current.id, checked)
        }
    }

    RelatedLinks {
        page: page
    }

    function applyScale() {
        scaleTimer.stop();
        if (current && Math.round(scaleSlider.value) !== current.scalePercent)
            screens.setScale(current.id, Math.round(scaleSlider.value) / 100);
    }

    function applyWarmth() {
        warmthTimer.stop();
        const kelvin = 6500 - Math.round(warmthSlider.value) * 100;
        if (night.temperature !== kelvin)
            night.setTemperature(kelvin);
        night.stopPreview();
    }

    Timer {
        id: scaleTimer
        interval: 700
        onTriggered: page.applyScale()
    }
    Timer {
        id: warmthTimer
        interval: 500
        onTriggered: page.applyWarmth()
    }

    // The resolutions of the screen picked.
    PickerSheet {
        id: resolutionSheet
        title: qsTr("Resolution")
        current: {
            const now = page.current ? page.current.resolutions.find(r => r.current) : undefined;
            return now ? now.width + "x" + now.height : "";
        }
        choices: (page.current ? page.current.resolutions : []).map(r => ({
                    title: r.text,
                    subtitle: [r.ratio, r.recommended ? qsTr("Recommended") : ""].filter(t => t !== "").join(" · "),
                    value: r.width + "x" + r.height,
                    keys: r.text.toLowerCase()
                }))
        onChosen: value => {
            const parts = value.split("x");
            page.screens.setResolution(page.current.id, parseInt(parts[0]), parseInt(parts[1]));
        }
    }

    // "Keep these settings?", up while a change waits for an answer: Revert is
    // where the focus starts, as in Plasma, and nothing outside it closes it.
    ConfirmDialog {
        id: keepDialog
        title: qsTr("Keep These Display Settings?")
        text: {
            const n = page.screens ? page.screens.revertSeconds : 0;
            return n === 1 ? qsTr("Going back to the earlier settings in 1 second.") : qsTr("Going back to the earlier settings in %1 seconds.").arg(n);
        }
        acceptText: qsTr("Keep")
        rejectText: qsTr("Revert")
        defaultButton: "reject"
        focusReject: true
        closePolicy: QQC2.Popup.NoAutoClose
        // Keep says so before the dialog closes; any other way out reverts.
        onAccepted: page.screens.keepChanges()
        onClosed: {
            if (page.screens && page.screens.confirming)
                page.screens.revertChanges();
        }
    }
    Connections {
        target: page.screens
        function onConfirmingChanged() {
            if (page.screens.confirming)
                keepDialog.open();
            else
                keepDialog.close();
        }
    }
}
