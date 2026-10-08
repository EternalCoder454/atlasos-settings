#include "launcher.h"

#include <KIO/ApplicationLauncherJob>
#include <KIO/CommandLauncherJob>
#include <KService>
#include <KJobWindows>

#include <QDebug>
#include <QGuiApplication>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QWindow>

bool Launcher::allowed(const QStringList &key, const QString &program)
{
    // kcmshell6 takes a moment to show its window; clicks meanwhile would
    // each open another.
    if (key == m_last && m_lastStarted.isValid() && m_lastStarted.elapsed() < 2000) {
        return false;
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
        return false;
    }
    return true;
}

void Launcher::recordStart(const QStringList &key)
{
    m_last = key;
    m_lastStarted.start();
    m_starts.append(m_clock.elapsed());
}

bool Launcher::run(const QStringList &argv)
{
    if (argv.isEmpty() || argv.first().isEmpty()) {
        return false;
    }
    const QString program = argv.first();
    if (!allowed(argv, program)) {
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
    recordStart(argv);
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

QString Launcher::runApplication(const QStringList &desktopNames)
{
    static const QRegularExpression plainId(QStringLiteral("\\A[A-Za-z0-9][A-Za-z0-9._-]{0,127}\\z"));
    for (const QString &name : desktopNames) {
        if (!plainId.match(name).hasMatch()) {
            continue;
        }
        const KService::Ptr service = KService::serviceByDesktopName(name);
        if (!service || !service->isValid() || !service->isApplication()) {
            continue;
        }
        const QStringList key{QStringLiteral("desktop:") + name};
        if (!allowed(key, name)) {
            // A repeat within 2 s: the first start is still on its way. Over
            // the limit: failed() was emitted, and nothing started.
            return m_last == key ? name : QString();
        }
        recordStart(key);
        auto *job = new KIO::ApplicationLauncherJob(service, this);
        if (QWindow *window = QGuiApplication::focusWindow()) {
            KJobWindows::setWindow(job, window);
        }
        connect(job, &KJob::result, this, [this, name](KJob *job) {
            if (job->error()) {
                qWarning().noquote() << "starting" << name << "failed:" << job->errorString();
                m_last.clear();
                Q_EMIT failed(name, job->errorString());
            }
        });
        job->start();
        return name;
    }
    return QString();
}
