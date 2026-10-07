// Tests of the page backends that write Plasma's own settings
// (appearanceconfig.h, inputconfig.h, notificationsconfig.h,
// accessibilityconfig.h): every write is read back with KConfig from a
// temporary $XDG_CONFIG_HOME and compared with the exact file, group and
// keys Plasma's modules write; the calls to KWin and Plasma's shell go to
// fake services on a private session bus (run the test under
// dbus-run-session). Never touches the user's config or session.

#include "accessibilityconfig.h"
#include "appearanceconfig.h"
#include "fakes.h"
#include "inputconfig.h"
#include "notificationsconfig.h"

#include <KConfig>
#include <KConfigGroup>

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

using namespace Qt::StringLiterals;

namespace
{
QTemporaryDir *root = nullptr;

QString dir(const char *name)
{
    return root->filePath(QString::fromLatin1(name));
}

void write(const QString &path, const QByteArray &content)
{
    QDir().mkpath(QFileInfo(path).absolutePath());
    QFile f(path);
    QVERIFY2(f.open(QIODevice::WriteOnly), qPrintable(path));
    f.write(content);
}

// A key of a file in the user's config folder; a nested group is written
// "Outer][Inner".
QVariant value(const QString &file, const QString &group, const QString &key)
{
    KConfig config(dir("config") + u'/' + file, KConfig::SimpleConfig);
    const QStringList path = group.split(u"]["_s);
    KConfigGroup g(&config, path.first());
    for (qsizetype i = 1; i < path.size(); ++i) {
        g = g.group(path.at(i));
    }
    return g.hasKey(key) ? QVariant(g.readEntry(key, QString())) : QVariant();
}

void clearUserFiles()
{
    QDir(dir("config")).removeRecursively();
    QDir(dir("data")).removeRecursively();
    QDir().mkpath(dir("config"));
    QDir().mkpath(dir("data"));
    // The pictures folder is read from user-dirs.dirs of the config home.
    write(dir("config") + u"/user-dirs.dirs"_s, ("XDG_PICTURES_DIR=\"" + dir("pictures") + "\"\n").toUtf8());
}

bool haveBus()
{
    return QDBusConnection::sessionBus().isConnected();
}

using namespace fakes;
}

class AppearanceTest : public QObject
{
    Q_OBJECT

private Q_SLOTS:
    void init()
    {
        clearUserFiles();
        // The system's: Telamon OS's schemes, a wallpaper, two Global Themes.
        const QString scheme = QStringLiteral("[General]\nColorScheme=%1\nName=AtlasOS %2\n[Colors:Window]\nBackgroundNormal=1,2,3\nForegroundNormal=4,5,6\n[Colors:Selection]\nBackgroundNormal=7,8,9\n[WM]\nactiveBackground=1,1,1\n");
        write(dir("sys") + u"/color-schemes/AtlasOSDark.colors"_s, scheme.arg(u"AtlasOSDark"_s, u"Dark"_s).toUtf8());
        write(dir("sys") + u"/color-schemes/AtlasOSLight.colors"_s, scheme.arg(u"AtlasOSLight"_s, u"Light"_s).toUtf8());
        write(dir("sys") + u"/color-schemes/TelamonDark.colors"_s, scheme.arg(u"TelamonDark"_s, u"Dark"_s).toUtf8());
        write(dir("sys") + u"/color-schemes/TelamonLight.colors"_s, scheme.arg(u"TelamonLight"_s, u"Light"_s).toUtf8());
        write(dir("etc") + u"/kdeglobals"_s, "[General]\nColorScheme=AtlasOSLight\n[Icons]\nTheme=Papirus\n");
        write(dir("etc") + u"/kwinrc"_s, "[org.kde.kdecoration2]\nlibrary=org.kde.kwin.aurorae\ntheme=__aurorae__svg__AtlasOS-Light\n");
        // XDG_DATA_DIRS holds the system's data, the user's is $XDG_DATA_HOME.
        write(dir("sys") + u"/wallpapers/AtlasOS/metadata.json"_s, R"({"KPlugin": {"Id": "AtlasOS", "Name": "AtlasOS Wave"}})");
        write(dir("sys") + u"/wallpapers/AtlasOS/contents/screenshot.jpg"_s, "x");
        write(dir("sys") + u"/wallpapers/AtlasOS-Login/contents/screenshot.jpg"_s, "x");
        write(dir("sys") + u"/wallpapers/Bad'Name/contents/screenshot.jpg"_s, "x");
        write(dir("sys") + u"/plasma/look-and-feel/org.atlasos.dark.desktop/metadata.json"_s, R"({"KPlugin": {"Id": "org.atlasos.dark.desktop", "Name": "AtlasOS Dark", "Description": "d"}})");
        write(dir("sys") + u"/plasma/look-and-feel/org.evil.desktop/metadata.json"_s, R"({"KPlugin": {"Id": "../../etc", "Name": "x"}})");
        write(dir("pictures") + u"/photo.jpg"_s, "x");
        write(dir("pictures") + u"/it's.jpg"_s, "x");
    }

    void readsTheSchemeLikeKvantumSync()
    {
        AppearanceConfig cfg;
        QVariantMap m = cfg.read();
        QCOMPARE(m.value(u"scheme"_s).toString(), u"AtlasOSLight"_s);
        QCOMPARE(m.value(u"dark"_s).toBool(), false);
        QCOMPARE(m.value(u"accentIsDefault"_s).toBool(), true);
        // The Global Theme's defaults are read before the system's.
        write(dir("config") + u"/kdedefaults/kdeglobals"_s, "[General]\nColorScheme=AtlasOSDark\n");
        QCOMPARE(cfg.read().value(u"scheme"_s).toString(), u"AtlasOSDark"_s);
        // The user's own kdeglobals wins over both.
        write(dir("config") + u"/kdeglobals"_s, "[General]\nColorScheme=AtlasOSLight\n");
        QCOMPARE(cfg.read().value(u"scheme"_s).toString(), u"AtlasOSLight"_s);
    }

