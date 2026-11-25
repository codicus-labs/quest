//! Settings read from the system.

/// The system language as a BCP 47 tag (`de-DE`), from the macOS preferences via `sys-locale`
/// (a Finder launch has no `LANG`); `en` when it can't be read.
pub fn system_language() -> String {
    sys_locale::get_locale().as_deref().and_then(language_tag).unwrap_or_else(|| "en".to_owned())
}

/// `de_DE.UTF-8` or `de-DE` → `de-DE`; `C`, `POSIX` and empty values say nothing.
pub fn language_tag(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next().unwrap_or_default().trim().replace('_', "-");
    let usable = !base.is_empty()
        && base != "C"
        && base != "POSIX"
        && base.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    usable.then_some(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags_come_from_locales() {
        assert_eq!(language_tag("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(language_tag("en-US").as_deref(), Some("en-US"));
        assert_eq!(language_tag("C"), None);
        assert_eq!(language_tag(""), None);
    }
}
