#pragma once

#include <QObject>

// The Plasma colour scheme in use, as kdeglobals names it ([General]
// ColorScheme, the user's file over the system's default). Read through
// KConfig; changed by plasma-apply-colorscheme, which Home starts through
// the Launcher (Plasma then writes the file and repaints), never by hand.
// Local files only: nothing here waits on D-Bus.
class ColorSchemeConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // The scheme's name ("AtlasOSLight"), or "" when none is set or the
    // name isn't one this passes to a program.
    Q_INVOKABLE QString current() const;

    // Whether `name` can be passed to plasma-apply-colorscheme: letters,
    // digits, space, dot, underscore and hyphen, not starting with a space
    // or hyphen.
    Q_INVOKABLE static bool validName(const QString &name);
};
