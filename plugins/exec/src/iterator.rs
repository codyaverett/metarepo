use metarepo_core::MetaConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    pub repo_url: String,
    pub exists: bool,
}

impl ProjectInfo {
    pub fn new(name: String, path: PathBuf, repo_url: String) -> Self {
        let exists = path.exists();
        Self {
            name,
            path,
            repo_url,
            exists,
        }
    }
    
    pub fn is_git_repo(&self) -> bool {
        if !self.exists {
            return false;
        }
        self.path.join(".git").exists()
    }
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
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.is_empty() {
                return true;
            }
            
            let mut current_pos = 0;
            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }
                
                if i == 0 && !pattern.starts_with('*') {
                    // Pattern doesn't start with *, so text must start with this part
                    if !text.starts_with(part) {
                        return false;
                    }
                    current_pos = part.len();
                } else if i == parts.len() - 1 && !pattern.ends_with('*') {
                    // Pattern doesn't end with *, so text must end with this part
                    if !text.ends_with(part) {
                        return false;
                    }
                } else {
                    // Find this part anywhere after current position
                    if let Some(pos) = text[current_pos..].find(part) {
                        current_pos += pos + part.len();
                    } else {
                        return false;
                    }
                }
            }
            
            true
        } else {
            // Exact match or substring match
            text == pattern || text.contains(pattern)
        }
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