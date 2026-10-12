//! Embedded offline message catalogs. UI messages use stable IDs; engine display labels
//! pass through an adapter without changing engine or scenario identifiers.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::OnceLock;

type Messages = BTreeMap<String, String>;

/// Register locales here after adding their catalogs. Missing translations use English.
pub struct Catalog {
    pub id: &'static str,
    pub name: &'static str,
    source: &'static str,
    messages: OnceLock<Messages>,
}

impl Catalog {
    const fn new(id: &'static str, name: &'static str, source: &'static str) -> Self {
        Self { id, name, source, messages: OnceLock::new() }
    }

    fn messages(&'static self) -> &'static Messages {
        self.messages.get_or_init(|| match serde_json::from_str(self.source) {
            Ok(messages) => messages,
            Err(error) => {
                log::error!("invalid embedded {} message catalog: {error}", self.id);
                Messages::new()
            }
        })
    }
}

static ENGLISH: Catalog = Catalog::new("en", "English", include_str!("locales/en.json"));
static JAPANESE: Catalog = Catalog::new("ja", "日本語", include_str!("locales/ja.json"));
pub const LOCALES: &[&Catalog] = &[&ENGLISH, &JAPANESE];

pub fn is_supported(id: &str) -> bool {
    LOCALES.iter().any(|catalog| catalog.id == id)
}

/// Immutable locale reference. Catalogs parse once; lookups are logarithmic.
#[derive(Clone, Copy)]
pub struct Locale(&'static Catalog);

impl Locale {
    pub fn for_language(id: &str) -> Self {
        Self(LOCALES.iter().copied().find(|catalog| catalog.id == id).unwrap_or(&ENGLISH))
    }

    pub fn name(self) -> &'static str {
        self.0.name
    }

    /// Stable message ID → translation → English → ID.
    pub fn text(self, id: &str) -> &str {
        self.0.messages().get(id).or_else(|| ENGLISH.messages().get(id)).map_or(id, String::as_str)
    }

    /// Translate source labels from engine/UI tables, preserving unknown labels.
    /// Also accepts stable IDs while table helpers migrate away from source text.
    pub fn label(self, label: &str) -> &str {
        static LABEL_IDS: OnceLock<BTreeMap<&'static str, &'static str>> = OnceLock::new();
        let ids = LABEL_IDS.get_or_init(|| {
            ENGLISH.messages().iter().filter(|(_, text)| !text.contains('{')).map(|(id, text)| (text.as_str(), id.as_str())).collect()
        });
        ids.get(label).map_or_else(|| self.text(label), |id| self.text(id))
    }

    pub fn format(self, id: &str, args: &[(&str, &str)]) -> String {
        interpolate(self.text(id), args)
    }

    /// Translate generated browser names and labels, preserving custom names otherwise.
    pub fn dynamic(self, text: &str) -> String {
        for (prefix, id) in [("Sketch", "browser.sketch_name"), ("Body", "browser.body_name"), ("Component", "browser.component_name")] {
            if let Some(number) = text.strip_prefix(prefix)
                && !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit())
            {
                return self.format(id, &[("number", number)]);
            }
        }
        for (prefix, id, arg) in [("Units: ", "browser.units", "units"), ("Precision: ", "browser.precision", "precision")] {
            if let Some(value) = text.strip_prefix(prefix) {
                return self.format(id, &[(arg, value)]);
            }
        }
        if let Some(count) = text.strip_prefix("Parameters (").and_then(|s| s.strip_suffix(')')) {
            return self.format("browser.parameters", &[("count", count)]);
        }
        if let Some(count) = text.strip_suffix(" degrees of freedom")
            && !count.is_empty()
            && count.chars().all(|c| c.is_ascii_digit())
        {
            return self.format("sketch.degrees_of_freedom", &[("count", count)]);
        }
        self.label(text).to_string()
    }
}

/// Append literal text, where a doubled closing brace stands for one brace.
fn push_literal(output: &mut String, text: &str) {
    let mut rest = text;
    while let Some(at) = rest.find("}}") {
        output.push_str(&rest[..at + 1]);
        rest = &rest[at + 2..];
    }
    output.push_str(rest);
}

