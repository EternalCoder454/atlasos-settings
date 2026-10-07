#include "appearanceconfig.h"

#include "kdeutil.h"

#include <KConfigWatcher>
#include <KSharedConfig>

#include <QColor>
#include <QDBusArgument>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QTimer>
#include <QUrl>

#include <memory>

using namespace Qt::StringLiterals;

namespace
{
// Atlas.Ui's violet per scheme: what the accent is when none is chosen
// (AtlasOS's colour schemes' DecorationFocus).
constexpr auto VioletLight = "#6858e2";
constexpr auto VioletDark = "#8a7af4";

constexpr auto HighContrastLight = "AtlasOSHighContrastLight";
constexpr auto HighContrastDark = "AtlasOSHighContrastDark";

// A name a tool is given as a program argument: plain characters only, no
// quote (plasma-apply-wallpaperimage builds a script around the name).
bool plainName(const QString &name, int max = 100)
{
    static const QRegularExpression re(u"^[A-Za-z0-9._-]+$"_s);
    return !name.isEmpty() && name.size() <= max && re.match(name).hasMatch();
}

bool plainPath(const QString &path)
{
    if (!path.startsWith(u'/') || path.size() > 1024 || path.contains(u'\'')) {
        return false;
    }
    for (const QChar c : path) {
        if (c.unicode() < 0x20 || c.unicode() == 0x7f) {
            return false;
        }
    }
    return true;
}

// The icon themes and window decorations of AtlasOS's Light and Dark
// themes. Anything else the user picked stays.
QString atlasIcons(bool dark)
{
    return dark ? u"Papirus-Dark"_s : u"Papirus"_s;
}
QString atlasDecoration(bool dark)
{
    return dark ? u"__aurorae__svg__AtlasOS-Dark"_s : u"__aurorae__svg__AtlasOS-Light"_s;
}

// ElectricBorder values of KWin's enum.
int edgeOf(const QString &corner)
{
    if (corner == u"topLeft"_s) {
        return 7;
    }
    if (corner == u"topRight"_s) {
        return 1;
    }
    if (corner == u"bottomRight"_s) {
        return 3;
    }
    if (corner == u"bottomLeft"_s) {
        return 5;
    }
    return -1;
}

QString keyOf(const QString &corner)
{
    return corner.left(1).toUpper() + corner.mid(1);
}

QString colorKey(const QColor &c)
{
    return QStringLiteral("%1,%2,%3").arg(c.red()).arg(c.green()).arg(c.blue());
}

// The colour scheme's groups, set for high contrast: pure backgrounds and
// foregrounds, with a yellow (dark) or blue (light) focus ring.
void makeHighContrast(KConfig &scheme, bool dark, const QString &id, const QString &name)
{
    const QColor bg = dark ? QColor(0, 0, 0) : QColor(255, 255, 255);
    const QColor bgAlt = dark ? QColor(26, 26, 26) : QColor(232, 232, 232);
    const QColor fg = dark ? QColor(255, 255, 255) : QColor(0, 0, 0);
    const QColor fgInactive = dark ? QColor(214, 214, 214) : QColor(64, 64, 64);
    const QColor focus = dark ? QColor(255, 235, 59) : QColor(0, 0, 204);
    const QColor link = dark ? QColor(102, 204, 255) : QColor(0, 0, 204);
    const QColor visited = dark ? QColor(221, 153, 255) : QColor(102, 0, 153);
    const QColor negative = dark ? QColor(255, 120, 120) : QColor(160, 0, 0);
    const QColor neutral = dark ? QColor(255, 176, 0) : QColor(140, 60, 0);
    const QColor positive = dark ? QColor(110, 255, 110) : QColor(0, 100, 0);

    for (const QString &name : {u"Window"_s, u"View"_s, u"Button"_s, u"Tooltip"_s, u"Complementary"_s, u"Header"_s}) {
        KConfigGroup g(&scheme, u"Colors:"_s + name);
        g.writeEntry("BackgroundNormal", colorKey(bg));
        g.writeEntry("BackgroundAlternate", colorKey(bgAlt));
        g.writeEntry("ForegroundNormal", colorKey(fg));
        g.writeEntry("ForegroundInactive", colorKey(fgInactive));
        g.writeEntry("ForegroundActive", colorKey(focus));
        g.writeEntry("ForegroundLink", colorKey(link));
        g.writeEntry("ForegroundVisited", colorKey(visited));
        g.writeEntry("ForegroundNegative", colorKey(negative));
        g.writeEntry("ForegroundNeutral", colorKey(neutral));
        g.writeEntry("ForegroundPositive", colorKey(positive));
        g.writeEntry("DecorationFocus", colorKey(focus));
        g.writeEntry("DecorationHover", colorKey(focus));
    }
    {
        // The chosen item: the focus colour with the opposite text.
        KConfigGroup g(&scheme, u"Colors:Selection"_s);
        g.writeEntry("BackgroundNormal", colorKey(focus));
        g.writeEntry("BackgroundAlternate", colorKey(focus));
        for (const char *key : {"ForegroundNormal", "ForegroundActive", "ForegroundInactive", "ForegroundLink", "ForegroundVisited", "ForegroundNegative", "ForegroundNeutral", "ForegroundPositive"}) {
            g.writeEntry(key, colorKey(dark ? QColor(0, 0, 0) : QColor(255, 255, 255)));
        }
        g.writeEntry("DecorationFocus", colorKey(fg));
        g.writeEntry("DecorationHover", colorKey(fg));
    }
    {
        KConfigGroup g(&scheme, u"WM"_s);
        g.writeEntry("activeBackground", colorKey(bg));
        g.writeEntry("activeForeground", colorKey(fg));
        g.writeEntry("activeBlend", colorKey(fg));
        g.writeEntry("inactiveBackground", colorKey(bg));
        g.writeEntry("inactiveForeground", colorKey(fgInactive));
        g.writeEntry("inactiveBlend", colorKey(fgInactive));
    }
    KConfigGroup general(&scheme, u"General"_s);
    general.writeEntry("ColorScheme", id);
    general.writeEntry("Name", name);
}

QString firstImageIn(const QString &dir)
{
    const QFileInfoList files = QDir(dir).entryInfoList({u"*.png"_s, u"*.jpg"_s, u"*.jpeg"_s, u"*.webp"_s}, QDir::Files | QDir::Readable, QDir::Name);
    // The middle of the list: not the smallest, not a huge one.
    return files.isEmpty() ? QString() : files.at(files.size() / 2).absoluteFilePath();
}

QString packagePreview(const QString &packageDir)
{
    for (const char *ext : {"png", "jpg", "jpeg", "webp"}) {
        const QString shot = packageDir + u"/contents/screenshot."_s + QString::fromLatin1(ext);
        if (QFileInfo::exists(shot)) {
            return shot;
        }
    }
    return firstImageIn(packageDir + u"/contents/images"_s);
}

QString packageName(const QString &packageDir, const QString &id)
{
    QFile f(packageDir + u"/metadata.json"_s);
    if (f.size() < 65536 && f.open(QIODevice::ReadOnly)) {
        const QString name = QJsonDocument::fromJson(f.readAll()).object().value(u"KPlugin"_s).toObject().value(u"Name"_s).toString().left(100);
        if (!name.isEmpty()) {
            return name;
        }
    }
    return id;
}

QString packageDirOf(const QString &id)
{
    for (const QString &root : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, u"wallpapers"_s, QStandardPaths::LocateDirectory)) {
        if (QFileInfo(root + u'/' + id).isDir()) {
            return root + u'/' + id;
        }
    }
    return {};
}
}

