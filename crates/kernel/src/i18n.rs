//! Fluent bundles contributed by modules, with English fallback.
//!
//! Every module carries `i18n/en.ftl` and `i18n/pt-BR.ftl` embedded with
//! `include_str!`. Missing keys return the key itself so a gap is visible in
//! the interface instead of an empty label; contract tests reject gaps before
//! release.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

/// Clock preference from `i18n.time.format`. `Auto` follows the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeFormat {
    Auto,
    H24,
    H12,
}

impl TimeFormat {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "24h" => Self::H24,
            "12h" => Self::H12,
            _ => Self::Auto,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::H24 => "24h",
            Self::H12 => "12h",
        }
    }
}

/// First day of the week from `i18n.date.week_start`. `Auto` follows the region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeekStart {
    Auto,
    Sunday,
    Monday,
}

impl WeekStart {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "sunday" => Self::Sunday,
            "monday" => Self::Monday,
            _ => Self::Auto,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Sunday => "sunday",
            Self::Monday => "monday",
        }
    }
}

impl Locale {
    pub fn parse(value: &str) -> Self {
        if value.trim().to_ascii_lowercase().starts_with("en") {
            Self::En
        } else {
            Self::PtBr
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }

    fn language_id(self) -> LanguageIdentifier {
        match self {
            Self::En => "en".parse().expect("valid en langid"),
            Self::PtBr => "pt-BR".parse().expect("valid pt-BR langid"),
        }
    }
}

/// Raw bundles of one module, kept in the registry until runtime startup.
#[derive(Clone, Debug)]
pub struct BundleSource {
    pub module: String,
    pub en: String,
    pub pt_br: String,
}

pub struct I18n {
    locale: Locale,
    /// Second choice when the locale lacks a key. English stays the last one.
    fallback: Locale,
    time_format: TimeFormat,
    week_start: WeekStart,
    en: FluentBundle<FluentResource>,
    pt_br: FluentBundle<FluentResource>,
}

impl I18n {
    pub fn build(sources: &[BundleSource], locale: Locale) -> (Self, Vec<String>) {
        let mut en = FluentBundle::new(vec![Locale::En.language_id()]);
        let mut pt_br = FluentBundle::new(vec![Locale::PtBr.language_id()]);
        en.set_use_isolating(false);
        pt_br.set_use_isolating(false);
        let mut warnings = Vec::new();
        for source in sources {
            add_source(&mut en, &source.module, "en", &source.en, &mut warnings);
            add_source(
                &mut pt_br,
                &source.module,
                "pt-BR",
                &source.pt_br,
                &mut warnings,
            );
        }
        (
            Self {
                locale,
                fallback: Locale::En,
                time_format: TimeFormat::Auto,
                week_start: WeekStart::Auto,
                en,
                pt_br,
            },
            warnings,
        )
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    /// Locale tried after the current one (`i18n.language.fallback`).
    pub fn fallback(&self) -> Locale {
        self.fallback
    }

    /// Sets the locale tried after the current one. English is always tried last.
    pub fn set_fallback(&mut self, locale: Locale) {
        self.fallback = locale;
    }

    pub fn time_format(&self) -> TimeFormat {
        self.time_format
    }

    pub fn set_time_format(&mut self, format: TimeFormat) {
        self.time_format = format;
    }

    pub fn week_start(&self) -> WeekStart {
        self.week_start
    }

    pub fn set_week_start(&mut self, week_start: WeekStart) {
        self.week_start = week_start;
    }

    pub fn t(&self, key: &str) -> String {
        self.t_args(key, &[])
    }

    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let preferred = self.bundle(self.locale);
        if let Some(text) = format_in(preferred, key, args) {
            return text;
        }
        if self.fallback != self.locale {
            if let Some(text) = format_in(self.bundle(self.fallback), key, args) {
                return text;
            }
        }
        if self.locale != Locale::En && self.fallback != Locale::En {
            if let Some(text) = format_in(&self.en, key, args) {
                return text;
            }
        }
        key.to_owned()
    }

    pub fn has(&self, locale: Locale, key: &str) -> bool {
        self.bundle(locale).get_message(&message_id(key)).is_some()
    }

