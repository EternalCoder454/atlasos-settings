pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import QtQuick.Controls as QQC2
import QtCore
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Accounts (the `users` page): your account first, as a header with your
// picture and name, then how you sign in (password, fingerprint, automatic
// login), then the other people on this computer and Add User, all through
// AccountsService; fingerprints through fprintd. Passwords go straight to
// the service and are cleared from the fields as soon as the sheet closes.
// Under Advanced: the login screen's settings.
SettingsPage {
    id: page

    // src/users_page.rs; it goes with the page.
    readonly property var sys: pageBackends ? pageBackends.create("users", page) : null
    // The other people, parsed from the backend's JSON.
    readonly property var others: {
        try {
            return JSON.parse(page.sys ? page.sys.othersJson : "[]");
        } catch (e) {
            return [];
        }
    }
    // The person whose sheet is open.
    property var person: ({})
    // The Launcher (cpp/launcher.h), to start the camera app.
    property var launcher: null
    // The camera apps, by desktop file ID: Camera (Plasma Camera, which the
    // image ships), then ones people may have added.
    readonly property var cameraApps: ["org.kde.plasma.camera", "org.kde.kamoso", "org.gnome.Snapshot", "org.gnome.Cheese"]
    // "": nothing asked; "open": a camera app was started; "missing": none installed.
    property string cameraState: ""

    // Starts the camera app to take a picture. Camera saves into Pictures,
    // where "Choose the Picture" then looks.
    function openCamera() {
        const started = page.launcher ? page.launcher.runApplication(page.cameraApps) : "";
        page.cameraState = started !== "" ? "open" : "missing";
    }

    function choosePicture(fromCamera: bool) {
        if (fromCamera)
            pictureDialog.currentFolder = StandardPaths.writableLocation(StandardPaths.PicturesLocation);
        pictureDialog.open();
    }

    readonly property var fingerNames: ({
            "right-index-finger": qsTr("Right Index Finger"),
            "right-middle-finger": qsTr("Right Middle Finger"),
            "right-ring-finger": qsTr("Right Ring Finger"),
            "right-little-finger": qsTr("Right Little Finger"),
            "right-thumb": qsTr("Right Thumb"),
            "left-index-finger": qsTr("Left Index Finger"),
            "left-middle-finger": qsTr("Left Middle Finger"),
            "left-ring-finger": qsTr("Left Ring Finger"),
            "left-little-finger": qsTr("Left Little Finger"),
            "left-thumb": qsTr("Left Thumb")
        })
    readonly property var allFingers: Object.keys(fingerNames)

    function accountType(admin: bool): string {
        return admin ? qsTr("Administrator") : qsTr("Standard User");
    }

    function fileUrlToPath(url: url): string {
        const s = url.toString();
        return s.startsWith("file://") ? decodeURIComponent(s.substring(7)) : "";
    }

    function pictureUrl(path: string): url {
        return path !== "" ? "file://" + encodeURI(path) : "";
    }

    Component.onCompleted: if (sys) sys.refresh()

    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        text: page.sys ? page.sys.error : ""
        shown: text !== ""
    }

    InfoBanner {
        Layout.fillWidth: true
        type: "info"
        closable: true
        shown: page.cameraState === "open"
        text: qsTr("Camera is open. Take your picture, then choose it here.")
        actions: [
            QQC2.Action {
                text: qsTr("Choose the Picture…")
                onTriggered: page.choosePicture(true)
            }
        ]
        onClosed: page.cameraState = ""
    }
    InfoBanner {
        Layout.fillWidth: true
        type: "warning"
        closable: true
        shown: page.cameraState === "missing"
        text: qsTr("No camera app is installed on this computer. Telamon Store has Snapshot, a simple one.")
        actions: [
            QQC2.Action {
                text: qsTr("Open Telamon Store")
                onTriggered: page.run(["telamon-store", "--app", "org.gnome.Snapshot"])
            }
        ]
        onClosed: page.cameraState = ""
    }

    // You, big: your picture and name, and what to change about them. The
    // picture and the name are found by their IDs, as rows are.
    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        Layout.leftMargin: TelamonStyle.spacingLarge
        spacing: TelamonStyle.spacingLarge * 1.5

        Item {
            id: pictureButton
            objectName: "picture"
            readonly property real size: Kirigami.Units.gridUnit * 5.5
            readonly property bool lit: hover.hovered || pictureButton.activeFocus
            Layout.preferredWidth: size
            Layout.preferredHeight: size
            Layout.alignment: Qt.AlignVCenter
            enabled: page.sys !== null && page.sys.available
            activeFocusOnTab: true
            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Change Picture")
            Accessible.focusable: true
            Accessible.onPressAction: pictureDialog.open()
            Keys.onSpacePressed: pictureDialog.open()
            Keys.onReturnPressed: pictureDialog.open()

            AccountAvatar {
                anchors.fill: parent
                size: pictureButton.size
                name: page.sys ? page.sys.meRealName : ""
                source: page.sys ? page.pictureUrl(page.sys.mePicture) : ""
                Accessible.ignored: true
            }
            // Darkens the picture under the pointer or the keyboard focus.
            Rectangle {
                anchors.fill: parent
                radius: width / 2
                color: Qt.alpha("black", 0.35) // telamon-lint: allow-raw (a scrim over a photo is dark in both themes)
                opacity: pictureButton.lit ? 1 : 0
                Behavior on opacity {
                    enabled: !TelamonStyle.reducedMotion
                    NumberAnimation {
                        duration: TelamonStyle.durationShort
                    }
                }
            }
            Rectangle {
                anchors.fill: parent
                anchors.margins: -2
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: TelamonStyle.focus
                visible: pictureButton.activeFocus
            }
            // The camera: takes a new picture with the camera app. A button of
            // its own on the picture (the picture itself opens the file chooser).
            Rectangle {
                id: cameraButton
                objectName: "camera"
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                // Whole, even pixels, so the symbol can sit on the exact middle.
                width: Math.round(Kirigami.Units.gridUnit * 0.9) * 2
                height: width
                radius: width / 2
                color: cameraHover.hovered || cameraButton.activeFocus ? TelamonStyle.accent : TelamonStyle.surfaceRaised
                border.width: 1
                border.color: TelamonStyle.controlBorder
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Take a Picture")
                Accessible.focusable: true
                Accessible.onPressAction: page.openCamera()
                Keys.onSpacePressed: page.openCamera()
                Keys.onReturnPressed: page.openCamera()

                Behavior on color {
                    enabled: !TelamonStyle.reducedMotion
                    ColorAnimation {
                        duration: TelamonStyle.durationShort
                    }
                }
                Symbol {
                    anchors.centerIn: parent
                    icon: Symbols.PhotoCamera
                    size: Math.round(parent.width * 0.275) * 2
                    color: cameraHover.hovered || cameraButton.activeFocus ? TelamonStyle.accentText : TelamonStyle.text
                }
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -2
                    radius: width / 2
                    color: "transparent"
                    border.width: 2
                    border.color: TelamonStyle.focus
                    visible: cameraButton.activeFocus
                }
                HoverHandler {
                    id: cameraHover
                    cursorShape: Qt.PointingHandCursor
                }
                // Takes the press, so it doesn't also open the file chooser.
                TapHandler {
                    gesturePolicy: TapHandler.ReleaseWithinBounds
                    onTapped: page.openCamera()
                }
                TelamonToolTip {
                    text: qsTr("Take a Picture")
                    shown: cameraHover.hovered || cameraButton.activeFocus
                }
            }
            HoverHandler {
                id: hover
                cursorShape: Qt.PointingHandCursor
            }
            TapHandler {
                onTapped: pictureDialog.open()
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: Kirigami.Units.largeSpacing

            ColumnLayout {
                objectName: "name"
                Layout.fillWidth: true
                spacing: 0

                TelamonLabel {
                    Layout.fillWidth: true
                    textStyle: TelamonLabel.Title
                    text: page.sys && page.sys.meRealName !== "" ? page.sys.meRealName : (page.sys && page.sys.meName !== "" ? page.sys.meName : qsTr("Your Account"))
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    Accessible.role: Accessible.Heading
                }
                TelamonLabel {
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: page.sys && page.sys.meName !== "" ? qsTr("%1 · %2").arg(page.sys.meName).arg(page.accountType(page.sys.meAdmin)) : ""
                    opacity: 0.65
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                }
            }
            RowLayout {
                spacing: Kirigami.Units.smallSpacing

                SecondaryButton {
                    text: qsTr("Change Picture")
                    enabled: page.sys !== null && page.sys.available
                    onClicked: pictureDialog.open()
                }
                SecondaryButton {
                    text: qsTr("Edit Name")
                    enabled: page.sys !== null && page.sys.available && !page.sys.busy
                    onClicked: nameSheet.open()
                }
            }
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Sign-In Options")

        SectionRow {
            objectName: "password"
            title: qsTr("Password")
            subtitle: qsTr("Change the password you sign in with")
            chevron: true
            enabled: page.sys !== null && page.sys.available
            leading: Symbol {
                icon: Symbols.Key
            }
            onClicked: passwordSheet.open()
        }
        SectionRow {
            objectName: "fingerprint"
            visible: page.sys !== null && page.sys.hasReader
            title: qsTr("Fingerprint")
            subtitle: qsTr("Unlock and approve changes with a touch")
            value: {
                const n = page.sys ? page.sys.fingers.length : 0;
                return n === 0 ? qsTr("None Added") : n === 1 ? qsTr("1 Finger") : qsTr("%1 Fingers").arg(n);
            }
            chevron: true
            leading: Symbol {
                icon: Symbols.Fingerprint
            }
            onClicked: fingerSheet.open()
        }
        SectionRow {
            objectName: "auto-login"
            title: qsTr("Automatic Login")
            subtitle: qsTr("Sign in as %1 when the computer starts, without a password").arg(page.sys && page.sys.meName !== "" ? page.sys.meName : qsTr("you"))
            showSwitch: true
            switchChecked: page.sys ? page.sys.autoLogin : false
            enabled: page.sys !== null && page.sys.available && !page.sys.busy
            leading: Symbol {
                icon: Symbols.Login
            }
            onSwitchToggled: checked => page.sys.changeAutoLogin(checked)
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Other Users")
        visible: page.sys !== null && page.sys.available

        Repeater {
            model: page.others

            SectionRow {
                id: otherRow
                required property var modelData
                Layout.fillWidth: true
                title: otherRow.modelData.realName
                subtitle: otherRow.modelData.name
                value: page.accountType(otherRow.modelData.admin)
                chevron: true
                leading: AccountAvatar {
                    name: otherRow.modelData.realName
                    source: page.pictureUrl(otherRow.modelData.picture)
                    size: Kirigami.Units.gridUnit * 2
                }
                onClicked: {
                    page.person = otherRow.modelData;
                    personSheet.open();
                }
            }
        }
        SectionRow {
            visible: page.others.length === 0
            title: qsTr("Only you use this computer.")
        }
        SectionRow {
            objectName: "add-user"
            title: qsTr("Add User")
            subtitle: qsTr("Give someone else their own account")
            chevron: true
            enabled: page.sys !== null && !page.sys.busy
            leading: Symbol {
                icon: Symbols.PersonAdd
            }
            onClicked: addSheet.open()
        }
    }

    // Only the login screen's own settings (the registry's folded KCM).
    AdvancedSection {
        page: page
    }

    RelatedLinks {
        page: page
    }

    // Your full name.
    TelamonDialog {
        id: nameSheet
        title: qsTr("Edit Name")
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            nameField.text = page.sys ? page.sys.meRealName : "";
            nameField.selectAll();
            nameField.forceActiveFocus();
        }
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: nameSheet.close()
            },
            PrimaryButton {
                text: qsTr("Save")
                enabled: page.sys !== null && page.sys.validRealName(nameField.text) && nameField.text.trim() !== page.sys.meRealName
                onClicked: {
                    page.sys.changeName(page.sys.meUid, nameField.text.trim());
                    nameSheet.close();
                }
            }
        ]

        TelamonLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("This is the name shown on the sign-in screen and in the menu.")
        }
        TelamonTextField {
            id: nameField
            Layout.fillWidth: true
            maximumLength: 128
            placeholderText: qsTr("Full name")
            onAccepted: if (page.sys && page.sys.validRealName(text) && text.trim() !== page.sys.meRealName) {
                page.sys.changeName(page.sys.meUid, text.trim());
                nameSheet.close();
            }
        }
    }

    // Your picture: a file, picked in the system's file dialog.
    FileDialog {
        id: pictureDialog
        title: qsTr("Choose a Picture")
        nameFilters: [qsTr("Pictures (*.png *.jpg *.jpeg *.webp *.gif *.svg)")]
        onAccepted: {
            const path = page.fileUrlToPath(selectedFile);
            if (page.sys && page.sys.validPicture(path))
                page.sys.changePicture(page.sys.meUid, path);
        }
    }

    // Your password.
    TelamonDialog {
        id: passwordSheet
        title: qsTr("Change Password")
        preferredWidth: Kirigami.Units.gridUnit * 26
        readonly property bool matches: newPassword.text === confirmPassword.text
        readonly property bool ready: newPassword.text.length > 0 && matches
        function clear() {
            newPassword.text = "";
            confirmPassword.text = "";
        }
        function submit() {
            if (!ready)
                return;
            page.sys.changePassword(page.sys.meUid, newPassword.text);
            passwordSheet.close();
        }
        onOpened: {
            clear();
            newPassword.forceActiveFocus();
        }
        onClosed: clear()
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: passwordSheet.close()
            },
            PrimaryButton {
                text: qsTr("Change Password")
                enabled: passwordSheet.ready
                onClicked: passwordSheet.submit()
            }
        ]

        TelamonLabel {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("You may be asked for your current password to confirm.")
        }
        TelamonPasswordField {
            id: newPassword
            Layout.fillWidth: true
            placeholderText: qsTr("New password")
            onAccepted: confirmPassword.forceActiveFocus()
            Accessible.name: qsTr("New password")
        }
        TelamonPasswordStrength {
            Layout.fillWidth: true
            score: page.sys ? page.sys.passwordScore(newPassword.text) : -1
        }
        TelamonPasswordField {
            id: confirmPassword
            Layout.fillWidth: true
            placeholderText: qsTr("Type it again")
            errorText: confirmPassword.text.length > 0 && !passwordSheet.matches ? qsTr("The passwords don't match") : ""
            onAccepted: passwordSheet.submit()
            Accessible.name: qsTr("New password again")
        }
    }

    // Fingerprints: the ones added, and adding one.
    TelamonDialog {
        id: fingerSheet
        title: qsTr("Fingerprint")
        preferredWidth: Kirigami.Units.gridUnit * 26
        readonly property bool scanning: page.sys !== null && page.sys.enrollState !== "idle" && page.sys.enrollState !== ""
        readonly property var freeFingers: page.allFingers.filter(f => !(page.sys ? page.sys.fingers : []).includes(f))
        onOpened: {
            if (page.sys)
                page.sys.cancelEnroll();
        }
        onClosed: if (page.sys) page.sys.cancelEnroll()
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: fingerSheet.close()
            }
        ]

        TelamonLabel {
            Layout.fillWidth: true
            visible: !fingerSheet.scanning
            wrapMode: Text.Wrap
            text: page.sys && page.sys.readerName !== "" ? qsTr("Reader: %1").arg(page.sys.readerName) : ""
        }

        Section {
            Layout.fillWidth: true
            visible: !fingerSheet.scanning && page.sys !== null && page.sys.fingers.length > 0

            Repeater {
                model: page.sys ? page.sys.fingers : []

                SectionRow {
                    id: fingerRow
                    required property string modelData
                    Layout.fillWidth: true
                    title: page.fingerNames[fingerRow.modelData] ?? fingerRow.modelData
                    leading: Symbol {
                        icon: Symbols.Fingerprint
                    }
                }
            }
        }

        Section {
            Layout.fillWidth: true
            visible: !fingerSheet.scanning

            SectionRow {
                title: qsTr("Finger to Add")
                visible: fingerSheet.freeFingers.length > 0

                TelamonComboBox {
                    id: fingerBox
                    width: Kirigami.Units.gridUnit * 12
                    model: fingerSheet.freeFingers.map(f => page.fingerNames[f])
                    Accessible.name: qsTr("Finger to add")
                }
            }
            SectionRow {
                title: qsTr("Add Fingerprint")
                visible: fingerSheet.freeFingers.length > 0
                clickable: true
                leading: Symbol {
                    icon: Symbols.Add
                }
                onClicked: page.sys.startEnroll(fingerSheet.freeFingers[fingerBox.currentIndex])
            }
            SectionRow {
                title: qsTr("Remove All Fingerprints")
                visible: page.sys !== null && page.sys.fingers.length > 0
                clickable: true
                leading: Symbol {
                    icon: Symbols.Delete
                }
                onClicked: removeFingers.open()
            }
        }

        // While a finger is being scanned.
        ColumnLayout {
            Layout.fillWidth: true
            visible: fingerSheet.scanning
            spacing: Kirigami.Units.largeSpacing

            Symbol {
                Layout.alignment: Qt.AlignHCenter
                icon: Symbols.Fingerprint
                size: Kirigami.Units.iconSizes.huge
                opacity: page.sys && page.sys.enrollState === "scanning" ? 1 : 0.6
            }
            TelamonLabel {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: page.sys ? page.sys.enrollMessage : ""
            }
            TelamonProgressBar {
                Layout.fillWidth: true
                value: page.sys && page.sys.enrollOf > 0 ? page.sys.enrollDone / page.sys.enrollOf : 0
                status: page.sys && page.sys.enrollState === "failed" ? "error" : "normal"
                text: page.sys ? qsTr("%1 of %2").arg(page.sys.enrollDone).arg(page.sys.enrollOf) : ""
            }
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                SecondaryButton {
                    visible: page.sys !== null && page.sys.enrollState === "scanning"
                    text: qsTr("Cancel")
                    onClicked: page.sys.cancelEnroll()
                }
                PrimaryButton {
                    visible: page.sys !== null && page.sys.enrollState !== "scanning"
                    text: qsTr("OK")
                    onClicked: page.sys.cancelEnroll()
                }
            }
        }
    }

    ConfirmDialog {
        id: removeFingers
        title: qsTr("Remove All Fingerprints?")
        text: qsTr("You will sign in and unlock with your password only.")
        acceptText: qsTr("Remove")
        destructive: true
        onAccepted: page.sys.removeFingerprints()
    }

    // Another person: their type, and removing them.
    TelamonDialog {
        id: personSheet
        title: page.person.realName ?? ""
        preferredWidth: Kirigami.Units.gridUnit * 24
        footerContent: [
            PrimaryButton {
                text: qsTr("Done")
                onClicked: personSheet.close()
            }
        ]

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            AccountAvatar {
                name: page.person.realName ?? ""
                source: page.pictureUrl(page.person.picture ?? "")
                size: Kirigami.Units.gridUnit * 3
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                TelamonLabel {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: page.person.realName ?? ""
                    font.bold: true
                }
                TelamonLabel {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: page.person.name ?? ""
                    opacity: 0.7
                }
            }
        }
        Section {
            Layout.fillWidth: true

            SectionRow {
                title: qsTr("Administrator")
                subtitle: qsTr("Can change settings for everyone and install software")
                showSwitch: true
                switchChecked: page.person.admin ?? false
                enabled: page.sys !== null && !page.sys.busy
                onSwitchToggled: checked => {
                    page.sys.changeAdmin(page.person.uid, checked);
                    personSheet.close();
                }
            }
            SectionRow {
                title: qsTr("Remove User")
                clickable: true
                enabled: page.sys !== null && !page.sys.busy
                leading: Symbol {
                    icon: Symbols.Delete
                }
                onClicked: removeUser.open()
            }
        }
    }

    ConfirmDialog {
        id: removeUser
        title: qsTr("Remove %1?").arg(page.person.realName ?? "")
        text: qsTr("They won't be able to sign in. You can keep their files or delete them too.")
        acceptText: qsTr("Remove, Keep Files")
        alternativeText: qsTr("Remove and Delete Files")
        defaultButton: "reject"
        destructive: true
        onAccepted: {
            page.sys.removeUser(page.person.uid, false);
            personSheet.close();
        }
        onAlternative: {
            page.sys.removeUser(page.person.uid, true);
            personSheet.close();
        }
    }

    // Add User.
    TelamonDialog {
        id: addSheet
        title: qsTr("Add User")
        preferredWidth: Kirigami.Units.gridUnit * 26
        property bool nameEdited: false
        readonly property bool matches: addPassword.text === addConfirm.text
        readonly property bool ready: page.sys !== null && page.sys.validRealName(addName.text) && page.sys.validUserName(addUser.text) && addPassword.text.length > 0 && matches
        function clear() {
            addName.text = "";
            addUser.text = "";
            addPassword.text = "";
            addConfirm.text = "";
            addAdmin.switchChecked = false;
            nameEdited = false;
        }
        function submit() {
            if (!ready)
                return;
            page.sys.addUser(addUser.text, addName.text.trim(), addAdmin.switchChecked, addPassword.text);
            addSheet.close();
        }
        onOpened: {
            clear();
            addName.forceActiveFocus();
        }
        onClosed: clear()
        footerContent: [
            SecondaryButton {
                text: qsTr("Cancel")
                onClicked: addSheet.close()
            },
            PrimaryButton {
                text: qsTr("Add User")
                enabled: addSheet.ready
                onClicked: addSheet.submit()
            }
        ]

        TelamonTextField {
            id: addName
            Layout.fillWidth: true
            maximumLength: 128
            placeholderText: qsTr("Full name")
            onTextChanged: if (!addSheet.nameEdited && page.sys)
                addUser.text = page.sys.suggestUserName(text)
            onAccepted: addUser.forceActiveFocus()
            Accessible.name: qsTr("Full name")
        }
        TelamonTextField {
            id: addUser
            Layout.fillWidth: true
            maximumLength: 32
            placeholderText: qsTr("Username")
            errorText: addUser.text.length > 0 && page.sys && !page.sys.validUserName(addUser.text) ? qsTr("Use lowercase letters, digits, - and _") : ""
            onTextEdited: addSheet.nameEdited = true
            onAccepted: addPassword.forceActiveFocus()
            Accessible.name: qsTr("Username")
        }
        TelamonPasswordField {
            id: addPassword
            Layout.fillWidth: true
            placeholderText: qsTr("Password")
            onAccepted: addConfirm.forceActiveFocus()
            Accessible.name: qsTr("Password")
        }
        TelamonPasswordStrength {
            Layout.fillWidth: true
            score: page.sys ? page.sys.passwordScore(addPassword.text) : -1
        }
        TelamonPasswordField {
            id: addConfirm
            Layout.fillWidth: true
            placeholderText: qsTr("Type it again")
            errorText: addConfirm.text.length > 0 && !addSheet.matches ? qsTr("The passwords don't match") : ""
            onAccepted: addSheet.submit()
            Accessible.name: qsTr("Password again")
        }
        Section {
            Layout.fillWidth: true

            SectionRow {
                id: addAdmin
                title: qsTr("Administrator")
                subtitle: qsTr("Can change settings for everyone and install software")
                showSwitch: true
                onSwitchToggled: checked => addAdmin.switchChecked = checked
            }
        }
    }
}
