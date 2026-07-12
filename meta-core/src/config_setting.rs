//! Declarative configuration settings for plugins and modules.
//!
//! A plugin describes the settings it understands by returning a list of
//! [`ConfigSetting`]s from [`crate::MetaPlugin::settings`]. The `meta config`
//! command aggregates these into a catalog so settings can be listed, read, and
//! written uniformly — instead of users hand-editing `.meta` and knowing each
//! block by heart. Values are stored under their dotted key in the workspace
//! config (e.g. `skill.dest`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The value type of a configurable setting. Drives CLI parsing, validation,
/// and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigValueType {
    /// A single string.
    String,
    /// A boolean (`true`/`false`).
    Bool,
    /// A signed integer.
    Integer,
    /// A list of strings (CLI input accepts a JSON array or a comma-separated
    /// list).
    StringList,
}

impl ConfigValueType {
    /// Short human label for help/listing output.
    pub fn label(self) -> &'static str {
        match self {
            ConfigValueType::String => "string",
            ConfigValueType::Bool => "bool",
            ConfigValueType::Integer => "int",
            ConfigValueType::StringList => "list",
        }
    }

    /// Inverse of [`label`](Self::label): recover a type from its short label.
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "string" => Some(ConfigValueType::String),
            "bool" => Some(ConfigValueType::Bool),
            "int" => Some(ConfigValueType::Integer),
            "list" => Some(ConfigValueType::StringList),
            _ => None,
        }
    }

    /// Parse a raw CLI string into a JSON value of this type. Returns a
    /// human-readable error string on mismatch so `meta config set` can reject
    /// bad input before writing.
    pub fn parse(self, raw: &str) -> Result<Value, String> {
        match self {
            ConfigValueType::String => Ok(Value::String(raw.to_string())),
            ConfigValueType::Bool => match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Ok(Value::Bool(true)),
                "false" | "0" | "no" | "off" => Ok(Value::Bool(false)),
                _ => Err(format!("expected a boolean (true/false), got '{}'", raw)),
            },
            ConfigValueType::Integer => raw
                .trim()
                .parse::<i64>()
                .map(|n| Value::Number(n.into()))
                .map_err(|_| format!("expected an integer, got '{}'", raw)),
            ConfigValueType::StringList => {
                let trimmed = raw.trim();
                if trimmed.starts_with('[') {
                    serde_json::from_str::<Vec<String>>(trimmed)
                        .map(|v| Value::Array(v.into_iter().map(Value::String).collect()))
                        .map_err(|_| format!("expected a JSON array of strings, got '{}'", raw))
                } else if trimmed.is_empty() {
                    Ok(Value::Array(Vec::new()))
                } else {
                    Ok(Value::Array(
                        trimmed
                            .split(',')
                            .map(|s| Value::String(s.trim().to_string()))
                            .collect(),
                    ))
                }
            }
        }
    }

    /// True if an existing JSON value already matches this type.
    pub fn matches(self, value: &Value) -> bool {
        match self {
            ConfigValueType::String => value.is_string(),
            ConfigValueType::Bool => value.is_boolean(),
            ConfigValueType::Integer => value.is_i64() || value.is_u64(),
            ConfigValueType::StringList => value
                .as_array()
                .map(|a| a.iter().all(Value::is_string))
                .unwrap_or(false),
        }
    }
}

/// A single setting a plugin or module declares as configurable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetting {
    /// Dotted key in the workspace config, e.g. `skill.dest`. The segment
    /// before the first `.` is the owning namespace (usually the plugin name).
    pub key: String,
    /// One-line description shown by `meta config list`.
    pub description: String,
    /// Default value (as a display string) used when the key is unset.
    pub default: Option<String>,
    /// Declared value type, used for validation and display.
    pub value_type: ConfigValueType,
    /// Environment variable that also controls this setting, if any. When set
    /// and currently present in the environment, `meta config list` notes that
    /// the env var overrides the configured value. Omitted for settings with no
    /// env equivalent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    /// Allowed values for a choice-constrained setting. When present, `meta
    /// config set` accepts only these values, and the TUI editor offers an
    /// inline cycle-picker instead of free-text entry. Used with
    /// `ConfigValueType::String`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
}