AppearanceConfig::AppearanceConfig(QObject *parent)
    : QObject(parent)
{
    m_watcher = KConfigWatcher::create(KSharedConfig::openConfig(u"kdeglobals"_s, KConfig::NoGlobals));
    connect(m_watcher.data(), &KConfigWatcher::configChanged, this, [this](const KConfigGroup &group, const QByteArrayList &) {
        const QString name = group.name();
        if (name.isEmpty() || name == u"General"_s || name == u"KDE"_s || name == u"Icons"_s) {
            scheduleRead();
        }
    });
}

void AppearanceConfig::scheduleRead()
{
    // Applying a scheme saves kdeglobals several times in a row: read once
    // they have settled.
    QTimer::singleShot(600, this, &AppearanceConfig::changed);
}

QString AppearanceConfig::currentScheme() const
{
    // As kvantum-sync reads it: the user's kdeglobals, then the Global
    // Theme's defaults (kdedefaults), then the system's.
    KConfig userConfig(kdeutil::configHome() + u"/kdeglobals"_s, KConfig::SimpleConfig);
    QString scheme = KConfigGroup(&userConfig, u"General"_s).readEntry("ColorScheme", QString());
    if (scheme.isEmpty()) {
        KConfig defaults(kdeutil::configHome() + u"/kdedefaults/kdeglobals"_s, KConfig::SimpleConfig);
        scheme = KConfigGroup(&defaults, u"General"_s).readEntry("ColorScheme", QString());
    }
    if (scheme.isEmpty()) {
        scheme = kdeutil::layered(u"kdeglobals"_s, u"General"_s, u"ColorScheme"_s);
    }
    return scheme;
}

