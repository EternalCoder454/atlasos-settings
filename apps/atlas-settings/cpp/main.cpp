// Starts Qt, makes Settings single-instance and loads the window. A second
// launch (`atlas-settings displays`, the systemsettings shim, KRunner) hands
// its arguments to this one and exits; they are read in Rust
// (src/backend.rs), never here.
#include "kcmcatalog.h"
#include "launcher.h"
#include "pagebackends.h"

#include <atlas/app.h>

#include <KDBusService>
#include <KWindowSystem>

#include <QApplication>
#include <QCommandLineParser>
#include <QDebug>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QSGRendererInterface>

#include <memory>

// Defined in src/lib.rs.
extern "C" void *atlas_backend_new();

static void activate(QObject *backend, const QStringList &arguments)
{
    if (!QMetaObject::invokeMethod(backend, "activate", Q_ARG(QStringList, arguments.mid(1)))) {
        qWarning() << "Backend.activate could not be called: launch arguments were dropped";
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
    atlas_app_init();
    // Drawn on the CPU like the other Atlas apps unless QT_QUICK_BACKEND says
    // otherwise.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_BACKEND")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    }

    QApplication app(argc, argv);
    atlas_app_ready();

    // Here only for --help and --version. Everything else, unknown options
    // too, goes on to the running window, which reads it in Rust
    // (settings-registry's launch.rs) and says what it refused: a second
    // launch exiting here with an error on stderr would show the user nothing.
    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("The settings of AtlasOS."));
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
    std::unique_ptr<QObject> backend(static_cast<QObject *>(atlas_backend_new()));
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
    engine->loadFromModule("net.eterneon.atlas.settings", "Main");
    if (engine->rootObjects().isEmpty()) {
        return 1;
    }

    QObject::connect(&service, &KDBusService::activateRequested, backend.get(), [e = engine.get(), b = backend.get()](const QStringList &arguments, const QString &) {
        raise(e);
        activate(b, arguments);
    });
    activate(backend.get(), QCoreApplication::arguments());

    const int code = app.exec();
    engine.reset();
    return code;
}