impl ConfigSetting {
    /// Declare a setting with a dotted `key`, `description`, and value type.
    pub fn new(
        key: impl Into<String>,
        description: impl Into<String>,
        value_type: ConfigValueType,
    ) -> Self {
        Self {
            key: key.into(),
            description: description.into(),
            default: None,
            value_type,
            env_var: None,
            choices: None,
        }
    }

    /// Attach a default value shown when the setting is unset.
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Attach the name of an environment variable that also controls this
    /// setting, so `meta config list` can flag when it is currently overriding
    /// the configured value.
    pub fn with_env(mut self, env_var: impl Into<String>) -> Self {
        self.env_var = Some(env_var.into());
        self
    }

    /// Constrain this setting to a fixed set of allowed values. `meta config
    /// set` then rejects anything outside the list, and the TUI editor offers an
    /// inline cycle-picker. Implies `ConfigValueType::String`.
    pub fn with_choices<I, S>(mut self, choices: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.choices = Some(choices.into_iter().map(Into::into).collect());
        self
    }

    /// Parse and validate `raw` against this setting's type and, if declared, its
    /// allowed `choices`. Returns the JSON value to store, or a message
    /// explaining why it was rejected.
    pub fn coerce(&self, raw: &str) -> Result<Value, String> {
        let value = self.value_type.parse(raw)?;
        if let Some(choices) = &self.choices {
            // Choice sets are string domains; compare on the string form.
            let s = match &value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if !choices.iter().any(|c| c == &s) {
                return Err(format!(
                    "'{}' is not an allowed value for '{}'. Choices: {}",
                    raw,
                    self.key,
                    choices.join(", ")
                ));
            }
        }
        Ok(value)
    }

    /// The namespace (segment before the first `.`), usually the plugin name.
    pub fn namespace(&self) -> &str {
        self.key.split('.').next().unwrap_or(&self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bool_parses_common_spellings() {
        assert_eq!(ConfigValueType::Bool.parse("true"), Ok(json!(true)));
        assert_eq!(ConfigValueType::Bool.parse("Off"), Ok(json!(false)));
        assert_eq!(ConfigValueType::Bool.parse("yes"), Ok(json!(true)));
        assert!(ConfigValueType::Bool.parse("maybe").is_err());
    }

    #[test]
    fn integer_parses_and_rejects() {
        assert_eq!(ConfigValueType::Integer.parse(" 42 "), Ok(json!(42)));
        assert!(ConfigValueType::Integer.parse("3.5").is_err());
    }

    #[test]
    fn coerce_without_choices_matches_parse() {
        let s = ConfigSetting::new("x", "d", ConfigValueType::Bool);
        assert_eq!(s.coerce("true"), Ok(json!(true)));
        assert!(s.coerce("maybe").is_err());
    }

    #[test]
    fn coerce_enforces_choices() {
        let s = ConfigSetting::new("mode", "d", ConfigValueType::String)
            .with_choices(["off", "required"]);
        assert_eq!(s.coerce("required"), Ok(json!("required")));
        assert_eq!(s.coerce("off"), Ok(json!("off")));

        let err = s.coerce("maybe").unwrap_err();
        assert!(
            err.contains("not an allowed value") && err.contains("off, required"),
            "error should list choices, got: {err}"
        );
    }

    #[test]
    fn list_accepts_comma_and_json() {
        assert_eq!(
            ConfigValueType::StringList.parse("a, b ,c"),
            Ok(json!(["a", "b", "c"]))
        );
        assert_eq!(
            ConfigValueType::StringList.parse(r#"["x","y"]"#),
            Ok(json!(["x", "y"]))
        );
        assert_eq!(ConfigValueType::StringList.parse(""), Ok(json!([])));
    }

    #[test]
    fn matches_checks_existing_type() {
        assert!(ConfigValueType::String.matches(&json!("s")));
        assert!(!ConfigValueType::String.matches(&json!(1)));
        assert!(ConfigValueType::StringList.matches(&json!(["a"])));
        assert!(!ConfigValueType::StringList.matches(&json!([1])));
    }

    #[test]
    fn namespace_is_first_segment() {
        let s = ConfigSetting::new("skill.dest", "d", ConfigValueType::String);
        assert_eq!(s.namespace(), "skill");
    }
}
