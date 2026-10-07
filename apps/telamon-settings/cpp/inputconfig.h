#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

// Keyboard & Mouse, written the way Plasma's own modules write them
// (docs/DESIGN.md, "Keyboard & Mouse"):
//
// - Input sources (keyboard layouts) are kxkbrc `[Layout]` as kcm_keyboard
//   saves them (`Use`, `LayoutList`, `VariantList`, `DisplayNames`), through
//   KConfig with a change notification; KWin and the keyboard daemon watch
//   the file. The list of layouts to add comes from xkeyboard-config's
//   evdev.xml, as the module reads it.
// - Key repeat and Num Lock are kcminputrc `[Keyboard]` (`RepeatDelay`,
//   `RepeatRate`, `NumLock`), which KWin watches.
// - Pointers and touchpads are KWin's own input devices
//   (org.kde.KWin.InputDevice on /org/kde/KWin/InputDevice/<name>): the
//   settings are properties KWin applies at once and keeps in kcminputrc
//   itself, which is all kcm_mouse and kcm_touchpad do too. Nothing here
//   waits for KWin: the devices are read and changed with asynchronous
//   calls.
class InputConfig : public QObject
{
    Q_OBJECT
    // The pointers found: {known, pointers, touchpads, speed (-1..1),
    // naturalScroll, tap, leftHanded, canSpeed, canNatural, canTap,
    // canLeftHanded}. `known` is false while KWin hasn't answered.
    Q_PROPERTY(QVariantMap devices READ devices NOTIFY devicesChanged)

public:
    explicit InputConfig(QObject *parent = nullptr);

    // The file with the layouts (xkeyboard-config's evdev.xml); only set by
    // tests.
    void setRulesPath(const QString &path)
    {
        m_rulesPath = path;
    }

    // The layouts in use: [{id ("us" or "us(dvorak)"), layout, variant,
    // name}].
    Q_INVOKABLE QVariantList layouts() const;
    // The layouts and variants that can be added, as [{id, layout, variant,
    // name, keys}]; read once.
    Q_INVOKABLE QVariantList availableLayouts();
    // The whole list at once, as ids: at least one, at most four (what XKB
    // allows). False when it isn't a list that can be written.
    Q_INVOKABLE bool setLayouts(const QStringList &ids);

    // {delay (ms), rate (per second), numLock: "on"|"off"|"keep"}
    Q_INVOKABLE QVariantMap keyboard() const;
    Q_INVOKABLE bool setRepeat(int delay, double rate);
    Q_INVOKABLE bool setNumLock(const QString &state);

    // Reads KWin's pointers and touchpads again.
    Q_INVOKABLE void refresh();
    // Pointer speed, -1 (slowest) to 1, for every pointer.
    Q_INVOKABLE void setSpeed(double speed);
    Q_INVOKABLE void setNaturalScroll(bool on);
    Q_INVOKABLE void setTapToClick(bool on);
    // Left handed: the buttons swap.
    Q_INVOKABLE void setLeftHanded(bool on);

    QVariantMap devices() const
    {
        return m_devices;
    }

    // Whether `id` is a layout name or "layout(variant)" that can be written.
    Q_INVOKABLE static bool validLayoutId(const QString &id);

Q_SIGNALS:
    void devicesChanged();
    void failed(const QString &message);

private:
    struct Device
    {
        QString sysName;
        bool touchpad = false;
        QVariantMap props;
    };

    void readDevice(const QString &sysName);
    void setProperty(const QString &name, const QVariant &value, bool touchpadOnly = false);
    void publish();

    QString m_rulesPath;
    QVariantList m_available;
    bool m_availableRead = false;
    QList<Device> m_list;
    QVariantMap m_devices{{QStringLiteral("known"), false}};
    int m_generation = 0;
    int m_waiting = 0;
};
