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

        for (path_str, _entry) in &config.projects {
            let path = base_path.join(path_str);
            let name = path_str.clone();
            let repo_url = config
                .get_project_url(path_str)
                .unwrap_or_else(|| "local".to_string());
            projects.push(ProjectInfo::new(name, path, repo_url));
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
                self.matches_pattern(&project.name, pattern)
                    || self.matches_pattern(&project.path.to_string_lossy(), pattern)
            });
            if !matches_include {
                return false;
            }
        }

        // Project must not match any exclude patterns
        if !self.exclude_patterns.is_empty() {
            let matches_exclude = self.exclude_patterns.iter().any(|pattern| {
                self.matches_pattern(&project.name, pattern)
                    || self.matches_pattern(&project.path.to_string_lossy(), pattern)
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
        self.projects
            .iter()
            .filter(|p| self.matches_patterns(p))
            .count()
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
    use std::fs;
    use tempfile::tempdir;

    fn create_test_config() -> MetaConfig {
        let mut config = MetaConfig::default();
        use metarepo_core::ProjectEntry;
        config.projects.insert(
            "project-a".to_string(),
            ProjectEntry::Url("https://github.com/user/project-a.git".to_string()),
        );
        config.projects.insert(
            "project-b".to_string(),
            ProjectEntry::Url("https://github.com/user/project-b.git".to_string()),
        );
        config.projects.insert(
            "lib-core".to_string(),
            ProjectEntry::Url("https://github.com/user/lib-core.git".to_string()),
        );
        config.projects.insert(
            "lib-utils".to_string(),
            ProjectEntry::Url("https://github.com/user/lib-utils.git".to_string()),
        );
        config.projects.insert(
            "test-project".to_string(),
            ProjectEntry::Url("https://github.com/user/test-project.git".to_string()),
        );
        config
    }

    #[test]
    fn test_project_info_new() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().join("project");
        fs::create_dir(&project_path).unwrap();

        let info = ProjectInfo::new(
            "project".to_string(),
            project_path.clone(),
            "https://github.com/user/repo.git".to_string(),
        );

        assert_eq!(info.name, "project");
        assert_eq!(info.path, project_path);
        assert_eq!(info.repo_url, "https://github.com/user/repo.git");
        assert!(info.exists);
    }

    #[test]
    fn test_project_info_is_git_repo() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().join("project");
        fs::create_dir(&project_path).unwrap();

        let mut info = ProjectInfo::new(
            "project".to_string(),
            project_path.clone(),
            "https://github.com/user/repo.git".to_string(),
        );

        // Initially not a git repo
        assert!(!info.is_git_repo());

        // Create .git directory
        fs::create_dir(project_path.join(".git")).unwrap();

        // Update info to check again
        info = ProjectInfo::new(
            "project".to_string(),
            project_path.clone(),
            "https://github.com/user/repo.git".to_string(),
        );
        assert!(info.is_git_repo());
    }

    #[test]
    fn test_project_iterator_basic() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        let iterator = ProjectIterator::new(&config, temp_dir.path());
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 5);

        // Check that all expected projects are present (order is not guaranteed with HashMap)
        let project_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
        assert!(project_names.contains(&"project-a".to_string()));
        assert!(project_names.contains(&"project-b".to_string()));
        assert!(project_names.contains(&"lib-core".to_string()));
        assert!(project_names.contains(&"lib-utils".to_string()));
        assert!(project_names.contains(&"test-project".to_string()));
    }

    #[test]
    fn test_project_iterator_with_include_patterns() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        // Include only projects starting with "lib"
        let iterator = ProjectIterator::new(&config, temp_dir.path())
            .with_include_patterns(vec!["lib*".to_string()]);
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 2);

        // Check that the correct projects are present (order is not guaranteed)
        let project_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
        assert!(project_names.contains(&"lib-core".to_string()));
        assert!(project_names.contains(&"lib-utils".to_string()));
    }

    #[test]
    fn test_project_iterator_with_exclude_patterns() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        // Exclude test projects
        let iterator = ProjectIterator::new(&config, temp_dir.path())
            .with_exclude_patterns(vec!["test*".to_string()]);
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 4);
        assert!(!projects.iter().any(|p| p.name == "test-project"));
    }

    #[test]
    fn test_project_iterator_filter_existing() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        // Create only some project directories
        fs::create_dir(temp_dir.path().join("project-a")).unwrap();
        fs::create_dir(temp_dir.path().join("lib-core")).unwrap();

        let iterator = ProjectIterator::new(&config, temp_dir.path()).filter_existing();
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 2);

        // Check that the correct projects are present (order is not guaranteed)
        let project_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
        assert!(project_names.contains(&"project-a".to_string()));
        assert!(project_names.contains(&"lib-core".to_string()));
    }

    #[test]
    fn test_project_iterator_filter_git_repos() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        // Create some project directories with .git folders
        let project_a = temp_dir.path().join("project-a");
        fs::create_dir(&project_a).unwrap();
        fs::create_dir(project_a.join(".git")).unwrap();

        let lib_core = temp_dir.path().join("lib-core");
        fs::create_dir(&lib_core).unwrap();
        fs::create_dir(lib_core.join(".git")).unwrap();

        // Create a directory without .git
        fs::create_dir(temp_dir.path().join("project-b")).unwrap();

        let iterator = ProjectIterator::new(&config, temp_dir.path()).filter_git_repos();
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 2);

        // Check that the correct projects are present (order is not guaranteed)
        let project_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
        assert!(project_names.contains(&"project-a".to_string()));
        assert!(project_names.contains(&"lib-core".to_string()));
    }

    #[test]
    fn test_matches_pattern_exact() {
        let temp_dir = tempdir().unwrap();
        let config = MetaConfig::default();
        let iterator = ProjectIterator::new(&config, temp_dir.path());

        assert!(iterator.matches_pattern("project", "project"));
        assert!(!iterator.matches_pattern("project", "other"));
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        let temp_dir = tempdir().unwrap();
        let config = MetaConfig::default();
        let iterator = ProjectIterator::new(&config, temp_dir.path());

        // Start wildcard
        assert!(iterator.matches_pattern("project-name", "*name"));
        assert!(iterator.matches_pattern("test-name", "*name"));
        assert!(!iterator.matches_pattern("project-other", "*name"));

        // End wildcard
        assert!(iterator.matches_pattern("project-name", "project*"));
        assert!(iterator.matches_pattern("project-test", "project*"));
        assert!(!iterator.matches_pattern("other-project", "project*"));

        // Middle wildcard
        assert!(iterator.matches_pattern("project-test-name", "project*name"));
        assert!(iterator.matches_pattern("project-name", "project*name"));
        assert!(!iterator.matches_pattern("other-test-name", "project*name"));

        // Multiple wildcards
        assert!(iterator.matches_pattern("lib-core-utils", "lib*core*"));
        assert!(iterator.matches_pattern("lib-test-core-main", "lib*core*"));
        assert!(!iterator.matches_pattern("lib-test-main", "lib*core*"));
    }

    #[test]
    fn test_matches_pattern_substring() {
        let temp_dir = tempdir().unwrap();
        let config = MetaConfig::default();
        let iterator = ProjectIterator::new(&config, temp_dir.path());

        // Substring matching when no wildcard
        assert!(iterator.matches_pattern("my-project-name", "project"));
        assert!(iterator.matches_pattern("project", "project"));
        assert!(!iterator.matches_pattern("other", "project"));
    }

    #[test]
    fn test_combined_include_exclude_patterns() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        // Include lib* but exclude *utils
        let iterator = ProjectIterator::new(&config, temp_dir.path())
            .with_include_patterns(vec!["lib*".to_string()])
            .with_exclude_patterns(vec!["*utils".to_string()]);
        let projects: Vec<ProjectInfo> = iterator.collect();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "lib-core");
    }

    #[test]
    fn test_iterator_count() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config();

        let iterator = ProjectIterator::new(&config, temp_dir.path());
        assert_eq!(iterator.count(), 5);

        let iterator = ProjectIterator::new(&config, temp_dir.path())
            .with_include_patterns(vec!["project*".to_string()]);
        assert_eq!(iterator.count(), 2);
    }
}
