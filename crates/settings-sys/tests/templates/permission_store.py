"""Portal PermissionStore mock template, for the settings-sys tests
(python-dbusmock has none). Parameters: Tables, {table: {id: {app: [perm]}}}.
It answers Lookup, SetPermission and DeletePermission, with the store's own
NotFound error for a table or ID it has nothing for.
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.freedesktop.impl.portal.PermissionStore"
MAIN_OBJ = "/org/freedesktop/impl/portal/PermissionStore"
MAIN_IFACE = "org.freedesktop.impl.portal.PermissionStore"
# A session service; the tests put it on their one private bus.
SYSTEM_BUS = False


def not_found(what):
    return dbus.exceptions.DBusException(what, name="org.freedesktop.portal.Error.NotFound")


def lookup(mock, table, id_):
    if table not in mock.tables or id_ not in mock.tables[table]:
        raise not_found(f"{table}/{id_}")
    return mock.tables[table][id_]


def set_permission(mock, table, create, id_, app, perms):
    if table not in mock.tables:
        if not create:
            raise not_found(table)
        mock.tables[table] = {}
    if id_ not in mock.tables[table]:
        if not create:
            raise not_found(id_)
        mock.tables[table][id_] = {}
    mock.tables[table][id_][app] = list(perms)


def delete_permission(mock, table, id_, app):
    entry = lookup(mock, table, id_)
    if app not in entry:
        raise not_found(app)
    del entry[app]


def load(mock, parameters):
    mock.tables = {t: {i: {a: list(p) for a, p in apps.items()} for i, apps in ids.items()} for t, ids in parameters.get("Tables", {}).items()}
    mock.lookup = lookup
    mock.set_permission = set_permission
    mock.delete_permission = delete_permission
    mock.AddMethods(
        MAIN_IFACE,
        [
            (
                "Lookup",
                "ss",
                "a{sas}v",
                "ret = (dbus.Dictionary(self.lookup(self, args[0], args[1]), signature='sas'), dbus.String(''))",
            ),
            ("SetPermission", "sbssas", "", "self.set_permission(self, args[0], args[1], args[2], args[3], args[4])"),
            ("DeletePermission", "sss", "", "self.delete_permission(self, args[0], args[1], args[2])"),
        ],
    )
