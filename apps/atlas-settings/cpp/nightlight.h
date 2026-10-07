#pragma once

#include <QObject>
#include <QString>

// Night Light as Plasma 6.7's Night Light page keeps it: kwinrc [NightColor]
// (Active, Mode 0 "always on" or 1 "sunrise and sunset", NightTemperature,
// DayTemperature), which KWin watches, and KWin's `org.kde.KWin.NightLight`
// on the session bus for whether it works here, and for previewing a
// temperature while a slider moves. The file is written through KConfig
// (atomically); the bus is only asked asynchronously, so the page never
// waits on it.
class NightLight : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool active READ active NOTIFY changed)
    // "sun" (sunrise and sunset) or "always".
    Q_PROPERTY(QString schedule READ schedule NOTIFY changed)
    // The temperature at night (or all day when "always"), in kelvin.
    Q_PROPERTY(int temperature READ temperature NOTIFY changed)
    // KWin said it can't change the screens' colour here (false until it
    // says so; true when KWin doesn't answer at all).
    Q_PROPERTY(bool available READ available NOTIFY changed)
    // The colour is warmed right now.
    Q_PROPERTY(bool running READ running NOTIFY changed)
    // The last failure in plain words; "" for none.
    Q_PROPERTY(QString error READ error NOTIFY changed)

public:
    static constexpr int MinTemperature = 1000;
    static constexpr int MaxTemperature = 6500;

    explicit NightLight(QObject *parent = nullptr);

    bool active() const
    {
        return m_active;
    }
    QString schedule() const
    {
        return m_always ? QStringLiteral("always") : QStringLiteral("sun");
    }
    int temperature() const
    {
        return m_temperature;
    }
    bool available() const
    {
        return m_available;
    }
    bool running() const
    {
        return m_running;
    }
    QString error() const
    {
        return m_error;
    }

    // Reads kwinrc and asks KWin again.
    Q_INVOKABLE void refresh();

    Q_INVOKABLE void setActive(bool on);
    // "sun" or "always"; anything else is ignored.
    Q_INVOKABLE void setSchedule(const QString &schedule);
    // Clamped to 1000 K (warmest) .. 6500 K (no filter), in steps of 100.
    Q_INVOKABLE void setTemperature(int kelvin);

    // KWin shows `kelvin` for a few seconds (while the slider moves), and
    // stops showing it.
    Q_INVOKABLE void preview(int kelvin);
    Q_INVOKABLE void stopPreview();

    // `kelvin` as a valid step.
    static int clampTemperature(int kelvin);

Q_SIGNALS:
    void changed();

private:
    void readFile();
    void askKWin();
    bool write(const QString &key, const QVariant &value);

    bool m_active = false;
    bool m_always = false;
    int m_temperature = 4500;
    bool m_available = true;
    bool m_running = false;
    QString m_error;
};
