#include "reverttimer.h"

RevertTimer::RevertTimer(QObject *parent, int intervalMs)
    : QObject(parent)
{
    m_timer.setInterval(intervalMs);
    m_timer.setTimerType(Qt::CoarseTimer);
    connect(&m_timer, &QTimer::timeout, this, &RevertTimer::step);
}

void RevertTimer::start(int seconds)
{
    m_remaining = qMax(0, seconds);
    if (m_remaining == 0) {
        m_timer.stop();
        Q_EMIT remainingChanged();
        Q_EMIT expired();
        return;
    }
    m_timer.start();
    Q_EMIT remainingChanged();
}

void RevertTimer::stop()
{
    const bool was = m_timer.isActive();
    m_timer.stop();
    m_remaining = 0;
    if (was) {
        Q_EMIT remainingChanged();
    }
}

void RevertTimer::step()
{
    if (m_remaining > 0) {
        --m_remaining;
    }
    if (m_remaining == 0) {
        m_timer.stop();
        Q_EMIT remainingChanged();
        Q_EMIT expired();
        return;
    }
    Q_EMIT remainingChanged();
}