    void darkAppliesTheSchemeAndTheThemesIconsAndDecoration()
    {
        AppearanceConfig cfg;
        QSignalSpy run(&cfg, &AppearanceConfig::run);
        cfg.setDark(true);
        QCOMPARE(run.size(), 1);
        // plasma-apply-colorscheme sets the scheme (and with it the default
        // accent of the scheme).
        QCOMPARE(run.at(0).at(0).toStringList(), (QStringList{u"plasma-apply-colorscheme"_s, u"--accent-color"_s, u"#8a7af4"_s, u"TelamonDark"_s}));
        QCOMPARE(value(u"kdeglobals"_s, u"Icons"_s, u"Theme"_s).toString(), u"Papirus-Dark"_s);
        QCOMPARE(value(u"kwinrc"_s, u"org.kde.kdecoration2"_s, u"theme"_s).toString(), u"__aurorae__svg__Telamon-Dark"_s);
        // The library is the system's already: KConfig leaves a value equal to
        // the default out of the user's file, and the session reads both.
        KConfig kwin(u"kwinrc"_s, KConfig::NoGlobals);
        QCOMPARE(KConfigGroup(&kwin, u"org.kde.kdecoration2"_s).readEntry("library", QString()), u"org.kde.kwin.aurorae"_s);
    }

    void aThemeTheUserPickedStays()
    {
        write(dir("config") + u"/kdeglobals"_s, "[Icons]\nTheme=breeze\n");
        write(dir("config") + u"/kwinrc"_s, "[org.kde.kdecoration2]\ntheme=__aurorae__svg__Other\n");
        AppearanceConfig cfg;
        cfg.setDark(true);
        QCOMPARE(value(u"kdeglobals"_s, u"Icons"_s, u"Theme"_s).toString(), u"breeze"_s);
        QCOMPARE(value(u"kwinrc"_s, u"org.kde.kdecoration2"_s, u"theme"_s).toString(), u"__aurorae__svg__Other"_s);
    }

