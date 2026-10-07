// The text parameter of an org.freedesktop.Application.ActivateAction call.
// The parameter list travels as D-Bus variants, and KDBusService hands one
// on as a QVariant that may still be wrapped (QDBusVariant, or a list of
// them). Anything but text is an empty string: the Rust side
// (settings-registry's launch.rs, action_args) decides what it means.
#pragma once

#include <QDBusVariant>
#include <QMetaType>
#include <QString>
#include <QVariant>
#include <QVariantList>

inline QString actionParameter(QVariant value)
{
    // Wrappers nest a level or two at most; the bound keeps a hostile value
    // from being followed for long.
    for (int depth = 0; depth < 4; ++depth) {
        if (value.canConvert<QDBusVariant>()) {
            value = qvariant_cast<QDBusVariant>(value).variant();
        } else if (value.metaType().id() == QMetaType::QVariantList) {
            const QVariantList list = value.toList();
            if (list.isEmpty()) {
                return {};
            }
            value = list.first();
        } else {
            break;
        }
    }
    return value.metaType().id() == QMetaType::QString ? value.toString() : QString();
}
