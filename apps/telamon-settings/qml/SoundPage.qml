pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Sound: the output device and its volume, the input device and its volume,
// and a volume for each app that is playing. All of it is PipeWire's sound
// server's (PulseAudio's API, through PulseAudioQt: cpp/soundmixer.h), which
// tells us when anything changes, from anywhere; nothing here is applied,
// every slider is live. System Sounds is under Advanced (Plasma's own page).
SettingsPage {
    id: page

    // It goes with the page.
    readonly property var mixer: pageBackends ? pageBackends.create("sound", page) : null
    // PulseAudioQt's default sink and source: volume, muted, description, name.
    readonly property var output: mixer ? mixer.defaultOutput : null
    readonly property var input: mixer ? mixer.defaultInput : null
    readonly property real normal: mixer ? mixer.normalVolume : 65536

    function symbolFor(kind: string): int {
        switch (kind) {
        case "headphones":
            return Symbols.Headphones;
        case "headset":
            return Symbols.HeadsetMic;
        case "speaker":
            return Symbols.Speaker;
        case "monitor":
            return Symbols.Monitor;
        case "bluetooth":
            return Symbols.Bluetooth;
        case "usb":
            return Symbols.Usb;
        case "microphone":
        case "webcam":
            return Symbols.Mic;
        default:
            return Symbols.VolumeUp;
        }
    }

    // The devices for a sheet; redone whenever one comes, goes or is renamed.
    function deviceChoices(forOutput: bool): var {
        if (!mixer || mixer.revision < 0)
            return [];
        return forOutput ? mixer.outputChoices() : mixer.inputChoices();
    }

    // Names, redone when a device is renamed or changes its connector.
    function labelOf(device: var): string {
        return mixer && mixer.revision >= 0 && device ? mixer.labelOf(device) : "";
    }
    function portOf(device: var): string {
        return mixer && mixer.revision >= 0 && device ? mixer.portOf(device) : "";
    }

    function percent(volume: var): int {
        return mixer ? mixer.percent(volume) : 0;
    }

    // Slider, percentage and mute button: one row's content, for a device or an app.
    // `target` is the object with volume and muted.
    component VolumeControl: RowLayout {
        id: control

        required property var target
        property string label
        property bool microphone: false

        Layout.fillWidth: true
        spacing: TelamonStyle.spacingLarge

        ToolbarButton {
            symbol: control.target && control.target.muted ? (control.microphone ? Symbols.MicOff : Symbols.VolumeOff) : (control.microphone ? Symbols.Mic : Symbols.VolumeUp)
            checkable: true
            checked: control.target !== null && control.target.muted
            focusable: true
            toolTipText: control.target && control.target.muted ? qsTr("Unmute") : qsTr("Mute")
            Accessible.name: control.target && control.target.muted ? qsTr("Unmute %1").arg(control.label) : qsTr("Mute %1").arg(control.label)
            onClicked: control.target.muted = !control.target.muted
        }
        TelamonSlider {
            Layout.fillWidth: true
            from: 0
            to: page.normal
            stepSize: page.normal / 100
            value: control.target ? control.target.volume : 0
            opacity: control.target && control.target.muted ? 0.5 : 1
            Accessible.name: qsTr("%1 volume").arg(control.label)
            onMoved: {
                control.target.volume = Math.round(value);
                // Moving a muted slider is asking for sound.
                if (control.target.muted)
                    control.target.muted = false;
            }
        }
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 3
            horizontalAlignment: Text.AlignRight
            text: qsTr("%1%").arg(page.percent(control.target ? control.target.volume : 0))
            textFormat: Text.PlainText
            Accessible.ignored: true
        }
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.mixer ? page.mixer.error : ""
        shown: text !== ""
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Output")

        SectionRow {
            objectName: "output"
            title: qsTr("Output Device")
            value: page.output ? page.labelOf(page.output) : qsTr("None")
            subtitle: page.output ? page.portOf(page.output) : qsTr("No sound output was found")
            chevron: page.mixer !== null && page.mixer.outputs.count > 1
            enabled: page.mixer !== null && page.mixer.ready && page.mixer.outputs.count > 0
            leading: Symbol {
                icon: page.symbolFor(page.mixer ? page.mixer.kindOf(page.output) : "other")
                visible: page.output !== null
            }
            onClicked: outputSheet.open()
        }
        SectionRow {
            objectName: "volume"
            title: qsTr("Volume")
            visible: page.output !== null
            content: [
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8.4
                    text: qsTr("Volume")
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                },
                VolumeControl {
                    target: page.output
                    label: qsTr("Output")
                }
            ]
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Input")

        SectionRow {
            objectName: "input"
            title: qsTr("Input Device")
            value: page.input ? page.labelOf(page.input) : qsTr("None")
            subtitle: page.input ? page.portOf(page.input) : qsTr("No microphone was found")
            chevron: page.mixer !== null && page.mixer.inputs.count > 1
            enabled: page.mixer !== null && page.mixer.ready && page.mixer.inputs.count > 0
            leading: Symbol {
                icon: page.symbolFor(page.mixer ? page.mixer.kindOf(page.input) : "microphone")
                visible: page.input !== null
            }
            onClicked: inputSheet.open()
        }
        SectionRow {
            title: qsTr("Input Volume")
            visible: page.input !== null
            content: [
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8.4
                    text: qsTr("Input Volume")
                    textFormat: Text.PlainText
                    Accessible.ignored: true
                },
                VolumeControl {
                    target: page.input
                    label: qsTr("Input")
                    microphone: true
                }
            ]
        }
    }

    Section {
        id: apps
        objectName: "apps"
        Layout.fillWidth: true
        title: qsTr("App Volume")

        SectionRow {
            visible: page.mixer !== null && page.mixer.apps.count === 0
            title: qsTr("No apps are playing sound")
            enabled: false
        }
        Repeater {
            model: page.mixer ? page.mixer.apps : null

            SectionRow {
                id: appRow

                required property var model

                title: model.label
                content: [
                    SettingsIcon {
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                        source: appRow.model.appIcon !== "" ? appRow.model.appIcon : "audio-volume-high-symbolic"
                        fallback: "audio-volume-high-symbolic"
                    },
                    QQC2.Label {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 7
                        text: appRow.model.label
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        Accessible.ignored: true
                    },
                    VolumeControl {
                        // The stream itself, with its volume and muted.
                        target: appRow.model.PulseObject
                        label: appRow.model.label
                    }
                ]
            }
        }
    }

    AdvancedSection {
        page: page
    }

    RelatedLinks {
        page: page
    }

    PickerSheet {
        id: outputSheet
        title: qsTr("Output Device")
        current: page.output ? page.output.name : ""
        choices: page.deviceChoices(true)
        onChosen: value => page.mixer.setDefaultOutput(value)
    }

    PickerSheet {
        id: inputSheet
        title: qsTr("Input Device")
        current: page.input ? page.input.name : ""
        choices: page.deviceChoices(false)
        onChosen: value => page.mixer.setDefaultInput(value)
    }
}