    void accentKeepsTheUsersColour()
    {
        AppearanceConfig cfg;
        QSignalSpy run(&cfg, &AppearanceConfig::run);
        cfg.setAccent(u"#E5487A"_s);
        QCOMPARE(run.at(0).at(0).toStringList(), (QStringList{u"plasma-apply-colorscheme"_s, u"--accent-color"_s, u"#e5487a"_s, u"TelamonLight"_s}));
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"accentColorFromWallpaper"_s).toString(), u"false"_s);
        // Not a colour: nothing happens.
        cfg.setAccent(u"red; rm -rf"_s);
        QCOMPARE(run.size(), 1);
        // The accent as the module writes it is read back.
        write(dir("config") + u"/kdeglobals"_s, "[General]\nColorScheme=AtlasOSLight\nAccentColor=229,72,122\n");
        QCOMPARE(cfg.read().value(u"accent"_s).toString(), u"#e5487a"_s);
        QCOMPARE(cfg.read().value(u"accentIsDefault"_s).toBool(), false);
        // A mode switch keeps it.
        cfg.setDark(true);
        QCOMPARE(run.last().at(0).toStringList().at(2), u"#e5487a"_s);
    }

    void accentFromWallpaperIsTheModulesKey()
    {
        AppearanceConfig cfg;
        cfg.setAccentFromWallpaper();
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"accentColorFromWallpaper"_s).toString(), u"true"_s);
        QVERIFY(cfg.read().value(u"accentFromWallpaper"_s).toBool());
        QVERIFY(!cfg.read().value(u"accentIsDefault"_s).toBool());
    }

    void highContrastMakesAScheme()
    {
        AppearanceConfig cfg;
        QSignalSpy run(&cfg, &AppearanceConfig::run);
        write(dir("config") + u"/kdeglobals"_s, "[General]\nColorScheme=AtlasOSDark\n");
        cfg.setHighContrast(true);
        QCOMPARE(run.at(0).at(0).toStringList(), (QStringList{u"plasma-apply-colorscheme"_s, u"TelamonHighContrastDark"_s}));
        const QString file = dir("data") + u"/color-schemes/TelamonHighContrastDark.colors"_s;
        QVERIFY(QFile::exists(file));
        KConfig scheme(file, KConfig::SimpleConfig);
        QCOMPARE(KConfigGroup(&scheme, u"Colors:Window"_s).readEntry("BackgroundNormal", QString()), u"0,0,0"_s);
        QCOMPARE(KConfigGroup(&scheme, u"Colors:Window"_s).readEntry("ForegroundNormal", QString()), u"255,255,255"_s);
        QCOMPARE(KConfigGroup(&scheme, u"General"_s).readEntry("ColorScheme", QString()), u"TelamonHighContrastDark"_s);
        // Dark by name, for kvantum-sync.
        QVERIFY(QStringLiteral("TelamonHighContrastDark").contains(u"Dark"_s));
        // On: read back (an AtlasOS-era name too), and a Light/Dark switch keeps it.
        write(dir("config") + u"/kdeglobals"_s, "[General]\nColorScheme=AtlasOSHighContrastDark\n");
        QVERIFY(cfg.read().value(u"highContrast"_s).toBool());
        QVERIFY(cfg.read().value(u"dark"_s).toBool());
        cfg.setDark(false);
        QCOMPARE(run.last().at(0).toStringList().last(), u"TelamonHighContrastLight"_s);
        QVERIFY(QFile::exists(dir("data") + u"/color-schemes/TelamonHighContrastLight.colors"_s));
        // Off: back to the usual scheme.
        cfg.setHighContrast(false);
        QCOMPARE(run.last().at(0).toStringList().last(), u"TelamonDark"_s);
    }

    void wallpapersOnlyKnownOnes()
    {
        AppearanceConfig cfg;
        const QVariantList list = cfg.wallpapers();
        QStringList ids;
        for (const QVariant &v : list) {
            ids << v.toMap().value(u"id"_s).toString();
        }
        QVERIFY(ids.contains(u"AtlasOS"_s));
        QVERIFY(ids.contains(dir("pictures") + u"/photo.jpg"_s));
        // The login screen's, and names a script could be built from, are not
        // offered.
        QVERIFY(!ids.contains(u"AtlasOS-Login"_s));
        QVERIFY(!ids.contains(u"Bad'Name"_s));
        QVERIFY(!ids.contains(dir("pictures") + u"/it's.jpg"_s));
        QCOMPARE(list.first().toMap().value(u"name"_s).toString(), u"AtlasOS Wave"_s);

        QSignalSpy run(&cfg, &AppearanceConfig::run);
        cfg.setWallpaper(u"/etc/passwd"_s);
        cfg.setWallpaper(u"nothing"_s);
        QCOMPARE(run.size(), 0);
        cfg.setWallpaper(u"AtlasOS"_s);
        QCOMPARE(run.at(0).at(0).toStringList(), (QStringList{u"plasma-apply-wallpaperimage"_s, u"AtlasOS"_s}));
        cfg.setWallpaper(dir("pictures") + u"/photo.jpg"_s);
        QCOMPARE(run.at(1).at(0).toStringList().last(), dir("pictures") + u"/photo.jpg"_s);
    }

    void currentWallpaperFromThePlasmaFile()
    {
        write(dir("config") + u"/plasma-org.kde.plasma.desktop-appletsrc"_s,
              "[Containments][1][Wallpaper][org.kde.image][General]\nImage=file:///usr/share/wallpapers/AtlasOS/\n");
        AppearanceConfig cfg;
        QCOMPARE(cfg.read().value(u"wallpaper"_s).toMap().value(u"id"_s).toString(), u"AtlasOS"_s);
        QCOMPARE(cfg.read().value(u"wallpaper"_s).toMap().value(u"name"_s).toString(), u"AtlasOS Wave"_s);
    }

    void wallpaperPicturesForThePreview()
    {
        const QString sys = dir("sys") + u"/wallpapers/"_s;
        // A package with one image per screen size, and a dark variant: the
        // largest up to 2560 wide, not the 5K one.
        write(sys + u"Pic/metadata.json"_s, R"({"KPlugin": {"Id": "Pic", "Name": "Pic"}})");
        write(sys + u"Pic/contents/screenshot.jpg"_s, "x");
        for (const char *name : {"1280x800.jpg", "2560x1600.png", "5120x3200.jpg", "notes.txt", "3840x2160.jxl"}) {
            write(sys + u"Pic/contents/images/"_s + QString::fromLatin1(name), "x");
        }
        write(sys + u"Pic/contents/images_dark/1920x1200.jpg"_s, "x");
        // Only too wide images: the narrowest.
        write(sys + u"Wide/metadata.json"_s, R"({"KPlugin": {"Id": "Wide", "Name": "Wide"}})");
        write(sys + u"Wide/contents/screenshot.jpg"_s, "x");
        write(sys + u"Wide/contents/images/7680x4320.jpg"_s, "x");
        write(sys + u"Wide/contents/images/5120x2880.jpg"_s, "x");
        // Telamon OS's own: what the preview draws when no wallpaper is set.
        write(sys + u"Telamon/metadata.json"_s, R"({"KPlugin": {"Id": "Telamon", "Name": "Telamon"}})");
        write(sys + u"Telamon/contents/images/1920x1200.jpg"_s, "x");

        AppearanceConfig cfg;
        const auto url = [](const QString &path) { return QUrl::fromLocalFile(path).toString(); };
        QVariantMap w = cfg.read().value(u"wallpaper"_s).toMap();
        QCOMPARE(w.value(u"id"_s).toString(), QString());
        QCOMPARE(w.value(u"picture"_s).toString(), url(sys + u"Telamon/contents/images/1920x1200.jpg"_s));
        QCOMPARE(w.value(u"pictureDark"_s).toString(), QString());

        const QString applets = dir("config") + u"/plasma-org.kde.plasma.desktop-appletsrc"_s;
        write(applets, "[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://" + sys.toUtf8() + "Pic/\n");
        w = cfg.read().value(u"wallpaper"_s).toMap();
        QCOMPARE(w.value(u"id"_s).toString(), u"Pic"_s);
        QCOMPARE(w.value(u"picture"_s).toString(), url(sys + u"Pic/contents/images/2560x1600.png"_s));
        QCOMPARE(w.value(u"pictureDark"_s).toString(), url(sys + u"Pic/contents/images_dark/1920x1200.jpg"_s));
        QCOMPARE(cfg.wallpaperPictures(u"Pic"_s).value(u"picture"_s).toString(), w.value(u"picture"_s).toString());
        QCOMPARE(cfg.wallpaperPictures(u"Wide"_s).value(u"picture"_s).toString(), url(sys + u"Wide/contents/images/5120x2880.jpg"_s));
        // Only what wallpapers() lists.
        QVERIFY(cfg.wallpaperPictures(u"/etc/passwd"_s).isEmpty());
        QVERIFY(cfg.wallpaperPictures(u"nothing"_s).isEmpty());
        QCOMPARE(cfg.wallpaperPictures(dir("pictures") + u"/photo.jpg"_s).value(u"picture"_s).toString(), url(dir("pictures") + u"/photo.jpg"_s));

        // A picture file: that file, and no dark one.
        write(applets, "[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://" + dir("pictures").toUtf8() + "/photo.jpg\n");
        w = cfg.read().value(u"wallpaper"_s).toMap();
        QCOMPARE(w.value(u"picture"_s).toString(), url(dir("pictures") + u"/photo.jpg"_s));
        QCOMPARE(w.value(u"pictureDark"_s).toString(), QString());

        // Two screens: the main one's (containment 0) wins, whatever the order;
        // a slideshow is not a picture.
        write(applets,
              "[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://" + dir("pictures").toUtf8() + "/photo.jpg\n"
              "[Containments][1]\nlastScreen=1\n"
              "[Containments][2][Wallpaper][org.kde.image][General]\nImage=file://" + sys.toUtf8() + "Pic/\n"
              "[Containments][2]\nlastScreen=0\n"
              "[Containments][3][Wallpaper][org.kde.image][General]\nImage=file://" + sys.toUtf8() + "Wide/\n"
              "[Containments][3]\nlastScreen=2\nwallpaperplugin=org.kde.slideshow\n");
        QCOMPARE(cfg.read().value(u"wallpaper"_s).toMap().value(u"id"_s).toString(), u"Pic"_s);
        write(applets,
              "[Containments][1][Wallpaper][org.kde.image][General]\nImage=file://" + sys.toUtf8() + "Wide/\n"
              "[Containments][1]\nlastScreen=0\nwallpaperplugin=org.kde.slideshow\n");
        QCOMPARE(cfg.read().value(u"wallpaper"_s).toMap().value(u"id"_s).toString(), QString());
    }

    void wallpaperFileChangesAreAnnounced()
    {
        // Plasma saves its applets file a while after a change, by replacing
        // it: the page is told, also for the second save.
        const QString applets = dir("config") + u"/plasma-org.kde.plasma.desktop-appletsrc"_s;
        write(applets, "[Containments][1]\nlastScreen=0\n");
        AppearanceConfig cfg;
        QSignalSpy changed(&cfg, &AppearanceConfig::changed);
        for (int i = 0; i < 2; ++i) {
            QFile::remove(applets + u".new"_s);
            write(applets + u".new"_s, "[Containments][1]\nlastScreen=0\nx=" + QByteArray::number(i) + "\n");
            QVERIFY(QFile::remove(applets));
            QVERIFY(QFile::rename(applets + u".new"_s, applets));
            QVERIFY2(changed.wait(3000), "no change announced");
            changed.clear();
        }
    }

    void globalThemes()
    {
        AppearanceConfig cfg;
        const QVariantList list = cfg.lookAndFeels();
        // The theme whose ID isn't its folder's name is dropped.
        QCOMPARE(list.size(), 1);
        QSignalSpy run(&cfg, &AppearanceConfig::run);
        cfg.setLookAndFeel(u"../../etc"_s);
        QCOMPARE(run.size(), 0);
        cfg.setLookAndFeel(u"org.atlasos.dark.desktop"_s);
        QCOMPARE(run.at(0).at(0).toStringList(), (QStringList{u"plasma-apply-lookandfeel"_s, u"--apply"_s, u"org.atlasos.dark.desktop"_s}));
    }

    void hotCornersAreKWinsKeys()
    {
        AppearanceConfig cfg;
        QCOMPARE(cfg.hotCorners().value(u"topLeft"_s).toString(), u"none"_s);
        QVERIFY(cfg.setHotCorner(u"topLeft"_s, u"desktop"_s));
        QCOMPARE(value(u"kwinrc"_s, u"ElectricBorders"_s, u"TopLeft"_s).toString(), u"ShowDesktop"_s);
        QVERIFY(cfg.setHotCorner(u"bottomRight"_s, u"overview"_s));
        // Overview is the effect's: the corner's number (ElectricBottomRight
        // is 3), and the corner itself is None.
        QCOMPARE(value(u"kwinrc"_s, u"Effect-overview"_s, u"BorderActivate"_s).toString(), u"3"_s);
        QCOMPARE(value(u"kwinrc"_s, u"ElectricBorders"_s, u"BottomRight"_s).toString(), u"None"_s);
        QVERIFY(cfg.setHotCorner(u"topRight"_s, u"lock"_s));
        QCOMPARE(value(u"kwinrc"_s, u"ElectricBorders"_s, u"TopRight"_s).toString(), u"LockScreen"_s);
        QCOMPARE(cfg.hotCorners().value(u"bottomRight"_s).toString(), u"overview"_s);
        QCOMPARE(cfg.hotCorners().value(u"topLeft"_s).toString(), u"desktop"_s);
        // Back to nothing removes the corner from the effect's list.
        QVERIFY(cfg.setHotCorner(u"bottomRight"_s, u"none"_s));
        QVERIFY(!value(u"kwinrc"_s, u"Effect-overview"_s, u"BorderActivate"_s).isValid());
        QVERIFY(!cfg.setHotCorner(u"middle"_s, u"none"_s));
        QVERIFY(!cfg.setHotCorner(u"topLeft"_s, u"KRunner; reboot"_s));
    }
};

