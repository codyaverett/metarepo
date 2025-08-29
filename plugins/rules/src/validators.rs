use anyhow::Result;
use regex::Regex;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    
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
}