/// One-pass substitution: argument values are literal, even when they contain braces.
/// Missing arguments remain visible rather than crashing or silently removing text.
/// A doubled brace stands for the brace itself, so a message can show a literal example
/// such as `{{"distance": 20}}` without it reading as a placeholder.
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        push_literal(&mut output, &rest[..open]);
        rest = &rest[open + 1..];
        if let Some(after) = rest.strip_prefix('{') {
            output.push('{');
            rest = after;
            continue;
        }
        let Some(close) = rest.find('}') else {
            output.push('{');
            push_literal(&mut output, rest);
            return output;
        };
        let name = &rest[..close];
        rest = &rest[close + 1..];
        if let Some(after) = rest.strip_prefix('}') {
            rest = after;
        }
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => output.push_str(value),
            None => {
                output.push('{');
                push_literal(&mut output, name);
                output.push('}');
            }
        }
    }
    push_literal(&mut output, rest);
    output
}

thread_local! {
    // Helpers without an app reference share a render scope. Nested renders restore
    // their caller's locale; no owned language strings or persistent global setting.
    static CURRENT: Cell<Locale> = const { Cell::new(Locale(&ENGLISH)) };
}

#[must_use]
pub struct LanguageScope {
    previous: Locale,
    // Drop on the thread where the scope was entered.
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

pub fn scope(id: &str) -> LanguageScope {
    let previous = CURRENT.with(|current| current.replace(Locale::for_language(id)));
    LanguageScope { previous, _thread: std::marker::PhantomData }
}

impl Drop for LanguageScope {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.previous));
    }
}