QString AppearanceConfig::accentHex() const
{
    KConfig config = kdeutil::user(u"kdeglobals"_s);
    const QColor color = KConfigGroup(&config, u"General"_s).readEntry("AccentColor", QColor());
    return color.isValid() ? color.name() : QString();
}

bool AppearanceConfig::validColor(const QString &hex)
{
    static const QRegularExpression re(u"^#[0-9a-fA-F]{6}$"_s);
    return re.match(hex).hasMatch();
}

QVariantMap AppearanceConfig::read() const
{
    const QString scheme = currentScheme();
    const bool dark = scheme.contains(u"Dark"_s);
    const bool highContrast = scheme.startsWith(u"AtlasOSHighContrast"_s);
    KConfig config = kdeutil::user(u"kdeglobals"_s);
    const bool fromWallpaper = KConfigGroup(&config, u"General"_s).readEntry("accentColorFromWallpaper", false);
    const QString accent = accentHex();
    const bool isDefault = !fromWallpaper && (accent.isEmpty() || accent.compare(QLatin1String(dark ? VioletDark : VioletLight), Qt::CaseInsensitive) == 0);

    // The wallpaper of the desktop: Plasma keeps it in the applets file
    // (read only; the change goes through plasma-apply-wallpaperimage).
    QVariantMap wallpaper{{u"id"_s, QString()}, {u"name"_s, QString()}, {u"preview"_s, QString()}};
    KConfig applets(u"plasma-org.kde.plasma.desktop-appletsrc"_s, KConfig::SimpleConfig);
    const KConfigGroup containments(&applets, u"Containments"_s);
    for (const QString &id : containments.groupList()) {
        const KConfigGroup c(&containments, id);
        const KConfigGroup general = c.group(u"Wallpaper"_s).group(u"org.kde.image"_s).group(u"General"_s);
        const QString image = general.readEntry("Image", QString());
        if (image.isEmpty()) {
            continue;
        }
        const QString path = QUrl(image).isLocalFile() ? QUrl(image).toLocalFile() : image;
        const QFileInfo info(path);
        if (info.isDir() || path.endsWith(u'/')) {
            const QString dirName = QDir(path).dirName();
            const QString dir = packageDirOf(dirName);
            wallpaper = {{u"id"_s, dirName},
                         {u"name"_s, dir.isEmpty() ? dirName : packageName(dir, dirName)},
                         {u"preview"_s, dir.isEmpty() ? QString() : QUrl::fromLocalFile(packagePreview(dir)).toString()}};
        } else {
            wallpaper = {{u"id"_s, path}, {u"name"_s, info.completeBaseName()}, {u"preview"_s, QUrl::fromLocalFile(path).toString()}};
        }
        break;
    }

    return {
        {u"scheme"_s, scheme},
        {u"dark"_s, dark},
        {u"highContrast"_s, highContrast},
        {u"accent"_s, accent},
        {u"accentFromWallpaper"_s, fromWallpaper},
        {u"accentIsDefault"_s, isDefault},
        {u"lookAndFeel"_s, kdeutil::layered(u"kdeglobals"_s, u"KDE"_s, u"LookAndFeelPackage"_s)},
        {u"wallpaper"_s, wallpaper},
    };
}