class ShellTest : public QObject
{
    Q_OBJECT

    FakeShell shell;
    FakeDesktops desktops;
    FakeKWin kwin;
    FakeEffects effects;
    bool ready = false;

private Q_SLOTS:
    void initTestCase()
    {
        if (!haveBus()) {
            QSKIP("no session bus: run under dbus-run-session");
        }
        auto bus = QDBusConnection::sessionBus();
        QVERIFY(bus.registerService(u"org.kde.plasmashell"_s));
        QVERIFY(bus.registerService(u"org.kde.KWin"_s));
        QVERIFY(bus.registerObject(u"/PlasmaShell"_s, &shell, QDBusConnection::ExportAllSlots));
        QVERIFY(bus.registerObject(u"/VirtualDesktopManager"_s, &desktops, QDBusConnection::ExportAllSlots | QDBusConnection::ExportAllProperties));
        QVERIFY(bus.registerObject(u"/KWin"_s, &kwin, QDBusConnection::ExportAllSlots));
        QVERIFY(bus.registerObject(u"/Effects"_s, &effects, QDBusConnection::ExportAllSlots));
        ready = true;
    }

    void init()
    {
        clearUserFiles();
        shell.scripts.clear();
        shell.answer = QStringLiteral(R"({"dock": {"location": "bottom", "hiding": "autohide", "height": 60}, "bar": {"count": 3, "hides": true}})");
        desktops.created.clear();
        desktops.count = 2;
        effects.calls.clear();
        kwin.reconfigured = 0;
    }

