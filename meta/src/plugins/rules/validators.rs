use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

pub trait Validator {
    fn validate(&self, path: &Path) -> Result<bool>;
    fn name(&self) -> &str;
}

pub struct PatternValidator {
    pattern: Regex,
    name: String,
}

impl PatternValidator {
    pub fn new(pattern: &str, name: &str) -> Result<Self> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            name: name.to_string(),
        })
    }
}

impl Validator for PatternValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if let Some(path_str) = path.to_str() {
            Ok(self.pattern.is_match(path_str))
        } else {
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct FileExistsValidator {
    name: String,
}

impl FileExistsValidator {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Validator for FileExistsValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        Ok(path.exists() && path.is_file())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct DirectoryExistsValidator {
    name: String,
}

impl DirectoryExistsValidator {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Validator for DirectoryExistsValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        Ok(path.exists() && path.is_dir())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct ContentValidator {
    pattern: Regex,
    name: String,
}

impl ContentValidator {
    pub fn new(pattern: &str, name: &str) -> Result<Self> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            name: name.to_string(),
        })
    }

    pub fn validate_content(&self, content: &str) -> bool {
        self.pattern.is_match(content)
    }
}

impl Validator for ContentValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if path.exists() && path.is_file() {
            let content = std::fs::read_to_string(path)?;
            Ok(self.validate_content(&content))
        } else {
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct NamingValidator {
    pattern: Regex,
    naming_pattern: Regex,
    name: String,
}

impl NamingValidator {
    pub fn new(pattern: &str, naming_pattern: &str, name: &str) -> Result<Self> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            naming_pattern: Regex::new(naming_pattern)?,
            name: name.to_string(),
        })
    }

    pub fn validate_name(&self, name: &str) -> bool {
        self.naming_pattern.is_match(name)
    }
}

impl Validator for NamingValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if let Some(path_str) = path.to_str() {
            if self.pattern.is_match(path_str) {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    return Ok(self.validate_name(file_name));
                }
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct SizeValidator {
    max_lines: Option<usize>,
    max_bytes: Option<usize>,
    name: String,
}

impl SizeValidator {
    pub fn new(max_lines: Option<usize>, max_bytes: Option<usize>, name: &str) -> Self {
        Self {
            max_lines,
            max_bytes,
            name: name.to_string(),
        }
    }

    pub fn count_lines(content: &str) -> usize {
        content.lines().count()
    }
}

impl Validator for SizeValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if path.exists() && path.is_file() {
            if let Some(max_bytes) = self.max_bytes {
                let metadata = fs::metadata(path)?;
                if metadata.len() as usize > max_bytes {
                    return Ok(false);
                }
            }

            if let Some(max_lines) = self.max_lines {
                let content = fs::read_to_string(path)?;
                if Self::count_lines(&content) > max_lines {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct SecurityValidator {
    forbidden_patterns: Vec<Regex>,
    forbidden_functions: Vec<String>,
    require_https: bool,
    name: String,
}

impl SecurityValidator {
    pub fn new(
        forbidden_patterns: Vec<String>,
        forbidden_functions: Vec<String>,
        require_https: bool,
        name: &str,
    ) -> Result<Self> {
        let patterns = forbidden_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            forbidden_patterns: patterns,
            forbidden_functions,
            require_https,
            name: name.to_string(),
        })
    }

    pub fn check_content(&self, content: &str) -> Vec<String> {
        let mut violations = Vec::new();

        for pattern in &self.forbidden_patterns {
            if pattern.is_match(content) {
                violations.push(format!("Found forbidden pattern: {:?}", pattern.as_str()));
            }
        }

        for func in &self.forbidden_functions {
            if content.contains(func) {
                violations.push(format!("Found forbidden function: {}", func));
            }
        }

        if self.require_https && content.contains("http://") {
            violations.push("Found non-HTTPS URL".to_string());
        }

        violations
    }
}

impl Validator for SecurityValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if path.exists() && path.is_file() {
            let content = fs::read_to_string(path)?;
            let violations = self.check_content(&content);
            return Ok(violations.is_empty());
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct DependencyValidator {
    forbidden: Vec<String>,
    required: Vec<(String, String)>,
    name: String,
}

impl DependencyValidator {
    pub fn new(forbidden: Vec<String>, required: Vec<(String, String)>, name: &str) -> Self {
        Self {
            forbidden,
            required,
            name: name.to_string(),
        }
    }

