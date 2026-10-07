#include "accessibilityconfig.h"

#include "kdeutil.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QFileInfo>

#include <cmath>

using namespace Qt::StringLiterals;

namespace
{
struct FontKey
{
    const char *group;
    const char *key;
    double fallback; // the size when the system file says nothing
};

// The six fonts "Adjust All Fonts" scales.
constexpr FontKey FontKeys[] = {
    {"General", "font", 10.0},
    {"General", "menuFont", 10.0},
    {"General", "toolBarFont", 9.0},
    {"General", "smallestReadableFont", 8.0},
    {"General", "fixed", 10.0},
    {"WM", "activeFont", 10.0},
};

struct Flag
{
    const char *name;
    const char *file;
    const char *group;
    const char *key;
    bool fallback;
};

constexpr Flag Flags[] = {
    {"sticky", "kaccessrc", "Keyboard", "StickyKeys", false},
    {"stickyLock", "kaccessrc", "Keyboard", "StickyKeysLatch", true},
    {"stickyOff", "kaccessrc", "Keyboard", "StickyKeysAutoOff", false},
    {"stickyBeep", "kaccessrc", "Keyboard", "StickyKeysBeep", false},
    {"toggleBeep", "kaccessrc", "Keyboard", "ToggleKeysBeep", false},
    {"mouseKeys", "kaccessrc", "Mouse", "MouseKeys", false},
    {"shake", "kwinrc", "Plugins", "shakecursorEnabled", true},
};

// The point size in a Qt font description ("IBM Plex Sans,10,-1,5,..."), or
// 0 when there is none.
double pointSize(const QString &font)
{
    const QStringList fields = font.split(u',');
    bool ok = false;
    const double size = fields.size() > 2 ? fields.at(1).toDouble(&ok) : 0.0;
    return ok && size > 0 ? size : 0.0;
}

QString withPointSize(const QString &font, double size)
{
    QStringList fields = font.split(u',');
    fields[1] = QString::number(size, 'g', 4);
    return fields.join(u',');
}

// The size the system sets for a font (the files under the user's), else
// `fallback`.
double systemSize(const FontKey &k)
{
    const QString user = kdeutil::configHome() + u"/kdeglobals"_s;
    for (const QString &file : QStandardPaths::locateAll(QStandardPaths::GenericConfigLocation, u"kdeglobals"_s)) {
        if (QFileInfo(file) == QFileInfo(user)) {
            continue;
        }
        KConfig config(file, KConfig::SimpleConfig);
        const double size = pointSize(KConfigGroup(&config, QString::fromLatin1(k.group)).readEntry(k.key, QString()));
        if (size > 0) {
            return size;
        }
    }
    return k.fallback;
}

void announceFonts()
{
    // What the Fonts module sends so running apps read their fonts again.
    QDBusConnection::sessionBus().send(QDBusMessage::createSignal(u"/KDEPlatformTheme"_s, u"org.kde.KDEPlatformTheme"_s, u"refreshFonts"_s));
}
}

QVariantMap AccessibilityConfig::read() const
{
    const FontKey &main = FontKeys[0];
    const double current = pointSize(kdeutil::layered(u"kdeglobals"_s, QString::fromLatin1(main.group), QString::fromLatin1(main.key)));
    const double scale = current > 0 ? std::round(current / systemSize(main) * 20.0) / 20.0 : 1.0;

    bool factorOk = false;
    const double factor = kdeutil::layered(u"kdeglobals"_s, u"KDE"_s, u"AnimationDurationFactor"_s).toDouble(&factorOk);

    QVariantMap map{
        {u"textScale"_s, std::clamp(scale, 0.8, 2.0)},
        {u"reduceMotion"_s, factorOk && factor == 0.0},
        {u"zoom"_s, kdeutil::layered(u"kwinrc"_s, u"Plugins"_s, u"zoomEnabled"_s, u"true"_s) != u"false"_s},
        {u"screenReader"_s, kdeutil::layered(u"kaccessrc"_s, u"ScreenReader"_s, u"Enabled"_s, u"false"_s) == u"true"_s},
        {u"orcaInstalled"_s, QFileInfo(u"/usr/bin/orca"_s).isExecutable()},
    };
    for (const Flag &f : Flags) {
        const QString value = kdeutil::layered(QString::fromLatin1(f.file), QString::fromLatin1(f.group), QString::fromLatin1(f.key));
        map.insert(QString::fromLatin1(f.name), value.isEmpty() ? f.fallback : value == u"true"_s);
    }
    return map;
}

