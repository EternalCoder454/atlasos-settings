#include "pagebackends.h"

#include "colorscheme.h"
#include "localeconfig.h"

#include <QDebug>
#include <QQmlEngine>

// Defined in src/lib.rs.
extern "C" void *atlas_page_backend_new(const char *kind);

QObject *PageBackends::create(const QString &kind, QObject *parent)
{
    QObject *object = nullptr;
    if (kind == QLatin1String("locale-config")) {
        object = new LocaleConfig;
    } else if (kind == QLatin1String("color-scheme")) {
        object = new ColorSchemeConfig;
    } else {
        object = static_cast<QObject *>(atlas_page_backend_new(kind.toUtf8().constData()));
    }
    if (!object) {
        qWarning() << "no page backend" << kind;
        return nullptr;
    }
    // The page owns it; QML's garbage collector never does.
    object->setParent(parent);
    QQmlEngine::setObjectOwnership(object, QQmlEngine::CppOwnership);
    return object;
}