void AppearanceConfig::applyScheme(const QString &scheme, const QString &accent)
{
    QStringList argv{u"plasma-apply-colorscheme"_s};
    if (!accent.isEmpty()) {
        argv << u"--accent-color"_s << accent;
    }
    argv << scheme;
    Q_EMIT run(argv);
    scheduleRead();
}

void AppearanceConfig::switchThemeParts(bool dark)
{
    // The icons and the window decoration of AtlasOS's own Light and Dark
    // themes follow; a theme the user picked is left alone.
    KConfig globals = kdeutil::user(u"kdeglobals"_s);
    const QString icons = kdeutil::layered(u"kdeglobals"_s, u"Icons"_s, u"Theme"_s);
    if ((icons == u"Papirus"_s || icons == u"Papirus-Dark"_s) && icons != atlasIcons(dark)) {
        KConfigGroup(&globals, u"Icons"_s).writeEntry("Theme", atlasIcons(dark), kdeutil::Notify);
        globals.sync();
    }
    const QString decoration = kdeutil::layered(u"kwinrc"_s, u"org.kde.kdecoration2"_s, u"theme"_s);
    if ((decoration == atlasDecoration(true) || decoration == atlasDecoration(false)) && decoration != atlasDecoration(dark)) {
        KConfig kwin = kdeutil::user(u"kwinrc"_s);
        KConfigGroup g(&kwin, u"org.kde.kdecoration2"_s);
        g.writeEntry("library", u"org.kde.kwin.aurorae"_s, kdeutil::Notify);
        g.writeEntry("theme", atlasDecoration(dark), kdeutil::Notify);
        kwin.sync();
        kdeutil::reconfigureKWin(this);
    }
}

bool AppearanceConfig::ensureHighContrastScheme(bool dark)
{
    const QString id = QLatin1String(dark ? HighContrastDark : HighContrastLight);
    const QString base = dark ? u"AtlasOSDark"_s : u"AtlasOSLight"_s;
    const QString source = QStandardPaths::locate(QStandardPaths::GenericDataLocation, u"color-schemes/"_s + base + u".colors"_s);
    const QString dir = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + u"/color-schemes"_s;
    if (source.isEmpty() || !QDir().mkpath(dir)) {
        return false;
    }
    const QString target = dir + u'/' + id + u".colors"_s;
    KConfig src(source, KConfig::SimpleConfig);
    std::unique_ptr<KConfig> out(src.copyTo(target));
    if (!out) {
        return false;
    }
    makeHighContrast(*out, dark, id, dark ? u"Telamon OS High Contrast Dark"_s : u"Telamon OS High Contrast Light"_s);
    return out->sync();
}

void AppearanceConfig::setDark(bool dark)
{
    const QVariantMap now = read();
    const bool hc = now.value(u"highContrast"_s).toBool();
    QString accent;
    if (!hc && !now.value(u"accentFromWallpaper"_s).toBool()) {
        // The default accent is the violet of the scheme about to be applied.
        accent = now.value(u"accentIsDefault"_s).toBool() ? QLatin1String(dark ? VioletDark : VioletLight) : now.value(u"accent"_s).toString();
    }
    QString scheme = dark ? u"AtlasOSDark"_s : u"AtlasOSLight"_s;
    if (hc) {
        if (!ensureHighContrastScheme(dark)) {
            Q_EMIT failed(tr("Settings couldn't make the high contrast colors."));
            return;
        }
        scheme = QLatin1String(dark ? HighContrastDark : HighContrastLight);
    }
    switchThemeParts(dark);
    applyScheme(scheme, accent);
}