    fn bundle(&self, locale: Locale) -> &FluentBundle<FluentResource> {
        match locale {
            Locale::En => &self.en,
            Locale::PtBr => &self.pt_br,
        }
    }
}

fn add_source(
    bundle: &mut FluentBundle<FluentResource>,
    module: &str,
    lang: &str,
    text: &str,
    warnings: &mut Vec<String>,
) {
    let resource = match FluentResource::try_new(text.to_owned()) {
        Ok(resource) => resource,
        Err((resource, errors)) => {
            for error in errors {
                warnings.push(format!("{module} i18n/{lang}.ftl: {error}"));
            }
            resource
        }
    };
    if let Err(errors) = bundle.add_resource(resource) {
        for error in errors {
            warnings.push(format!("{module} i18n/{lang}.ftl: {error}"));
        }
    }
}

/// Fluent message ids cannot contain `.`; the code-facing keys use dots and
/// are translated to hyphens when reading a bundle.
fn message_id(key: &str) -> String {
    key.replace('.', "-")
}

fn format_in(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: &[(&str, &str)],
) -> Option<String> {
    let message = bundle.get_message(&message_id(key))?;
    let pattern = message.value()?;
    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
        fluent_args.set(*name, *value);
    }
    let mut errors = Vec::new();
    let text = bundle
        .format_pattern(pattern, Some(&fluent_args), &mut errors)
        .into_owned();
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> BundleSource {
        BundleSource {
            module: "hello".into(),
            en: "hello-greeting = Hello, { $name }!\nhello-only-en = English only\n".into(),
            pt_br: "hello-greeting = Olá, { $name }!\nhello-only-pt = Somente pt\n".into(),
        }
    }

    #[test]
    fn formats_with_arguments_in_the_selected_locale() {
        let (i18n, warnings) = I18n::build(&[source()], Locale::PtBr);
        assert!(warnings.is_empty());
        assert_eq!(
            i18n.t_args("hello.greeting", &[("name", "Ana")]),
            "Olá, Ana!"
        );
        assert_eq!(
            i18n.t_args("hello.greeting", &[("name", "Ana")]),
            "Olá, Ana!"
        );
    }

    #[test]
    fn falls_back_to_english_then_to_the_key() {
        let (mut i18n, _) = I18n::build(&[source()], Locale::PtBr);
        assert_eq!(i18n.t("hello.only-en"), "English only");
        i18n.set_locale(Locale::En);
        assert_eq!(i18n.t("hello.only-en"), "English only");
        assert_eq!(i18n.t("hello.missing"), "hello.missing");
    }

    #[test]
    fn fallback_locale_is_tried_before_english() {
        let (mut i18n, _) = I18n::build(&[source()], Locale::En);
        assert_eq!(i18n.fallback(), Locale::En);
        i18n.set_fallback(Locale::PtBr);
        assert_eq!(i18n.t("hello.only-pt"), "Somente pt");
        assert_eq!(i18n.t("hello.only-en"), "English only");
        assert_eq!(i18n.t("hello.missing"), "hello.missing");
    }

    #[test]
    fn time_and_week_preferences_parse_and_round_trip() {
        assert_eq!(TimeFormat::parse("24h"), TimeFormat::H24);
        assert_eq!(TimeFormat::parse("12H"), TimeFormat::H12);
        assert_eq!(TimeFormat::parse("anything"), TimeFormat::Auto);
        assert_eq!(TimeFormat::H24.id(), "24h");
        assert_eq!(WeekStart::parse("monday"), WeekStart::Monday);
        assert_eq!(WeekStart::parse("Sunday"), WeekStart::Sunday);
        assert_eq!(WeekStart::parse(""), WeekStart::Auto);
        assert_eq!(WeekStart::Monday.id(), "monday");

        let (mut i18n, _) = I18n::build(&[source()], Locale::En);
        assert_eq!(i18n.time_format(), TimeFormat::Auto);
        assert_eq!(i18n.week_start(), WeekStart::Auto);
        i18n.set_time_format(TimeFormat::H12);
        i18n.set_week_start(WeekStart::Monday);
        assert_eq!(i18n.time_format(), TimeFormat::H12);
        assert_eq!(i18n.week_start(), WeekStart::Monday);
    }

    #[test]
    fn reports_which_keys_exist_in_each_locale() {
        let (i18n, _) = I18n::build(&[source()], Locale::PtBr);
        assert!(i18n.has(Locale::En, "hello.only-en"));
        assert!(!i18n.has(Locale::PtBr, "hello.only-en"));
        assert!(i18n.has(Locale::PtBr, "hello.greeting"));
    }
}
