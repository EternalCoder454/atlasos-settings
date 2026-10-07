#pragma once

#include <QElapsedTimer>
#include <QObject>
#include <QStringList>

// Starts another program for QML: kcmshell6 for a KCM, Telamon Updater for
// Updates. Through KIO, so the new window gets the activation token and
// comes to the front on Wayland.
class Launcher : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // Starts argv[0], a program in /usr/bin found by name (never PATH), with
    // the rest as its arguments and no shell between. False at once for an
    // empty list; a program that is missing, or can't be started, is
    // reported through failed(). The same command again within 2 s (a double
    // click, a link sent twice) is not started again, and no more than
    // MaxStarts programs start in any 10 s (links from other programs can
    // ask for them).
    Q_INVOKABLE bool run(const QStringList &argv);

Q_SIGNALS:
    void failed(const QString &program, const QString &message);

private:
    static constexpr int MaxStarts = 5;
    QStringList m_last;
    QElapsedTimer m_lastStarted;
    QList<qint64> m_starts; // m_clock times of the recent starts
    QElapsedTimer m_clock;
};
