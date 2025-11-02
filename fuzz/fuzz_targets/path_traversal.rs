#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Path, PathBuf};

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        test_path_traversal_protection(input);
    }
});

fn test_path_traversal_protection(input: &str) {
    // Test that path validation prevents escaping the base directory
    let base = Path::new("/tmp/test_workspace");

    // Any input containing these patterns should be rejected
    let dangerous_patterns = [
        "..",
        "../",
        "..\\",
        "%2e%2e",
        "%252e%252e",
        "..;",
        "..",
        "..%00",
    ];

    for pattern in dangerous_patterns {
        if input.contains(pattern) {
            assert!(!is_safe_path(base, input));
            return;
        }
    }

    // Absolute paths should be rejected
    if input.starts_with('/') || input.starts_with('\\') {
        assert!(!is_safe_path(base, input));
        return;
    }

    // URL schemes should be rejected
    if input.contains("://") {
        assert!(!is_safe_path(base, input));
        return;
    }

    // Null bytes should be rejected
    if input.contains('\x00') {
        assert!(!is_safe_path(base, input));
        return;
    }
}

fn is_safe_path(base: &Path, untrusted: &str) -> bool {
    // Check for obvious dangerous patterns
    if untrusted.contains("..") ||
       untrusted.starts_with('/') ||
       untrusted.starts_with('\\') ||
       untrusted.contains('\x00') ||
       untrusted.contains("://") {
        return false;
    }

    // Try to resolve the path
    let joined = base.join(untrusted);

    // In a real implementation, we would canonicalize and check
    // For fuzzing, we just check basic safety
    match joined.to_str() {
        Some(path_str) => {
            // The resolved path should start with the base path
            path_str.starts_with(base.to_str().unwrap_or("/tmp/test_workspace"))
        }
        None => false,
    }
}