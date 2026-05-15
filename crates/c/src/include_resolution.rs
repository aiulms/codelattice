//! C include resolution using compile_commands.json data.
//!
//! Resolves `#include` directives to project files using include paths
//! extracted from compile_commands.json. Falls back to filename-unique
//! matching when no compile database is available.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::compile_commands::CompileCommandDb;
use crate::extractors::include::{CInclude, CIncludeKind};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How the include was resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum CResolvedIncludeKind {
    /// Found in same directory as source file.
    SameDirectory,
    /// Found via -iquote directory.
    QuoteIncludeDir,
    /// Found via -I project include directory.
    ProjectIncludeDir,
    /// Forced include (-include flag).
    ForcedInclude,
    /// System/external header, no project edge.
    SystemExternal,
    /// Could not resolve to any file.
    Unresolved,
    /// Multiple candidates with same basename.
    Ambiguous,
}

/// Result of resolving a single include directive.
#[derive(Debug, Clone)]
pub struct CResolvedInclude {
    /// The include path as written in source.
    pub include_path: String,
    /// Resolved target file (None if unresolved/ambiguous/system).
    pub target_file: Option<PathBuf>,
    /// How the resolution was performed.
    pub resolution_kind: CResolvedIncludeKind,
    /// Confidence score (0.0–1.0).
    pub confidence: Option<f64>,
    /// Human-readable reason string.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Confidence constants
// ---------------------------------------------------------------------------

const CONFIDENCE_SAME_DIR: f64 = 0.95;
const CONFIDENCE_QUOTE_DIR: f64 = 0.90;
const CONFIDENCE_PROJECT_DIR: f64 = 0.85;
const CONFIDENCE_ANGLE_PROJECT: f64 = 0.75;
const CONFIDENCE_FORCED: f64 = 0.70;
const CONFIDENCE_FILENAME_UNIQUE: f64 = 0.60;

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// C include resolver backed by compile_commands.json data.
pub struct CIncludeResolver {
    project_root: PathBuf,
    compile_db: Option<CompileCommandDb>,
    /// All project header files (absolute paths).
    #[allow(dead_code)]
    header_files: Vec<PathBuf>,
    /// Basename -> list of absolute paths with that basename.
    basename_index: BTreeMap<String, Vec<PathBuf>>,
}

impl CIncludeResolver {
    /// Build a resolver from project data.
    pub fn build(
        project_root: &Path,
        source_files: &[PathBuf],
        header_files: &[PathBuf],
        compile_db: Option<CompileCommandDb>,
    ) -> Self {
        let mut all_headers: Vec<PathBuf> = header_files.to_vec();
        // Source files can also be included (e.g., .c files included by test runners)
        all_headers.extend(source_files.iter().cloned());
        all_headers.sort();
        all_headers.dedup();

        let mut basename_index: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for h in &all_headers {
            if let Some(name) = h.file_name().and_then(|n| n.to_str()) {
                basename_index
                    .entry(name.to_string())
                    .or_default()
                    .push(h.clone());
            }
        }

        Self {
            project_root: project_root.to_path_buf(),
            compile_db,
            header_files: all_headers,
            basename_index,
        }
    }

    /// Resolve a single include directive from a source file.
    pub fn resolve(&self, source_file: &Path, include: &CInclude) -> CResolvedInclude {
        match include.kind {
            CIncludeKind::Local => self.resolve_local(source_file, &include.path),
            CIncludeKind::System => self.resolve_system(source_file, &include.path),
        }
    }

