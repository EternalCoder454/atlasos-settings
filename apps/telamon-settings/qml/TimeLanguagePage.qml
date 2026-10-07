pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Telamon.Ui

// Time & Language: network time and the time zone (timedated), the
// language and 24-hour time (Plasma's plasma-localerc, as its Region &
// Language page writes it), and under Advanced the formats and the login
// screen's language (localed).
SettingsPage {
    id: page

    // src/time_language.rs and cpp/localeconfig.h; they go with the page.
    readonly property var sys: pageBackends ? pageBackends.create("time-language", page) : null
    readonly property var localeConfig: pageBackends ? pageBackends.create("locale-config", page) : null
    // plasma-localerc as read: {lang, language, time, numeric}.
    property var prefs: localeConfig ? localeConfig.read() : ({})
    // A write to plasma-localerc failed, or a change waits for sign-in.
    property string localeNotice: ""
    property bool changedLanguage: false

    // The locale the session runs with when plasma-localerc says nothing.
    readonly property string sessionLocale: Qt.locale().name
    readonly property string language: prefs.lang || sessionLocale
    readonly property string formats: prefs.numeric || language
    readonly property string timeLocale: prefs.time || formats
    readonly property bool uses24Hour: page.is24Hour(timeLocale)

    function is24Hour(locale: string): bool {
        return !/a/i.test(Qt.locale(locale).timeFormat(Locale.ShortFormat));
    }

    // "Deutsch (Deutschland)" for "de_DE.UTF-8".
    function languageName(locale: string): string {
        if (locale === "" || locale.startsWith("C") || locale === "POSIX")
            return qsTr("Not Set");
        const l = Qt.locale(locale);
        const name = l.nativeLanguageName;
        if (name === "")
            return locale;
        const cap = name.charAt(0).toLocaleUpperCase(locale) + name.slice(1);
        return l.nativeTerritoryName !== "" ? qsTr("%1 (%2)").arg(cap).arg(l.nativeTerritoryName) : cap;
    }

    // "Berlin" and "Europe" for "Europe/Berlin".
    function zoneCity(zone: string): string {
        return zone.split("/").pop().replace(/_/g, " ");
    }
    function zoneRegion(zone: string): string {
        return zone.split("/").slice(0, -1).join(" › ").replace(/_/g, " ");
    }

    // The languages offered, with the ones set now when they aren't listed.
    function languageChoices(extra: var): var {
        const list = page.sys ? page.sys.languages() : [];
        for (const l of extra) {
            if (l !== "" && !list.includes(l) && page.sys && page.sys.validLocale(l))
                list.push(l);
        }
        return list.map(l => ({
                    title: page.languageName(l),
                    subtitle: "",
                    value: l,
                    keys: l.toLowerCase()
                })).sort((a, b) => a.title.localeCompare(b.title));
    }

    // A locale of the same language as the formats that writes times with
    // (or without) AM and PM, for LC_TIME; "" when the formats already do.
    function timeLocaleFor(want24: bool): string {
        if (page.is24Hour(page.formats) === want24)
            return "";
        const lang = page.formats.split(/[_.@]/)[0];
        const regions = want24 ? ["GB", "IE", "DE", "FR", "ES", "IT", "NL", "SE", "DK", "PT", "BR", "RU", "JP", "CN"] : ["US", "CA", "AU", "IN", "PH", "NZ", "EG", "MX", "KR", "TW"];
        for (const r of regions) {
            const candidate = lang + "_" + r;
            if (Qt.locale(candidate).name === candidate && page.is24Hour(candidate) === want24)
                return candidate + ".UTF-8";
        }
        return want24 ? "en_GB.UTF-8" : "en_US.UTF-8";
    }

    function wrote(ok: bool) {
        if (!ok)
            page.localeNotice = qsTr("Settings couldn't save the language settings.");
        else
            page.changedLanguage = true;
        page.prefs = page.localeConfig.read();
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
        type: "warning"
        text: page.localeNotice
        shown: text !== ""
        closable: true
        onClosed: page.localeNotice = ""
    }
    InfoBanner {
        Layout.fillWidth: true
        type: "information"
        text: qsTr("The new language and formats are used after you sign out and back in.")
        shown: page.changedLanguage
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Time")

        SectionRow {
            objectName: "automatic"
            title: qsTr("Set Time Automatically")
            subtitle: {
                if (!page.sys || !page.sys.available)
                    return "";
                if (!page.sys.canNtp)
                    return qsTr("No time service is installed");
                if (page.sys.ntp)
                    return page.sys.synced ? qsTr("Synchronized with time servers") : qsTr("Waiting for time servers");
                return "";
            }
            showSwitch: true
            switchChecked: page.sys ? page.sys.ntp : false
            enabled: page.sys !== null && page.sys.available && page.sys.canNtp && !page.sys.busy
            onSwitchToggled: checked => page.sys.changeNtp(checked)
        }
        SectionRow {
            objectName: "timezone"
            title: qsTr("Time Zone")
            value: page.sys && page.sys.timezone !== "" ? page.zoneCity(page.sys.timezone) : ""
            subtitle: page.sys && page.sys.timezone !== "" ? page.zoneRegion(page.sys.timezone) : ""
            chevron: true
            busy: page.sys !== null && page.sys.busy
            enabled: page.sys !== null && page.sys.available
            onClicked: {
                page.sys.loadZones();
                zoneSheet.open();
            }
        }
        SectionRow {
            objectName: "time-format"
            title: qsTr("24-Hour Time")
            subtitle: Qt.locale(page.timeLocale).toString(new Date(2026, 0, 1, 15, 30), Qt.locale(page.timeLocale).timeFormat(Locale.ShortFormat))
            showSwitch: true
            switchChecked: page.uses24Hour
            enabled: page.localeConfig !== null
            onSwitchToggled: checked => page.wrote(page.localeConfig.setTimeFormat(page.timeLocaleFor(checked)))
        }
    }

    Section {
        Layout.fillWidth: true
        title: qsTr("Language")

        SectionRow {
            objectName: "language"
            title: qsTr("Language")
            value: page.languageName(page.language)
            chevron: true
            enabled: page.localeConfig !== null
            onClicked: languageSheet.open()
        }
    }

    AdvancedSection {
        page: page

        SectionRow {
            objectName: "formats"
            title: qsTr("Formats")
            subtitle: qsTr("Numbers, dates, currency and measurements")
            enabled: page.localeConfig !== null

            TelamonComboBox {
                id: formatsBox
                readonly property var locales: page.sys ? [""].concat(page.sys.languages()) : [""]
                width: Kirigami.Units.gridUnit * 12
                filterable: true
                model: formatsBox.locales.map(l => l === "" ? qsTr("Same as Language") : page.languageName(l))
                currentIndex: Math.max(0, formatsBox.locales.indexOf(page.prefs.numeric ?? ""))
                onActivated: index => page.wrote(page.localeConfig.setFormats(formatsBox.locales[index] ?? ""))
                Accessible.name: qsTr("Formats")
            }
        }
        SectionRow {
            objectName: "system-language"
            title: qsTr("Login Screen Language")
            subtitle: qsTr("Also used for new users")
            value: page.sys ? page.languageName(page.sys.systemLanguage) : ""
            chevron: true
            busy: page.sys !== null && page.sys.busy
            enabled: page.sys !== null && page.sys.loaded
            onClicked: systemLanguageSheet.open()
        }
    }

    RelatedLinks {
        page: page
    }

    PickerSheet {
        id: zoneSheet
        title: qsTr("Time Zone")
        current: page.sys ? page.sys.timezone : ""
        placeholderText: page.sys && page.sys.zones.length === 0 ? qsTr("Loading…") : qsTr("No Time Zone Found")
        choices: (page.sys ? page.sys.zones : []).map(z => ({
                    title: page.zoneCity(z),
                    subtitle: page.zoneRegion(z),
                    value: z,
                    keys: z.toLowerCase()
                }))
        onChosen: value => page.sys.changeTimezone(value)
    }

    PickerSheet {
        id: languageSheet
        title: qsTr("Language")
        current: page.language
        choices: page.languageChoices([page.language])
        onChosen: value => page.wrote(page.localeConfig.setLanguage(value))
    }

    PickerSheet {
        id: systemLanguageSheet
        title: qsTr("Login Screen Language")
        current: page.sys ? page.sys.systemLanguage : ""
        choices: page.languageChoices([page.sys ? page.sys.systemLanguage : "", page.language])
        onChosen: value => page.sys.changeSystemLanguage(value)
    }
}
