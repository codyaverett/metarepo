use crate::ProjectInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub projects: HashMap<String, String>,
    #[serde(default)]
    pub plugins: Option<HashMap<String, String>>,
}

pub struct ProjectIterator {
    projects: Vec<ProjectInfo>,
    current: usize,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl ProjectIterator {
    pub fn new(config: &MetaConfig, base_path: &Path) -> Self {
        let mut projects = Vec::new();
        
        for (path_str, repo_url) in &config.projects {
            let path = base_path.join(path_str);
            let name = path_str.clone();
            projects.push(ProjectInfo::new(name, path, repo_url.clone()));
        }
        
        Self {
            projects,
            current: 0,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }
    
    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }
    
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }
    
    pub fn filter_existing(mut self) -> Self {
        self.projects.retain(|p| p.exists);
        self
    }
    
    pub fn filter_git_repos(mut self) -> Self {
        self.projects.retain(|p| p.is_git_repo());
        self
    }
    
    fn matches_patterns(&self, project: &ProjectInfo) -> bool {
        // If include patterns are specified, project must match at least one
        if !self.include_patterns.is_empty() {
            let matches_include = self.include_patterns.iter().any(|pattern| {
                self.matches_pattern(&project.name, pattern) || 
                self.matches_pattern(&project.path.to_string_lossy(), pattern)
            });
            if !matches_include {
                return false;
            }
        }
        
        // Project must not match any exclude patterns
        if !self.exclude_patterns.is_empty() {
            let matches_exclude = self.exclude_patterns.iter().any(|pattern| {
                self.matches_pattern(&project.name, pattern) || 
                self.matches_pattern(&project.path.to_string_lossy(), pattern)
            });
            if matches_exclude {
                return false;
            }
        }
        
        true
    }
    
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        // Simple pattern matching - can be enhanced with glob patterns later
        if pattern.contains('*') {
            // Basic wildcard support
            let pattern = pattern.replace('*', ".*");
            if let Ok(regex) = regex::Regex::new(&pattern) {
                return regex.is_match(text);
            }
        }
        
        // Exact match or substring match
        text == pattern || text.contains(pattern)
    }
    
    pub fn collect_all(self) -> Vec<ProjectInfo> {
        self.collect()
    }
    
    pub fn count(&self) -> usize {
        self.projects.iter().filter(|p| self.matches_patterns(p)).count()
    }
}

impl Iterator for ProjectIterator {
    type Item = ProjectInfo;
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.projects.len() {
            let project = &self.projects[self.current];
            self.current += 1;
            
            if self.matches_patterns(project) {
                return Some(project.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;
    
    #[test]
    fn test_project_iterator() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path();
        
        // Create test directory structure
        std::fs::create_dir_all(base_path.join("app1")).unwrap();
        std::fs::create_dir_all(base_path.join("service1")).unwrap();
        
        let mut projects = HashMap::new();
        projects.insert("app1".to_string(), "https://github.com/test/app1.git".to_string());
        projects.insert("service1".to_string(), "https://github.com/test/service1.git".to_string());
        projects.insert("missing".to_string(), "https://github.com/test/missing.git".to_string());
        
        let config = MetaConfig {
            ignore: vec![],
            projects,
            plugins: None,
        };
        
        let iterator = ProjectIterator::new(&config, base_path);
        let all_projects: Vec<_> = iterator.collect();
        
        assert_eq!(all_projects.len(), 3);
        
        // Test filtering existing projects
        let iterator = ProjectIterator::new(&config, base_path).filter_existing();
        let existing_projects: Vec<_> = iterator.collect();
        
        assert_eq!(existing_projects.len(), 2);
        assert!(existing_projects.iter().all(|p| p.exists));
    }
    
    #[test]
    fn test_pattern_matching() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path();
        
        let mut projects = HashMap::new();
        projects.insert("app1".to_string(), "https://github.com/test/app1.git".to_string());
        projects.insert("service1".to_string(), "https://github.com/test/service1.git".to_string());
        projects.insert("app2".to_string(), "https://github.com/test/app2.git".to_string());
        
        let config = MetaConfig {
            ignore: vec![],
            projects,
            plugins: None,
        };
        
        // Test include patterns
        let iterator = ProjectIterator::new(&config, base_path)
            .with_include_patterns(vec!["app".to_string()]);
        let filtered: Vec<_> = iterator.collect();
        
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.name.contains("app")));
        
        // Test exclude patterns
        let iterator = ProjectIterator::new(&config, base_path)
            .with_exclude_patterns(vec!["service".to_string()]);
        let filtered: Vec<_> = iterator.collect();
        
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| !p.name.contains("service")));
    }
}