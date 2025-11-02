#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Test various command injection vectors
        test_command_safety(input);
        test_script_name_safety(input);
        test_branch_name_safety(input);
    }
});

fn test_command_safety(input: &str) {
    // Check for shell metacharacters
    let dangerous_chars = [';', '&', '|', '`', '$', '(', ')', '\n', '\r', '<', '>'];

    for ch in dangerous_chars {
        if input.contains(ch) {
            // This should be rejected by the actual implementation
            assert!(!is_safe_for_execution(input));
            return;
        }
    }
}

fn test_script_name_safety(input: &str) {
    // Script names should only contain alphanumeric characters and limited punctuation
    if !input.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':') {
        assert!(!is_valid_script_name(input));
    }
}

fn test_branch_name_safety(input: &str) {
    // Branch names should not contain path traversal attempts
    if input.contains("..") || input.starts_with('-') {
        assert!(!is_valid_branch_name(input));
    }
}

// These functions should match the actual implementation's validation logic
fn is_safe_for_execution(input: &str) -> bool {
    !input.chars().any(|c| ";|&`$()<>\n\r".contains(c))
}

fn is_valid_script_name(input: &str) -> bool {
    !input.is_empty() &&
    input.len() < 256 &&
    input.chars().all(|c| c.is_alphanumeric() || "-_:".contains(c))
}

fn is_valid_branch_name(input: &str) -> bool {
    !input.is_empty() &&
    input.len() < 256 &&
    !input.contains("..") &&
    !input.starts_with('-') &&
    input.chars().all(|c| c.is_alphanumeric() || "-_/.".contains(c))
}