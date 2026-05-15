//! compile_commands.json parser for C++ projects.
//!
//! Parses the JSON compilation database format used by CMake, Bear, and other
//! build tools. Extracts include directories (-I, -isystem, -iquote),
//! defines (-D), and forced includes (-include) from each compile command.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A parsed entry from compile_commands.json.
#[derive(Debug, Clone)]
pub struct CompileCommandEntry {
    /// Source file path (as listed in the compile command).
    pub file: PathBuf,
    /// Working directory for the compile command.
    pub directory: PathBuf,
    /// Quote include directories (-iquote).
    pub quote_include_dirs: Vec<PathBuf>,
    /// Project include directories (-I).
    pub project_include_dirs: Vec<PathBuf>,
    /// System include directories (-isystem).
    pub system_include_dirs: Vec<PathBuf>,
    /// Preprocessor defines (-D).
    pub defines: Vec<(String, Option<String>)>,
    /// Forced includes (-include).
    pub forced_includes: Vec<PathBuf>,
}

/// Compilation database indexed by file path.
#[derive(Debug, Clone, Default)]
pub struct CompileCommandDb {
    pub entries_by_file: BTreeMap<PathBuf, CompileCommandEntry>,
}

// ---------------------------------------------------------------------------
// JSON deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawEntry {
    directory: String,
    file: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    arguments: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load a compile_commands.json file from disk.
pub fn load_compile_commands(path: &Path) -> Result<CompileCommandDb, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let raw_entries: Vec<RawEntry> =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let project_root = path
        .parent()
        .ok_or_else(|| "compile_commands.json has no parent directory".to_string())?
        .to_path_buf();

    let mut db = CompileCommandDb::default();

    for raw in raw_entries {
        let directory = PathBuf::from(&raw.directory);
        let directory = if directory.is_relative() {
            project_root.join(&directory)
        } else {
            directory
        };

        let file_path = PathBuf::from(&raw.file);
        let file_path = if file_path.is_relative() {
            directory.join(&file_path)
        } else {
            file_path
        };

        // Prefer arguments array over command string
        let args: Vec<String> = if let Some(ref arguments) = raw.arguments {
            arguments.clone()
        } else if let Some(ref command) = raw.command {
            shell_split(command)
        } else {
            Vec::new()
        };

        let mut entry = CompileCommandEntry {
            file: file_path.clone(),
            directory,
            quote_include_dirs: Vec::new(),
            project_include_dirs: Vec::new(),
            system_include_dirs: Vec::new(),
            defines: Vec::new(),
            forced_includes: Vec::new(),
        };

        let dir_for_parse = entry.directory.clone();
        parse_compiler_flags(&args, &dir_for_parse, &project_root, &mut entry);

        db.entries_by_file.insert(file_path, entry);
    }

    Ok(db)
}

impl CompileCommandDb {
    /// Look up the compile command entry for a specific source file.
    pub fn for_file(&self, file: &Path) -> Option<&CompileCommandEntry> {
        self.entries_by_file.get(file)
    }
}

// ---------------------------------------------------------------------------
// Shell splitting
// ---------------------------------------------------------------------------

/// Light shell splitting: split by spaces, handle single/double quotes.
fn shell_split(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    current.push(c);
                }
            }
            '"' => loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => {
                        if let Some(escaped) = chars.next() {
                            current.push(escaped);
                        }
                    }
                    Some(c) => current.push(c),
                    None => break,
                }
            },
            ' ' | '\t' => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}

// ---------------------------------------------------------------------------
// Flag parsing
// ---------------------------------------------------------------------------

fn parse_compiler_flags(
    args: &[String],
    directory: &Path,
    project_root: &Path,
    entry: &mut CompileCommandEntry,
) {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "-I" {
            i += 1;
            if i < args.len() {
                if let Some(p) = resolve_dir(&args[i], directory, project_root) {
                    entry.project_include_dirs.push(p);
                }
            }
        } else if let Some(dir) = arg.strip_prefix("-I") {
            if !dir.is_empty() {
                if let Some(p) = resolve_dir(dir, directory, project_root) {
                    entry.project_include_dirs.push(p);
                }
            }
        } else if arg == "-isystem" {
            i += 1;
            if i < args.len() {
                if let Some(p) = resolve_dir(&args[i], directory, project_root) {
                    entry.system_include_dirs.push(p);
                }
            }
        } else if let Some(dir) = arg.strip_prefix("-isystem") {
            if !dir.is_empty() {
                if let Some(p) = resolve_dir(dir, directory, project_root) {
                    entry.system_include_dirs.push(p);
                }
            }
        } else if arg == "-iquote" {
            i += 1;
            if i < args.len() {
                if let Some(p) = resolve_dir(&args[i], directory, project_root) {
                    entry.quote_include_dirs.push(p);
                }
            }
        } else if let Some(dir) = arg.strip_prefix("-iquote") {
            if !dir.is_empty() {
                if let Some(p) = resolve_dir(dir, directory, project_root) {
                    entry.quote_include_dirs.push(p);
                }
            }
        } else if arg == "-include" {
            i += 1;
            if i < args.len() {
                if let Some(p) = resolve_file(&args[i], directory, project_root) {
                    entry.forced_includes.push(p);
                }
            }
        } else if let Some(file) = arg.strip_prefix("-include") {
            if !file.is_empty() {
                if let Some(p) = resolve_file(file, directory, project_root) {
                    entry.forced_includes.push(p);
                }
            }
        } else if arg == "-D" {
            i += 1;
            if i < args.len() {
                entry.defines.push(parse_define(&args[i]));
            }
        } else if let Some(def) = arg.strip_prefix("-D") {
            if !def.is_empty() {
                entry.defines.push(parse_define(def));
            }
        }

        i += 1;
    }

    entry.quote_include_dirs.sort();
    entry.project_include_dirs.sort();
    entry.system_include_dirs.sort();
    entry.forced_includes.sort();
}

fn parse_define(input: &str) -> (String, Option<String>) {
    if let Some(eq_pos) = input.find('=') {
        let name = input[..eq_pos].to_string();
        let value = input[eq_pos + 1..].to_string();
        (name, Some(value))
    } else {
        (input.to_string(), None)
    }
}

fn resolve_dir(path_str: &str, directory: &Path, project_root: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(path_str);
    if path.is_absolute() {
        Some(path)
    } else {
        let from_dir = directory.join(&path);
        if from_dir.exists() {
            Some(from_dir)
        } else {
            let from_root = project_root.join(&path);
            if from_root.exists() {
                Some(from_root)
            } else {
                Some(from_dir)
            }
        }
    }
}

fn resolve_file(path_str: &str, directory: &Path, project_root: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(path_str);
    if path.is_absolute() && path.is_file() {
        return Some(path);
    }

    let from_dir = directory.join(&path);
    if from_dir.is_file() {
        return Some(from_dir);
    }

    let from_root = project_root.join(&path);
    if from_root.is_file() {
        return Some(from_root);
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_split_simple() {
        let result = shell_split("c++ -Iinclude -c src/main.cpp");
        assert_eq!(result, vec!["c++", "-Iinclude", "-c", "src/main.cpp"]);
    }

    #[test]
    fn test_parse_define_with_value() {
        assert_eq!(
            parse_define("APP_VERSION=1"),
            ("APP_VERSION".into(), Some("1".into()))
        );
    }
}
