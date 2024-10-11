use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Filter {
    #[serde(default)]
    #[serde(deserialize_with = "string_to_regex")]
    match_app_id: Option<Regex>,
    #[serde(default)]
    #[serde(deserialize_with = "string_to_regex")]
    match_title: Option<Regex>,
    replace_app_id: Option<String>,
    replace_title: Option<String>,
}

fn string_to_regex<'de, D>(d: D) -> Result<Option<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = <Option<String>>::deserialize(d)?;

    if let Some(s) = s {
        match format!("^{s}$").parse() {
            Ok(regex) => Ok(Some(regex)),
            Err(err) => Err(D::Error::custom(err)),
        }
    } else {
        Ok(None)
    }
}

#[derive(Debug, PartialEq)]
pub enum FilterResult {
    Replace(Replacement),
    Match,
    Skip,
}

#[derive(Default, Debug, PartialEq)]
pub struct Replacement {
    pub replace_app_id: Option<String>,
    pub replace_title: Option<String>,
}

impl Filter {
    fn is_valid(&self) -> bool {
        self.match_app_id.is_some() || self.match_title.is_some()
    }

    fn is_match(&self, app_id: &str, title: &str) -> bool {
        if let Some(match_app_id) = &self.match_app_id {
            if !match_app_id.is_match(app_id) {
                return false;
            };
        };
        if let Some(match_title) = &self.match_title {
            if !match_title.is_match(title) {
                return false;
            };
        };

        true
    }

    fn replace(regex: &Option<Regex>, source: &str, replacement: &str) -> String {
        if let Some(regex) = regex {
            // Avoid using the more expensive regexp replacements when unnecessary.
            if regex.captures_len() > 1 {
                return regex.replace(source, replacement).to_string();
            }
        }
        replacement.to_owned()
    }

    pub fn apply(&self, app_id: &str, title: &str) -> FilterResult {
        if !self.is_valid() || !self.is_match(app_id, title) {
            return FilterResult::Skip;
        }
        if self.replace_app_id.is_none() && self.replace_title.is_none() {
            return FilterResult::Match;
        }

        let mut replacement = Replacement::default();
        if let Some(new_app_id) = &self.replace_app_id {
            replacement.replace_app_id =
                Some(Self::replace(&self.match_app_id, app_id, new_app_id));
        }
        if let Some(new_title) = &self.replace_title {
            replacement.replace_title = Some(Self::replace(&self.match_title, title, new_title));
        }
        FilterResult::Replace(replacement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::no_match(
        (Some("firefox"), Some("Title")),
        (None, Some("Secret")),
        ("org.kde.dolphin", "/home/user"),
        None
    )]
    #[case::app_id_match(
        (Some(".*dolphin"), Some("Title")),
        (None, Some("Secret")),
        ("org.kde.dolphin", "/home/user"),
        None
    )]
    #[case::title_match(
        (Some("firefox"), Some("/home/user")),
        (None, Some("Secret")),
        ("org.kde.dolphin", "/home/user"),
        None
    )]
    #[case::replace_title(
        (Some(".*dolphin"), Some("/home/user")),
        (None, Some("Secret")),
        ("org.kde.dolphin", "/home/user"),
        Some((None, Some("Secret")))
    )]
    #[case::replace_app_id(
        (Some(".*dolphin"), Some("/home/user")),
        (Some("FM"), None),
        ("org.kde.dolphin", "/home/user"),
        Some((Some("FM"), None))
    )]
    #[case::replace(
        (Some(".*dolphin"), Some("/home/user")),
        (Some("FM"), None),
        ("org.kde.dolphin", "/home/user"),
        Some((Some("FM"), None))
    )]
    #[case::replace_with_catch(
        (Some("org\\.kde\\.(.*)"), None),
        (Some("$1"), None),
        ("org.kde.dolphin", "/home/user"),
        Some((Some("dolphin"), None))
    )]
    #[case::skip_empty_matches(
        (None, None),
        (None, Some("Secret")),
        ("org.kde.dolphin", "/home/user"),
        None
    )]
    #[case::match_only(
        (Some("org\\.kde\\.(.*)"), None),
        (None, None),
        ("org.kde.dolphin", "/home/user"),
        Some((None, None))
    )]
    fn replacement(
        #[case] matches: (Option<&str>, Option<&str>),
        #[case] replaces: (Option<&str>, Option<&str>),
        #[case] data: (&str, &str),
        #[case] expect_replacement: Option<(Option<&str>, Option<&str>)>,
    ) {
        let (match_app_id, match_title) = matches;
        let (replace_app_id, replace_title) = replaces;
        let (app_id, title) = data;

        let option_string = |s: &str| s.to_string();
        let filter = Filter {
            match_app_id: match_app_id.map(|s| format!("^{s}$").parse().unwrap()),
            match_title: match_title.map(|s| format!("^{s}$").parse().unwrap()),
            replace_app_id: replace_app_id.map(option_string),
            replace_title: replace_title.map(option_string),
        };

        let replacement = filter.apply(app_id, title);
        let expect_replacement = match expect_replacement {
            None => FilterResult::Skip,
            Some((None, None)) => FilterResult::Match,
            Some((replace_app_id, replace_title)) => FilterResult::Replace(Replacement {
                replace_app_id: replace_app_id.map(Into::into),
                replace_title: replace_title.map(Into::into),
            }),
        };
        assert_eq!(expect_replacement, replacement);
    }
}