void AppearanceConfig::setHighContrast(bool on)
{
    const QVariantMap now = read();
    const bool dark = now.value(u"dark"_s).toBool();
    if (on) {
        if (!ensureHighContrastScheme(dark)) {
            Q_EMIT failed(tr("Settings couldn't make the high contrast colors."));
            return;
        }
        applyScheme(QLatin1String(dark ? HighContrastDark : HighContrastLight), QString());
    } else {
        const bool custom = !now.value(u"accentIsDefault"_s).toBool() && !now.value(u"accentFromWallpaper"_s).toBool();
        applyScheme(dark ? u"AtlasOSDark"_s : u"AtlasOSLight"_s, custom ? now.value(u"accent"_s).toString() : QLatin1String(dark ? VioletDark : VioletLight));
    }
}

void AppearanceConfig::setAccent(const QString &hex)
{
    if (!hex.isEmpty() && !validColor(hex)) {
        return;
    }
    const QVariantMap now = read();
    if (now.value(u"highContrast"_s).toBool()) {
        return;
    }
    const bool dark = now.value(u"dark"_s).toBool();
    {
        KConfig config = kdeutil::user(u"kdeglobals"_s);
        KConfigGroup g(&config, u"General"_s);
        g.writeEntry("accentColorFromWallpaper", false, kdeutil::Notify);
        if (!hex.isEmpty() && hex.compare(QLatin1String(dark ? VioletDark : VioletLight), Qt::CaseInsensitive) != 0) {
            g.writeEntry("LastUsedCustomAccentColor", QColor(hex), kdeutil::Notify);
        }
        config.sync();
    }
    applyScheme(now.value(u"scheme"_s).toString(), hex.isEmpty() ? QLatin1String(dark ? VioletDark : VioletLight) : hex.toLower());
}

void AppearanceConfig::setAccentFromWallpaper()
{
    if (read().value(u"highContrast"_s).toBool()) {
        return;
    }
    KConfig config = kdeutil::user(u"kdeglobals"_s);
    KConfigGroup(&config, u"General"_s).writeEntry("accentColorFromWallpaper", true, kdeutil::Notify);
    config.sync();
    scheduleRead();
}

QVariantList AppearanceConfig::wallpapers() const
{
    QVariantList list;
    QStringList seen;
    for (const QString &root : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, u"wallpapers"_s, QStandardPaths::LocateDirectory)) {
        const QFileInfoList dirs = QDir(root).entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::Readable, QDir::Name);
        for (const QFileInfo &dir : dirs) {
            const QString id = dir.fileName();
            // The login screen's picture is not a desktop wallpaper.
            if (!plainName(id) || id.endsWith(u"-Login"_s) || seen.contains(id) || list.size() >= 80) {
                continue;
            }
            const QString preview = packagePreview(dir.absoluteFilePath());
            if (preview.isEmpty()) {
                continue;
            }
            seen << id;
            list.append(QVariantMap{{u"id"_s, id},
                                    {u"name"_s, packageName(dir.absoluteFilePath(), id)},
                                    {u"preview"_s, QUrl::fromLocalFile(preview).toString()},
                                    {u"kind"_s, u"package"_s}});
        }
    }
    const QString pictures = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
    const QFileInfoList images = QDir(pictures).entryInfoList({u"*.png"_s, u"*.jpg"_s, u"*.jpeg"_s, u"*.webp"_s, u"*.bmp"_s}, QDir::Files | QDir::Readable, QDir::Name | QDir::IgnoreCase);
    int count = 0;
    for (const QFileInfo &image : images) {
        if (!plainPath(image.absoluteFilePath()) || image.size() == 0 || ++count > 120) {
            continue;
        }
        list.append(QVariantMap{{u"id"_s, image.absoluteFilePath()},
                                {u"name"_s, image.completeBaseName().left(100)},
                                {u"preview"_s, QUrl::fromLocalFile(image.absoluteFilePath()).toString()},
                                {u"kind"_s, u"image"_s}});
    }
    return list;
}

