#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantMap>

// Accessibility as Plasma keeps it (docs/DESIGN.md, "Accessibility"),
// written through KConfig with a change notification, as the modules do:
//
// - Text size scales the six fonts of kdeglobals (`[General] font`,
//   `fixed`, `smallestReadableFont`, `toolBarFont`, `menuFont` and `[WM]
//   activeFont`) the way "Adjust All Fonts" in the Fonts module does, from
//   the sizes the system sets.
// - Reduce Motion is `[KDE] AnimationDurationFactor` = 0, Plasma's
//   animation speed turned to instant, which KWin, Plasma, the
//   desktop portal and Atlas.Ui read.
// - The screen reader is `kaccessrc [ScreenReader] Enabled` (kaccess starts
//   Orca) and the desktop's accessibility switch in GSettings, as
//   kcm_access sets it.
// - Zoom is kwinrc `[Plugins] zoomEnabled`, and the KWin effect.
// - Sticky keys and the rest are kaccessrc `[Keyboard]` and `[Mouse]`, and
//   the pointer shake kwinrc `[Plugins] shakecursorEnabled`.
// High contrast is a colour scheme, which Appearance applies.
class AccessibilityConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // {textScale, reduceMotion, zoom, screenReader, orcaInstalled, and the
    // flags of setFlag()}
    Q_INVOKABLE QVariantMap read() const;

    // The fonts, 0.8 to 2 times the system's sizes (in steps of a twentieth).
    Q_INVOKABLE bool setTextScale(double scale);
    Q_INVOKABLE bool setReduceMotion(bool on);
    Q_INVOKABLE bool setZoom(bool on);
    // Turns Orca on or off; the page starts `gsettings` through run().
    Q_INVOKABLE bool setScreenReader(bool on);
    // One of "sticky", "stickyLock", "stickyOff", "stickyBeep", "toggleBeep",
    // "mouseKeys", "shake".
    Q_INVOKABLE bool setFlag(const QString &name, bool on);

Q_SIGNALS:
    // A program for the page to start through the Launcher.
    void run(const QStringList &argv);
};
