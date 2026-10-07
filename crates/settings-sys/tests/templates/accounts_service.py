"""AccountsService mock template, for the settings-sys tests (python-dbusmock
has none). Parameters: Users, a list of {Uid, UserName, RealName, IconFile,
AccountType, AutomaticLogin}.

The mock keeps what a test needs to check on the user's object: the
crypt(3) hash given to SetPassword is in the property TestPasswordHash.
"""

# SPDX-License-Identifier: MIT

import dbus

BUS_NAME = "org.freedesktop.Accounts"
MAIN_OBJ = "/org/freedesktop/Accounts"
MAIN_IFACE = "org.freedesktop.Accounts"
USER_IFACE = "org.freedesktop.Accounts.User"
SYSTEM_BUS = True


def not_found(what):
    return dbus.exceptions.DBusException(f"no such user: {what}", name="org.freedesktop.Accounts.Error.Failed")


def user_path(uid):
    return f"/org/freedesktop/Accounts/User{uid}"


def add_user(mock, uid, name, real, icon, account_type, auto):
    props = {
        "Uid": dbus.UInt64(uid),
        "UserName": name,
        "RealName": real,
        "IconFile": icon,
        "AccountType": dbus.Int32(account_type),
        "Locked": dbus.Boolean(False),
        "AutomaticLogin": dbus.Boolean(auto),
        "PasswordMode": dbus.Int32(0),
        "LocalAccount": dbus.Boolean(True),
        "SystemAccount": dbus.Boolean(False),
        "TestPasswordHash": "",
    }
    set_ = lambda prop: f'self.Set("{USER_IFACE}", "{prop}", args[0])'
    mock.AddObject(
        user_path(uid),
        USER_IFACE,
        dbus.Dictionary(props, signature="sv"),
        [
            ("SetRealName", "s", "", set_("RealName")),
            ("SetIconFile", "s", "", set_("IconFile")),
            ("SetAccountType", "i", "", set_("AccountType")),
            ("SetAutomaticLogin", "b", "", set_("AutomaticLogin")),
            ("SetLocked", "b", "", set_("Locked")),
            ("SetPassword", "ss", "", set_("TestPasswordHash")),
        ],
    )
    mock.users.append(uid)
    return user_path(uid)


def create_user(self, name, real, account_type):
    for uid in self.users:
        if str(self.objects_by_uid(uid).Get(USER_IFACE, "UserName")) == name:
            raise dbus.exceptions.DBusException(
                f"user {name} exists", name="org.freedesktop.Accounts.Error.UserExists"
            )
    uid = self.next_uid
    self.next_uid += 1
    return add_user(self, uid, name, real, "", account_type, False)


def delete_user(self, uid):
    if uid not in self.users:
        raise not_found(uid)
    self.RemoveObject(user_path(uid))
    self.users.remove(uid)


def find_user_by_id(self, uid):
    if uid not in self.users:
        raise not_found(uid)
    return user_path(uid)


def load(mock, parameters):
    import dbusmock

    mock.users = []
    mock.next_uid = 1001
    mock.create_user = create_user
    mock.delete_user = delete_user
    mock.find_user_by_id = find_user_by_id
    mock.objects_by_uid = lambda uid: dbusmock.get_object(user_path(uid))
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("ListCachedUsers", "", "ao", "ret = [dbus.ObjectPath(f'/org/freedesktop/Accounts/User{u}') for u in self.users]"),
            ("FindUserById", "x", "o", "ret = self.find_user_by_id(self, args[0])"),
            ("CreateUser", "ssi", "o", "ret = self.create_user(self, args[0], args[1], args[2])"),
            ("DeleteUser", "xb", "", "self.delete_user(self, args[0])"),
        ],
    )
    for u in parameters.get("Users", []):
        add_user(
            mock,
            u["Uid"],
            u["UserName"],
            u.get("RealName", ""),
            u.get("IconFile", ""),
            u.get("AccountType", 0),
            u.get("AutomaticLogin", False),
        )
    mock.next_uid = max(mock.users + [1000]) + 1
