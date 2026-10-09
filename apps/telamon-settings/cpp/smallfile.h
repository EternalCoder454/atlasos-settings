#pragma once

#include <QByteArray>
#include <QFile>
#include <QFileInfo>
#include <QString>

// Reading files other programs may have made (a wallpaper's metadata, an
// autostart entry) without trusting their size or kind. No Qt D-Bus, so the
// config tests can use it.
namespace smallfile
{
// The contents of the file at `path` when it is a regular file of at most
// `max` bytes, else a null array: for files other programs may have made. A
// link to a regular file is followed; a pipe, a device (/dev/zero reports a
// size of 0 and never ends) or a file bigger than `max` is not read, and
// what is read is bounded again at `max` in case the file grew since.
inline QByteArray read(const QString &path, qint64 max)
{
    const QFileInfo info(path);
    if (!info.isFile() || info.size() > max) {
        return {};
    }
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        return {};
    }
    const QByteArray data = f.read(max + 1);
    return data.size() > max ? QByteArray() : (data.isNull() ? QByteArray("") : data);
}

// Whether `path` is a regular file of at most `max` bytes (see read).
inline bool is(const QString &path, qint64 max)
{
    const QFileInfo info(path);
    return info.isFile() && info.size() <= max;
}
}