    fn resolve_local(&self, source_file: &Path, include_path: &str) -> CResolvedInclude {
        let entry = self
            .compile_db
            .as_ref()
            .and_then(|db| db.for_file(source_file));

        // 1. Same directory as source file
        if let Some(parent) = source_file.parent() {
            let candidate = parent.join(include_path);
            if candidate.is_file() {
                return CResolvedInclude {
                    include_path: include_path.to_string(),
                    target_file: Some(candidate),
                    resolution_kind: CResolvedIncludeKind::SameDirectory,
                    confidence: Some(CONFIDENCE_SAME_DIR),
                    reason: "c-local-include-same-directory".to_string(),
                };
            }
        }

        // 2. -iquote directories
        if let Some(ref e) = entry {
            for dir in &e.quote_include_dirs {
                let candidate = dir.join(include_path);
                if candidate.is_file() {
                    return CResolvedInclude {
                        include_path: include_path.to_string(),
                        target_file: Some(candidate),
                        resolution_kind: CResolvedIncludeKind::QuoteIncludeDir,
                        confidence: Some(CONFIDENCE_QUOTE_DIR),
                        reason: "c-quote-include-dir".to_string(),
                    };
                }
            }
        }

        // 3. -I project directories
        if let Some(ref e) = entry {
            for dir in &e.project_include_dirs {
                let candidate = dir.join(include_path);
                if candidate.is_file() {
                    return CResolvedInclude {
                        include_path: include_path.to_string(),
                        target_file: Some(candidate),
                        resolution_kind: CResolvedIncludeKind::ProjectIncludeDir,
                        confidence: Some(CONFIDENCE_PROJECT_DIR),
                        reason: "c-project-include-dir".to_string(),
                    };
                }
            }
        }

        // 4. Project root
        let root_candidate = self.project_root.join(include_path);
        if root_candidate.is_file() {
            return CResolvedInclude {
                include_path: include_path.to_string(),
                target_file: Some(root_candidate),
                resolution_kind: CResolvedIncludeKind::ProjectIncludeDir,
                confidence: Some(CONFIDENCE_PROJECT_DIR),
                reason: "c-project-include-dir".to_string(),
            };
        }

        // 5. Filename-unique fallback
        let basename = Path::new(include_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(include_path);

        if let Some(candidates) = self.basename_index.get(basename) {
            if candidates.len() == 1 {
                return CResolvedInclude {
                    include_path: include_path.to_string(),
                    target_file: Some(candidates[0].clone()),
                    resolution_kind: CResolvedIncludeKind::ProjectIncludeDir,
                    confidence: Some(CONFIDENCE_FILENAME_UNIQUE),
                    reason: "c-filename-unique-fallback".to_string(),
                };
            }
            // Multiple candidates with same basename → ambiguous
            return CResolvedInclude {
                include_path: include_path.to_string(),
                target_file: None,
                resolution_kind: CResolvedIncludeKind::Ambiguous,
                confidence: None,
                reason: "c-include-ambiguous".to_string(),
            };
        }

        CResolvedInclude {
            include_path: include_path.to_string(),
            target_file: None,
            resolution_kind: CResolvedIncludeKind::Unresolved,
            confidence: None,
            reason: "c-include-unresolved".to_string(),
        }
    }

    fn resolve_system(&self, source_file: &Path, include_path: &str) -> CResolvedInclude {
        let entry = self
            .compile_db
            .as_ref()
            .and_then(|db| db.for_file(source_file));

        // Check -I dirs for project headers
        if let Some(ref e) = entry {
            for dir in &e.project_include_dirs {
                let candidate = dir.join(include_path);
                if candidate.is_file() {
                    // Check if it's a project file
                    if candidate.starts_with(&self.project_root) {
                        return CResolvedInclude {
                            include_path: include_path.to_string(),
                            target_file: Some(candidate),
                            resolution_kind: CResolvedIncludeKind::ProjectIncludeDir,
                            confidence: Some(CONFIDENCE_ANGLE_PROJECT),
                            reason: "c-project-include-angle-resolved".to_string(),
                        };
                    }
                }
            }
        }

        CResolvedInclude {
            include_path: include_path.to_string(),
            target_file: None,
            resolution_kind: CResolvedIncludeKind::SystemExternal,
            confidence: None,
            reason: "c-system-include-external".to_string(),
        }
    }

    /// Resolve forced includes for a source file.
    pub fn resolve_forced_includes(&self, source_file: &Path) -> Vec<CResolvedInclude> {
        let entry = match self
            .compile_db
            .as_ref()
            .and_then(|db| db.for_file(source_file))
        {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut results = Vec::new();
        for forced_path in &entry.forced_includes {
            if forced_path.is_file() && forced_path.starts_with(&self.project_root) {
                results.push(CResolvedInclude {
                    include_path: forced_path.to_string_lossy().to_string(),
                    target_file: Some(forced_path.clone()),
                    resolution_kind: CResolvedIncludeKind::ForcedInclude,
                    confidence: Some(CONFIDENCE_FORCED),
                    reason: "c-forced-include".to_string(),
                });
            }
        }
        results
    }
}
