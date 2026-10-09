// Tests for the KConfig-backed settings: each writes into a temporary
// XDG_CONFIG_HOME (never the real one) and checks the exact keys and values
// in the files, as the KCMs write them.

#include "../cpp/autostartconfig.h"
#include "../cpp/colorscheme.h"
#include "../cpp/defaultapps.h"
#include "../cpp/localeconfig.h"
#include "../cpp/powerconfig.h"
#include "../cpp/screenlockconfig.h"

#include <QFile>
#include <QTemporaryDir>
#include <QtTest>

using namespace Qt::StringLiterals;

class ConfigTests : public QObject
{
    Q_OBJECT

    QTemporaryDir m_dir;

    QString path(const QString &name) const
    {
        return m_dir.filePath(name);
    }

    // The file's lines, without blanks, in order.
    QStringList lines(const QString &name) const
    {
        QFile f(path(name));
        if (!f.open(QIODevice::ReadOnly)) {
            return {};
        }
        QStringList out;
        for (const QString &l : QString::fromUtf8(f.readAll()).split(u'\n')) {
            if (!l.trimmed().isEmpty()) {
                out << l;
            }
        }
        return out;
    }

    void put(const QString &name, const QByteArray &text) const
    {
        QFile f(path(name));
        QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
        f.write(text);
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY(m_dir.isValid());
        qputenv("XDG_CONFIG_HOME", m_dir.path().toUtf8());
        QStandardPaths::setTestModeEnabled(false);
        // The system's autostart folder and applications, as fixtures.
        qputenv("XDG_CONFIG_DIRS", path(u"sys"_s).toUtf8());
        qputenv("XDG_DATA_HOME", path(u"data"_s).toUtf8());
        qputenv("XDG_DATA_DIRS", path(u"share"_s).toUtf8());
        qputenv("XDG_CACHE_HOME", path(u"cache"_s).toUtf8());
        QVERIFY(QDir().mkpath(path(u"sys/autostart"_s)));
        QVERIFY(QDir().mkpath(path(u"share/applications"_s)));
        // The shared MIME database, which KService needs for file types.
        QVERIFY(QFile::link(u"/usr/share/mime"_s, path(u"share/mime"_s)));
        auto desktop = [&](const QString &where, const QString &name, const QByteArray &body) {
            put(where + u'/' + name, "[Desktop Entry]\nType=Application\n" + body);
        };
        desktop(u"sys/autostart"_s, u"sys-shown.desktop"_s, "Name=System Shown\nExec=sys-shown\nIcon=sys-icon\n");
        desktop(u"sys/autostart"_s, u"sys-hidden.desktop"_s, "Name=Not For People\nExec=x\nNoDisplay=true\n");
        desktop(u"sys/autostart"_s, u"sys-gnome.desktop"_s, "Name=Gnome Only\nExec=x\nOnlyShowIn=GNOME;\n");
        desktop(u"sys/autostart"_s, u"sys-off.desktop"_s, "Name=Off By Itself\nExec=x\nHidden=true\n");
        desktop(u"share/applications"_s, u"browser-a.desktop"_s,
                "Name=Browser A\nExec=browser-a %u\nCategories=Network;WebBrowser;\n"
                "MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\n");
        desktop(u"share/applications"_s, u"browser-b.desktop"_s,
                "Name=Browser B\nExec=browser-b %u\nCategories=Network;WebBrowser;\n"
                "MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\n");
        desktop(u"share/applications"_s, u"term-a.desktop"_s, "Name=Terminal A\nExec=term-a --new-window %F\nCategories=System;TerminalEmulator;\n");
        desktop(u"share/applications"_s, u"term-b.desktop"_s, "Name=Terminal B\nExec=term-b\nCategories=System;TerminalEmulator;\n");
        desktop(u"share/applications"_s, u"viewer-a.desktop"_s, "Name=Viewer A\nExec=viewer-a %f\nMimeType=image/png;image/jpeg;\n");
        desktop(u"share/applications"_s, u"viewer-b.desktop"_s, "Name=Viewer B\nExec=viewer-b %f\nMimeType=image/png;\n");
        desktop(u"share/applications"_s, u"hidden.desktop"_s, "Name=Hidden App\nExec=h\nNoDisplay=true\nCategories=WebBrowser;\n");
    }

