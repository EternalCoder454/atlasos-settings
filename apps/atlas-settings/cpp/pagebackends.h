#pragma once

#include <QObject>

// Makes a page's backend when the page is shown, parented to the page so it
// goes when the page does (docs/DESIGN.md, "Threads"). The Rust ones come
// from src/lib.rs (atlas_page_backend_new); `locale-config` is the KConfig
// side of Time & Language (localeconfig.h); `color-scheme` is Home's Light or
// Dark switch (colorscheme.h); `displays` and `night-light` are Displays'
// (screenconfig.h, nightlight.h), `sound` is Sound's (soundmixer.h);
// `appearance`, `input`, `notifications` and `accessibility` are the KConfig
// and KWin side of those pages (appearanceconfig.h and the like).
class PageBackends : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // A new backend of `kind` owned by `parent`, or null for an unknown
    // kind.
    Q_INVOKABLE QObject *create(const QString &kind, QObject *parent);
};
