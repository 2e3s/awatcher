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

#[derive(Default)]
pub struct Replacement {
    pub replace_app_id: Option<String>,
    pub replace_title: Option<String>,
}

impl Filter {
    fn is_valid(&self) -> bool {
        let is_match_set = self.match_app_id.is_some() || self.match_title.is_some();
        let is_replacement_set = self.replace_app_id.is_some() || self.replace_title.is_some();

        is_match_set && is_replacement_set
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

    pub fn replacement(&self, app_id: &str, title: &str) -> Option<Replacement> {
        if !self.is_valid() || !self.is_match(app_id, title) {
            return None;
        }

        let mut replacement = Replacement::default();
        if let Some(new_app_id) = &self.replace_app_id {
            replacement.replace_app_id =
                Some(Self::replace(&self.match_app_id, app_id, new_app_id));
        }
        if let Some(new_title) = &self.replace_title {
            replacement.replace_title = Some(Self::replace(&self.match_title, title, new_title));
        }
        Some(replacement)
    }
}
