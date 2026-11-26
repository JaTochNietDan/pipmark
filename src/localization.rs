//! Local UI strings only; app names, paths, IDs and CLI data remain untouched.
use crate::model::Language;
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
static ACTIVE: AtomicUsize = AtomicUsize::new(1);
fn catalog() -> &'static HashMap<String, [String; 5]> {
    static CATALOG: OnceLock<HashMap<String, [String; 5]>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("locales.json")).expect("embedded translations")
    })
}
pub fn from_tag(tag: &str) -> Option<Language> {
    let tag = tag.replace('_', "-").to_ascii_lowercase();
    let parts: Vec<_> = tag.split('-').collect();
    match parts[0] {
        "en" => Some(Language::English),
        "de" => Some(Language::German),
        "ja" => Some(Language::Japanese),
        "ko" => Some(Language::Korean),
        "zh" => Some(
            if !parts.contains(&"hans")
                && (parts.contains(&"hant")
                    || parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")))
            {
                Language::TraditionalChinese
            } else {
                Language::SimplifiedChinese
            },
        ),
        _ => None,
    }
}
pub fn configure(language: Language) {
    let resolved = if language == Language::System {
        objc2_foundation::NSLocale::preferredLanguages()
            .iter()
            .find_map(|tag| from_tag(&tag.to_string()))
            .unwrap_or(Language::English)
    } else {
        language
    };
    ACTIVE.store(resolved as usize, Ordering::Relaxed);
}
pub fn translate_in(language: Language, text: &str) -> &str {
    let index = match language {
        Language::German => 0,
        Language::Japanese => 1,
        Language::Korean => 2,
        Language::SimplifiedChinese => 3,
        Language::TraditionalChinese => 4,
        _ => return text,
    };
    catalog()
        .get(text)
        .map(|values| values[index].as_str())
        .unwrap_or(text)
}
pub fn text(text: &str) -> &str {
    translate_in(
        Language::ALL
            .get(ACTIVE.load(Ordering::Relaxed))
            .copied()
            .unwrap_or(Language::English),
        text,
    )
}
/// Substitute only placeholders in the template, never inside inserted values.
pub fn format_in(language: Language, template: &str, values: &[(&str, &str)]) -> String {
    let mut rest = translate_in(language, template);
    let mut output = String::new();
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            output.push_str(&rest[start..]);
            return output;
        };
        let key = &rest[start + 1..end];
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            output.push_str(value);
        } else {
            output.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}
pub fn format(template: &str, values: &[(&str, &str)]) -> String {
    format_in(
        Language::ALL
            .get(ACTIVE.load(Ordering::Relaxed))
            .copied()
            .unwrap_or(Language::English),
        template,
        values,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translations_preserve_template_fields() {
        let fields = |value: &str| {
            let mut result: Vec<_> = value
                .split('{')
                .skip(1)
                .filter_map(|part| part.split_once('}').map(|(field, _)| field.to_owned()))
                .collect();
            result.sort();
            result
        };
        for (key, translations) in catalog() {
            for value in translations {
                assert_eq!(
                    fields(key),
                    fields(value),
                    "translation fields for {key}: {value}"
                );
            }
        }
    }
    #[test]
    fn inserted_values_are_never_reinterpreted() {
        assert_eq!(
            format_in(
                Language::German,
                "Open {app}",
                &[("app", "App {count}"), ("count", "7")]
            ),
            "App {count} öffnen"
        );
        assert_eq!(
            format_in(
                Language::Japanese,
                "{app} — {count} unread",
                &[("app", "日本語 {app}"), ("count", "12")]
            ),
            "日本語 {app} — 未読12件"
        );
        assert_eq!(
            format_in(Language::English, "unknown {missing} {", &[]),
            "unknown {missing} {"
        );
    }
    #[test]
    fn locale_tags_and_fallbacks() {
        assert_eq!(from_tag("de-DE"), Some(Language::German));
        assert_eq!(from_tag("zh_Hant_TW"), Some(Language::TraditionalChinese));
        assert_eq!(from_tag("zh-HK"), Some(Language::TraditionalChinese));
        assert_eq!(from_tag("zh-CN"), Some(Language::SimplifiedChinese));
        assert_eq!(from_tag("zh-Hans-HK"), Some(Language::SimplifiedChinese));
        assert_eq!(from_tag("fr-FR"), None);
        assert_eq!(translate_in(Language::Japanese, "Settings…"), "設定…");
        assert_eq!(translate_in(Language::German, "My app name"), "My app name");
        assert!(
            catalog()
                .values()
                .all(|values| values.iter().all(|s| !s.is_empty()))
        );
    }
    #[test]
    fn language_persists_and_old_settings_follow_system() {
        let mut settings: crate::model::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.language, Language::System);
        settings.language = Language::TraditionalChinese;
        let restored: crate::model::Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.language, Language::TraditionalChinese);
    }
}