    pub fn check_package_json(&self, content: &str) -> Result<Vec<String>> {
        let mut violations = Vec::new();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            let deps = json["dependencies"].as_object();
            let dev_deps = json["devDependencies"].as_object();

            for forbidden_pkg in &self.forbidden {
                if deps.map_or(false, |d| d.contains_key(forbidden_pkg))
                    || dev_deps.map_or(false, |d| d.contains_key(forbidden_pkg))
                {
                    violations.push(format!("Forbidden dependency: {}", forbidden_pkg));
                }
            }

            for (pkg, _version) in &self.required {
                if !deps.map_or(false, |d| d.contains_key(pkg))
                    && !dev_deps.map_or(false, |d| d.contains_key(pkg))
                {
                    violations.push(format!("Missing required dependency: {}", pkg));
                }
            }
        }

        Ok(violations)
    }
}

impl Validator for DependencyValidator {
    fn validate(&self, path: &Path) -> Result<bool> {
        if path.file_name() == Some(std::ffi::OsStr::new("package.json")) {
            let content = fs::read_to_string(path)?;
            let violations = self.check_package_json(&content)?;
            return Ok(violations.is_empty());
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_pattern_validator() {
        let validator = PatternValidator::new(r".*\.rs$", "rust_file").unwrap();

        assert!(validator.validate(Path::new("test.rs")).unwrap());
        assert!(!validator.validate(Path::new("test.js")).unwrap());
    }

    #[test]
    fn test_file_exists_validator() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let validator = FileExistsValidator::new("file_exists");

        assert!(validator.validate(&file_path).unwrap());
        assert!(!validator.validate(Path::new("nonexistent.txt")).unwrap());
    }

    #[test]
    fn test_directory_exists_validator() {
        let temp = tempdir().unwrap();
        let dir_path = temp.path().join("test_dir");
        fs::create_dir(&dir_path).unwrap();

        let validator = DirectoryExistsValidator::new("dir_exists");

        assert!(validator.validate(&dir_path).unwrap());
        assert!(!validator.validate(Path::new("nonexistent_dir")).unwrap());
    }

    #[test]
    fn test_content_validator() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.rs");
        fs::write(&file_path, "#[test]\nfn test_function() {}").unwrap();

        let validator = ContentValidator::new(r"#\[test\]", "has_test").unwrap();

        assert!(validator.validate(&file_path).unwrap());

        let file_path2 = temp.path().join("test2.rs");
        fs::write(&file_path2, "fn regular_function() {}").unwrap();

        assert!(!validator.validate(&file_path2).unwrap());
    }

    #[test]
    fn test_naming_validator() {
        let validator =
            NamingValidator::new(r".*\.tsx$", r"^[A-Z][a-zA-Z0-9]+\.tsx$", "react_component")
                .unwrap();

        assert!(validator.validate(Path::new("ComponentName.tsx")).unwrap());
        assert!(!validator.validate(Path::new("componentName.tsx")).unwrap());
        assert!(validator.validate(Path::new("test.js")).unwrap()); // Not matching pattern, so OK
    }

    #[test]
    fn test_size_validator() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.txt");

        let content = "line1\nline2\nline3\nline4\nline5";
        fs::write(&file_path, content).unwrap();

        let validator = SizeValidator::new(Some(3), None, "max_lines");
        assert!(!validator.validate(&file_path).unwrap());

        let validator = SizeValidator::new(Some(10), None, "max_lines");
        assert!(validator.validate(&file_path).unwrap());

        let validator = SizeValidator::new(None, Some(10), "max_bytes");
        assert!(!validator.validate(&file_path).unwrap());

        let validator = SizeValidator::new(None, Some(100), "max_bytes");
        assert!(validator.validate(&file_path).unwrap());
    }

    #[test]
    fn test_security_validator() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.js");

        let content = "const apiKey = 'secret123';\neval('dangerous');";
        fs::write(&file_path, content).unwrap();

        let validator = SecurityValidator::new(
            vec!["apiKey.*=.*['\"]".to_string()],
            vec!["eval".to_string()],
            true,
            "security",
        )
        .unwrap();

        assert!(!validator.validate(&file_path).unwrap());

        let safe_content = "const data = fetchData();";
        let safe_path = temp.path().join("safe.js");
        fs::write(&safe_path, safe_content).unwrap();

        assert!(validator.validate(&safe_path).unwrap());
    }
}
