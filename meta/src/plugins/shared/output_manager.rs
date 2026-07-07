use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ProjectOutput {
    pub name: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub duration: Option<Duration>,
    pub status: JobStatus,
    pub start_time: Option<Instant>,
    pub command: Option<String>,
}

impl ProjectOutput {
    pub fn new(name: String) -> Self {
        Self {
            name,
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: None,
            duration: None,
            status: JobStatus::Pending,
            start_time: None,
            command: None,
        }
    }

    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.start_time = Some(Instant::now());
    }

    pub fn complete(&mut self, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>) {
        if let Some(start_time) = self.start_time {
            self.duration = Some(start_time.elapsed());
        }
        self.exit_code = Some(exit_code);
        self.stdout = stdout;
        self.stderr = stderr;
        self.status = if exit_code == 0 {
            JobStatus::Completed
        } else {
            JobStatus::Failed
        };
    }
}

pub struct OutputManager {
    outputs: Arc<Mutex<HashMap<String, ProjectOutput>>>,
    project_order: Vec<String>,
    total_projects: usize,
    start_time: Instant,
}

impl OutputManager {
    pub fn new(project_names: Vec<String>) -> Self {
        let mut outputs = HashMap::new();
        for name in &project_names {
            outputs.insert(name.clone(), ProjectOutput::new(name.clone()));
        }

        Self {
            outputs: Arc::new(Mutex::new(outputs)),
            project_order: project_names.clone(),
            total_projects: project_names.len(),
            start_time: Instant::now(),
        }
    }

    pub fn get_project_output(&self, name: &str) -> Option<ProjectOutput> {
        self.outputs.lock().unwrap().get(name).cloned()
    }

    pub fn start_project(&self, name: &str) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            output.start();
        }
    }

    pub fn set_project_command(&self, name: &str, command: String) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            output.command = Some(command);
        }
    }

    pub fn complete_project(&self, name: &str, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            output.complete(exit_code, stdout, stderr);
        }
    }

    /// Append a chunk of streamed stdout for a running project. Used by the live
    /// run view, which reads child output incrementally instead of receiving it
    /// all at once via [`complete_project`].
    pub fn append_stdout(&self, name: &str, chunk: &[u8]) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            output.stdout.extend_from_slice(chunk);
        }
    }

    /// Append a chunk of streamed stderr for a running project.
    pub fn append_stderr(&self, name: &str, chunk: &[u8]) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            output.stderr.extend_from_slice(chunk);
        }
    }

    /// Mark a streamed project finished, recording its exit code and duration
    /// without replacing the buffers already filled by
    /// [`append_stdout`]/[`append_stderr`]. Counterpart to [`complete_project`]
    /// for the streaming path.
    pub fn finish_project(&self, name: &str, exit_code: i32) {
        if let Some(output) = self.outputs.lock().unwrap().get_mut(name) {
            if let Some(start_time) = output.start_time {
                output.duration = Some(start_time.elapsed());
            }
            output.exit_code = Some(exit_code);
            output.status = if exit_code == 0 {
                JobStatus::Completed
            } else {
                JobStatus::Failed
            };
        }
    }

    pub fn get_status_summary(&self) -> (usize, usize, usize) {
        let outputs = self.outputs.lock().unwrap();
        let mut completed = 0;
        let mut running = 0;
        let mut failed = 0;

        for output in outputs.values() {
            match output.status {
                JobStatus::Completed => completed += 1,
                JobStatus::Running => running += 1,
                JobStatus::Failed => failed += 1,
                JobStatus::Pending => {}
            }
        }

        (completed + failed, running, failed)
    }

    pub fn all_completed(&self) -> bool {
        let outputs = self.outputs.lock().unwrap();
        outputs
            .values()
            .all(|o| matches!(o.status, JobStatus::Completed | JobStatus::Failed))
    }

    pub fn display_final_results(&self) {
        let outputs = self.outputs.lock().unwrap();
        let total_duration = self.start_time.elapsed();

        // Clear progress line and print completion message
        print!("\r\x1b[K");
        println!(
            "✓ All projects completed in {:.1}s\n",
            total_duration.as_secs_f32()
        );

        let mut success_count = 0;
        let mut failed_projects = Vec::new();

        // Display results in original order
        for project_name in &self.project_order {
            if let Some(output) = outputs.get(project_name) {
                self.display_project_result(output);

                match output.status {
                    JobStatus::Completed => success_count += 1,
                    JobStatus::Failed => failed_projects.push(project_name.clone()),
                    _ => {}
                }
            }
        }

        // Summary
        println!("\n  {}", "─".repeat(60).bright_black());
        println!(
            "  {} {} completed, {} failed",
            "Summary:".bright_black(),
            success_count.to_string().green(),
            if !failed_projects.is_empty() {
                failed_projects.len().to_string().red()
            } else {
                "0".bright_black()
            }
        );

        if !failed_projects.is_empty() {
            println!("  {} {}", "Failed:".red(), failed_projects.join(", ").red());
        }
    }

    fn display_project_result(&self, output: &ProjectOutput) {
        let duration_str = if let Some(duration) = output.duration {
            format!("({:.1}s)", duration.as_secs_f32())
        } else {
            "(unknown)".to_string()
        };

        println!(
            "  {} {} {}",
            "📦".blue(),
            output.name.bold(),
            duration_str.bright_black()
        );

        // Display command if available
        if let Some(command) = &output.command {
            println!("     {} {}", "►".bright_black(), command.bright_white());
        }

        // Display stdout if present
        if !output.stdout.is_empty() {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            for line in stdout_str.lines() {
                if !line.trim().is_empty() {
                    println!("     {}", line);
                }
            }
        }

        // Display result
        match output.status {
            JobStatus::Completed => {
                println!("     {} {}", "✅".green(), "Completed successfully".green());
            }
            JobStatus::Failed => {
                println!("     {} {}", "❌".red(), "Failed".red());

                // Display stderr if present
                if !output.stderr.is_empty() {
                    let stderr_str = String::from_utf8_lossy(&output.stderr);
                    for line in stderr_str.lines() {
                        if !line.trim().is_empty() {
                            println!("     {} {}", "⚠".yellow(), line.red());
                        }
                    }
                }
            }
            _ => {
                println!("     {} {}", "⏸".yellow(), "Unknown status".yellow());
            }
        }

        println!(); // Empty line between projects
    }
}

