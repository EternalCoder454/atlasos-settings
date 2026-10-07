#include "launcher.h"

#include <KIO/CommandLauncherJob>
#include <KJobWindows>

#include <QDebug>
#include <QGuiApplication>
#include <QStandardPaths>
#include <QWindow>

bool Launcher::run(const QStringList &argv)
{
    if (argv.isEmpty() || argv.first().isEmpty()) {
        return false;
    }
    const QString program = argv.first();
    // kcmshell6 takes a moment to show its window; clicks meanwhile would
    // each open another.
    if (argv == m_last && m_lastStarted.isValid() && m_lastStarted.elapsed() < 2000) {
        return true;
    }
    if (!m_clock.isValid()) {
        m_clock.start();
    }
    const qint64 now = m_clock.elapsed();
    m_starts.removeIf([now](qint64 t) {
        return now - t > 10000;
    });
    if (m_starts.size() >= MaxStarts) {
        qWarning().noquote() << "not starting" << program << ": too many windows opened in the last 10 s";
        Q_EMIT failed(program, tr("Too many windows were opened at once. Try again in a moment."));
        return true;
    }
    // Only the system's own programs, by name: a program of the same name
    // earlier in PATH (~/.local/bin, say) is not run in their place.
    const QString path = program.contains(QLatin1Char('/')) ? QString() : QStandardPaths::findExecutable(program, {QStringLiteral("/usr/bin")});
    if (path.isEmpty()) {
        qWarning().noquote() << "not starting" << program << ": not installed in /usr/bin";
        Q_EMIT failed(program, tr("It isn't installed."));
        return true;
    }
    m_last = argv;
    m_lastStarted.start();
    m_starts.append(now);
    // An executable and an argument list: KIO starts it without a shell.
    auto *job = new KIO::CommandLauncherJob(path, argv.mid(1), this);
    // The window asking, for the activation token.
    if (QWindow *window = QGuiApplication::focusWindow()) {
        KJobWindows::setWindow(job, window);
    }
    connect(job, &KJob::result, this, [this, program](KJob *job) {
        if (job->error()) {
            qWarning().noquote() << "starting" << program << "failed:" << job->errorString();
            // Trying again right away is allowed after a failure.
            m_last.clear();
            Q_EMIT failed(program, job->errorString());
        }
    });
    job->start();
    return true;
}
