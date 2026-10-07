pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Atlas.Ui

// Settings' window: a short list of pages down a sidebar, with a search over
// pages and the settings on them, and the page beside it.
AtlasWindow {
    id: root

    // Set from main.cpp through setInitialProperties().
    // The registry, search and launch arguments (src/backend.rs).
    required property var backend
    // Starts kcmshell6 and Atlas Updater (cpp/launcher.h).
    required property var launcher
    // The installed KCMs, for More Settings (cpp/kcmcatalog.h).
    required property var kcmCatalog
    // Makes a page's backends while it is shown (cpp/pagebackends.h).
    required property var pageBackends

    // The pages in sidebar order (settings-registry), read once. Other
    // Plasma Settings ("more") opens from System, not the sidebar.
    readonly property var pages: JSON.parse(backend.pagesJson())
    readonly property var sidebarPages: pages.filter(p => p.kind !== "more")
    // The page shown, and the setting on it a link asked for ("" for none).
    property string pageId: ""
    property string itemId: ""
    readonly property var currentPage: pages.find(p => p.id === pageId) ?? null
    readonly property bool searching: searchField.query.trim() !== ""
    // A refused argument or a program that didn't start; "" for none.
    property string notice: ""

    // The first page of a new install.
    readonly property string firstPage: "home"

    // The pages that are built, by ID; any other shows PendingPage. A page
    // is made only while it is shown, and its backend with it.
    readonly property var nativePages: ({
            "time-language": timeLanguagePage,
            "system": systemPage,
            "home": homePage,
            "network": networkPage,
            "devices": devicesPage,
            "displays": displaysPage,
            "sound": soundPage,
            "appearance": appearancePage,
            "input": inputPage,
            "notifications": notificationsPage,
            "accessibility": accessibilityPage,
            "power": powerPage,
            "users": usersPage,
            "privacy": privacyPage,
            "apps": appsPage
        })

    title: AtlasApp.name
    width: Kirigami.Units.gridUnit * 56
    height: Kirigami.Units.gridUnit * 40
    minimumWidth: Kirigami.Units.gridUnit * 22
    minimumHeight: Kirigami.Units.gridUnit * 20
    stateKey: "main"
    visible: true
    LayoutMirroring.enabled: Qt.locale().textDirection === Qt.RightToLeft
    LayoutMirroring.childrenInherit: true

    // Shows page `id` from its top, or at setting `item`.
    function openPage(id: string, item: string) {
        if (!pages.some(p => p.id === id))
            return;
        // The page again (a sidebar click on the page shown, a link to
        // another setting on it): made anew, so it opens at its top or at
        // the setting with Advanced closed, never where it was left.
        const again = id === pageId && !searching;
        searchField.text = "";
        pageId = id;
        itemId = item;
        saved.setValue("Page", id);
        if (again) {
            contentLoader.active = false;
            contentLoader.active = true;
        }
    }

    // The page shown last time, or the first page.
    function openHome() {
        const last = saved.value("Page", firstPage);
        openPage(pages.some(p => p.id === last) ? last : firstPage, "");
    }

    function openKcm(name: string, args: string) {
        const cmd = backend.kcmCommand(name, args);
        if (cmd.length === 0) {
            notice = qsTr("“%1” isn't a settings module.").arg(name);
            return;
        }
        // kcmshell6 itself would fail without a word to the user.
        if (!kcmCatalog.installed(cmd[1])) {
            notice = qsTr("The settings module “%1” isn't installed.").arg(cmd[1]);
            return;
        }
        run(cmd);
    }

    function run(argv: var) {
        if (!launcher.run(argv))
            notice = qsTr("Couldn't start %1.").arg(argv[0] ?? "");
    }

    // A page's entry in the sidebar.
    component NavItem: SidebarItem {
        required property var modelData
        Layout.fillWidth: true
        text: modelData.title
        symbol: Symbols.codepoint(modelData.symbol)
        selected: !root.searching && root.pageId === modelData.id
        // Always the page's top, never a spot left from an earlier visit.
        onClicked: root.openPage(modelData.id, "")
    }

    AtlasSettings {
        id: saved
        group: "Window"
    }

    Connections {
        target: root.backend
        function onRequested(kind: string, first: string, second: string) {
            switch (kind) {
            case "page":
                root.openPage(first, second);
                break;
            case "kcm":
                root.openPage("more", "");
                root.openKcm(first, second);
                break;
            case "search":
                if (root.pageId === "")
                    root.openHome();
                searchField.text = first;
                searchField.forceActiveFocus();
                break;
            default:
                // A plain launch keeps the page a running window shows.
                if (root.pageId === "")
                    root.openHome();
                break;
            }
        }
        function onRefused(text: string) {
            root.notice = qsTr("Ignored %1.").arg(text);
        }
    }

    Connections {
        target: root.launcher
        function onFailed(program: string, message: string) {
            root.notice = qsTr("Couldn't start %1: %2").arg(program).arg(message);
        }
    }

    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: searchField.forceActiveFocus()
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            // A layout in a layout fills by default; the page takes the rest.
            Layout.fillWidth: false
            Layout.fillHeight: true
            Layout.preferredWidth: sidebar.compact ? Kirigami.Units.gridUnit * 4 : Kirigami.Units.gridUnit * 14
            spacing: 0

            // The search field sits here; a folded sidebar is too narrow for
            // it, so then it moves above the page (compactSearchSlot).
            Item {
                id: wideSearchSlot
                Layout.fillWidth: true
                implicitHeight: sidebar.compact ? 0 : searchField.implicitHeight + Kirigami.Units.largeSpacing * 2
            }

            AtlasSidebar {
                id: sidebar
                Layout.fillWidth: true
                Layout.fillHeight: true
                compact: root.sidebarCollapsed

                Repeater {
                    model: root.sidebarPages
                    NavItem {}
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            Item {
                id: compactSearchSlot
                Layout.fillWidth: true
                implicitHeight: sidebar.compact ? searchField.implicitHeight + Kirigami.Units.largeSpacing * 2 : 0
            }

            InfoBanner {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                type: "warning"
                text: root.notice
                shown: root.notice !== ""
                closable: true
                onClosed: root.notice = ""
            }

            Loader {
                id: contentLoader
                Layout.fillWidth: true
                Layout.fillHeight: true
                sourceComponent: {
                    if (root.searching)
                        return searchPage;
                    if (!root.currentPage)
                        return null;
                    if (root.currentPage.kind === "more")
                        return morePage;
                    return root.nativePages[root.currentPage.id] ?? pendingPage;
                }
            }
        }
    }

    // One field, in whichever slot is in use, so its text and focus never
    // have to be kept in step between two. The slots are never hidden (that
    // would take the field's focus): the unused one is 0 high.
    SearchField {
        id: searchField
        parent: sidebar.compact ? compactSearchSlot : wideSearchSlot
        x: Kirigami.Units.largeSpacing
        y: Kirigami.Units.largeSpacing
        width: parent.width - Kirigami.Units.largeSpacing * 2
        placeholderText: qsTr("Search Settings")
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Escape && text !== "") {
                text = "";
                event.accepted = true;
            } else {
                const results = contentLoader.item as SearchPage;
                if (results)
                    event.accepted = results.handleKey(event);
            }
        }
    }

    Component {
        id: searchPage
        SearchPage {
            backend: root.backend
            query: searchField.query.trim()
            onChosen: (pageId, itemId, kcm) => {
                root.openPage(pageId, itemId);
                if (kcm !== "")
                    root.openKcm(kcm, "");
            }
        }
    }
    Component {
        id: pendingPage
        PendingPage {
            entry: root.currentPage
            onOpenKcm: name => root.openKcm(name, "")
        }
    }
    // The native pages, each wired to the window the same way.
    Component {
        id: timeLanguagePage
        TimeLanguagePage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: systemPage
        SystemPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: homePage
        HomePage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: networkPage
        NetworkPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: devicesPage
        DevicesPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: displaysPage
        DisplaysPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: soundPage
        SoundPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: appearancePage
        AppearancePage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: inputPage
        InputPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: notificationsPage
        NotificationsPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: accessibilityPage
        AccessibilityPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: powerPage
        PowerPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: usersPage
        UsersPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: privacyPage
        PrivacyPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: appsPage
        AppsPage {
            entry: root.currentPage
            itemId: root.itemId
            pages: root.pages
            pageBackends: root.pageBackends
            onOpenPage: (id, item) => root.openPage(id, item)
            onOpenKcm: name => root.openKcm(name, "")
            onRun: argv => root.run(argv)
        }
    }
    Component {
        id: morePage
        MoreSettingsPage {
            backend: root.backend
            kcmCatalog: root.kcmCatalog
            onOpenKcm: name => root.openKcm(name, "")
        }
    }
}
