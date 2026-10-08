//! Which language porch writes in: Korean or English (ADR 0011). The setting
//! is "system", "ko" or "en"; "system" follows the first of macOS's preferred
//! languages. porch-hook never asks.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Everything saved before languages existed was written in Korean.
    #[default]
    Ko,
    En,
}

/// What `Settings.language` may hold.
pub const SETTINGS: [&str; 3] = ["system", "ko", "en"];

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ko => "ko",
            Lang::En => "en",
        }
    }

    /// A saved code. Nothing, or anything unknown, is Korean: it was saved
    /// before languages existed.
    pub fn from_code(code: Option<&str>) -> Lang {
        if code == Some("en") {
            Lang::En
        } else {
            Lang::Ko
        }
    }

    /// The language a setting value asks for: "ko" and "en" as they are,
    /// anything else follows the system.
    pub fn resolve(setting: &str) -> Lang {
        match setting {
            "ko" => Lang::Ko,
            "en" => Lang::En,
            _ => from_locale(sys_locale::get_locale().as_deref()),
        }
    }

    /// The language the settings file asks for now.
    pub fn current() -> Lang {
        Lang::resolve(&crate::settings::load().language)
    }
}

/// Korean when the system's first language is Korean, English otherwise. A
/// locale that cannot be read keeps Korean, as porch was before languages existed.
fn from_locale(locale: Option<&str>) -> Lang {
    match locale {
        Some(l) if !l.to_ascii_lowercase().starts_with("ko") => Lang::En,
        _ => Lang::Ko,
    }
}

/// Whether `s` has a Hangul syllable or jamo. Keeps Korean out of English
/// prompts and material.
#[cfg(test)]
pub(crate) fn has_hangul(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7A3}'))
}

/// Every key of the JSON example a prompt ends with, in order. Two prompts
/// for the same job must ask for the same shape.
#[cfg(test)]
pub(crate) fn example_keys(prompt: &str) -> Vec<String> {
    fn walk(v: &serde_json::Value, keys: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    keys.push(k.clone());
                    walk(x, keys);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, keys)),
            _ => {}
        }
    }
    let start = prompt.rfind("\n{").expect("prompt ends with a JSON example") + 1;
    let v: serde_json::Value = serde_json::from_str(&prompt[start..]).expect("the example is JSON");
    let mut keys = Vec::new();
    walk(&v, &mut keys);
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_values_resolve() {
        assert_eq!(Lang::resolve("ko"), Lang::Ko);
        assert_eq!(Lang::resolve("en"), Lang::En);
    }

    #[test]
    fn system_locale_picks_korean_only_for_korean() {
        assert_eq!(from_locale(Some("ko-KR")), Lang::Ko);
        assert_eq!(from_locale(Some("ko")), Lang::Ko);
        assert_eq!(from_locale(Some("KO_kr")), Lang::Ko);
        assert_eq!(from_locale(Some("en-US")), Lang::En);
        assert_eq!(from_locale(Some("ja-JP")), Lang::En);
        assert_eq!(from_locale(Some("zh-Hans-CN")), Lang::En);
        assert_eq!(from_locale(None), Lang::Ko);
    }

    #[test]
    fn saved_codes() {
        assert_eq!(Lang::from_code(None), Lang::Ko);
        assert_eq!(Lang::from_code(Some("ko")), Lang::Ko);
        assert_eq!(Lang::from_code(Some("en")), Lang::En);
        assert_eq!(Lang::from_code(Some("fr")), Lang::Ko);
        assert_eq!((Lang::Ko.code(), Lang::En.code()), ("ko", "en"));
    }

    #[test]
    fn hangul_is_found() {
        assert!(has_hangul("build 가"));
        assert!(has_hangul("ㄱ"));
        assert!(!has_hangul("Café, naïve, 日本"));
    }

    #[test]
    fn example_keys_walk_the_last_json() {
        let p = "Rules {\"x\": 1}\nOutput:\n{\"a\": \"\", \"b\": [{\"c\": 1}], \"d\": {\"e\": []}}";
        assert_eq!(example_keys(p), ["a", "b", "c", "d", "e"]);
    }
}