pub struct ProgressIndicator {
    manager: Arc<OutputManager>,
    handle: Option<thread::JoinHandle<()>>,
    task_name: String,
}

impl ProgressIndicator {
    pub fn new(manager: Arc<OutputManager>, task_name: String) -> Self {
        Self {
            manager,
            handle: None,
            task_name,
        }
    }

    pub fn start(&mut self) {
        let manager = Arc::clone(&self.manager);
        let task_name = self.task_name.clone();

        let handle = thread::spawn(move || {
            let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut spinner_index = 0;
            let start_time = Instant::now();

            loop {
                let (completed, running, _failed) = manager.get_status_summary();
                let total = manager.total_projects;
                let elapsed = start_time.elapsed().as_secs();

                // Show progress line
                let spinner = spinner_chars[spinner_index % spinner_chars.len()];
                let progress_text = if running > 0 {
                    format!(
                        "🚀 Running '{}' {} {}/{} projects • {}s elapsed",
                        task_name,
                        spinner.to_string().cyan(),
                        completed,
                        total,
                        elapsed
                    )
                } else {
                    format!(
                        "🚀 Completed '{}' • {}/{} projects • {}s elapsed",
                        task_name, completed, total, elapsed
                    )
                };

                print!("\r\x1b[K{}", progress_text);
                io::stdout().flush().unwrap();

                if manager.all_completed() {
                    break;
                }

                thread::sleep(Duration::from_millis(100));
                spinner_index += 1;
            }
        });

        self.handle = Some(handle);
    }

    pub fn stop(self) {
        if let Some(handle) = self.handle {
            handle.join().unwrap();
        }
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;

    #[test]
    fn append_then_finish_accumulates_output() {
        let mgr = OutputManager::new(vec!["a".to_string()]);
        mgr.start_project("a");
        mgr.append_stdout("a", b"hello ");
        mgr.append_stdout("a", b"world");
        mgr.append_stderr("a", b"warn");
        mgr.finish_project("a", 0);

        let out = mgr.get_project_output("a").unwrap();
        assert_eq!(out.stdout, b"hello world");
        assert_eq!(out.stderr, b"warn");
        assert_eq!(out.exit_code, Some(0));
        assert_eq!(out.status, JobStatus::Completed);
        assert!(out.duration.is_some());
        assert!(mgr.all_completed());
    }

    #[test]
    fn nonzero_exit_marks_failed() {
        let mgr = OutputManager::new(vec!["a".to_string(), "b".to_string()]);
        mgr.start_project("a");
        mgr.finish_project("a", 3);
        let out = mgr.get_project_output("a").unwrap();
        assert_eq!(out.exit_code, Some(3));
        assert_eq!(out.status, JobStatus::Failed);

        // b never finished, so the batch is not complete yet.
        assert!(!mgr.all_completed());
        let (done, _running, failed) = mgr.get_status_summary();
        assert_eq!((done, failed), (1, 1));
    }
}
