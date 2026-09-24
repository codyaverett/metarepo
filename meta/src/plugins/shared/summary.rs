//! End-of-run results table shared by every multi-project fan-out (exec, git
//! pull/push/fetch/checkout/sync, run, project update).

use colored::*;
use std::time::Duration;

const DETAIL_MAX: usize = 72;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Ok,
    Failed,
    Skipped,
    Cloned,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
            Outcome::Cloned => "cloned",
        }
    }

    fn paint(self, padded: &str) -> ColoredString {
        match self {
            Outcome::Ok => padded.green(),
            Outcome::Failed => padded.red().bold(),
            Outcome::Skipped => padded.yellow(),
            Outcome::Cloned => padded.cyan(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SummaryRow {
    pub name: String,
    pub outcome: Outcome,
    pub duration: Option<Duration>,
    pub detail: String,
}

impl SummaryRow {
    pub fn new(name: impl Into<String>, outcome: Outcome, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            outcome,
            duration: None,
            detail: detail.into(),
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Pick the one line of command output worth showing in the table: the
/// preferred stream is stdout on success and stderr on failure, falling back
/// to the other (git fetch/push report success on stderr). A git diffstat
/// line wins over the last line because `create mode ...` noise follows it.
pub fn output_detail(stdout: &[u8], stderr: &[u8], ok: bool) -> String {
    let (first, second) = if ok {
        (stdout, stderr)
    } else {
        (stderr, stdout)
    };
    let detail = pick_line(first);
    if detail.is_empty() {
        pick_line(second)
    } else {
        detail
    }
}

fn pick_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .iter()
        .find(|l| l.contains("file changed") || l.contains("files changed"))
        .or(lines.last())
        .map(|l| l.to_string())
        .unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{}…", cut)
    }
}

/// Plain-text table lines (no color), so layout is testable and padding is
/// computed before ANSI codes are added.
fn layout(rows: &[SummaryRow]) -> (usize, usize, Vec<[String; 4]>) {
    let cells: Vec<[String; 4]> = rows
        .iter()
        .map(|r| {
            [
                r.name.clone(),
                r.outcome.label().to_string(),
                r.duration
                    .map(|d| format!("{:.1}s", d.as_secs_f32()))
                    .unwrap_or_default(),
                truncate(&r.detail, DETAIL_MAX),
            ]
        })
        .collect();
    let name_w = cells
        .iter()
        .map(|c| c[0].chars().count())
        .max()
        .unwrap_or(0)
        .max("Project".len());
    let time_w = cells
        .iter()
        .map(|c| c[2].len())
        .max()
        .unwrap_or(0)
        .max("Time".len());
    (name_w, time_w, cells)
}

/// Print the results table. A single row adds nothing over the output right
/// above it, so the table only appears for two or more targets.
pub fn print_summary(rows: &[SummaryRow]) {
    if rows.len() < 2 {
        return;
    }
    let (name_w, time_w, cells) = layout(rows);
    let status_w = "skipped".len();

    println!();
    println!(
        "  {}",
        format!(
            "{:<name_w$}  {:<status_w$}  {:>time_w$}  Detail",
            "Project", "Status", "Time"
        )
        .bold()
    );
    println!(
        "  {}",
        "─".repeat(name_w + status_w + time_w + 12).bright_black()
    );
    for (row, c) in rows.iter().zip(&cells) {
        println!(
            "  {:<name_w$}  {}  {:>time_w$}  {}",
            c[0],
            row.outcome.paint(&format!("{:<status_w$}", c[1])),
            c[2].bright_black(),
            c[3]
        );
    }

    let count = |o: Outcome| rows.iter().filter(|r| r.outcome == o).count();
    let mut parts = vec![format!("{} ok", count(Outcome::Ok)).green().to_string()];
    for (o, color) in [
        (Outcome::Cloned, Color::Cyan),
        (Outcome::Skipped, Color::Yellow),
        (Outcome::Failed, Color::Red),
    ] {
        let n = count(o);
        if n > 0 {
            parts.push(format!("{} {}", n, o.label()).color(color).to_string());
        }
    }
    println!("  {}", parts.join(", "));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_prefers_diffstat_then_last_line_then_other_stream() {
        let pull = b"Updating a..b\nFast-forward\n src/x.rs | 2 +-\n 1 file changed, 1 insertion(+)\n create mode 100644 y\n";
        assert_eq!(
            output_detail(pull, b"", true),
            "1 file changed, 1 insertion(+)"
        );
        assert_eq!(
            output_detail(b"Already up to date.\n", b"", true),
            "Already up to date."
        );
        // fetch/push report on stderr even on success.
        assert_eq!(
            output_detail(b"", b"   a..b  main -> main\n", true),
            "a..b  main -> main"
        );
        // failure prefers stderr's last line (the fatal after the hints).
        assert_eq!(
            output_detail(b"out\n", b"hint: x\nfatal: diverged\n", false),
            "fatal: diverged"
        );
    }

    #[test]
    fn layout_pads_to_widest_name_and_truncates_detail() {
        let rows = vec![
            SummaryRow::new("a", Outcome::Ok, "x".repeat(100))
                .with_duration(Duration::from_millis(1500)),
            SummaryRow::new("longer-name", Outcome::Skipped, "dirty"),
        ];
        let (name_w, time_w, cells) = layout(&rows);
        assert_eq!(name_w, "longer-name".len());
        assert_eq!(time_w, "Time".len());
        assert_eq!(cells[0][2], "1.5s");
        assert_eq!(cells[1][2], "");
        assert_eq!(cells[0][3].chars().count(), DETAIL_MAX);
    }
}