    void readsTheDockAndTheTopBar()
    {
        AppearanceConfig cfg;
        QSignalSpy spy(&cfg, &AppearanceConfig::shellChanged);
        cfg.refreshShell();
        QTRY_VERIFY(spy.size() >= 1);
        const QVariantMap s = cfg.shell();
        QVERIFY(s.value(u"known"_s).toBool());
        QCOMPARE(s.value(u"dock"_s).toMap().value(u"autohide"_s).toBool(), true);
        QCOMPARE(s.value(u"dock"_s).toMap().value(u"height"_s).toInt(), 60);
        QCOMPARE(s.value(u"dock"_s).toMap().value(u"location"_s).toString(), u"bottom"_s);
        QCOMPARE(s.value(u"topBar"_s).toMap().value(u"hides"_s).toBool(), true);
        QTRY_COMPARE(cfg.desktopCount(), 2);
    }

    void aShellThatSaysNonsenseIsUnknown()
    {
        shell.answer = QStringLiteral("not json");
        AppearanceConfig cfg;
        QSignalSpy spy(&cfg, &AppearanceConfig::shellChanged);
        cfg.refreshShell();
        QTest::qWait(200);
        QVERIFY(!cfg.shell().value(u"known"_s).toBool());
    }

    void dockChangesAreScriptsOfFixedParts()
    {
        AppearanceConfig cfg;
        cfg.setDockAutoHide(false);
        cfg.setDockSize(72);
        cfg.setDockSize(5000);
        cfg.setDockPosition(u"left"_s);
        cfg.setDockPosition(u"'; evil(); '"_s);
        cfg.setTopBarHides(false);
        QTRY_VERIFY(shell.scripts.size() >= 5);
        QVERIFY(shell.scripts.at(0).contains(u"p.hiding = 'none'"_s));
        QVERIFY(shell.scripts.at(0).contains(u"org.atlasos.dockseparator"_s));
        QVERIFY(shell.scripts.at(1).contains(u"p.height = 72;"_s));
        // Held to the largest size.
        QVERIFY(shell.scripts.at(2).contains(u"p.height = 96;"_s));
        QVERIFY(shell.scripts.at(3).contains(u"p.location = 'left'"_s));
        // The bad position wasn't sent: the next script is the top bar's.
        QVERIFY(shell.scripts.at(4).contains(u"p.hiding = 'windowsgobelow'"_s));
        QVERIFY(!shell.scripts.join(u' ').contains(u"evil"_s));
    }

    void desktopsAreCreatedThroughKWin()
    {
        AppearanceConfig cfg;
        cfg.refreshShell();
        QTRY_COMPARE(cfg.desktopCount(), 2);
        cfg.setDesktopCount(4);
        QTRY_COMPARE(desktops.created.size(), 2);
        QCOMPARE(desktops.created.at(0), u"2:Desktop 3"_s);
        QCOMPARE(desktops.created.at(1), u"3:Desktop 4"_s);
    }

    void hotCornersAskKWinToReadAgain()
    {
        AppearanceConfig cfg;
        QVERIFY(cfg.setHotCorner(u"topLeft"_s, u"overview"_s));
        QTRY_VERIFY(kwin.reconfigured >= 1);
        QTRY_VERIFY(effects.calls.contains(u"reconfigure:overview"_s));
    }

    void zoomTurnsTheEffectOnAndOff()
    {
        AccessibilityConfig a11y;
        QVERIFY(a11y.setZoom(false));
        QCOMPARE(value(u"kwinrc"_s, u"Plugins"_s, u"zoomEnabled"_s).toString(), u"false"_s);
        QTRY_VERIFY(effects.calls.contains(u"unload:zoom"_s));
        QVERIFY(!a11y.read().value(u"zoom"_s).toBool());
        QVERIFY(a11y.setZoom(true));
        QTRY_VERIFY(effects.calls.contains(u"load:zoom"_s));
    }
};

class InputTest : public QObject
{
    Q_OBJECT

    FakeManager manager;
    FakeDevice mouse;
    FakeDevice pad;
    bool bus = false;

    void writeRules(const QString &path)
    {
        write(path,
              "<?xml version=\"1.0\"?><xkbConfigRegistry><layoutList>"
              "<layout><configItem><name>us</name><shortDescription>en</shortDescription><description>English (US)</description></configItem>"
              "<variantList><variant><configItem><name>dvorak</name><description>English (Dvorak)</description></configItem></variant></variantList></layout>"
              "<layout><configItem><name>de</name><description>German</description></configItem><variantList/></layout>"
              "<layout><configItem><name>bad name</name><description>Nope</description></configItem></layout>"
              "</layoutList></xkbConfigRegistry>");
    }

private Q_SLOTS:
    void initTestCase()
    {
        if (!haveBus()) {
            return;
        }
        auto session = QDBusConnection::sessionBus();
        bus = session.registerService(u"org.kde.KWin"_s) || session.interface()->isServiceRegistered(u"org.kde.KWin"_s);
        if (!bus) {
            return;
        }
        pad.touchpad = true;
        pad.naturalScroll = true;
        pad.pointerAcceleration = 0.4;
        manager.names = {u"event1"_s, u"event2"_s};
        session.registerObject(u"/org/kde/KWin/InputDevice"_s, &manager, QDBusConnection::ExportAllProperties);
        session.registerObject(u"/org/kde/KWin/InputDevice/event1"_s, &mouse, QDBusConnection::ExportAllProperties);
        session.registerObject(u"/org/kde/KWin/InputDevice/event2"_s, &pad, QDBusConnection::ExportAllProperties);
    }

    void init()
    {
        clearUserFiles();
    }

