pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui
import "dates.js" as Dates

// Privacy & Security's "Review Crash Reports": the reports waiting for the
// person's decision, each with exactly the data that would be sent, and the
// ones sent in the last 90 days. Nothing is sent unless they press Send on
// that report. `reports` is src/crash_reports.rs.
TelamonDialog {
    id: sheet

    required property var reports

    title: qsTr("Crash Reports")
    preferredWidth: Kirigami.Units.gridUnit * 36
    onAboutToShow: if (reports) {
        reports.refresh();
        showSent = false;
    }
    footerContent: [
        PrimaryButton {
            text: qsTr("Done")
            onClicked: sheet.close()
        }
    ]

    readonly property var pending: reports && reports.reportsJson.length > 0 ? JSON.parse(reports.reportsJson) : []
    readonly property var sent: reports && reports.sentJson.length > 0 ? JSON.parse(reports.sentJson) : []
    property bool showSent: false

    InfoBanner {
        id: errorBanner
        Layout.fillWidth: true
        type: "error"
        text: sheet.reports ? sheet.reports.error : ""
        shown: text !== ""
    }
    InfoBanner {
        Layout.fillWidth: true
        type: "info"
        text: sheet.reports ? sheet.reports.info : ""
        shown: text !== ""
    }

    TelamonEmptyState {
        Layout.fillWidth: true
        visible: sheet.pending.length === 0
        iconName: "tools-report-bug"
        title: sheet.reports && sheet.reports.loaded ? qsTr("No crash reports waiting") : qsTr("Looking for crash reports…")
        text: qsTr("When something crashes, the report shows up here and nothing is sent unless you say so.")
    }

    Repeater {
        model: sheet.pending
        delegate: ColumnLayout {
            id: card
            required property var modelData
            property bool showPayload: false
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Section {
                Layout.fillWidth: true
                title: qsTr("%1 %2").arg(card.modelData.appName).arg(card.modelData.appVersion)
                footer: card.modelData.message

                SectionRow {
                    title: qsTr("Category")
                    value: card.modelData.category + " · " + card.modelData.type
                }
                SectionRow {
                    title: qsTr("When")
                    value: Dates.longDate(card.modelData.time)
                }
                SectionRow {
                    title: qsTr("Telamon OS")
                    value: card.modelData.osVersion + (card.modelData.channel ? " (" + card.modelData.channel + ")" : "")
                }
                SectionRow {
                    title: qsTr("Previous version")
                    value: card.modelData.previousVersion || qsTr("none")
                }
                SectionRow {
                    title: qsTr("Kernel")
                    value: card.modelData.kernel
                }
                SectionRow {
                    title: qsTr("Graphics")
                    value: card.modelData.gpu + (card.modelData.gpuDriver ? " · " + card.modelData.gpuDriver : "")
                }
                SectionRow {
                    title: qsTr("Uptime")
                    value: card.modelData.uptime
                }
            }

            Section {
                Layout.fillWidth: true
                title: qsTr("Stack Trace")
                visible: card.modelData.stacktrace.length > 0
                TelamonCodeView {
                    Layout.fillWidth: true
                    // Text lines up with the rows' text (SectionRow pads 12).
                    Layout.leftMargin: TelamonStyle.spacingLarge
                    Layout.rightMargin: TelamonStyle.spacingSmall
                    Layout.topMargin: TelamonStyle.spacingSmall
                    Layout.bottomMargin: TelamonStyle.spacingSmall
                    framed: false
                    showCopy: true
                    maximumHeight: Kirigami.Units.gridUnit * 10
                    text: card.modelData.stacktrace
                    Accessible.name: qsTr("Stack Trace")
                }
            }

            Section {
                Layout.fillWidth: true
                SectionRow {
                    title: card.showPayload ? qsTr("Hide Exact Data") : qsTr("Show Exact Data")
                    subtitle: qsTr("Exactly what would be sent")
                    chevron: true
                    disclosure: true
                    expanded: card.showPayload
                    onClicked: card.showPayload = !card.showPayload
                }
                TelamonCodeView {
                    visible: card.showPayload
                    Layout.fillWidth: true
                    Layout.leftMargin: TelamonStyle.spacingLarge
                    Layout.rightMargin: TelamonStyle.spacingSmall
                    Layout.topMargin: TelamonStyle.spacingSmall
                    Layout.bottomMargin: TelamonStyle.spacingSmall
                    framed: false
                    showCopy: true
                    maximumHeight: Kirigami.Units.gridUnit * 14
                    text: card.modelData.payload
                    Accessible.name: qsTr("Exact data")
                }
            }

            TelamonLabel {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                text: qsTr("Sending posts this report as a public issue on GitHub. Anyone can read it, including the stack trace and your Telamon OS version, kernel, CPU, GPU and memory.")
                textStyle: TelamonLabel.Caption
                wrapMode: Text.WordWrap
                visible: sheet.reports && sheet.reports.hasServer
            }

            Flow {
                Layout.fillWidth: true
                spacing: Kirigami.Units.largeSpacing
                PrimaryButton {
                    text: qsTr("Send")
                    visible: sheet.reports && sheet.reports.hasServer
                    enabled: sheet.reports && !sheet.reports.busy
                    onClicked: sheet.reports.sendReport(card.modelData.eventId)
                }
                SecondaryButton {
                    variant: TelamonButton.Destructive
                    text: qsTr("Don't Send")
                    enabled: sheet.reports && !sheet.reports.busy
                    onClicked: sheet.reports.discardReport(card.modelData.eventId)
                }
                TextButton {
                    text: qsTr("Report on GitHub Instead")
                    visible: card.modelData.githubUrl.length > 0 && sheet.reports.isSafeLink(card.modelData.githubUrl)
                    enabled: sheet.reports && !sheet.reports.busy
                    onClicked: {
                        // Discard only if a browser really opened.
                        if (Qt.openUrlExternally(card.modelData.githubUrl)) {
                            sheet.reports.discardReport(card.modelData.eventId);
                        }
                    }
                }
            }
        }
    }

    Section {
        Layout.fillWidth: true
        SectionRow {
            title: qsTr("Sent Reports")
            subtitle: sheet.sent.length === 0 ? qsTr("Reports you send are listed here for 90 days.") : (sheet.sent.length === 1 ? qsTr("1 report") : qsTr("%1 reports").arg(sheet.sent.length))
            chevron: sheet.sent.length > 0
            disclosure: sheet.sent.length > 0
            expanded: sheet.showSent
            clickable: sheet.sent.length > 0
            onClicked: sheet.showSent = !sheet.showSent
        }
        Repeater {
            model: sheet.showSent ? sheet.sent : []
            delegate: SectionRow {
                id: row
                required property var modelData
                title: row.modelData.appName + " " + row.modelData.appVersion
                subtitle: Dates.longDate(row.modelData.time) + " · Telamon OS " + row.modelData.osVersion + (row.modelData.sentEventId ? " · " + qsTr("event %1").arg(row.modelData.sentEventId) : "")
                chevron: true
                onClicked: {
                    payloadDialog.payload = row.modelData.payload;
                    payloadDialog.open();
                }
                // A trailing item of the row, so Section's separators and
                // corners still see plain SectionRows.
                TextButton {
                    text: qsTr("View on GitHub")
                    visible: row.modelData.issueUrl.length > 0 && sheet.reports.isSafeLink(row.modelData.issueUrl)
                    onClicked: Qt.openUrlExternally(row.modelData.issueUrl)
                }
            }
        }
    }

    ConfirmDialog {
        id: payloadDialog
        property string payload: ""
        title: qsTr("What Was Sent")
        acceptText: qsTr("Close")
        showReject: false

        TelamonCodeView {
            Layout.fillWidth: true
            text: payloadDialog.payload
            showCopy: true
            maximumHeight: Kirigami.Units.gridUnit * 16
            Accessible.name: qsTr("Sent data")
        }
    }
}