bool AccessibilityConfig::setTextScale(double scale)
{
    if (!(scale >= 0.8 && scale <= 2.0)) {
        return false;
    }
    scale = std::round(scale * 20.0) / 20.0;
    KConfig config = kdeutil::user(u"kdeglobals"_s);
    for (const FontKey &k : FontKeys) {
        KConfigGroup g(&config, QString::fromLatin1(k.group));
        const QString current = kdeutil::layered(u"kdeglobals"_s, QString::fromLatin1(k.group), QString::fromLatin1(k.key));
        if (pointSize(current) <= 0) {
            // A font that is not set stays the toolkit's.
            continue;
        }
        // Half points, as the Fonts module's spin boxes step.
        const double size = std::round(systemSize(k) * scale * 2.0) / 2.0;
        g.writeEntry(k.key, withPointSize(current, size), kdeutil::Notify);
    }
    const bool ok = config.sync();
    announceFonts();
    return ok;
}

bool AccessibilityConfig::setReduceMotion(bool on)
{
    KConfig config = kdeutil::user(u"kdeglobals"_s);
    KConfigGroup g(&config, u"KDE"_s);
    if (on) {
        g.writeEntry("AnimationDurationFactor", 0.0, kdeutil::Notify);
    } else {
        // Back to the system's speed.
        g.deleteEntry("AnimationDurationFactor", kdeutil::Notify);
    }
    const bool ok = config.sync();
    kdeutil::reconfigureKWin(this);
    return ok;
}

bool AccessibilityConfig::setZoom(bool on)
{
    KConfig config = kdeutil::user(u"kwinrc"_s);
    KConfigGroup(&config, u"Plugins"_s).writeEntry("zoomEnabled", on, kdeutil::Notify);
    const bool ok = config.sync();
    // As the Desktop Effects module turns an effect on or off at once.
    kdeutil::call(this, u"org.kde.KWin"_s, u"/Effects"_s, u"org.kde.kwin.Effects"_s, on ? u"loadEffect"_s : u"unloadEffect"_s, {u"zoom"_s});
    return ok;
}

bool AccessibilityConfig::setScreenReader(bool on)
{
    KConfig config = kdeutil::user(u"kaccessrc"_s);
    KConfigGroup(&config, u"ScreenReader"_s).writeEntry("Enabled", on, kdeutil::Notify);
    const bool ok = config.sync();
    // kcm_access also sets the desktop's accessibility switch, which is what
    // makes GTK and Qt apps talk to the screen reader.
    Q_EMIT run({u"gsettings"_s, u"set"_s, u"org.gnome.desktop.a11y.applications"_s, u"screen-reader-enabled"_s, on ? u"true"_s : u"false"_s});
    return ok;
}

bool AccessibilityConfig::setFlag(const QString &name, bool on)
{
    for (const Flag &f : Flags) {
        if (name != QLatin1String(f.name)) {
            continue;
        }
        KConfig config = kdeutil::user(QString::fromLatin1(f.file));
        KConfigGroup(&config, QString::fromLatin1(f.group)).writeEntry(f.key, on, kdeutil::Notify);
        const bool ok = config.sync();
        if (QLatin1String(f.file) == u"kwinrc"_s) {
            kdeutil::reconfigureKWin(this);
        }
        return ok;
    }
    return false;
}