    void layoutsAreWrittenAsKcmKeyboardWritesThem()
    {
        InputConfig cfg;
        QVERIFY(cfg.setLayouts({u"us"_s, u"us(dvorak)"_s, u"de"_s}));
        QCOMPARE(value(u"kxkbrc"_s, u"Layout"_s, u"Use"_s).toString(), u"true"_s);
        QCOMPARE(value(u"kxkbrc"_s, u"Layout"_s, u"LayoutList"_s).toString(), u"us,us,de"_s);
        QCOMPARE(value(u"kxkbrc"_s, u"Layout"_s, u"VariantList"_s).toString(), u",dvorak,"_s);
        const QVariantList back = cfg.layouts();
        QCOMPARE(back.size(), 3);
        QCOMPARE(back.at(1).toMap().value(u"id"_s).toString(), u"us(dvorak)"_s);
        QCOMPARE(back.at(1).toMap().value(u"variant"_s).toString(), u"dvorak"_s);
        // One layout without a variant: an empty list, not "\0".
        QVERIFY(cfg.setLayouts({u"de"_s}));
        QCOMPARE(value(u"kxkbrc"_s, u"Layout"_s, u"LayoutList"_s).toString(), u"de"_s);
        QCOMPARE(value(u"kxkbrc"_s, u"Layout"_s, u"VariantList"_s).toString(), QString());
        QCOMPARE(cfg.layouts().size(), 1);
    }

    void aListThatCantBeWrittenIsRefused()
    {
        InputConfig cfg;
        QVERIFY(!cfg.setLayouts({}));
        QVERIFY(!cfg.setLayouts({u"us"_s, u"de"_s, u"fr"_s, u"es"_s, u"it"_s}));
        QVERIFY(!cfg.setLayouts({u"us"_s, u"us"_s}));
        QVERIFY(!cfg.setLayouts({u"us,de"_s}));
        QVERIFY(!cfg.setLayouts({u"us(dvorak"_s}));
        QVERIFY(!QFile::exists(dir("config") + u"/kxkbrc"_s));
    }

    void theLayoutsToAddComeFromXkeyboardConfig()
    {
        InputConfig cfg;
        const QString rules = dir("rules") + u"/evdev.xml"_s;
        writeRules(rules);
        cfg.setRulesPath(rules);
        const QVariantList list = cfg.availableLayouts();
        QStringList ids;
        for (const QVariant &v : list) {
            ids << v.toMap().value(u"id"_s).toString();
        }
        QCOMPARE(ids, (QStringList{u"us"_s, u"us(dvorak)"_s, u"de"_s}));
        QCOMPARE(list.at(0).toMap().value(u"name"_s).toString(), u"English (US)"_s);
        QCOMPARE(list.at(1).toMap().value(u"name"_s).toString(), u"English (Dvorak)"_s);
    }

    void keyRepeatAndNumLockAreKcminputrc()
    {
        InputConfig cfg;
        QCOMPARE(cfg.keyboard().value(u"delay"_s).toInt(), 600);
        QCOMPARE(cfg.keyboard().value(u"numLock"_s).toString(), u"keep"_s);
        QVERIFY(cfg.setRepeat(300, 40.0));
        QCOMPARE(value(u"kcminputrc"_s, u"Keyboard"_s, u"RepeatDelay"_s).toString(), u"300"_s);
        QCOMPARE(value(u"kcminputrc"_s, u"Keyboard"_s, u"RepeatRate"_s).toString(), u"40"_s);
        QVERIFY(!cfg.setRepeat(5, 40.0));
        QVERIFY(!cfg.setRepeat(300, 5000.0));
        QVERIFY(cfg.setNumLock(u"on"_s));
        QCOMPARE(value(u"kcminputrc"_s, u"Keyboard"_s, u"NumLock"_s).toString(), u"0"_s);
        QCOMPARE(cfg.keyboard().value(u"numLock"_s).toString(), u"on"_s);
        QVERIFY(cfg.setNumLock(u"off"_s));
        QCOMPARE(value(u"kcminputrc"_s, u"Keyboard"_s, u"NumLock"_s).toString(), u"1"_s);
        QVERIFY(cfg.setNumLock(u"keep"_s));
        QCOMPARE(value(u"kcminputrc"_s, u"Keyboard"_s, u"NumLock"_s).toString(), u"2"_s);
        QVERIFY(!cfg.setNumLock(u"maybe"_s));
    }

    void pointersAreKWinsDevices()
    {
        if (!bus) {
            QSKIP("no session bus");
        }
        InputConfig cfg;
        QSignalSpy spy(&cfg, &InputConfig::devicesChanged);
        cfg.refresh();
        QTRY_VERIFY(spy.size() >= 1);
        QVariantMap d = cfg.devices();
        QVERIFY(d.value(u"known"_s).toBool());
        QCOMPARE(d.value(u"pointers"_s).toInt(), 2);
        QCOMPARE(d.value(u"touchpads"_s).toInt(), 1);
        QVERIFY(d.value(u"canTap"_s).toBool());
        // The mouse answers for the speed, the touchpad for natural scrolling.
        QCOMPARE(d.value(u"speed"_s).toDouble(), 0.0);
        QCOMPARE(d.value(u"naturalScroll"_s).toBool(), true);

        cfg.setSpeed(0.6);
        cfg.setNaturalScroll(false);
        cfg.setLeftHanded(true);
        cfg.setTapToClick(true);
        QTRY_COMPARE(mouse.pointerAcceleration, 0.6);
        QTRY_COMPARE(pad.pointerAcceleration, 0.6);
        QTRY_COMPARE(pad.naturalScroll, false);
        QTRY_COMPARE(mouse.leftHanded, true);
        QTRY_COMPARE(pad.tapToClick, true);
        // Tapping is the touchpad's only.
        QCOMPARE(mouse.tapToClick, false);
        // Held between -1 and 1.
        cfg.setSpeed(7.0);
        QTRY_COMPARE(mouse.pointerAcceleration, 1.0);
    }

