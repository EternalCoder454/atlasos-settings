pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui
import "dates.js" as Dates

// Updates: the system's own update (Telamon OS's image, through the system
// helper), app updates (Flatpak) and firmware (fwupd). What was Telamon
// Updater's window. Up top, how the system stands and the one thing to do
// ("Check for Updates", "Download Update", "Restart to Update"); under it
// What's New, App Updates and Firmware Updates; under Advanced Go Back, the
// update channel, the background switch for apps and the history (in
// sheets). Crash reports live on Privacy & Security.
//
// `updates` is src/updates_page.rs, made by Main.qml once and kept for the
// life of the window, so an update goes on while another page is shown.
SettingsPage {
    id: page

    required property var updates

    // The OS logo for the up-to-date state, if the icon theme has it.
    readonly property string logoIcon: osLogoProbe.valid ? osLogoProbe.source : (distroLogoProbe.valid ? "distributor-logo" : "checkmark")
    Kirigami.Icon {
        id: osLogoProbe
        visible: false
        width: 0
        height: 0
        source: page.updates.osLogo
    }
    Kirigami.Icon {
        id: distroLogoProbe
        visible: false
        width: 0
        height: 0
        source: "distributor-logo"
    }

    readonly property var apps: page.updates.appsJson.length > 0 ? JSON.parse(page.updates.appsJson) : []
    readonly property var firmware: page.updates.firmwareJson.length > 0 ? JSON.parse(page.updates.firmwareJson) : ({
            "updates": [],
            "pending": [],
            "note": ""
        })
    readonly property bool installingFirmware: page.updates.firmwareBusy && page.updates.firmwareOp === "installFirmware"
    // Errors from these operations belong to the hero; the others (apps)
    // stay in their own section.
    readonly property bool heroError: ["check", "download", "restart", "rollback", "cancelRollback", "switch", "status", "timer"].indexOf(page.updates.errorOp) >= 0
    readonly property bool retryable: ["check", "status", "download"].indexOf(page.updates.errorOp) >= 0
    // A failed download is not retried while a restart waits (it could replace a queued rollback).
    readonly property bool canRetry: page.hasError && page.retryable && !(page.updates.errorOp === "download" && page.restartReady)
    readonly property bool hasError: page.updates.errorText.length > 0 && page.heroError && page.updates.restarting !== true && (!page.updates.loaded || !page.updates.busy)
    // busyOp is only meaningful together with busy.
    readonly property string busyOp: page.updates.busy ? page.updates.busyOp : ""
    readonly property bool downloading: page.busyOp === "download"
    // A change of state is running: show its own text, not "Checking".
    readonly property bool working: page.updates.restarting === true || ["rollback", "cancelRollback", "switch"].indexOf(page.busyOp) >= 0
    readonly property bool rollbackQueued: page.updates.rollbackQueued === true
    readonly property bool availableIsRollback: page.updates.availableIsRollback === true
    // The update failed its startup checks here and was undone (bad-image-digests).
    readonly property bool availableIsBad: page.updates.availableIsBad === true
    // Something is waiting for a restart (an update, a switch or a go back).
    readonly property bool restartReady: page.updates.hasStaged || page.updates.restartNeeded || page.rollbackQueued
    readonly property bool checking: (!page.updates.loaded && !page.hasError) || page.busyOp === "check"
    readonly property bool restarting: page.updates.restarting === true

    // The helper's progress while a download (or a channel switch) runs.
    readonly property string stage: page.downloading || page.busyOp === "switch" ? page.updates.progressStage : ""
    // Installing is counted in steps, not bytes: one step (bootc importing
    // the image) can take a minute, and a percentage would sit still that
    // long. So the bar moves on its own and the text names the step.
    readonly property real fraction: page.stage === "downloading" && page.updates.progressTotal > 0 ? Math.min(1, page.updates.progressDone / page.updates.progressTotal) : -1
    readonly property string progressText: {
        if (page.stage === "downloading") {
            if (page.fraction >= 0) {
                return qsTr("%1 of %2 · %3%").arg(page.size(page.updates.progressDone)).arg(page.size(page.updates.progressTotal)).arg(Math.floor(page.fraction * 100));
            }
            return page.updates.progressDone > 0 ? qsTr("%1 downloaded").arg(page.size(page.updates.progressDone)) : "";
        }
        if (page.stage === "installing") {
            var total = page.updates.progressTotal;
            var step = total > 0 ? qsTr("Step %1 of %2").arg(Math.min(total, page.updates.progressDone + 1)).arg(total) : "";
            var d = page.sentenceCase(page.updates.progressDetail);
            return d.length > 0 && step.length > 0 ? step + " · " + d : step + d;
        }
        return "";
    }
    // bootc writes "Importing Image", rpm-ostree "Writing OSTree commit":
    // capitalised words after the first are lowered, names such as OSTree kept.
    function sentenceCase(text) {
        return text.split(" ").map(function (w, i) {
            return i > 0 && /^[A-Z][a-z]+$/.test(w) ? w.toLowerCase() : w;
        }).join(" ");
    }

    // The clock for "Restart Tonight" (23:00 today, offered until 22:30) and
    // "Last checked: Today at …". Ticks while shown.
    property double now: Date.now()
    onVisibleChanged: page.now = Date.now()
    onRestartReadyChanged: page.now = Date.now()
    Timer {
        interval: 60 * 1000
        repeat: true
        triggeredOnStart: true
        running: page.visible
        onTriggered: page.now = Date.now()
    }
    function tonightAt(nowMs) {
        var d = new Date(nowMs);
        d.setHours(23, 0, 0, 0);
        return nowMs < d.getTime() - 30 * 60 * 1000 ? Math.floor(d.getTime() / 1000) : 0;
    }
    readonly property double tonight: page.tonightAt(page.now)

    function size(bytes) {
        if (bytes >= 1e9) {
            return qsTr("%1 GB").arg((bytes / 1e9).toLocaleString(Qt.locale(), "f", 1));
        }
        if (bytes >= 1e6) {
            return qsTr("%1 MB").arg(Math.round(bytes / 1e6));
        }
        return qsTr("%1 kB").arg(Math.round(bytes / 1e3));
    }

    function copyDetails() {
        TelamonClipboard.setText(page.updates.errorText);
        copied.restart();
    }

    function retry() {
        if (page.updates.errorOp === "download") {
            page.updates.downloadUpdate();
        } else if (!page.updates.loaded) {
            // refreshStatus is silent: clear the old error first.
            page.updates.dismissMessages();
            page.updates.refreshStatus();
        } else {
            page.check();
        }
    }

    function check() {
        page.updates.checkForUpdate();
        page.updates.checkFirmware();
    }

    function version(v, date) {
        return date.length > 0 ? qsTr("%1  (%2)").arg(v).arg(Dates.longDate(date)) : v;
    }

    // The last time the app list was looked at, so a visit shortly after
    // another doesn't ask Flathub again.
    property double lastAppsCheck: 0

    Component.onCompleted: {
        updates.pageOpened();
        updates.loadNotes();
        if (Date.now() - page.lastAppsCheck > 10 * 60 * 1000) {
            page.lastAppsCheck = Date.now();
            updates.checkApps();
            updates.checkFirmware();
        }
        // `telamon-settings updates check` (the tray's "Check for Updates").
        if (itemId === "check" && !page.checking && !page.downloading && !page.working && !page.restartReady) {
            page.check();
        }
    }
    Component.onDestruction: updates.pageClosed()

    // The staged/available version can change under us (inotify, a check).
    Connections {
        target: page.updates
        function onStagedVersionChanged() {
            page.updates.loadNotes();
        }
        function onAvailableVersionChanged() {
            page.updates.loadNotes();
        }
    }

    // ---- the system ----

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: qsTr("Developer test data: this is not your real system.")
        shown: page.updates.fixturesActive
    }
    InfoBanner {
        id: errorBanner
        Layout.fillWidth: true
        type: "error"
        // The hero shows the system's own errors; this is for the rest.
        text: !page.hasError ? page.updates.errorText : ""
        closable: true
        onClosed: page.updates.dismissMessages()
        // Closing breaks a plain binding; this one re-applies on the next message.
        Binding {
            target: errorBanner
            property: "shown"
            value: errorBanner.text.length > 0
        }
    }
    InfoBanner {
        id: infoBanner
        Layout.fillWidth: true
        type: "info"
        text: page.updates.infoText
        closable: true
        onClosed: page.updates.dismissMessages()
        Binding {
            target: infoBanner
            property: "shown"
            value: infoBanner.text.length > 0
        }
    }

    StatusHero {
        id: hero
        objectName: "check"
        badgeUnits: 7
        ringWidth: 5
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.gridUnit
        Layout.bottomMargin: Kirigami.Units.largeSpacing
        busy: page.checking || page.downloading || page.working
        progress: page.fraction
        showBar: page.downloading || page.stage.length > 0
        barText: page.progressText
        tint: {
            if (page.hasError) {
                return Kirigami.Theme.negativeTextColor;
            }
            if (page.restartReady) {
                return TelamonStyle.accent;
            }
            if (page.updates.updateAvailable && page.availableIsBad) {
                return Kirigami.Theme.neutralTextColor;
            }
            return page.updates.updateAvailable ? TelamonStyle.accent : Kirigami.Theme.positiveTextColor;
        }
        // Up to date: the OS logo in its own colours with a check badge,
        // not a tinted circle. The logo is `LOGO=` from os-release, then
        // `distributor-logo`, then a plain check.
        readonly property bool upToDate: iconName === page.logoIcon && page.logoIcon !== "checkmark"
        showTintCircle: !upToDate
        iconIsMask: !upToDate
        cornerBadgeIcon: upToDate ? "checkmark" : ""
        iconName: {
            if (page.hasError) {
                return "dialog-error";
            }
            if (page.working) {
                return "view-refresh";
            }
            if (page.checking) {
                return "view-refresh";
            }
            if (page.downloading) {
                // An update goes up a version: an up arrow, not a download's.
                return Qt.resolvedUrl("icons/update-arrow.svg");
            }
            if (page.rollbackQueued) {
                return "edit-undo";
            }
            if (page.restartReady) {
                return "system-reboot";
            }
            if (page.updates.updateAvailable && page.availableIsBad) {
                return "dialog-warning";
            }
            if (page.updates.updateAvailable) {
                return "update-medium";
            }
            return page.logoIcon;
        }
        headline: {
            if (page.hasError) {
                return page.updates.loaded ? qsTr("Something went wrong") : qsTr("Could not read the system state");
            }
            if (!page.updates.loaded) {
                return qsTr("Reading the system state…");
            }
            if (page.working) {
                return page.updates.restarting === true ? qsTr("Restarting System…") : qsTr("Applying your change…");
            }
            if (page.checking) {
                return qsTr("Checking for updates…");
            }
            if (page.downloading) {
                return page.stage === "installing" ? qsTr("Installing %1…").arg(page.updates.availableVersion) : qsTr("Downloading %1…").arg(page.updates.availableVersion);
            }
            if (page.rollbackQueued) {
                return qsTr("Restart to go back to %1").arg(page.updates.rollbackTarget);
            }
            if (page.restartReady) {
                return qsTr("Restart to finish updating");
            }
            if (page.updates.updateAvailable && page.availableIsBad) {
                return qsTr("Version %1 didn't start properly").arg(page.updates.availableVersion);
            }
            if (page.updates.updateAvailable && page.availableIsRollback) {
                return qsTr("You went back from version %1").arg(page.updates.availableVersion);
            }
            if (page.updates.updateAvailable) {
                return qsTr("Telamon OS %1 is available").arg(page.updates.availableVersion);
            }
            return qsTr("Telamon OS is up to date");
        }
        subtitle: {
            if (page.hasError) {
                return page.updates.errorText;
            }
            if (page.updates.restarting === true) {
                return qsTr("Saving your session…");
            }
            if (page.downloading) {
                return qsTr("Keep using your computer. The update is set up on the side and starts when you restart.");
            }
            if (page.checking || page.working) {
                // Never repeat the headline.
                return page.updates.busyText === hero.headline ? qsTr("This takes a moment.") : page.updates.busyText;
            }
            var when = page.updates.scheduledAt > 0 ? " " + qsTr("Restart scheduled for %1.").arg(Dates.atTime(page.updates.scheduledAt)) : "";
            if (page.rollbackQueued) {
                return qsTr("The previous version starts after the restart.") + when;
            }
            if (page.updates.hasStaged) {
                return qsTr("Version %1 is downloaded and waits for a restart.").arg(page.updates.stagedVersion) + when;
            }
            if (page.restartReady) {
                return qsTr("A restart finishes the change you made.") + when;
            }
            if (page.updates.updateAvailable && page.availableIsBad) {
                return qsTr("It failed its startup checks on this computer and was undone. It won't download on its own; a newer version will.");
            }
            if (page.updates.updateAvailable && page.availableIsRollback) {
                return qsTr("It won't download on its own; download it again if you like.");
            }
            if (page.updates.updateAvailable) {
                return qsTr("It can be downloaded now. You are on %1.").arg(page.updates.currentVersion);
            }
            var last = page.updates.lastChecked > 0 && page.updates.loaded ? " " + qsTr("Last checked: %1").arg(Dates.relative(page.updates.lastChecked, page.now)) : "";
            return qsTr("Version %1. Updates download in the background.").arg(page.updates.currentVersion) + last;
        }

        PrimaryButton {
            text: page.restarting ? qsTr("Restarting System…") : (page.rollbackQueued ? qsTr("Restart Now") : qsTr("Restart to Update"))
            visible: page.restartReady
            enabled: !page.updates.busy && !page.working && !page.installingFirmware
            onClicked: page.updates.restartNow()
        }
        SecondaryButton {
            text: qsTr("Restart Tonight")
            visible: page.restartReady && !page.working && page.updates.scheduledAt === 0 && page.tonight > 0
            enabled: !page.installingFirmware
            TelamonToolTip {
                text: qsTr("Restarts at %1. You get a notification 5 minutes before.").arg(new Date(page.tonight * 1000).toLocaleTimeString(Qt.locale(), Qt.locale().timeFormat(1)))
                shown: parent.hovered || parent.visualFocus
            }
            onClicked: {
                var at = page.tonightAt(Date.now());
                page.now = Date.now();
                if (at > 0) {
                    page.updates.scheduleRestart(at);
                }
            }
        }
        SecondaryButton {
            text: qsTr("Pick a Time…")
            visible: page.restartReady && !page.working && page.updates.scheduledAt === 0
            enabled: !page.installingFirmware
            onClicked: scheduleDialog.open()
        }
        SecondaryButton {
            text: qsTr("Cancel Scheduled Restart")
            visible: page.updates.scheduledAt > 0 && !page.working
            onClicked: page.updates.cancelRestart()
        }
        // A queued go back can be undone before the restart.
        SecondaryButton {
            text: qsTr("Don't Go Back")
            visible: page.rollbackQueued && !page.working
            enabled: !page.updates.busy
            onClicked: page.updates.cancelRollback()
        }
        PrimaryButton {
            text: qsTr("Download Update")
            visible: page.updates.updateAvailable && !page.updates.hasStaged && !page.restartReady && !page.availableIsRollback && !page.availableIsBad && !page.hasError && !page.checking && !page.downloading
            enabled: !page.updates.busy
            onClicked: page.updates.downloadUpdate()
        }
        SecondaryButton {
            text: qsTr("Download Anyway")
            visible: page.updates.updateAvailable && !page.updates.hasStaged && !page.restartReady && (page.availableIsRollback || page.availableIsBad) && !page.hasError && !page.checking && !page.downloading
            enabled: !page.updates.busy
            onClicked: page.availableIsBad ? badDialog.open() : page.updates.downloadUpdate()
        }
        // One primary pill at most: with a restart waiting, Try again is secondary.
        PrimaryButton {
            text: qsTr("Try Again")
            visible: page.canRetry && !page.restartReady
            onClicked: page.retry()
        }
        SecondaryButton {
            text: qsTr("Try Again")
            visible: page.canRetry && page.restartReady
            onClicked: page.retry()
        }
        SecondaryButton {
            text: copied.running ? qsTr("Copied") : qsTr("Copy Details")
            Accessible.name: qsTr("Copy the error details")
            visible: page.hasError
            onClicked: page.copyDetails()
        }
        SecondaryButton {
            text: qsTr("Dismiss")
            Accessible.name: qsTr("Dismiss error")
            visible: page.hasError && !page.canRetry
            onClicked: page.updates.dismissMessages()
        }
        SecondaryButton {
            text: qsTr("Check for Updates")
            visible: !page.hasError && !page.restartReady && !page.checking && !page.downloading && !page.working
            enabled: !page.updates.busy
            onClicked: page.check()
        }
    }

    Section {
        Layout.fillWidth: true
        visible: page.updates.notesState !== "none" && page.updates.notesState !== ""

        SectionRow {
            objectName: "notes"
            title: qsTr("What's New in %1").arg(page.updates.notesVersion)
            subtitle: page.updates.notesState === "loading" ? qsTr("Loading release notes…") : (page.updates.notesState === "missing" ? qsTr("No release notes for this version") : (page.updates.notesState === "error" ? (page.updates.notesError.length > 0 ? page.updates.notesError : qsTr("Could not load the release notes. Check your internet connection.")) : ""))
            clickable: page.updates.notesState === "ready"
            chevron: page.updates.notesState === "ready"
            onClicked: notesSheet.open()
        }
    }

    // ---- flatpak apps ----
    Section {
        id: appsSection
        objectName: "apps"
        Layout.fillWidth: true
        title: qsTr("App Updates")

        SectionRow {
            visible: page.updates.appsBusy
            title: page.updates.appsStatus
            busy: page.updates.appsBusy
        }
        SectionRow {
            visible: page.updates.appsError.length > 0
            iconName: "dialog-error"
            title: page.updates.appsError
        }
        SectionRow {
            visible: !page.updates.appsBusy && page.apps.length === 0 && page.updates.appsError.length === 0
            iconName: "checkmark"
            title: qsTr("All apps are up to date.")
        }
        Repeater {
            model: page.apps
            delegate: SectionRow {
                id: appRow
                required property var modelData
                iconName: appRow.modelData.icon ? appRow.modelData.icon : (appRow.modelData.runtime ? "preferences-system-plugin" : "applications-all")
                title: appRow.modelData.name
                // Held back by a background round: say what it wants before
                // the user presses Update Apps.
                subtitle: appRow.modelData.asks ? qsTr("Asks for new permissions: %1").arg(appRow.modelData.asks) : (appRow.modelData.runtime ? qsTr("Runtime") : qsTr("App")) + " · " + appRow.modelData.branch + " · " + (appRow.modelData.system ? qsTr("System") : qsTr("User"))
                value: appRow.modelData.size_text
            }
        }
        SectionRow {
            visible: page.apps.length > 0
            title: page.apps.length === 1 ? qsTr("1 app can be updated") : qsTr("%n apps can be updated", "", page.apps.length)
            SecondaryButton {
                text: qsTr("Update Apps")
                enabled: !page.updates.appsBusy
                onClicked: page.updates.updateApps()
            }
        }
        SectionRow {
            title: qsTr("Check for App Updates")
            Accessible.name: qsTr("Check for App Updates")
            clickable: !page.updates.appsBusy
            chevron: true
            onClicked: page.updates.checkApps()
        }
    }

    // ---- firmware (fwupd) ----
    Section {
        id: firmwareSection
        objectName: "firmware"
        Layout.fillWidth: true
        title: qsTr("Firmware Updates")
        visible: page.updates.firmwareAvailable

        SectionRow {
            visible: page.updates.firmwareBusy
            title: page.installingFirmware ? (page.updates.firmwareStatus.length > 0 ? page.updates.firmwareStatus + (page.updates.firmwarePercent >= 0 ? " · " + page.updates.firmwarePercent + "%" : "") + "…" : qsTr("Installing firmware…")) : qsTr("Looking for firmware updates…")
            subtitle: page.installingFirmware ? page.updates.firmwareRequest : ""
            busy: page.updates.firmwareBusy
        }
        SectionRow {
            visible: page.updates.firmwareError.length > 0
            iconName: "dialog-error"
            title: page.updates.firmwareError
        }
        SectionRow {
            visible: !page.updates.firmwareBusy && page.firmware.updates.length === 0 && page.firmware.pending.length === 0 && page.updates.firmwareError.length === 0
            iconName: "checkmark"
            title: qsTr("Firmware is up to date.")
        }
        SectionRow {
            visible: page.firmware.note.length > 0
            iconName: "dialog-information"
            title: page.firmware.note
        }
        Repeater {
            model: page.firmware.pending
            delegate: SectionRow {
                id: pendRow
                required property var modelData
                iconName: pendRow.modelData.state === "failed" ? "dialog-error" : "system-reboot"
                title: pendRow.modelData.device
                subtitle: pendRow.modelData.state === "failed" ? (pendRow.modelData.text.length > 0 ? qsTr("Version %1 did not install: %2").arg(pendRow.modelData.version).arg(pendRow.modelData.text) : qsTr("Version %1 did not install.").arg(pendRow.modelData.version)) : (pendRow.modelData.state === "shutdown" ? qsTr("Shut down to finish installing %1").arg(pendRow.modelData.version) : qsTr("Restart to finish installing %1").arg(pendRow.modelData.version))
            }
        }
        Repeater {
            model: page.firmware.updates
            delegate: SectionRow {
                id: fwRow
                required property var modelData
                iconName: "cpu"
                title: fwRow.modelData.device
                subtitle: [qsTr("%1 → %2").arg(fwRow.modelData.current).arg(fwRow.modelData.version), fwRow.modelData.vendor, fwRow.modelData.summary, fwRow.modelData.trusted ? "" : qsTr("Not signed by a trusted source")].filter(function (t) {
                    return t.length > 0;
                }).join(" · ")
                TelamonBadge {
                    visible: fwRow.modelData.important
                    text: qsTr("Important")
                    type: "warning"
                }
                SecondaryButton {
                    text: qsTr("Details")
                    Accessible.name: qsTr("Details, %1").arg(fwRow.modelData.device)
                    onClicked: {
                        detailsDialog.row = fwRow.modelData;
                        detailsDialog.open();
                    }
                }
                SecondaryButton {
                    text: qsTr("Install")
                    visible: fwRow.modelData.trusted
                    Accessible.name: qsTr("Install firmware for %1").arg(fwRow.modelData.device)
                    enabled: !page.updates.firmwareBusy
                    onClicked: {
                        firmwareDialog.row = fwRow.modelData;
                        firmwareDialog.open();
                    }
                }
            }
        }
        SectionRow {
            visible: page.updates.firmwareRestart
            iconName: "system-reboot"
            title: qsTr("Restart to finish installing firmware")
            SecondaryButton {
                text: page.restarting ? qsTr("Restarting System…") : qsTr("Restart to Update")
                enabled: !page.updates.busy && !page.working && !page.installingFirmware
                onClicked: page.updates.restartNow()
            }
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "go-back"
            title: qsTr("Go Back to the Previous Version")
            subtitle: page.updates.hasRollback ? qsTr("If something stopped working after an update") : qsTr("There is no previous version to go back to yet")
            value: page.updates.hasRollback ? page.updates.rollbackVersion : ""
            chevron: page.updates.hasRollback
            clickable: page.updates.hasRollback
            enabled: page.updates.hasRollback && !page.updates.busy && !page.restarting
            onClicked: goBackSheet.open()
        }
        SectionRow {
            objectName: "channel"
            title: qsTr("Update Channel")
            subtitle: qsTr("How early you get new versions")
            value: page.updates.channel === "testing" ? qsTr("Testing") : (page.updates.channel === "stable" ? qsTr("Stable") : (page.updates.loaded ? qsTr("Custom") : ""))
            chevron: true
            enabled: !page.updates.busy
            onClicked: channelSheet.open()
        }
        SectionRow {
            objectName: "automatic-apps"
            title: qsTr("Update Apps in the Background")
            subtitle: page.updates.appsAuto ? qsTr("Apps update by themselves, but not on a metered connection or a low battery. An app that asks for new permissions waits for you.") : qsTr("Off: you get a notification when app updates are ready, and Update Apps installs them.")
            showSwitch: true
            switchChecked: page.updates.appsAuto
            onSwitchToggled: checked => page.updates.enableBackgroundApps(checked)
        }
        SectionRow {
            objectName: "history"
            title: qsTr("Update History")
            subtitle: qsTr("The versions this computer has run, what changed in each, and the app updates")
            chevron: true
            onClicked: historySheet.open()
        }
    }

    RelatedLinks {
        page: page
    }

    // ---- dialogs and sheets ----

    ConfirmDialog {
        id: badDialog
        title: qsTr("Download %1 Anyway?").arg(page.updates.availableVersion)
        text: qsTr("This version didn't pass its startup checks on this computer, and Telamon OS went back to the version before it. It will probably fail again.")
        acceptText: qsTr("Download Anyway")
        focusReject: true
        // The state can change under an open dialog (a background read).
        onAccepted: {
            if (page.availableIsBad) {
                page.updates.downloadUpdate();
            }
        }
    }
    onAvailableIsBadChanged: {
        if (!page.availableIsBad) {
            badDialog.close();
        }
    }

    ConfirmDialog {
        id: firmwareDialog
        property var row: ({})
        title: qsTr("Install Firmware %2 for %1?").arg(firmwareDialog.row.device ?? "").arg(firmwareDialog.row.version ?? "")
        text: {
            var t = qsTr("Keep the computer plugged in, and don't unplug %1 or turn the computer off until it finishes.").arg(firmwareDialog.row.device ?? "");
            if (firmwareDialog.row.shutdown) {
                t += " " + qsTr("It finishes when you shut down.");
            } else if (firmwareDialog.row.reboot) {
                t += " " + qsTr("It finishes when you restart.");
            }
            return t;
        }
        acceptText: qsTr("Install")
        focusReject: true
        onAccepted: page.updates.installFirmware(firmwareDialog.row.id, firmwareDialog.row.version, firmwareDialog.row.checksum ?? "")
    }

    ConfirmDialog {
        id: detailsDialog
        property var row: ({})
        title: detailsDialog.row.device ?? ""
        text: (detailsDialog.row.description ?? "").length > 0 ? detailsDialog.row.description : qsTr("This update has no release notes.")
        showReject: false
        acceptText: qsTr("Close")
    }

    Timer {
        id: copied
        interval: 2000
    }

    ConfirmDialog {
        id: scheduleDialog
        title: qsTr("Pick a Restart Time")
        text: qsTr("Telamon Updater restarts your computer at this time. You get a notification 5 minutes before, and apps get to save their work first.")
        acceptText: qsTr("Schedule Restart")
        closeOnAccept: false
        property string problem: ""
        onAccepted: {
            var d = new Date();
            if (dayBox.currentIndex === 1) {
                d.setDate(d.getDate() + 1);
            }
            d.setHours(timePicker.hours, timePicker.minutes, 0, 0);
            if (d.getTime() <= Date.now()) {
                scheduleDialog.problem = qsTr("That time has already passed. Pick a later time.");
                return;
            }
            page.updates.scheduleRestart(Math.floor(d.getTime() / 1000));
            scheduleDialog.close();
        }
        onAboutToShow: {
            scheduleDialog.problem = "";
            var d = new Date(Date.now() + 60 * 60 * 1000);
            dayBox.currentIndex = d.getDate() !== new Date().getDate() ? 1 : 0;
            timePicker.hours = d.getHours();
            timePicker.minutes = 0;
        }

        TelamonLabel {
            Layout.fillWidth: true
            visible: scheduleDialog.problem.length > 0
            text: scheduleDialog.problem
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Accessible.role: Accessible.AlertMessage
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            TelamonComboBox {
                id: dayBox
                Layout.fillWidth: true
                onActivated: scheduleDialog.problem = ""
                model: [qsTr("Today"), qsTr("Tomorrow")]
                Accessible.name: qsTr("Day")
            }
            TelamonTimePicker {
                id: timePicker
                onEdited: scheduleDialog.problem = ""
            }
        }
    }

    // What's New: the release notes of the version waiting or on offer.
    TelamonDialog {
        id: notesSheet
        title: qsTr("What's New in %1").arg(page.updates.notesVersion)
        preferredWidth: Kirigami.Units.gridUnit * 32
        footerContent: [
            SecondaryButton {
                text: qsTr("Close")
                onClicked: notesSheet.close()
            }
        ]
        NotesText {
            Layout.fillWidth: true
            html: page.updates.notesHtml
            plain: page.updates.notesPlain
            onLinkClicked: link => {
                if (page.updates.isSafeLink(link)) {
                    Qt.openUrlExternally(link);
                }
            }
        }
    }

    // Go Back: the previous version, which stays on the computer for this.
    TelamonDialog {
        id: goBackSheet
        title: qsTr("Go Back to %1?").arg(page.updates.rollbackVersion)
        preferredWidth: Kirigami.Units.gridUnit * 26
        readonly property bool bad: page.updates.rollbackIsBad === true
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: goBackSheet.close()
            },
            PrimaryButton {
                text: goBackSheet.bad ? qsTr("Go Back Anyway") : (page.updates.rollbackDate.length > 0 ? qsTr("Go Back to %1 (%2)").arg(page.updates.rollbackVersion).arg(Dates.shortDate(page.updates.rollbackDate)) : qsTr("Go Back to %1").arg(page.updates.rollbackVersion))
                enabled: !page.updates.busy && !page.restarting
                onClicked: {
                    page.updates.rollback();
                    goBackSheet.close();
                }
            }
        ]
        TelamonLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: goBackSheet.bad ? qsTr("This version didn't pass its startup checks on this computer and was undone, so it will probably fail again. Your files and settings stay as they are.") : qsTr("The next restart starts the previous version. Your files and settings stay as they are, and you can update again later.")
        }
        Section {
            Layout.fillWidth: true
            SectionRow {
                title: qsTr("Current")
                value: page.version(page.updates.currentVersion, page.updates.currentDate)
            }
            SectionRow {
                title: qsTr("Previous")
                value: page.version(page.updates.rollbackVersion, page.updates.rollbackDate)
            }
        }
    }

    // The update channel: stable or testing.
    TelamonDialog {
        id: channelSheet
        title: qsTr("Update Channel")
        preferredWidth: Kirigami.Units.gridUnit * 28
        // What the user picked; starts at the channel the system follows.
        property string choice: page.updates.channel
        onAboutToShow: choice = page.updates.channel
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: channelSheet.close()
            },
            PrimaryButton {
                text: qsTr("Switch Channel")
                enabled: !page.updates.busy && channelSheet.choice !== "" && channelSheet.choice !== page.updates.channel
                onClicked: {
                    switchConfirm.open();
                }
            }
        ]
        Section {
            Layout.fillWidth: true
            title: qsTr("How early do you want updates?")
            footer: page.updates.loaded && page.updates.channel === "" ? qsTr("This system follows a custom image, not Stable or Testing. Pick a channel to switch to it.") : ""

            SectionRow {
                title: qsTr("Stable")
                subtitle: qsTr("A new version about once a week. Recommended.")
                clickable: true
                radio: true
                checkmark: channelSheet.choice === "stable"
                Accessible.role: Accessible.RadioButton
                Accessible.checked: channelSheet.choice === "stable"
                onClicked: channelSheet.choice = "stable"
            }
            SectionRow {
                title: qsTr("Testing")
                subtitle: qsTr("A new version every day. You get changes first, and things may break more often.")
                clickable: true
                radio: true
                checkmark: channelSheet.choice === "testing"
                Accessible.role: Accessible.RadioButton
                Accessible.checked: channelSheet.choice === "testing"
                onClicked: channelSheet.choice = "testing"
            }
        }
    }
    ConfirmDialog {
        id: switchConfirm
        title: qsTr("Switch to the %1 Channel?").arg(channelSheet.choice === "testing" ? qsTr("Testing") : qsTr("Stable"))
        text: qsTr("The new channel's latest version downloads and waits for a restart. Your files and settings stay as they are.")
        acceptText: qsTr("Switch Channel")
        focusReject: true
        onAccepted: {
            page.updates.switchChannel(channelSheet.choice);
            channelSheet.close();
        }
    }

    // History: the versions this computer ran, with what changed in each,
    // and the app updates installed here.
    TelamonDialog {
        id: historySheet
        title: qsTr("Update History")
        preferredWidth: Kirigami.Units.gridUnit * 36
        onAboutToShow: {
            page.updates.loadHistory();
            page.updates.loadChangelog();
        }
        footerContent: [
            SecondaryButton {
                text: qsTr("Close")
                onClicked: historySheet.close()
            }
        ]

        readonly property var items: page.updates.changelogJson.length > 0 ? JSON.parse(page.updates.changelogJson) : []
        readonly property var appEntries: page.updates.appHistoryJson.length > 0 ? JSON.parse(page.updates.appHistoryJson) : []
        readonly property string loadState: page.updates.changelogState
        // Versions the user opened or closed; the newest starts open.
        property var toggled: ({})

        function isOpen(version, index) {
            var t = historySheet.toggled[version] === true;
            return index === 0 ? !t : t;
        }
        function toggle(version) {
            var t = Object.assign({}, historySheet.toggled);
            t[version] = !(t[version] === true);
            historySheet.toggled = t;
        }
        function badge(item) {
            switch (item.state) {
            case "current":
                return qsTr("Running now");
            case "staged":
                return qsTr("Downloaded, starts after a restart");
            case "available":
                return qsTr("Available to download");
            case "ran":
                return qsTr("Ran on this computer from %1").arg(Dates.longDate(item.first_booted));
            default:
                return qsTr("Came with a later update");
            }
        }
        function icon(item) {
            switch (item.state) {
            case "current":
                return "checkmark";
            case "staged":
                return "system-reboot";
            case "available":
                return "update-medium";
            default:
                return "view-history";
            }
        }

        // One status read can change all three versions: one reload for them.
        Connections {
            target: page.updates
            enabled: historySheet.opened
            function onCurrentVersionChanged() {
                Qt.callLater(page.updates.loadChangelog);
            }
            function onStagedVersionChanged() {
                Qt.callLater(page.updates.loadChangelog);
            }
            function onAvailableVersionChanged() {
                Qt.callLater(page.updates.loadChangelog);
            }
        }

        TelamonEmptyState {
            Layout.fillWidth: true
            visible: historySheet.items.length === 0 && (historySheet.loadState === "loading" || historySheet.loadState === "")
            iconName: "view-list-text"
            title: qsTr("Loading the history…")
        }
        TelamonEmptyState {
            Layout.fillWidth: true
            visible: historySheet.items.length === 0 && historySheet.loadState === "error"
            iconName: "dialog-error"
            title: qsTr("Could not load the history")
            text: page.updates.changelogNote.length > 0 ? page.updates.changelogNote : qsTr("Check your internet connection.")
            actionText: qsTr("Try Again")
            actionSymbol: Symbols.Refresh
            onTriggered: page.updates.loadChangelog()
        }
        TelamonEmptyState {
            Layout.fillWidth: true
            visible: historySheet.items.length === 0 && historySheet.loadState === "ready" && historySheet.appEntries.length === 0
            iconName: "view-history"
            title: qsTr("No history yet")
            text: qsTr("Each version this computer starts, and each app update, is listed here, newest first.")
        }
        // A list saved earlier, shown while offline.
        TelamonLabel {
            Layout.fillWidth: true
            visible: historySheet.items.length > 0 && page.updates.changelogNote.length > 0
            text: page.updates.changelogNote
            wrapMode: Text.Wrap
            opacity: 0.7
            textFormat: Text.PlainText
        }

        Repeater {
            model: historySheet.items
            delegate: Section {
                id: card
                required property var modelData
                required property int index
                Layout.fillWidth: true
                readonly property bool open: historySheet.isOpen(card.modelData.version, card.index)

                SectionRow {
                    iconName: historySheet.icon(card.modelData)
                    title: qsTr("Telamon OS %1").arg(card.modelData.version)
                    subtitle: {
                        var t = historySheet.badge(card.modelData);
                        if (card.modelData.date.length > 0) {
                            t += " · " + qsTr("Released %1").arg(Dates.longDate(card.modelData.date));
                        }
                        return t;
                    }
                    chevron: true
                    disclosure: true
                    expanded: card.open
                    Accessible.name: title + ", " + subtitle
                    onClicked: historySheet.toggle(card.modelData.version)
                }
                Item {
                    visible: card.open
                    Layout.fillWidth: true
                    implicitHeight: (card.modelData.html.length > 0 ? notes.implicitHeight : none.implicitHeight) + Kirigami.Units.largeSpacing * 2
                    NotesText {
                        id: notes
                        visible: card.modelData.html.length > 0
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        html: card.modelData.html
                        plain: card.modelData.plain
                        onLinkClicked: link => {
                            if (page.updates.isSafeLink(link)) {
                                Qt.openUrlExternally(link);
                            }
                        }
                    }
                    TelamonLabel {
                        id: none
                        visible: card.modelData.html.length === 0
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        text: qsTr("No release notes for this version.")
                        opacity: 0.7
                        wrapMode: Text.Wrap
                    }
                }
            }
        }

        Section {
            Layout.fillWidth: true
            visible: historySheet.appEntries.length > 0
            title: qsTr("App Updates")
            Repeater {
                model: historySheet.appEntries
                delegate: SectionRow {
                    id: appHistoryRow
                    required property var modelData
                    iconName: appHistoryRow.modelData.runtime ? "preferences-system-plugin" : "applications-all"
                    title: appHistoryRow.modelData.to ? qsTr("%1 %2").arg(appHistoryRow.modelData.name).arg(appHistoryRow.modelData.to) : appHistoryRow.modelData.name
                    subtitle: {
                        var when = Dates.longDate(new Date(appHistoryRow.modelData.at * 1000).toISOString());
                        var t = appHistoryRow.modelData.auto ? qsTr("Updated in the background on %1").arg(when) : qsTr("Updated on %1").arg(when);
                        if (appHistoryRow.modelData.from && appHistoryRow.modelData.to && appHistoryRow.modelData.from !== appHistoryRow.modelData.to) {
                            t += " · " + qsTr("was %1").arg(appHistoryRow.modelData.from);
                        }
                        return t;
                    }
                }
            }
        }
    }
}
