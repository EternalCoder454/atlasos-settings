#pragma once

#include <QObject>
#include <QVariantList>
#include <QVariantMap>

// The default apps as kcm_componentchooser keeps them: the user's
// ~/.config/mimeapps.list ([Default Applications], [Added Associations]),
// read through KApplicationTrader/KService (which also read the system's
// lists), and for the terminal kdeglobals [General] TerminalApplication and
// TerminalService. Only apps KService knows can be chosen. Local files
// only.
//
// A kind stands for the file types a person thinks of together: the web
// browser (http, https and HTML), email (mailto), the file manager
// (folders), the terminal, music, video and images.
class DefaultApps : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // The kinds, in the order a page lists them: [{id, title}].
    Q_INVOKABLE static QVariantList kinds();

    // The default app of `kind`: {id, name, icon}; id is "" when none.
    Q_INVOKABLE QVariantMap current(const QString &kind) const;

    // The apps that can be the default of `kind`: [{id, name, icon,
    // comment}], by name.
    Q_INVOKABLE QVariantList choices(const QString &kind) const;

    // Makes `id` (a desktop file name from `choices`) the default of
    // `kind`. False for an unknown kind or an app that isn't one of its
    // choices, or when a file can't be written.
    Q_INVOKABLE bool setDefault(const QString &kind, const QString &id);

    // The file types a kind covers.
    Q_INVOKABLE static QStringList mimeTypes(const QString &kind);
};