    void withoutAKWinTheDevicesAreUnknown()
    {
        if (bus) {
            QSKIP("a fake KWin is up");
        }
        InputConfig cfg;
        cfg.refresh();
        QTest::qWait(300);
        QVERIFY(!cfg.devices().value(u"known"_s).toBool());
    }
};

class NotificationsTest : public QObject
{
    Q_OBJECT

private Q_SLOTS:
    void init()
    {
        clearUserFiles();
        write(dir("sys") + u"/applications/org.example.mail.desktop"_s, "[Desktop Entry]\nName=Mail\nIcon=mail-client\nType=Application\n");
        write(dir("sys") + u"/knotifications6/chat.notifyrc"_s, "[Global]\nName=Chat\nIconName=chat\nDesktopEntry=org.example.chat\n");
        write(dir("config") + u"/plasmanotifyrc"_s, "[Applications][org.example.mail]\nShowPopups=false\n[Applications][org.example.gone]\nShowPopups=false\n[Applications][@other]\nShowPopups=false\n");
    }

    void doNotDisturbIsTheUntilKey()
    {
        NotificationsConfig cfg;
        QVERIFY(!cfg.doNotDisturb().value(u"active"_s).toBool());
        QVERIFY(cfg.setDoNotDisturb(u"1h"_s));
        QVERIFY(cfg.doNotDisturb().value(u"active"_s).toBool());
        const qint64 until = cfg.doNotDisturb().value(u"until"_s).toLongLong();
        const qint64 now = QDateTime::currentSecsSinceEpoch();
        QVERIFY(until > now + 3500 && until < now + 3700);
        QVERIFY(value(u"plasmanotifyrc"_s, u"DoNotDisturb"_s, u"Until"_s).isValid());
        QVERIFY(cfg.setDoNotDisturb(u"forever"_s));
        QVERIFY(cfg.doNotDisturb().value(u"forever"_s).toBool());
        QVERIFY(cfg.setDoNotDisturb(u"tomorrow"_s));
        QVERIFY(cfg.doNotDisturb().value(u"until"_s).toLongLong() > now);
        QVERIFY(cfg.setDoNotDisturb(u"off"_s));
        QVERIFY(!value(u"plasmanotifyrc"_s, u"DoNotDisturb"_s, u"Until"_s).isValid());
        QVERIFY(!cfg.doNotDisturb().value(u"active"_s).toBool());
        QVERIFY(!cfg.setDoNotDisturb(u"whenever"_s));
        // A time in the past is not "on".
        {
            KConfig config(dir("config") + u"/plasmanotifyrc"_s, KConfig::SimpleConfig);
            KConfigGroup(&config, u"DoNotDisturb"_s).writeEntry("Until", QDateTime(QDate(2001, 1, 1), QTime(10, 0)));
        }
        QVERIFY(!cfg.doNotDisturb().value(u"active"_s).toBool());
    }

    void appsAreThoseWithANameAndTheirKeysAreThePlasmaOnes()
    {
        NotificationsConfig cfg;
        const QVariantList apps = cfg.apps();
        QStringList ids;
        for (const QVariant &v : apps) {
            ids << v.toMap().value(u"id"_s).toString();
        }
        // The app that is gone, and the catch-all, aren't listed.
        QCOMPARE(ids, (QStringList{u"org.example.chat"_s, u"org.example.mail"_s}));
        const QVariantMap mail = apps.at(1).toMap();
        QCOMPARE(mail.value(u"name"_s).toString(), u"Mail"_s);
        QCOMPARE(mail.value(u"banners"_s).toBool(), false);
        QCOMPARE(mail.value(u"allowed"_s).toBool(), true);

        QVERIFY(cfg.setAppBanners(u"org.example.mail"_s, true));
        QCOMPARE(value(u"plasmanotifyrc"_s, u"Applications][org.example.mail"_s, u"ShowPopups"_s).toString(), u"true"_s);
        QVERIFY(cfg.setAppAllowed(u"org.example.chat"_s, false));
        const QString g = u"Applications][org.example.chat"_s;
        QCOMPARE(value(u"plasmanotifyrc"_s, g, u"ShowPopups"_s).toString(), u"false"_s);
        QCOMPARE(value(u"plasmanotifyrc"_s, g, u"ShowInHistory"_s).toString(), u"false"_s);
        QCOMPARE(value(u"plasmanotifyrc"_s, g, u"ShowBadges"_s).toString(), u"false"_s);
        QCOMPARE(cfg.apps().at(0).toMap().value(u"allowed"_s).toBool(), false);
        QVERIFY(!cfg.setAppAllowed(u"x]\n[Evil"_s, true));
        QVERIFY(!cfg.setAppBanners(u"../x"_s, true));
    }

    void popupsAreTheNotificationsGroup()
    {
        NotificationsConfig cfg;
        QCOMPARE(cfg.popups().value(u"position"_s).toInt(), 0);
        QCOMPARE(cfg.popups().value(u"timeout"_s).toInt(), 5);
        QVERIFY(cfg.setPopupPosition(3));
        QCOMPARE(value(u"plasmanotifyrc"_s, u"Notifications"_s, u"PopupPosition"_s).toString(), u"3"_s);
        QVERIFY(!cfg.setPopupPosition(9));
        QVERIFY(cfg.setPopupTimeout(10));
        QCOMPARE(value(u"plasmanotifyrc"_s, u"Notifications"_s, u"PopupTimeout"_s).toString(), u"10000"_s);
        QCOMPARE(cfg.popups().value(u"timeout"_s).toInt(), 10);
        QVERIFY(cfg.setPopupTimeout(0));
        QCOMPARE(value(u"plasmanotifyrc"_s, u"Notifications"_s, u"ShowPopupTimeout"_s).toString(), u"false"_s);
        QCOMPARE(cfg.popups().value(u"timeout"_s).toInt(), 0);
        QVERIFY(!cfg.setPopupTimeout(100000));
    }
};