void AppearanceConfig::setWallpaper(const QString &id)
{
    // Only what wallpapers() offers.
    for (const QVariant &v : wallpapers()) {
        const QVariantMap w = v.toMap();
        if (w.value(u"id"_s).toString() == id) {
            Q_EMIT run({u"plasma-apply-wallpaperimage"_s, id});
            scheduleRead();
            return;
        }
    }
}

QVariantList AppearanceConfig::lookAndFeels() const
{
    QVariantList list;
    QStringList seen;
    for (const QString &root : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, u"plasma/look-and-feel"_s, QStandardPaths::LocateDirectory)) {
        const QFileInfoList dirs = QDir(root).entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::Readable, QDir::Name);
        for (const QFileInfo &dir : dirs) {
            QFile f(dir.absoluteFilePath() + u"/metadata.json"_s);
            if (f.size() > 65536 || !f.open(QIODevice::ReadOnly)) {
                continue;
            }
            const QJsonObject plugin = QJsonDocument::fromJson(f.readAll()).object().value(u"KPlugin"_s).toObject();
            const QString id = plugin.value(u"Id"_s).toString();
            if (!plainName(id) || id != dir.fileName() || seen.contains(id) || list.size() >= 60) {
                continue;
            }
            seen << id;
            QString name = plugin.value(u"Name"_s).toString().left(100);
            list.append(QVariantMap{{u"id"_s, id},
                                    {u"name"_s, name.isEmpty() ? id : name},
                                    {u"description"_s, plugin.value(u"Description"_s).toString().left(200)}});
        }
    }
    return list;
}

void AppearanceConfig::setLookAndFeel(const QString &id)
{
    for (const QVariant &v : lookAndFeels()) {
        if (v.toMap().value(u"id"_s).toString() == id) {
            Q_EMIT run({u"plasma-apply-lookandfeel"_s, u"--apply"_s, id});
            scheduleRead();
            return;
        }
    }
}

void AppearanceConfig::runShellScript(const QString &script)
{
    kdeutil::call(this, u"org.kde.plasmashell"_s, u"/PlasmaShell"_s, u"org.kde.PlasmaShell"_s, u"evaluateScript"_s, {script}, [this](const QDBusMessage &reply) {
        if (reply.type() == QDBusMessage::ErrorMessage) {
            Q_EMIT failed(tr("Plasma's shell didn't answer, so the dock wasn't changed."));
        }
        // Plasma saves a panel's settings a moment after the change.
        QTimer::singleShot(700, this, &AppearanceConfig::refreshShell);
    });
}

