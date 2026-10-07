#include "pagebackends.h"

#include "accessibilityconfig.h"
#include "appearanceconfig.h"
#include "colorscheme.h"
#include "inputconfig.h"
#include "localeconfig.h"
#include "nightlight.h"
#include "notificationsconfig.h"
#include "screenconfig.h"
#include "soundmixer.h"

#include <QDebug>
#include <QQmlEngine>

// Defined in src/lib.rs.
extern "C" void *atlas_page_backend_new(const char *kind);

QObject *PageBackends::create(const QString &kind, QObject *parent)
{
    QObject *object = nullptr;
    if (kind == QLatin1String("locale-config")) {
        object = new LocaleConfig;
    } else if (kind == QLatin1String("displays")) {
        object = new ScreenConfig;
    } else if (kind == QLatin1String("night-light")) {
        object = new NightLight;
    } else if (kind == QLatin1String("sound")) {
        object = new SoundMixer;
    } else if (kind == QLatin1String("color-scheme")) {
        object = new ColorSchemeConfig;
    } else if (kind == QLatin1String("appearance")) {
        object = new AppearanceConfig;
    } else if (kind == QLatin1String("input")) {
        object = new InputConfig;
    } else if (kind == QLatin1String("notifications")) {
        object = new NotificationsConfig;
    } else if (kind == QLatin1String("accessibility")) {
        object = new AccessibilityConfig;
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