pub fn tr(id: &str) -> &str {
    CURRENT.with(|current| current.get().text(id))
}
pub fn tr_label(label: &str) -> &str {
    CURRENT.with(|current| current.get().label(label))
}
pub fn format(id: &str, args: &[(&str, &str)]) -> String {
    CURRENT.with(|current| current.get().format(id, args))
}
pub fn tr_dyn(text: &str) -> String {
    CURRENT.with(|current| current.get().dynamic(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Placeholder names in a catalog value; a doubled brace is literal text, not one.
    fn placeholders(text: &str) -> BTreeSet<&str> {
        let mut result = BTreeSet::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            rest = &rest[open + 1..];
            if let Some(after) = rest.strip_prefix('{') {
                rest = after;
                continue;
            }
            let Some(close) = rest.find('}') else { panic!("unclosed placeholder in {text:?}") };
            let name = &rest[..close];
            assert!(!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "bad placeholder {name:?}");
            result.insert(name);
            rest = &rest[close + 1..];
            if let Some(after) = rest.strip_prefix('}') {
                rest = after;
            }
        }
        result
    }

    #[test]
    fn catalogs_have_valid_ids_and_matching_placeholders() {
        let english: Messages = serde_json::from_str(ENGLISH.source).unwrap();
        let mut ids = BTreeSet::new();
        for catalog in LOCALES {
            assert!(ids.insert(catalog.id), "duplicate locale");
            let messages: Messages = serde_json::from_str(catalog.source).unwrap();
            assert!(!messages.is_empty());
            for (id, text) in messages {
                let original = english.get(&id).expect("unknown message ID");
                assert_eq!(placeholders(&text), placeholders(original), "{}: {id}", catalog.id);
            }
        }
    }

    #[test]
    fn direct_ui_message_ids_exist_in_english_catalog() {
        // Catch a misspelled key before it is shown verbatim in the UI. Engine-table labels
        // deliberately use the separate label adapter and fall back to their source text.
        for source in [
            include_str!("toolbar.rs"),
            include_str!("prefs.rs"),
            include_str!("documents.rs"),
            include_str!("dialogs.rs"),
            include_str!("browser.rs"),
            include_str!("sketch_palette.rs"),
        ] {
            for call in [".tr(\"", "i18n::tr(\"", "i18n::format(\""] {
                for tail in source.split(call).skip(1) {
                    let id = tail.split('"').next().unwrap();
                    assert!(ENGLISH.messages().contains_key(id), "missing English message: {id}");
                }
            }
        }
    }

    /// Every command, toolbar tab and panel the app can show has a catalog entry, so nothing in
    /// the toolbar or the command palette falls back to its English source text. Add the entry
    /// with the command that introduces the label.
    #[test]
    fn every_command_label_is_translated() {
        let english: Messages = serde_json::from_str(ENGLISH.source).unwrap();
        let japanese: Messages = serde_json::from_str(JAPANESE.source).unwrap();
        let english_labels: BTreeSet<&str> = english.values().map(String::as_str).collect();
        let mut missing: BTreeSet<&str> = BTreeSet::new();
        for spec in solvecraft_engine::command_specs() {
            if !english_labels.contains(spec.label) {
                missing.insert(spec.label);
            }
        }
        for tab in crate::toolbar::TABS {
            if !english_labels.contains(tab) {
                missing.insert(tab);
            }
            for panel in crate::toolbar::panels(tab) {
                if !english_labels.contains(panel) {
                    missing.insert(panel);
                }
            }
        }
        assert!(missing.is_empty(), "labels with no catalog entry: {missing:?}");
        // Every English entry carries a translation; the shared catalog test above already
        // checks that both catalogs hold the same IDs.
        for id in english.keys() {
            assert!(japanese.contains_key(id), "no translation for {id}");
        }
    }

    /// label() looks an English source text up in the English catalog, so one text must not
    /// stand for two different translations.
    #[test]
    fn one_english_label_has_one_translation() {
        let english: Messages = serde_json::from_str(ENGLISH.source).unwrap();
        let japanese: Messages = serde_json::from_str(JAPANESE.source).unwrap();
        let mut first: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
        for (id, text) in &english {
            let translation = japanese.get(id).map(String::as_str).unwrap_or(text);
            match first.get(text.as_str()) {
                Some((_, other)) => assert_eq!(*other, translation, "{text}: {id} and the entry before it disagree"),
                None => {
                    first.insert(text.as_str(), (id.as_str(), translation));
                }
            }
        }
    }

    #[test]
    fn keys_and_engine_labels_translate_with_english_fallback() {
        let ja = Locale::for_language("ja");
        assert_eq!(ja.text("ui.preferences"), "環境設定");
        assert_eq!(ja.label("Extrude"), "押し出し");
        assert_eq!(ja.label("CREATE SKETCH"), "スケッチを作成");
        assert_eq!(ja.label("NonExistentKey"), "NonExistentKey");
        assert_eq!(Locale::for_language("unknown").text("ui.preferences"), "Preferences");
        static PARTIAL: Catalog = Catalog::new("test", "Test", r#"{"ui.preferences":"Prefs"}"#);
        assert_eq!(Locale(&PARTIAL).text("ui.preferences"), "Prefs");
        assert_eq!(Locale(&PARTIAL).text("ui.extrude"), "Extrude");
    }

    #[test]
    fn a_doubled_brace_is_literal_text() {
        assert_eq!(interpolate("Extrude {{\"distance\": 20}} now", &[]), "Extrude {\"distance\": 20} now");
        assert_eq!(interpolate("{{a}} {x}", &[("x", "b")]), "{a} b");
        assert_eq!(interpolate("keep }} as is", &[]), "keep } as is");
        assert_eq!(placeholders("Extrude {{\"distance\": 20}} at {x}"), BTreeSet::from(["x"]));
    }

    #[test]
    fn templates_allow_reordering_and_literal_braces_in_values() {
        assert_eq!(interpolate("{second} then {first}", &[("first", "{second}"), ("second", "B")]), "B then {second}");
        assert_eq!(interpolate("{missing} / {", &[]), "{missing} / {");
        assert_eq!(Locale::for_language("ja").format("document.save_changes", &[("name", "My {design}")]), "My {design} への変更を保存しますか？");
        assert_eq!(Locale::for_language("en").format("dialog.selected_count", &[("count", "2")]), "↖ 2 selected");
    }

    #[test]
    fn browser_patterns_preserve_numbers_and_custom_names() {
        let ja = Locale::for_language("ja");
        assert_eq!(ja.dynamic("Sketch1"), "スケッチ1");
        assert_eq!(ja.dynamic("Body2"), "ボディ2");
        assert_eq!(ja.dynamic("Component12"), "コンポーネント12");
        assert_eq!(ja.dynamic("Units: mm"), "単位: mm");
        assert_eq!(ja.dynamic("Precision: 3"), "精度: 3");
        assert_eq!(ja.dynamic("Parameters (5)"), "パラメータ (5)");
        assert_eq!(ja.dynamic("4 degrees of freedom"), "自由度 4");
        assert_eq!(ja.dynamic("Sketch of bracket"), "Sketch of bracket");
        assert_eq!(Locale::for_language("en").dynamic("Sketch1"), "Sketch1");
    }

    #[test]
    fn render_scopes_restore_the_previous_language() {
        let _english = scope("en");
        {
            let _japanese = scope("ja");
            assert_eq!(tr("ui.preferences"), "環境設定");
            {
                let _nested = scope("en");
                assert_eq!(tr("ui.preferences"), "Preferences");
            }
            assert_eq!(tr("ui.preferences"), "環境設定");
        }
        assert_eq!(tr("ui.preferences"), "Preferences");
    }
}
