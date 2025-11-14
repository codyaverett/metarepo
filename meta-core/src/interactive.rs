use anyhow::{anyhow, Result};
use console::style;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use std::io::{self, IsTerminal};

/// Controls behavior when running in non-interactive mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonInteractiveMode {
    /// Fail with an error when required input is missing
    Fail,
    /// Use sensible defaults for missing inputs (only for optional args)
    Defaults,
}

impl std::str::FromStr for NonInteractiveMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "fail" => Ok(NonInteractiveMode::Fail),
            "defaults" => Ok(NonInteractiveMode::Defaults),
            other => Err(anyhow!(
                "Invalid non-interactive mode: '{}'. Use 'fail' or 'defaults'",
                other
            )),
        }
    }
}

/// Detects if we're running in an interactive TTY
pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Prompts for a required text input
///
/// # Arguments
/// * `prompt` - The prompt text to display
/// * `default` - Optional default value
/// * `allow_empty` - Whether to allow empty input
/// * `non_interactive` - How to behave when not in a TTY
///
/// # Examples
/// ```no_run
/// # use metarepo_core::interactive::*;
/// # fn main() -> anyhow::Result<()> {
/// let name = prompt_text(
///     "Project name",
///     None,
///     false,
///     NonInteractiveMode::Fail,
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn prompt_text(
    prompt: &str,
    default: Option<&str>,
    allow_empty: bool,
    non_interactive: NonInteractiveMode,
) -> Result<String> {
    if !is_interactive() {
        return handle_non_interactive(non_interactive, prompt, default.map(|s| s.to_string()));
    }

    loop {
        let mut input = Input::<String>::new();
        input = input.with_prompt(format!("{}", style(format!("→ {}", prompt)).cyan()));

        if let Some(default_val) = default {
            input = input.default(default_val.to_string());
        }

        match input.interact_text() {
            Ok(value) => {
                if !allow_empty && value.trim().is_empty() {
                    eprintln!("{}", style("  ✗ Input cannot be empty").red());
                    continue;
                }
                return Ok(value);
            }
            Err(e) if is_eof_error(&e) => {
                // Handle Ctrl+D or EOF
                return Err(anyhow!("Cancelled by user"));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Prompts for a URL input with optional validation
///
/// # Arguments
/// * `prompt` - The prompt text
/// * `default` - Optional default value
/// * `required` - Whether the input is required
/// * `non_interactive` - How to behave when not in a TTY
pub fn prompt_url(
    prompt: &str,
    default: Option<&str>,
    required: bool,
    non_interactive: NonInteractiveMode,
) -> Result<Option<String>> {
    if !is_interactive() {
        let result =
            handle_non_interactive(non_interactive, prompt, default.map(|s| s.to_string()));
        match result {
            Ok(val) => {
                if val.is_empty() && !required {
                    return Ok(None);
                }
                Ok(Some(val))
            }
            Err(e) => Err(e),
        }
    } else {
        let label = if required {
            prompt.to_string()
        } else {
            format!("{} (optional)", prompt)
        };

        loop {
            let mut input = Input::<String>::new();
            input = input.with_prompt(format!("{}", style(format!("→ {}", label)).cyan()));

            if let Some(default_val) = default {
                input = input.default(default_val.to_string());
            }

            match input.interact_text() {
                Ok(value) => {
                    if value.trim().is_empty() {
                        if !required {
                            return Ok(None);
                        }
                        eprintln!("{}", style("  ✗ URL cannot be empty").red());
                        continue;
                    }

                    // Basic URL validation
                    if !is_valid_url(&value) {
                        eprintln!(
                            "{}",
                            style(
                                "  ✗ Invalid URL format. Expected http(s)://, git@, or file path"
                            )
                            .red()
                        );
                        continue;
                    }

                    return Ok(Some(value));
                }
                Err(e) if is_eof_error(&e) => {
                    return Err(anyhow!("Cancelled by user"));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Prompts for a yes/no confirmation
///
/// # Arguments
/// * `prompt` - The prompt text
/// * `default` - Default value if user just presses enter
/// * `non_interactive` - How to behave when not in a TTY
pub fn prompt_confirm(
    prompt: &str,
    default: bool,
    non_interactive: NonInteractiveMode,
) -> Result<bool> {
    if !is_interactive() {
        match non_interactive {
            NonInteractiveMode::Fail => {
                Err(anyhow!(
                    "Interactive confirmation required for: '{}'. Use --non-interactive=defaults or provide --force",
                    prompt
                ))
            }
            NonInteractiveMode::Defaults => Ok(default),
        }
    } else {
        let confirm = Confirm::new()
            .with_prompt(format!("{}", style(format!("→ {}", prompt)).cyan()))
            .default(default)
            .interact_opt()?;

        match confirm {
            Some(value) => Ok(value),
            None => Err(anyhow!("Cancelled by user")),
        }
    }
}

/// Prompts for a single selection from a list
///
/// # Arguments
/// * `prompt` - The prompt text
/// * `items` - List of items to choose from
/// * `default_index` - Index of the default selection
/// * `non_interactive` - How to behave when not in a TTY
pub fn prompt_select<S: Into<String>>(
    prompt: &str,
    items: Vec<S>,
    default_index: Option<usize>,
    non_interactive: NonInteractiveMode,
) -> Result<String> {
    let items: Vec<String> = items.into_iter().map(|s| s.into()).collect();

    if !is_interactive() {
        return handle_non_interactive_select(non_interactive, prompt, &items, default_index);
    }

    if items.is_empty() {
        return Err(anyhow!("No items to select from"));
    }

    let select = Select::new();
    let select = select.with_prompt(format!("{}", style(format!("→ {}", prompt)).cyan()));

    let mut select_with_items = select;
    for item in &items {
        select_with_items = select_with_items.item(item);
    }

    if let Some(idx) = default_index {
        if idx < items.len() {
            select_with_items = select_with_items.default(idx);
        }
    }

    match select_with_items.interact_opt()? {
        Some(idx) => Ok(items[idx].clone()),
        None => Err(anyhow!("Cancelled by user")),
    }
}

/// Prompts for multiple selections from a list
///
/// # Arguments
/// * `prompt` - The prompt text
/// * `items` - List of items to choose from
/// * `defaults` - Indices of default selections
/// * `non_interactive` - How to behave when not in a TTY
pub fn prompt_multiselect<S: Into<String>>(
    prompt: &str,
    items: Vec<S>,
    defaults: Vec<usize>,
    non_interactive: NonInteractiveMode,
) -> Result<Vec<String>> {
    let items: Vec<String> = items.into_iter().map(|s| s.into()).collect();

    if !is_interactive() {
        return handle_non_interactive_multiselect(non_interactive, prompt, &items, defaults);
    }

    if items.is_empty() {
        return Err(anyhow!("No items to select from"));
    }

    let select = MultiSelect::new();
    let select = select.with_prompt(format!("{}", style(format!("→ {}", prompt)).cyan()));

    let mut select_with_items = select;
    for item in &items {
        select_with_items = select_with_items.item(item);
    }

    for idx in defaults {
        if idx < items.len() {
            select_with_items = select_with_items.item_checked(idx, true);
        }
    }

    match select_with_items.interact_opt()? {
        Some(indices) => {
            if indices.is_empty() {
                Err(anyhow!("At least one item must be selected"))
            } else {
                Ok(indices.into_iter().map(|i| items[i].clone()).collect())
            }
        }
        None => Err(anyhow!("Cancelled by user")),
    }
}

// ============================================================================
// Private helper functions
// ============================================================================

/// Handles input when not in interactive mode
fn handle_non_interactive(
    mode: NonInteractiveMode,
    prompt: &str,
    default: Option<String>,
) -> Result<String> {
    match mode {
        NonInteractiveMode::Fail => {
            Err(anyhow!(
                "Interactive input required for '{}' and no default provided. Use --non-interactive=defaults or provide the value explicitly",
                prompt
            ))
        }
        NonInteractiveMode::Defaults => {
            default.ok_or_else(|| anyhow!(
                "No default value available for '{}' in non-interactive mode",
                prompt
            ))
        }
    }
}

/// Handles selection when not in interactive mode
fn handle_non_interactive_select(
    mode: NonInteractiveMode,
    prompt: &str,
    items: &[String],
    default: Option<usize>,
) -> Result<String> {
    match mode {
        NonInteractiveMode::Fail => {
            Err(anyhow!(
                "Interactive selection required for '{}'. Use --non-interactive=defaults or provide the value explicitly",
                prompt
            ))
        }
        NonInteractiveMode::Defaults => {
            let idx = default.ok_or_else(|| anyhow!(
                "No default selection available for '{}' in non-interactive mode",
                prompt
            ))?;
            Ok(items
                .get(idx)
                .ok_or_else(|| anyhow!("Default index {} out of range", idx))?
                .clone())
        }
    }
}

/// Handles multi-select when not in interactive mode
fn handle_non_interactive_multiselect(
    mode: NonInteractiveMode,
    prompt: &str,
    items: &[String],
    defaults: Vec<usize>,
) -> Result<Vec<String>> {
    match mode {
        NonInteractiveMode::Fail => {
            Err(anyhow!(
                "Interactive selection required for '{}'. Use --non-interactive=defaults or provide values explicitly",
                prompt
            ))
        }
        NonInteractiveMode::Defaults => {
            if defaults.is_empty() {
                Err(anyhow!(
                    "No default selection available for '{}' in non-interactive mode",
                    prompt
                ))
            } else {
                Ok(defaults
                    .iter()
                    .map(|idx| {
                        items
                            .get(*idx)
                            .ok_or_else(|| anyhow!("Default index {} out of range", idx)).cloned()
                    })
                    .collect::<Result<Vec<_>>>()?)
            }
        }
    }
}

/// Validates a URL format (basic check)
fn is_valid_url(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("git@")
        || url.starts_with("./")
        || url.starts_with("../")
        || url.starts_with("/")
        || !url.contains(' ')
}

/// Checks if an error is due to EOF (Ctrl+D)
fn is_eof_error(error: &dyn std::error::Error) -> bool {
    error.to_string().contains("EOF")
        || error.to_string().contains("end of file")
        || error.to_string().contains("Ctrl+D")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_interactive_mode_parsing() {
        assert_eq!(
            "fail".parse::<NonInteractiveMode>().unwrap(),
            NonInteractiveMode::Fail
        );
        assert_eq!(
            "defaults".parse::<NonInteractiveMode>().unwrap(),
            NonInteractiveMode::Defaults
        );
        assert_eq!(
            "FAIL".parse::<NonInteractiveMode>().unwrap(),
            NonInteractiveMode::Fail
        );
        assert!("invalid".parse::<NonInteractiveMode>().is_err());
    }

    #[test]
    fn test_valid_url() {
        assert!(is_valid_url("https://github.com/user/repo.git"));
        assert!(is_valid_url("http://example.com"));
        assert!(is_valid_url("git@github.com:user/repo.git"));
        assert!(is_valid_url("./local/path"));
        assert!(is_valid_url("../relative/path"));
        assert!(is_valid_url("/absolute/path"));
        assert!(!is_valid_url("invalid url with spaces"));
    }

    #[test]
    fn test_handle_non_interactive_fail() {
        let result = handle_non_interactive(NonInteractiveMode::Fail, "test", None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Interactive input required"));
    }

    #[test]
    fn test_handle_non_interactive_defaults() {
        let result = handle_non_interactive(
            NonInteractiveMode::Defaults,
            "test",
            Some("default_value".to_string()),
        );
        assert_eq!(result.unwrap(), "default_value");
    }

    #[test]
    fn test_handle_non_interactive_defaults_no_default() {
        let result = handle_non_interactive(NonInteractiveMode::Defaults, "test", None);
        assert!(result.is_err());
    }
}
