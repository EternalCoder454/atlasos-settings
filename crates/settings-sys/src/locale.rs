//! systemd-localed (`org.freedesktop.locale1`): the system's language, which
//! the login screen and new users get. The user's own language and formats
//! are Plasma's (`plasma-localerc`, written through KConfig in
//! `cpp/localeconfig.cpp`). Changes are localed's polkit action
//! (`org.freedesktop.locale1.set-locale`).

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::{Error, ErrorKind};

#[zbus::proxy(
    interface = "org.freedesktop.locale1",
    default_service = "org.freedesktop.locale1",
    default_path = "/org/freedesktop/locale1",
    gen_async = false,
    blocking_name = "LocaleProxy"
)]
trait Locale {
    #[zbus(property)]
    fn locale(&self) -> zbus::Result<Vec<String>>;

    fn set_locale(&self, locale: &[&str], interactive: bool) -> zbus::Result<()>;
}

/// The longest locale name accepted (`ca_ES.UTF-8@valencia` is 20).
pub const MAX_LOCALE: usize = 32;

/// The languages offered, as glibc locales: the ones Plasma is translated
/// into most completely, with the usual region for each. A language set
/// some other way is shown too.
pub const LANGUAGES: &[&str] = &[
    "ar_EG.UTF-8",
    "bg_BG.UTF-8",
    "ca_ES.UTF-8",
    "cs_CZ.UTF-8",
    "da_DK.UTF-8",
    "de_AT.UTF-8",
    "de_CH.UTF-8",
    "de_DE.UTF-8",
    "el_GR.UTF-8",
    "en_AU.UTF-8",
    "en_CA.UTF-8",
    "en_GB.UTF-8",
    "en_IE.UTF-8",
    "en_IN.UTF-8",
    "en_NZ.UTF-8",
    "en_US.UTF-8",
    "es_ES.UTF-8",
    "es_MX.UTF-8",
    "et_EE.UTF-8",
    "eu_ES.UTF-8",
    "fi_FI.UTF-8",
    "fr_BE.UTF-8",
    "fr_CA.UTF-8",
    "fr_CH.UTF-8",
    "fr_FR.UTF-8",
    "gl_ES.UTF-8",
    "he_IL.UTF-8",
    "hi_IN.UTF-8",
    "hu_HU.UTF-8",
    "id_ID.UTF-8",
    "it_IT.UTF-8",
    "ja_JP.UTF-8",
    "ka_GE.UTF-8",
    "ko_KR.UTF-8",
    "lt_LT.UTF-8",
    "lv_LV.UTF-8",
    "nb_NO.UTF-8",
    "nl_BE.UTF-8",
    "nl_NL.UTF-8",
    "pl_PL.UTF-8",
    "pt_BR.UTF-8",
    "pt_PT.UTF-8",
    "ro_RO.UTF-8",
    "ru_RU.UTF-8",
    "sk_SK.UTF-8",
    "sl_SI.UTF-8",
    "sr_RS.UTF-8",
    "sv_SE.UTF-8",
    "ta_IN.UTF-8",
    "th_TH.UTF-8",
    "tr_TR.UTF-8",
    "uk_UA.UTF-8",
    "vi_VN.UTF-8",
    "zh_CN.UTF-8",
    "zh_TW.UTF-8",
];

/// Whether `l` looks like a glibc locale name: `C`, `POSIX`, or a language
/// of 2 or 3 letters, an optional `_REGION`, `.codeset` and `@modifier`.
pub fn valid_locale(l: &str) -> bool {
    if l.is_empty() || l.len() > MAX_LOCALE {
        return false;
    }
    if matches!(l, "C" | "POSIX" | "C.UTF-8" | "C.utf8") {
        return true;
    }
    let (rest, modifier) = match l.split_once('@') {
        Some((r, m)) => (r, Some(m)),
        None => (l, None),
    };
    let (rest, codeset) = match rest.split_once('.') {
        Some((r, c)) => (r, Some(c)),
        None => (rest, None),
    };
    let (lang, region) = match rest.split_once('_') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    (2..=3).contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase())
        && region.is_none_or(|r| {
            (2..=3).contains(&r.len())
                && r.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
        && codeset.is_none_or(|c| {
            !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && modifier.is_none_or(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_lowercase()))
}

/// `LANG` from localed's `Locale` (`["LANG=de_DE.UTF-8", "LC_TIME=..."]`),
/// or "" when it isn't set or isn't a locale name.
fn lang_of(entries: &[String]) -> String {
    entries
        .iter()
        .take(32)
        .find_map(|e| e.strip_prefix("LANG="))
        .filter(|l| valid_locale(l))
        .unwrap_or_default()
        .to_string()
}

pub struct Localed {
    conn: zbus::blocking::Connection,
    interactive: zbus::blocking::Connection,
}

impl Localed {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Localed {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    /// The system's language (`LANG`), or "" when none is set.
    pub fn language(&self) -> Result<String, Error> {
        Ok(lang_of(&LocaleProxy::new(&self.conn)?.locale()?))
    }

    /// Sets the system's language: `LANG` alone, so the formats follow it.
    /// polkit may ask for a password.
    pub fn set_language(&self, locale: &str) -> Result<(), Error> {
        if !valid_locale(locale) {
            return Err(Error::new(
                ErrorKind::Refused,
                format!("not a locale name: {locale:?}"),
            ));
        }
        let entry = format!("LANG={locale}");
        Ok(LocaleProxy::new(&self.interactive)?.set_locale(&[entry.as_str()], true)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_names() {
        for ok in [
            "C",
            "C.UTF-8",
            "en_US.UTF-8",
            "de_DE",
            "ca_ES.UTF-8@valencia",
            "sr_RS@latin",
            "ast_ES.UTF-8",
            "es_419.UTF-8",
        ] {
            assert!(valid_locale(ok), "{ok}");
        }
        for bad in [
            "",
            "english",
            "EN_us",
            "en_US.UTF-8\n",
            "../../etc",
            "en_US;rm",
            "en_US.",
            "en_US@",
            &format!("en_US.{}", "a".repeat(40)),
        ] {
            assert!(!valid_locale(bad), "{bad:?}");
        }
        assert!(LANGUAGES.iter().all(|l| valid_locale(l)));
    }

    #[test]
    fn lang_entry() {
        let e = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            lang_of(&e(&["LC_TIME=en_GB.UTF-8", "LANG=de_DE.UTF-8"])),
            "de_DE.UTF-8"
        );
        assert_eq!(lang_of(&e(&["LANG=$(reboot)"])), "");
        assert_eq!(lang_of(&e(&[])), "");
    }
}
