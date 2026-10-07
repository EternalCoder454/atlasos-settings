#pragma once

#include "reverttimer.h"

#include <QObject>
#include <QPoint>
#include <QString>
#include <QTimer>
#include <QVariantList>

#include <KScreen/Config>

#include <functional>

// The screens, through libkscreen (Plasma's own library: KWin's output
// management on Wayland), as the Displays page needs them. Lists the connected
// screens with their modes, scale and place, and changes one thing at a time:
// each change is applied at once, and a change that can leave the screen
// unusable (resolution, scale, rate, rotation, HDR, arrangement) starts the
// "Keep these settings?" countdown, which puts the old settings back when it
// runs out, as Plasma's Display Configuration does. Nothing blocks: libkscreen's
// operations run in the event loop.
class ScreenConfig : public QObject
{
    Q_OBJECT
    // The first read answered (or failed).
    Q_PROPERTY(bool loaded READ loaded NOTIFY changed)
    // A change is being applied.
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
    // The last failure in plain words; "" for none.
    Q_PROPERTY(QString error READ error NOTIFY changed)
    // Said once after settings were put back; "" for none.
    Q_PROPERTY(QString notice READ notice NOTIFY changed)
    // The connected screens, in the order of their places (left to right,
    // top to bottom); see outputs() in screenconfig.cpp for the keys.
    Q_PROPERTY(QVariantList outputs READ outputs NOTIFY changed)
    Q_PROPERTY(bool perOutputScaling READ perOutputScaling NOTIFY changed)
    Q_PROPERTY(bool primarySupported READ primarySupported NOTIFY changed)
    // "Keep these settings?" is up, with `revertSeconds` left.
    Q_PROPERTY(bool confirming READ confirming NOTIFY confirmingChanged)
    Q_PROPERTY(int revertSeconds READ revertSeconds NOTIFY confirmingChanged)

public:
    // `revertAfter` is the length of the countdown in seconds.
    explicit ScreenConfig(QObject *parent = nullptr);
    ~ScreenConfig() override;

    bool loaded() const
    {
        return m_loaded;
    }
    bool busy() const
    {
        return m_busy;
    }
    QString error() const
    {
        return m_error;
    }
    QString notice() const
    {
        return m_notice;
    }
    QVariantList outputs() const
    {
        return m_outputs;
    }
    bool perOutputScaling() const;
    bool primarySupported() const;
    bool confirming() const
    {
        return m_confirming;
    }
    int revertSeconds() const
    {
        return m_countdown.remaining();
    }

    // Reads the screens again.
    Q_INVOKABLE void refresh();

    // Screen `id` (the "id" of an entry of outputs): its resolution, scale,
    // refresh rate, rotation (0, 90, 180 or 270), HDR, the primary one.
    Q_INVOKABLE void setResolution(int id, int width, int height);
    Q_INVOKABLE void setScale(int id, double scale);
    Q_INVOKABLE void setRefreshRate(int id, double hertz);
    Q_INVOKABLE void setRotation(int id, int degrees);
    Q_INVOKABLE void setHdr(int id, bool on);
    Q_INVOKABLE void setPrimary(int id);

    // Where screen `id` lands when dragged to `x`, `y` (logical pixels):
    // touching another screen, never over one. For the preview while dragging.
    Q_INVOKABLE QPoint snapped(int id, int x, int y) const;
    // Moves screen `id` to where snapped() puts `x`, `y`.
    Q_INVOKABLE void moveOutput(int id, int x, int y);

    // The answers to "Keep these settings?".
    Q_INVOKABLE void keepChanges();
    Q_INVOKABLE void revertChanges();

    // Seconds the countdown starts from.
    static constexpr int RevertSeconds = 15;

    // Test hook: how fast the countdown steps (ms) and how long it lasts.
    void setCountdown(int seconds, int intervalMs);

Q_SIGNALS:
    void changed();
    void confirmingChanged();

private:
    using Edit = std::function<bool(const KScreen::OutputPtr &, const KScreen::ConfigPtr &)>;

    void publish();
    void change(int id, bool risky, const Edit &edit);
    void apply(const KScreen::ConfigPtr &next, bool risky);
    void startConfirm();
    void finishConfirm();
    void setError(const QString &text);
    bool hdrCapable(const KScreen::OutputPtr &output) const;
    QList<QRect> placesOf(const KScreen::ConfigPtr &config, int except = -1) const;

    KScreen::ConfigPtr m_config; // as the compositor has it
    KScreen::ConfigPtr m_previous; // before the change being confirmed
    QVariantList m_outputs;
    QString m_error;
    QString m_notice;
    bool m_loaded = false;
    bool m_busy = false;
    bool m_confirming = false;
    bool m_reading = false;
    int m_countdownSeconds = RevertSeconds;
    RevertTimer m_countdown;
    QTimer m_reload; // external changes, coalesced
    bool m_fake = false;
};