void AppearanceConfig::refreshShell()
{
    // The same questions the AtlasOS menu bar toggle asks: Plasma's script
    // API reads the dock (the panel with the task manager and the dock
    // separator) and the top bar's islands (floating, fitted panels at the
    // top). "dodgewindows" is a bar that hides under windows.
    static const QString script = QStringLiteral(
        "var out = {dock: null, bar: null};"
        "var all = panels();"
        "var docks = all.filter(function (p) {"
        "  return p.widgets('org.kde.plasma.icontasks').length > 0 && p.widgets('org.atlasos.dockseparator').length > 0;"
        "});"
        "if (docks.length > 0) { out.dock = {location: docks[0].location, hiding: docks[0].hiding, height: docks[0].height}; }"
        "var bar = all.filter(function (p) { return p.location == 'top' && p.floating && p.lengthMode == 'fit'; });"
        "out.bar = {count: bar.length, hides: bar.some(function (p) { return p.hiding == 'dodgewindows'; })};"
        "print(JSON.stringify(out));");
    kdeutil::call(this, u"org.kde.plasmashell"_s, u"/PlasmaShell"_s, u"org.kde.PlasmaShell"_s, u"evaluateScript"_s, {script}, [this](const QDBusMessage &reply) {
        QVariantMap shell{{u"known"_s, false}};
        if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
            const QByteArray text = reply.arguments().at(0).toString().left(4096).toUtf8();
            const QJsonObject root = QJsonDocument::fromJson(text).object();
            if (!root.isEmpty()) {
                const QJsonObject dock = root.value(u"dock"_s).toObject();
                const QString location = dock.value(u"location"_s).toString();
                const bool found = !dock.isEmpty() && (location == u"bottom"_s || location == u"left"_s || location == u"right"_s || location == u"top"_s);
                const QJsonObject bar = root.value(u"bar"_s).toObject();
                shell = {
                    {u"known"_s, true},
                    {u"dock"_s,
                     QVariantMap{{u"found"_s, found},
                                 {u"location"_s, location},
                                 {u"autohide"_s, dock.value(u"hiding"_s).toString() == u"autohide"_s},
                                 {u"height"_s, std::clamp(dock.value(u"height"_s).toInt(60), 16, 400)}}},
                    {u"topBar"_s, QVariantMap{{u"found"_s, bar.value(u"count"_s).toInt() > 0}, {u"hides"_s, bar.value(u"hides"_s).toBool()}}},
                };
            }
        }
        if (shell != m_shell) {
            m_shell = shell;
            Q_EMIT shellChanged();
        }
    });

    kdeutil::call(this,
                  u"org.kde.KWin"_s,
                  u"/VirtualDesktopManager"_s,
                  u"org.freedesktop.DBus.Properties"_s,
                  u"GetAll"_s,
                  {u"org.kde.KWin.VirtualDesktopManager"_s},
                  [this](const QDBusMessage &reply) {
                      int count = 0;
                      QStringList ids;
                      if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
                          const QVariantMap props = qdbus_cast<QVariantMap>(reply.arguments().at(0));
                          count = std::clamp(props.value(u"count"_s).toInt(), 0, 100);
                          const QDBusArgument arg = props.value(u"desktops"_s).value<QDBusArgument>();
                          if (arg.currentSignature() == u"a(uss)"_s) {
                              arg.beginArray();
                              while (!arg.atEnd() && ids.size() < 100) {
                                  uint position = 0;
                                  QString id;
                                  QString name;
                                  arg.beginStructure();
                                  arg >> position >> id >> name;
                                  arg.endStructure();
                                  ids << id;
                              }
                              arg.endArray();
                          }
                      }
                      if (count != m_desktopCount || ids != m_desktopIds) {
                          m_desktopCount = count;
                          m_desktopIds = ids;
                          Q_EMIT desktopsChanged();
                      }
                  });
}

void AppearanceConfig::setDockAutoHide(bool hide)
{
    runShellScript(u"panels().forEach(function (p) {"
                   "  if (p.widgets('org.kde.plasma.icontasks').length > 0 && p.widgets('org.atlasos.dockseparator').length > 0) { p.hiding = '"_s
                   + (hide ? u"autohide"_s : u"none"_s) + u"'; }});"_s);
}

void AppearanceConfig::setDockSize(int pixels)
{
    const int px = std::clamp(pixels, 40, 96);
    runShellScript(u"panels().forEach(function (p) {"
                   "  if (p.widgets('org.kde.plasma.icontasks').length > 0 && p.widgets('org.atlasos.dockseparator').length > 0) { p.height = "_s
                   + QString::number(px) + u"; }});"_s);
}

void AppearanceConfig::setDockPosition(const QString &location)
{
    if (location != u"bottom"_s && location != u"left"_s && location != u"right"_s) {
        return;
    }
    runShellScript(u"panels().forEach(function (p) {"
                   "  if (p.widgets('org.kde.plasma.icontasks').length > 0 && p.widgets('org.atlasos.dockseparator').length > 0) { p.location = '"_s
                   + location + u"'; }});"_s);
}

