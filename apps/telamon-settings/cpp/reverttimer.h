#pragma once

#include <QObject>
#include <QTimer>

// The countdown behind "Keep these settings?": counts `seconds` down to 0,
// one step per `interval`, and then says expired(). The page shows
// remaining() and reverts when it expires. The interval is a parameter only
// so that the test doesn't wait whole seconds.
class RevertTimer : public QObject
{
    Q_OBJECT
    Q_PROPERTY(int remaining READ remaining NOTIFY remainingChanged)
    Q_PROPERTY(bool running READ running NOTIFY remainingChanged)

public:
    explicit RevertTimer(QObject *parent = nullptr, int intervalMs = 1000);

    int remaining() const
    {
        return m_remaining;
    }
    bool running() const
    {
        return m_timer.isActive();
    }

    // (Re)starts counting from `seconds`.
    void start(int seconds);
    void setInterval(int ms)
    {
        m_timer.setInterval(ms);
    }
    // Stops without expiring.
    void stop();

Q_SIGNALS:
    void remainingChanged();
    void expired();

private:
    void step();

    QTimer m_timer;
    int m_remaining = 0;
};
