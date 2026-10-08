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

    // Starts the first of `desktopNames` that is installed (desktop file IDs
    // without ".desktop", such as "org.kde.plasma.camera"), from its desktop
    // file through KIO's ApplicationLauncherJob: the file's own Exec line, no
    // shell, Flatpaks and all. Returns the name that was started, or "" when
    // none is installed (nothing is started and failed() is not emitted; the
    // caller says so). A name that is not a plain desktop file ID is skipped.
    // Counts against the same limits as run().
    Q_INVOKABLE QString runApplication(const QStringList &desktopNames);

Q_SIGNALS:
    void failed(const QString &program, const QString &message);

private:
    // False when `key` is a repeat within 2 s or more than MaxStarts programs
    // started in 10 s (failed() is emitted for the second).
    bool allowed(const QStringList &key, const QString &program);
    // Remembers that `key` was started now.
    void recordStart(const QStringList &key);

    static constexpr int MaxStarts = 5;
    QStringList m_last;
    QElapsedTimer m_lastStarted;
    QList<qint64> m_starts; // m_clock times of the recent starts
    QElapsedTimer m_clock;
};
