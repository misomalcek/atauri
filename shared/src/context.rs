use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Context Engine - builds full project awareness ONCE, not incrementally
/// This is the anti-Kilo-Code: scan project structure upfront,
/// give model complete context from first message.
///
/// Maps to Codex Symbiosis: Context Continuum + Knowledge Scaffolding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub name: String,
    pub tech_stack: Vec<String>,
    pub key_files: Vec<FileInfo>,
    pub structure_summary: String,
    pub total_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub kind: FileKind,
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileKind {
    Config,
    Source,
    Test,
    Documentation,
    Style,
    Asset,
    Other,
}

impl ProjectContext {
    /// Scan a project directory and build complete context
    /// This runs ONCE when you open a project, not on every query
    pub fn scan(root: &Path) -> Self {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        let mut key_files = Vec::new();
        let mut tech_stack = Vec::new();
        let mut total_files = 0;

        // Use ignore crate to respect .gitignore
        let walker = ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .max_depth(Some(5))
            .build();

        for entry in walker.flatten() {
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            total_files += 1;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();

            // Detect tech stack from config files
            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            match filename.as_str() {
                "package.json" => tech_stack.push("Node.js".into()),
                "Cargo.toml" => tech_stack.push("Rust".into()),
                "pyproject.toml" | "setup.py" => tech_stack.push("Python".into()),
                "tsconfig.json" => tech_stack.push("TypeScript".into()),
                "svelte.config.js" => tech_stack.push("Svelte".into()),
                "next.config.js" | "next.config.ts" => tech_stack.push("Next.js".into()),
                "docker-compose.yml" | "Dockerfile" => tech_stack.push("Docker".into()),
                _ => {}
            }

            // Classify and collect key files
            let kind = classify_file(&relative, &ext);
            let lines = std::fs::read_to_string(path)
                .map(|c| c.lines().count())
                .unwrap_or(0);

            // Keep config files and larger source files as "key"
            let is_key = matches!(kind, FileKind::Config)
                || lines > 50
                || filename.contains("README")
                || filename.contains("CLAUDE");

            if is_key {
                key_files.push(FileInfo {
                    path: relative,
                    kind,
                    lines,
                });
            }
        }

        tech_stack.sort();
        tech_stack.dedup();
        key_files.sort_by(|a, b| b.lines.cmp(&a.lines));
        key_files.truncate(50); // Top 50 key files

        let structure_summary = Self::build_summary(&key_files, &tech_stack, total_files);

        ProjectContext {
            root: root.to_path_buf(),
            name,
            tech_stack,
            key_files,
            structure_summary,
            total_files,
        }
    }

    /// Build human-readable summary for model context injection
    fn build_summary(files: &[FileInfo], stack: &[String], total: usize) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("Tech stack: {}\n", stack.join(", ")));
        summary.push_str(&format!("Total files: {total}\n"));
        summary.push_str("Key files:\n");
        for f in files.iter().take(20) {
            summary.push_str(&format!("  {} ({} lines)\n", f.path, f.lines));
        }
        summary
    }

    /// Generate system prompt context for AI models
    /// This is what gets prepended to every conversation
    pub fn to_system_context(&self) -> String {
        format!(
            "## Project: {}\n\n{}\n\nYou have full context of this project. \
             Do NOT search or explore - you already know the structure. \
             Act directly on what you know.",
            self.name, self.structure_summary
        )
    }
}

fn classify_file(path: &str, ext: &str) -> FileKind {
    if path.contains("test") || path.contains("spec") {
        return FileKind::Test;
    }
    match ext {
        "json" | "toml" | "yaml" | "yml" | "env" | "config" => FileKind::Config,
        "rs" | "ts" | "js" | "py" | "svelte" | "tsx" | "jsx" | "go" => FileKind::Source,
        "md" | "txt" | "rst" => FileKind::Documentation,
        "css" | "scss" | "less" => FileKind::Style,
        "png" | "jpg" | "svg" | "ico" | "woff" | "woff2" => FileKind::Asset,
        _ => FileKind::Other,
    }
}