void AppearanceConfig::setTopBarHides(bool hides)
{
    // As the AtlasOS menu bar toggle sets it: "windowsgobelow" keeps the
    // islands over the windows, "dodgewindows" hides them under windows.
    runShellScript(u"panels().filter(function (p) { return p.location == 'top' && p.floating && p.lengthMode == 'fit'; })"
                   ".forEach(function (p) { p.hiding = '"_s
                   + (hides ? u"dodgewindows"_s : u"windowsgobelow"_s) + u"'; });"_s);
}

void AppearanceConfig::setDesktopCount(int count)
{
    const int want = std::clamp(count, 1, 20);
    const int have = m_desktopCount;
    if (have <= 0 || want == have) {
        return;
    }
    const QString service = u"org.kde.KWin"_s;
    const QString path = u"/VirtualDesktopManager"_s;
    const QString iface = u"org.kde.KWin.VirtualDesktopManager"_s;
    if (want > have) {
        for (int i = have; i < want; ++i) {
            kdeutil::call(this, service, path, iface, u"createDesktop"_s, {QVariant::fromValue<uint>(uint(i)), tr("Desktop %1").arg(i + 1)});
        }
    } else if (m_desktopIds.size() == have) {
        // The last ones go, as the module removes them.
        for (int i = have - 1; i >= want; --i) {
            kdeutil::call(this, service, path, iface, u"removeDesktop"_s, {m_desktopIds.at(i)});
        }
    } else {
        return;
    }
    QTimer::singleShot(500, this, &AppearanceConfig::refreshShell);
}

QVariantMap AppearanceConfig::hotCorners() const
{
    KConfig kwin(u"kwinrc"_s, KConfig::NoGlobals);
    const KConfigGroup borders(&kwin, u"ElectricBorders"_s);
    const QList<int> overview = KConfigGroup(&kwin, u"Effect-overview"_s).readEntry("BorderActivate", QList<int>());
    QVariantMap result;
    for (const QString &corner : {u"topLeft"_s, u"topRight"_s, u"bottomLeft"_s, u"bottomRight"_s}) {
        QString action = u"none"_s;
        const QString value = borders.readEntry(keyOf(corner), u"None"_s);
        if (overview.contains(edgeOf(corner))) {
            action = u"overview"_s;
        } else if (value == u"ShowDesktop"_s) {
            action = u"desktop"_s;
        } else if (value == u"LockScreen"_s) {
            action = u"lock"_s;
        } else if (value != u"None"_s) {
            // KRunner, Activities or the launcher, set in KWin's own module.
            action = u"other"_s;
        }
        result.insert(corner, action);
    }
    return result;
}

bool AppearanceConfig::setHotCorner(const QString &corner, const QString &action)
{
    const int edge = edgeOf(corner);
    if (edge < 0 || (action != u"none"_s && action != u"overview"_s && action != u"desktop"_s && action != u"lock"_s)) {
        return false;
    }
    KConfig kwin = kdeutil::user(u"kwinrc"_s);
    KConfigGroup borders(&kwin, u"ElectricBorders"_s);
    KConfigGroup effect(&kwin, u"Effect-overview"_s);
    QList<int> overview = effect.readEntry("BorderActivate", QList<int>());
    overview.removeAll(edge);
    if (action == u"overview"_s) {
        overview.append(edge);
    }
    if (overview.isEmpty()) {
        effect.deleteEntry("BorderActivate", kdeutil::Notify);
    } else {
        effect.writeEntry("BorderActivate", overview, kdeutil::Notify);
    }
    borders.writeEntry(keyOf(corner), action == u"desktop"_s ? u"ShowDesktop"_s : action == u"lock"_s ? u"LockScreen"_s : u"None"_s, kdeutil::Notify);
    const bool ok = kwin.sync();
    kdeutil::reconfigureKWin(this);
    kdeutil::call(this, u"org.kde.KWin"_s, u"/Effects"_s, u"org.kde.kwin.Effects"_s, u"reconfigureEffect"_s, {u"overview"_s});
    return ok;
}