class AccessibilityTest : public QObject
{
    Q_OBJECT

private Q_SLOTS:
    void init()
    {
        clearUserFiles();
        write(dir("etc") + u"/kdeglobals"_s,
              "[General]\nfont=IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\nmenuFont=IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\n"
              "toolBarFont=IBM Plex Sans,9,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\nsmallestReadableFont=IBM Plex Sans,8,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\n"
              "fixed=JetBrains Mono,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\n[WM]\nactiveFont=IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\n"
              "[KDE]\nAnimationDurationFactor=0.7071\n");
    }

    void textSizeScalesEveryFontFromTheSystemsSizes()
    {
        AccessibilityConfig cfg;
        QCOMPARE(cfg.read().value(u"textScale"_s).toDouble(), 1.0);
        QVERIFY(cfg.setTextScale(1.25));
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"font"_s).toString(), u"IBM Plex Sans,12.5,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"toolBarFont"_s).toString(), u"IBM Plex Sans,11.5,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"smallestReadableFont"_s).toString(), u"IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"fixed"_s).toString(), u"JetBrains Mono,12.5,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QCOMPARE(value(u"kdeglobals"_s, u"WM"_s, u"activeFont"_s).toString(), u"IBM Plex Sans,12.5,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QCOMPARE(cfg.read().value(u"textScale"_s).toDouble(), 1.25);
        // Scaling again starts from the system's sizes, not the scaled ones.
        QVERIFY(cfg.setTextScale(1.0));
        QCOMPARE(value(u"kdeglobals"_s, u"General"_s, u"font"_s).toString(), u"IBM Plex Sans,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"_s);
        QVERIFY(!cfg.setTextScale(9.0));
        QVERIFY(!cfg.setTextScale(0.1));
    }

    void reduceMotionIsPlasmasAnimationFactor()
    {
        AccessibilityConfig cfg;
        QVERIFY(!cfg.read().value(u"reduceMotion"_s).toBool());
        QVERIFY(cfg.setReduceMotion(true));
        QCOMPARE(value(u"kdeglobals"_s, u"KDE"_s, u"AnimationDurationFactor"_s).toString(), u"0"_s);
        QVERIFY(cfg.read().value(u"reduceMotion"_s).toBool());
        QVERIFY(cfg.setReduceMotion(false));
        // Off: the user's key goes and the system's speed (0.7071) is back.
        QVERIFY(!value(u"kdeglobals"_s, u"KDE"_s, u"AnimationDurationFactor"_s).isValid());
        QVERIFY(!cfg.read().value(u"reduceMotion"_s).toBool());
    }

    void screenReaderIsKaccessrcAndGSettings()
    {
        AccessibilityConfig cfg;
        QSignalSpy run(&cfg, &AccessibilityConfig::run);
        QVERIFY(cfg.setScreenReader(true));
        QCOMPARE(value(u"kaccessrc"_s, u"ScreenReader"_s, u"Enabled"_s).toString(), u"true"_s);
        QCOMPARE(run.at(0).at(0).toStringList(),
                 (QStringList{u"gsettings"_s, u"set"_s, u"org.gnome.desktop.a11y.applications"_s, u"screen-reader-enabled"_s, u"true"_s}));
        QVERIFY(cfg.read().value(u"screenReader"_s).toBool());
    }

    void keyboardAndMouseHelpsAreKaccessrcKeys()
    {
        AccessibilityConfig cfg;
        QVERIFY(cfg.setFlag(u"sticky"_s, true));
        QCOMPARE(value(u"kaccessrc"_s, u"Keyboard"_s, u"StickyKeys"_s).toString(), u"true"_s);
        QVERIFY(cfg.setFlag(u"stickyLock"_s, false));
        QCOMPARE(value(u"kaccessrc"_s, u"Keyboard"_s, u"StickyKeysLatch"_s).toString(), u"false"_s);
        QVERIFY(cfg.setFlag(u"mouseKeys"_s, true));
        QCOMPARE(value(u"kaccessrc"_s, u"Mouse"_s, u"MouseKeys"_s).toString(), u"true"_s);
        QVERIFY(cfg.setFlag(u"shake"_s, false));
        QCOMPARE(value(u"kwinrc"_s, u"Plugins"_s, u"shakecursorEnabled"_s).toString(), u"false"_s);
        QVERIFY(!cfg.setFlag(u"StickyKeys"_s, true));
        QVERIFY(cfg.read().value(u"sticky"_s).toBool());
        QVERIFY(!cfg.read().value(u"stickyLock"_s).toBool());
        // A flag nobody set reads as the module's default.
        QVERIFY(!cfg.read().value(u"stickyBeep"_s).toBool());
    }
};

int main(int argc, char **argv)
{
    QTemporaryDir tmp;
    root = &tmp;
    if (!tmp.isValid()) {
        return 1;
    }
    // Everything the backends read is under the temporary folder.
    qputenv("XDG_CONFIG_HOME", dir("config").toUtf8());
    qputenv("XDG_DATA_HOME", dir("data").toUtf8());
    qputenv("XDG_CONFIG_DIRS", dir("etc").toUtf8());
    qputenv("XDG_DATA_DIRS", dir("sys").toUtf8());
    qputenv("XDG_CACHE_HOME", dir("cache").toUtf8());
    qputenv("XDG_RUNTIME_DIR", dir("run").toUtf8());
    QDir().mkpath(dir("run"));
    QFile::setPermissions(dir("run"), QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    QCoreApplication app(argc, argv);
    int failed = 0;
    {
        AppearanceTest t;
        failed += QTest::qExec(&t, argc, argv);
    }
    {
        ShellTest t;
        failed += QTest::qExec(&t, argc, argv);
    }
    {
        InputTest t;
        failed += QTest::qExec(&t, argc, argv);
    }
    {
        NotificationsTest t;
        failed += QTest::qExec(&t, argc, argv);
    }
    {
        AccessibilityTest t;
        failed += QTest::qExec(&t, argc, argv);
    }
    return failed;
}

#include "pageconfig_test.moc"
