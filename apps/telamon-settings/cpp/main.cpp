// Starts Qt, makes Settings single-instance and loads the window. A second
// launch (`telamon-settings displays`, the systemsettings shim, KRunner) hands
// its arguments to this one and exits; they are read in Rust
// (src/backend.rs), never here.
#include "actionparam.h"
#include "kcmcatalog.h"
#include "launcher.h"
#include "legacyservice.h"
#include "pagebackends.h"

#include <telamon/app.h>

#include <KDBusService>
#include <KWindowSystem>

#include <QApplication>
#include <QCommandLineParser>
#include <QDebug>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QSGRendererInterface>

#include <memory>

// Defined in src/lib.rs and src/updates_page.rs.
extern "C" void *telamon_backend_new();
extern "C" void telamon_updates_flush();

static void activate(QObject *backend, const QStringList &arguments)
{
    if (!QMetaObject::invokeMethod(backend, "activate", Q_ARG(QStringList, arguments.mid(1)))) {
        qWarning() << "Backend.activate could not be called: launch arguments were dropped";
    }
}

// ActivateAction("open", [link]) and ("open-app", [desktop file ID]), the
// Launcher's deep links; read in Rust like any launch argument.
static void activateAction(QObject *backend, const QString &action, const QString &parameter)
{
    if (!QMetaObject::invokeMethod(backend, "activateAction", Q_ARG(QString, action), Q_ARG(QString, parameter))) {
        qWarning() << "Backend.activateAction could not be called: the action was dropped";
    }
}

static void raise(QQmlApplicationEngine *engine)
{
    auto *window = qobject_cast<QQuickWindow *>(engine->rootObjects().value(0));
    if (!window) {
        return;
    }
    // Out of minimized, back to maximized or normal as it was.
    window->setWindowStates(window->windowStates() & ~Qt::WindowMinimized);
    window->show();
    // The launcher's activation token: without it Wayland keeps the window
    // down.
    KWindowSystem::updateStartupId(window);
    KWindowSystem::activateWindow(window);
}

int main(int argc, char *argv[])
{
    telamon_app_init();
    // Drawn on the CPU like the other Atlas apps unless QT_QUICK_BACKEND says
    // otherwise.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_BACKEND")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    }

    QApplication app(argc, argv);
    telamon_app_ready();

    // Here only for --help and --version. Everything else, unknown options
    // too, goes on to the running window, which reads it in Rust
    // (settings-registry's launch.rs) and says what it refused: a second
    // launch exiting here with an error on stderr would show the user nothing.
    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("The settings of Telamon OS."));
    parser.addHelpOption();
    parser.addVersionOption();
    parser.addOption({QStringLiteral("page"), QStringLiteral("Open the page <page>."), QStringLiteral("page")});
    parser.addOption({QStringLiteral("kcm"), QStringLiteral("Open what the KCM <name> opens in System Settings."), QStringLiteral("name")});
    parser.addOption({QStringLiteral("args"), QStringLiteral("Arguments for the KCM given with --kcm."), QStringLiteral("text")});
    parser.addOption({QStringLiteral("search"), QStringLiteral("Search the settings for <text>."), QStringLiteral("text")});
    parser.addPositionalArgument(QStringLiteral("page"), QStringLiteral("A page to open, such as displays or sound."), QStringLiteral("[page [setting]]"));
    // parse(), not process(): process() exits on an unknown option.
    parser.parse(QCoreApplication::arguments());
    if (parser.isSet(QStringLiteral("help")) || parser.isSet(QStringLiteral("help-all"))) {
        parser.showHelp();
    }
    if (parser.isSet(QStringLiteral("version"))) {
        parser.showVersion();
    }

    // One instance per session. A second launch's arguments come here through
    // activateRequested; without a session bus each launch runs on its own.
    KDBusService service(KDBusService::Unique | KDBusService::NoExitOnFailure);

    // These outlive the engine: the window's bindings read them until the
    // engine is gone.
    std::unique_ptr<QObject> backend(static_cast<QObject *>(telamon_backend_new()));
    Launcher launcher;
    KcmCatalog kcmCatalog;
    PageBackends pageBackends;
    auto engine = std::make_unique<QQmlApplicationEngine>();
    QObject::connect(engine.get(), &QQmlApplicationEngine::objectCreationFailed, &app, [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine->setInitialProperties({
        {QStringLiteral("backend"), QVariant::fromValue(backend.get())},
        {QStringLiteral("launcher"), QVariant::fromValue(&launcher)},
        {QStringLiteral("kcmCatalog"), QVariant::fromValue(&kcmCatalog)},
        {QStringLiteral("pageBackends"), QVariant::fromValue(&pageBackends)},
    });
    engine->loadFromModule("net.eterneon.telamon.settings", "Main");
    if (engine->rootObjects().isEmpty()) {
        return 1;
    }

    QObject::connect(&service, &KDBusService::activateRequested, backend.get(), [e = engine.get(), b = backend.get()](const QStringList &arguments, const QString &) {
        raise(e);
        activate(b, arguments);
    });
    // org.freedesktop.Application.ActivateAction, as the Telamon OS Launcher
    // calls it for a search result (docs/DESIGN.md, "Entry points"). KDBusService
    // has set the caller's activation token by now; raise() uses it.
    QObject::connect(&service, &KDBusService::activateActionRequested, backend.get(), [e = engine.get(), b = backend.get()](const QString &action, const QVariant &parameter) {
        raise(e);
        activateAction(b, action, actionParameter(parameter));
    });
    // The name this program had (net.eterneon.atlas.settings) still answers
    // org.freedesktop.Application for the Launcher and other programs that
    // have not moved to the new one; dropped in the release after this one.
    // The calls come to the same place as the new name's: a plain activation
    // raises the window, and ActivateAction goes through the same reader of
    // actions (launch.rs, action_args), so a caller of the old name can ask
    // for nothing the new name would refuse.
    LegacyService legacy;
    QObject::connect(&legacy, &LegacyService::activateRequested, backend.get(), [e = engine.get(), b = backend.get()](const QStringList &arguments, const QString &token) {
        if (!token.isEmpty()) {
            KWindowSystem::setCurrentXdgActivationToken(token);
        }
        raise(e);
        // The arguments have no program name, which activate() skips, so it
        // is put in front.
        activate(b, QStringList{QStringLiteral("telamon-settings")} + arguments);
    });
    QObject::connect(&legacy, &LegacyService::activateActionRequested, backend.get(), [e = engine.get(), b = backend.get()](const QString &action, const QString &parameter, const QString &token) {
        if (!token.isEmpty()) {
            KWindowSystem::setCurrentXdgActivationToken(token);
        }
        raise(e);
        activateAction(b, action, parameter);
    });
    legacy.registerOnSessionBus();
    activate(backend.get(), QCoreApplication::arguments());

    // Closing the last window quits, unless an update is being staged or apps
    // or firmware are being installed (Main.qml's keepRunning): then the
    // window only hides, and Main.qml quits when that ends. The screen glow
    // and the system helper's work don't depend on this window; apps and
    // firmware are installed from this process.
    app.setQuitOnLastWindowClosed(false);
    auto *window = qobject_cast<QQuickWindow *>(engine->rootObjects().value(0));
    auto quitIfIdle = [window, &app] {
        if (window && !window->isVisible() && !window->property("keepRunning").toBool()) {
            app.quit();
        }
    };
    QObject::connect(&app, &QGuiApplication::lastWindowClosed, &app, quitIfIdle);

    const int code = app.exec();
    engine.reset();
    // Notifications still being sent, and the tray told of the last change.
    telamon_updates_flush();
    return code;
}
