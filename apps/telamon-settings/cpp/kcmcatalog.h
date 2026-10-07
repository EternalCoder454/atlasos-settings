#pragma once

#include <QObject>
#include <QSet>
#include <QVariantList>

// The KCMs installed for System Settings, for "More Settings": read from the
// plugins' own metadata (KPluginMetaData), so names and descriptions come in
// the desktop's language, as System Settings shows them.
class KcmCatalog : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // Every desktop KCM, sorted by name: {id, name, description, icon,
    // category}. Read from disk on the first call and kept: the GUI thread
    // reads every plugin's metadata once, not on each visit to More
    // Settings. KCMs installed while Settings runs show after a restart.
    Q_INVOKABLE QVariantList list();

    // Whether KCM `name` is installed where kcmshell6 looks for it. kcmshell6
    // says nothing a user sees when it isn't, so Settings checks first. A
    // KCM found is remembered; one not found is looked for again next time.
    Q_INVOKABLE bool installed(const QString &name);

private:
    QVariantList m_list;
    bool m_listed = false;
    QSet<QString> m_installed;
};
