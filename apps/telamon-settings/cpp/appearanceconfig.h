#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

#include <KConfigWatcher>

// Appearance: Light or Dark, the accent colour, the wallpaper, the dock and
// the other look settings, written the way Plasma's own modules write them
// (docs/DESIGN.md, "Appearance"):
//
// - The colour scheme is applied by plasma-apply-colorscheme (the same tool
//   Plasma's Colours page uses), which writes `[General] ColorScheme` and the
//   scheme's colours into kdeglobals and announces them, so GTK, Qt and
//   kvantum-sync (which watches those keys) follow. Settings never starts
//   the tool itself: it asks the page to, with run(), and the page hands the
//   program to the Launcher (argv, no shell).
// - The icon theme and the window decoration that go with Light or Dark are
//   written through KConfig with a change notification.
// - The wallpaper goes through plasma-apply-wallpaperimage the same way.
// - The dock and the top bar are Plasma panels: they are read and changed
//   with Plasma's own panel scripting (org.kde.PlasmaShell.evaluateScript),
//   never by editing plasma-org.kde.plasma.desktop-appletsrc.
// - Virtual desktops go through KWin's D-Bus interface, as the KCM does.
// - Hot corners are kwinrc `[ElectricBorders]` and `[Effect-overview]`,
//   as KWin's Screen Edges module writes them.
class AppearanceConfig : public QObject
{
    Q_OBJECT
    // What the panels say: {known, dock: {found, location, autohide, height},
    // topBar: {found, hides}}.
    Q_PROPERTY(QVariantMap shell READ shell NOTIFY shellChanged)
    // KWin's virtual desktops: how many, and their IDs (empty until known).
    Q_PROPERTY(int desktopCount READ desktopCount NOTIFY desktopsChanged)

public:
    explicit AppearanceConfig(QObject *parent = nullptr);

    // {scheme, dark, highContrast, accent ("#rrggbb" or "" for the scheme's
    // own), accentFromWallpaper, accentIsDefault, lookAndFeel, wallpaper:
    // {id, name, preview, picture, pictureDark}}. `picture` is the image of
    // the main screen's wallpaper (file URL; Telamon OS's own when none is
    // set), `pictureDark` the one for a dark colour scheme ("" when it has no
    // other).
    Q_INVOKABLE QVariantMap read() const;

    // Light (false) or Dark (true): the scheme, and the icons and window
    // decoration of Telamon OS's own themes that go with it.
    Q_INVOKABLE void setDark(bool dark);
    // The high contrast variant of the colour scheme, or the usual one.
    Q_INVOKABLE void setHighContrast(bool on);
    // "#rrggbb", or "" for the scheme's own accent.
    Q_INVOKABLE void setAccent(const QString &hex);
    Q_INVOKABLE void setAccentFromWallpaper();

    // [{id, name, preview (file URL), kind: "package"|"image"}]: the
    // installed wallpapers and the pictures in ~/Pictures.
    Q_INVOKABLE QVariantList wallpapers() const;
    Q_INVOKABLE void setWallpaper(const QString &id);
    // {picture, pictureDark} of a wallpaper wallpapers() lists, as read()'s
    // wallpaper has them, so the page can show a choice before Plasma has
    // saved it.
    Q_INVOKABLE QVariantMap wallpaperPictures(const QString &id) const;

    // [{id, name, description}]: the Global Themes installed.
    Q_INVOKABLE QVariantList lookAndFeels() const;
    Q_INVOKABLE void setLookAndFeel(const QString &id);

    // Reads the panels and the virtual desktops again (asynchronously).
    Q_INVOKABLE void refreshShell();
    Q_INVOKABLE void setDockAutoHide(bool hide);
    // Thickness in pixels, between 40 and 96.
    Q_INVOKABLE void setDockSize(int pixels);
    // "bottom", "left" or "right".
    Q_INVOKABLE void setDockPosition(const QString &location);
    // Whether the top bar hides under windows (else it stays over them).
    Q_INVOKABLE void setTopBarHides(bool hides);

    // KWin's virtual desktops: 1 to 20.
    Q_INVOKABLE void setDesktopCount(int count);

    // {topLeft, topRight, bottomLeft, bottomRight}: each "none", "overview",
    // "desktop" (Show Desktop) or "lock".
    Q_INVOKABLE QVariantMap hotCorners() const;
    Q_INVOKABLE bool setHotCorner(const QString &corner, const QString &action);

    // Whether `hex` is "#rrggbb".
    Q_INVOKABLE static bool validColor(const QString &hex);

    QVariantMap shell() const
    {
        return m_shell;
    }
    int desktopCount() const
    {
        return m_desktopCount;
    }

Q_SIGNALS:
    // kdeglobals changed (or a change was asked for): read() again.
    void changed();
    void shellChanged();
    void desktopsChanged();
    // A program for the page to start through the Launcher.
    void run(const QStringList &argv);
    // Something couldn't be changed, in plain words.
    void failed(const QString &message);

private:
    QString currentScheme() const;
    QString accentHex() const;
    void applyScheme(const QString &scheme, const QString &accent);
    bool ensureHighContrastScheme(bool dark);
    void switchThemeParts(bool dark);
    void runShellScript(const QString &script);
    void scheduleRead();

    KConfigWatcher::Ptr m_watcher;
    QVariantMap m_shell;
    int m_desktopCount = 0;
    QStringList m_desktopIds;
};