    void init()
    {
        QFile::remove(path(u"powerdevilrc"_s));
        QFile::remove(path(u"kscreenlockerrc"_s));
        QFile::remove(path(u"mimeapps.list"_s));
        QFile::remove(path(u"kdeglobals"_s));
        QDir(path(u"autostart"_s)).removeRecursively();
    }

    void powerDefaultsWithNoFile()
    {
        PowerConfig c;
        QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 600);
        QCOMPARE(ac[u"sleep"_s].toInt(), 900);
        QCOMPARE(ac[u"lid"_s].toInt(), 1);
        QCOMPARE(c.read(u"Battery"_s)[u"sleep"_s].toInt(), 600);
        QVERIFY(c.read(u"Nonsense"_s).isEmpty());
    }

    void powerReadsWhatThePowerKcmWrote()
    {
        // The file as kcm_powerdevilprofilesconfig left it for a desktop.
        put(u"powerdevilrc"_s,
            "[AC][Display]\nDimDisplayIdleTimeoutSec=-1\nDimDisplayWhenIdle=false\n"
            "TurnOffDisplayIdleTimeoutSec=-1\nTurnOffDisplayWhenIdle=false\n\n"
            "[AC][SuspendAndShutdown]\nAutoSuspendAction=0\nPowerButtonAction=8\n");
        PowerConfig c;
        const QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 0);
        QCOMPARE(ac[u"sleep"_s].toInt(), 0);
        QCOMPARE(ac[u"powerButton"_s].toInt(), 8);
    }

    void powerWritesTheKeysTheKcmWrites()
    {
        put(u"powerdevilrc"_s, "[Other]\nKeep=me\n");
        PowerConfig c;
        QVERIFY(c.setScreenOff(u"AC"_s, 300));
        QVERIFY(c.setSleep(u"AC"_s, 1800));
        QVERIFY(c.setLid(u"AC"_s, 2));
        QVERIFY(c.setPowerButton(u"AC"_s, 64));
        const QStringList l = lines(u"powerdevilrc"_s);
        QVERIFY2(l.contains(u"[AC][Display]"_s), qPrintable(l.join(u'|')));
        QVERIFY(l.contains(u"TurnOffDisplayWhenIdle=true"_s));
        QVERIFY(l.contains(u"TurnOffDisplayIdleTimeoutSec=300"_s));
        QVERIFY(l.contains(u"[AC][SuspendAndShutdown]"_s));
        QVERIFY(l.contains(u"AutoSuspendAction=1"_s));
        QVERIFY(l.contains(u"AutoSuspendIdleTimeoutSec=1800"_s));
        QVERIFY(l.contains(u"LidAction=2"_s));
        QVERIFY(l.contains(u"PowerButtonAction=64"_s));
        // What was there stays.
        QVERIFY(l.contains(u"[Other]"_s));
        QVERIFY(l.contains(u"Keep=me"_s));

        const QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 300);
        QCOMPARE(ac[u"sleep"_s].toInt(), 1800);
        QCOMPARE(ac[u"lid"_s].toInt(), 2);
        QCOMPARE(ac[u"powerButton"_s].toInt(), 64);
    }

    void powerNeverIsTheKcmsNever()
    {
        PowerConfig c;
        QVERIFY(c.setScreenOff(u"AC"_s, 0));
        QVERIFY(c.setSleep(u"AC"_s, 0));
        const QStringList l = lines(u"powerdevilrc"_s);
        QVERIFY(l.contains(u"TurnOffDisplayWhenIdle=false"_s));
        QVERIFY(l.contains(u"TurnOffDisplayIdleTimeoutSec=-1"_s));
        QVERIFY(l.contains(u"AutoSuspendAction=0"_s));
        QCOMPARE(c.read(u"AC"_s)[u"screenOff"_s].toInt(), 0);
        QCOMPARE(c.read(u"AC"_s)[u"sleep"_s].toInt(), 0);
    }

    void powerKeepsHibernate()
    {
        put(u"powerdevilrc"_s, "[Battery][SuspendAndShutdown]\nAutoSuspendAction=2\n");
        PowerConfig c;
        QVERIFY(c.setSleep(u"Battery"_s, 600));
        QVERIFY(lines(u"powerdevilrc"_s).contains(u"AutoSuspendAction=2"_s));
    }

    void powerProfilesAreIndependent()
    {
        PowerConfig c;
        QVERIFY(c.setSleep(u"Battery"_s, 300));
        QCOMPARE(c.read(u"Battery"_s)[u"sleep"_s].toInt(), 300);
        QCOMPARE(c.read(u"AC"_s)[u"sleep"_s].toInt(), 900);
        QVERIFY(lines(u"powerdevilrc"_s).contains(u"[Battery][SuspendAndShutdown]"_s));
    }

    void powerRefusesNonsense()
    {
        PowerConfig c;
        QVERIFY(!c.setScreenOff(u"AC"_s, -5));
        QVERIFY(!c.setScreenOff(u"AC"_s, 10 * 24 * 3600));
        QVERIFY(!c.setScreenOff(u"../AC"_s, 60));
        QVERIFY(!c.setLid(u"AC"_s, 12345));
        QVERIFY(!c.setPowerButton(u"AC"_s, 2));
        QVERIFY(lines(u"powerdevilrc"_s).isEmpty());
    }

    void powerClampsWhatTheFileSays()
    {
        put(u"powerdevilrc"_s, "[AC][Display]\nTurnOffDisplayIdleTimeoutSec=99999999\n");
        PowerConfig c;
        QCOMPARE(c.read(u"AC"_s)[u"screenOff"_s].toInt(), 24 * 3600);
    }

    void screenLockDefaults()
    {
        ScreenLockConfig c;
        const QVariantMap m = c.read();
        QCOMPARE(m[u"autoLock"_s].toBool(), true);
        QCOMPARE(m[u"minutes"_s].toInt(), 5);
        QCOMPARE(m[u"onWake"_s].toBool(), true);
    }

    void screenLockReadsWhatTheKcmWrote()
    {
        put(u"kscreenlockerrc"_s, "[Daemon]\nAutolock=false\nTimeout=0\n");
        ScreenLockConfig c;
        const QVariantMap m = c.read();
        QCOMPARE(m[u"autoLock"_s].toBool(), false);
        QCOMPARE(m[u"minutes"_s].toInt(), 0);
    }

    void screenLockWritesTheKcmsKeys()
    {
        put(u"kscreenlockerrc"_s, "[Greeter]\nWallpaperPlugin=x\n");
        ScreenLockConfig c;
        QVERIFY(c.setLockAfter(10));
        QVERIFY(c.setLockOnWake(false));
        const QStringList l = lines(u"kscreenlockerrc"_s);
        QVERIFY(l.contains(u"[Daemon]"_s));
        QVERIFY(l.contains(u"Autolock=true"_s));
        QVERIFY(l.contains(u"Timeout=10"_s));
        QVERIFY(l.contains(u"LockOnResume=false"_s));
        QVERIFY(l.contains(u"WallpaperPlugin=x"_s));
        QCOMPARE(c.read()[u"minutes"_s].toInt(), 10);
        QCOMPARE(c.read()[u"onWake"_s].toBool(), false);

        QVERIFY(c.setLockAfter(0));
        QVERIFY(lines(u"kscreenlockerrc"_s).contains(u"Autolock=false"_s));
        QCOMPARE(c.read()[u"autoLock"_s].toBool(), false);
        QVERIFY(!c.setLockAfter(-1));
        QVERIFY(!c.setLockAfter(1000000));
    }

    void autostartListsWhatPeopleKnow()
    {
        AutostartConfig c;
        const QVariantList list = c.entries();
        QStringList ids;
        for (const QVariant &e : list) {
            ids << e.toMap()[u"id"_s].toString();
        }
        // Not the NoDisplay one, not the one for GNOME only.
        QCOMPARE(ids, (QStringList{u"sys-off.desktop"_s, u"sys-shown.desktop"_s}));
        const QVariantMap shown = list[1].toMap();
        QCOMPARE(shown[u"name"_s].toString(), u"System Shown"_s);
        QCOMPARE(shown[u"icon"_s].toString(), u"sys-icon"_s);
        QCOMPARE(shown[u"enabled"_s].toBool(), true);
        QCOMPARE(shown[u"system"_s].toBool(), true);
        QCOMPARE(list[0].toMap()[u"enabled"_s].toBool(), false);
    }

    void autostartTurnsASystemEntryOffByAMaskNotByEditingIt()
    {
        AutostartConfig c;
        const QByteArray before = [&] {
            QFile f(path(u"sys/autostart/sys-shown.desktop"_s));
            return f.open(QIODevice::ReadOnly) ? f.readAll() : QByteArray();
        }();
        QVERIFY(c.setEnabled(u"sys-shown.desktop"_s, false));
        QVERIFY(!c.isEnabled(u"sys-shown.desktop"_s));
        const QStringList l = lines(u"autostart/sys-shown.desktop"_s);
        QCOMPARE(l, (QStringList{u"[Desktop Entry]"_s, u"Hidden=true"_s, u"Type=Application"_s}));
        // The system's file is as it was.
        QFile f(path(u"sys/autostart/sys-shown.desktop"_s));
        QVERIFY(f.open(QIODevice::ReadOnly));
        QVERIFY(f.readAll() == before);
        // On again: the mask goes.
        QVERIFY(c.setEnabled(u"sys-shown.desktop"_s, true));
        QVERIFY(c.isEnabled(u"sys-shown.desktop"_s));
        QVERIFY(!QFileInfo::exists(path(u"autostart/sys-shown.desktop"_s)));
    }

    void autostartCanTurnOnWhatTheSystemShipsOff()
    {
        AutostartConfig c;
        QVERIFY(!c.isEnabled(u"sys-off.desktop"_s));
        QVERIFY(c.setEnabled(u"sys-off.desktop"_s, true));
        QVERIFY(c.isEnabled(u"sys-off.desktop"_s));
        QVERIFY(lines(u"autostart/sys-off.desktop"_s).contains(u"Hidden=false"_s));
        // And an entry that doesn't exist anywhere can't be turned on.
        QVERIFY(!c.setEnabled(u"nowhere.desktop"_s, true));
    }

    void autostartAddsAndRemovesAnApp()
    {
        AutostartConfig c;
        QVERIFY(c.add(u"viewer-a.desktop"_s));
        QVERIFY(QFileInfo::exists(path(u"autostart/viewer-a.desktop"_s)));
        QVERIFY(lines(u"autostart/viewer-a.desktop"_s).contains(u"Exec=viewer-a %f"_s));
        QVERIFY(c.isEnabled(u"viewer-a.desktop"_s));
        QVERIFY(c.setEnabled(u"viewer-a.desktop"_s, false));
        QVERIFY(lines(u"autostart/viewer-a.desktop"_s).contains(u"Hidden=true"_s));
        QVERIFY(c.remove(u"viewer-a.desktop"_s));
        QVERIFY(!QFileInfo::exists(path(u"autostart/viewer-a.desktop"_s)));
        // A system entry can't be removed, only turned off.
        QVERIFY(!c.remove(u"sys-shown.desktop"_s));
        QVERIFY(QFileInfo::exists(path(u"sys/autostart/sys-shown.desktop"_s)));
        // Not an app that is installed.
        QVERIFY(!c.add(u"not-installed.desktop"_s));
    }

    // A link in the folder that someone else put there leads to a file of
    // theirs choosing: Settings doesn't write through it.
    void autostartDoesNotWriteThroughAStrangeLink()
    {
        AutostartConfig c;
        QVERIFY(QDir().mkpath(path(u"autostart"_s)));
        put(u"precious"_s, "keep me\n");
        QVERIFY(QFile::link(path(u"precious"_s), path(u"autostart/evil.desktop"_s)));
        QVERIFY(QFile::link(path(u"nowhere"_s), path(u"autostart/dangling.desktop"_s)));
        QVERIFY(!c.setEnabled(u"evil.desktop"_s, false));
        QVERIFY(!c.setEnabled(u"evil.desktop"_s, true));
        QVERIFY(!c.setEnabled(u"dangling.desktop"_s, false));
        // An ordinary entry still adds; when a link stands in its place it doesn't.
        QVERIFY(c.add(u"viewer-a.desktop"_s));
        QVERIFY(QFile::remove(path(u"autostart/viewer-a.desktop"_s)));
        QVERIFY(QFile::link(path(u"precious"_s), path(u"autostart/viewer-a.desktop"_s)));
        QVERIFY(!c.add(u"viewer-a.desktop"_s));
        QCOMPARE(lines(u"precious"_s), (QStringList{u"keep me"_s}));
        QVERIFY(!QFileInfo::exists(path(u"nowhere"_s)));
        QVERIFY(QFileInfo(path(u"autostart/evil.desktop"_s)).isSymLink());
    }

    // A link to a .desktop file (a dotfiles folder) is the file: it is written
    // through, as it was before.
    void autostartWritesThroughALinkToADesktopFile()
    {
        AutostartConfig c;
        QVERIFY(QDir().mkpath(path(u"autostart"_s)));
        QVERIFY(QDir().mkpath(path(u"dotfiles"_s)));
        put(u"dotfiles/mine.desktop"_s, "[Desktop Entry]\nType=Application\nName=Mine\nExec=mine\n");
        QVERIFY(QFile::link(path(u"dotfiles/mine.desktop"_s), path(u"autostart/mine.desktop"_s)));
        QVERIFY(c.setEnabled(u"mine.desktop"_s, false));
        QVERIFY(lines(u"dotfiles/mine.desktop"_s).contains(u"Hidden=true"_s));
    }

    void autostartRefusesOddNames()
    {
        AutostartConfig c;
        for (const QString &bad : {u"../x.desktop"_s, u"a/b.desktop"_s, u"x.txt"_s, u".desktop"_s, QString(), u"a b.desktop"_s, u"a.desktop\n"_s, u"a.desktop\n\n"_s}) {
            QVERIFY2(!AutostartConfig::validId(bad), qPrintable(bad));
            QVERIFY(!c.setEnabled(bad, false));
            QVERIFY(!c.remove(bad));
            QVERIFY(!c.add(bad));
        }
        QVERIFY(!QFileInfo::exists(path(u"autostart"_s)) || QDir(path(u"autostart"_s)).isEmpty(QDir::Files));
    }

    // Names that reach a tool or a file are checked from their first to their
    // last character: a name that ends in a newline is not a plain name (a
    // regular expression's $ would let it through).
    void validatorsTakeNothingAfterTheEnd()
    {
        QVERIFY(ColorSchemeConfig::validName(u"BreezeDark"_s));
        QVERIFY(!ColorSchemeConfig::validName(u"BreezeDark\n"_s));
        QVERIFY(!ColorSchemeConfig::validName(u"-h"_s));
        QVERIFY(LocaleConfig::validLocale(u"en_US.UTF-8"_s));
        QVERIFY(!LocaleConfig::validLocale(u"en_US.UTF-8\n"_s));
    }

    void defaultAppsOfferOnlyWhatFits()
    {
        DefaultApps d;
        auto ids = [&](const QString &kind) {
            QStringList out;
            for (const QVariant &v : d.choices(kind)) {
                out << v.toMap()[u"id"_s].toString();
            }
            return out;
        };
        QCOMPARE(ids(u"browser"_s), (QStringList{u"browser-a.desktop"_s, u"browser-b.desktop"_s}));
        QCOMPARE(ids(u"terminal"_s), (QStringList{u"term-a.desktop"_s, u"term-b.desktop"_s}));
        QCOMPARE(ids(u"images"_s), (QStringList{u"viewer-a.desktop"_s, u"viewer-b.desktop"_s}));
        QVERIFY(d.choices(u"nonsense"_s).isEmpty());
    }

    void defaultAppsWriteMimeappsListAsTheKcmDoes()
    {
        DefaultApps d;
        QVERIFY(d.setDefault(u"browser"_s, u"browser-b.desktop"_s));
        const QStringList l = lines(u"mimeapps.list"_s);
        QVERIFY2(l.contains(u"[Default Applications]"_s), qPrintable(l.join(u'|')));
        QVERIFY(l.contains(u"x-scheme-handler/http=browser-b.desktop;"_s));
        QVERIFY(l.contains(u"x-scheme-handler/https=browser-b.desktop;"_s));
        QVERIFY(l.contains(u"text/html=browser-b.desktop;"_s));
        QVERIFY(l.contains(u"[Added Associations]"_s));
        QCOMPARE(d.current(u"browser"_s)[u"id"_s].toString(), u"browser-b.desktop"_s);
        QVERIFY(d.setDefault(u"browser"_s, u"browser-a.desktop"_s));
        QCOMPARE(d.current(u"browser"_s)[u"id"_s].toString(), u"browser-a.desktop"_s);
        // The default is the one chosen; the earlier one stays an association.
        const QStringList l2 = lines(u"mimeapps.list"_s);
        QVERIFY(l2.contains(u"text/html=browser-a.desktop;"_s));
        QVERIFY(l2.contains(u"text/html=browser-a.desktop;browser-b.desktop;"_s));
    }

    void defaultAppsKeepWhatWasInMimeappsList()
    {
        put(u"mimeapps.list"_s, "[Default Applications]\ntext/plain=editor.desktop;\n\n[Removed Associations]\nimage/png=viewer-a.desktop;\n");
        DefaultApps d;
        QVERIFY(d.setDefault(u"images"_s, u"viewer-a.desktop"_s));
        const QStringList l = lines(u"mimeapps.list"_s);
        QVERIFY(l.contains(u"text/plain=editor.desktop;"_s));
        QVERIFY(l.contains(u"image/png=viewer-a.desktop;"_s));
        // The app is no longer removed for the type.
        QVERIFY(!l.contains(u"[Removed Associations]"_s));
    }

    void defaultAppsTerminalGoesToKdeglobals()
    {
        DefaultApps d;
        QVERIFY(d.setDefault(u"terminal"_s, u"term-a.desktop"_s));
        const QStringList l = lines(u"kdeglobals"_s);
        QVERIFY2(l.contains(u"[General]"_s), qPrintable(l.join(u'|')));
        QVERIFY(l.contains(u"TerminalApplication=term-a"_s));
        QVERIFY(l.contains(u"TerminalService=term-a.desktop"_s));
        QVERIFY(!QFileInfo::exists(path(u"mimeapps.list"_s)));
        QCOMPARE(d.current(u"terminal"_s)[u"id"_s].toString(), u"term-a.desktop"_s);
    }

    void defaultAppsRefuseWhatIsNotAChoice()
    {
        DefaultApps d;
        QVERIFY(!d.setDefault(u"browser"_s, u"viewer-a.desktop"_s));
        QVERIFY(!d.setDefault(u"browser"_s, u"hidden.desktop"_s));
        QVERIFY(!d.setDefault(u"browser"_s, u"../../evil.desktop"_s));
        QVERIFY(!d.setDefault(u"nonsense"_s, u"browser-a.desktop"_s));
        QVERIFY(!d.setDefault(u"browser"_s, QString()));
        QVERIFY(!QFileInfo::exists(path(u"mimeapps.list"_s)));
    }
};

QTEST_GUILESS_MAIN(ConfigTests)
#include "configtests.moc"
