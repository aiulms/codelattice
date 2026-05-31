use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

fn normalize_language(language: &str) -> String {
    match language.to_ascii_lowercase().as_str() {
        "ts" => "typescript".to_string(),
        "js" => "javascript".to_string(),
        "c++" => "cpp".to_string(),
        other => other.to_string(),
    }
}

fn read_to_string(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn push_manifest(manifests: &mut Vec<Value>, root: &Path, file: &str) -> Option<String> {
    let path = root.join(file);
    if path.exists() {
        manifests.push(json!({
            "path": file,
            "kind": "manifest",
            "staticOnly": true,
        }));
        read_to_string(&path)
    } else {
        None
    }
}

fn strip_quotes(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn push_dep(
    deps: &mut BTreeMap<String, Value>,
    name: &str,
    version: &str,
    source: &str,
    manifest: &str,
) {
    let clean = strip_quotes(name);
    if clean.is_empty() || clean.starts_with('#') {
        return;
    }
    deps.entry(clean.clone()).or_insert_with(|| {
        json!({
            "name": clean,
            "version": strip_quotes(version),
            "source": source,
            "manifest": manifest,
            "staticOnly": true,
        })
    });
}

fn parse_cargo_toml(content: &str, deps: &mut BTreeMap<String, Value>) -> Option<String> {
    let mut section = String::new();
    let mut package_name = None;
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).to_string();
            continue;
        }
        if section == "package" && line.starts_with("name") {
            if let Some((_, value)) = line.split_once('=') {
                package_name = Some(strip_quotes(value));
            }
            continue;
        }
        if matches!(
            section.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            if let Some((name, value)) = line.split_once('=') {
                push_dep(deps, name, value, &section, "Cargo.toml");
            }
        }
    }
    package_name
}

fn parse_package_json(
    content: &str,
    deps: &mut BTreeMap<String, Value>,
) -> (Option<String>, Vec<Value>) {
    let parsed: Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return (None, Vec::new()),
    };
    let package_name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(obj) = parsed.get(section).and_then(|v| v.as_object()) {
            for (name, version) in obj {
                push_dep(
                    deps,
                    name,
                    version.as_str().unwrap_or(""),
                    section,
                    "package.json",
                );
            }
        }
    }
    let scripts = parsed
        .get("scripts")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .take(8)
                .map(|(name, command)| {
                    json!({
                        "name": name,
                        "command": command.as_str().unwrap_or(""),
                        "staticOnly": true,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (package_name, scripts)
}

fn parse_requirements(content: &str, deps: &mut BTreeMap<String, Value>, manifest: &str) {
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('-') {
            continue;
        }
        let split_at = line
            .find(|c: char| matches!(c, '=' | '<' | '>' | '~' | '!'))
            .unwrap_or(line.len());
        let name = &line[..split_at];
        let version = &line[split_at..];
        push_dep(deps, name, version, "requirements", manifest);
    }
}

fn parse_pyproject(content: &str, deps: &mut BTreeMap<String, Value>) -> Option<String> {
    let mut package_name = None;
    let mut in_dependencies = false;
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_dependencies = false;
        }
        if line.starts_with("name") && package_name.is_none() {
            if let Some((_, value)) = line.split_once('=') {
                package_name = Some(strip_quotes(value));
            }
        }
        if line.starts_with("dependencies") && line.contains('[') {
            in_dependencies = true;
            continue;
        }
        if in_dependencies {
            if line.starts_with(']') {
                in_dependencies = false;
                continue;
            }
            let dep = line.trim_end_matches(',').trim();
            if !dep.is_empty() {
                let split_at = dep
                    .find(|c: char| matches!(c, '=' | '<' | '>' | '~' | '!'))
                    .unwrap_or(dep.len());
                push_dep(
                    deps,
                    &strip_quotes(&dep[..split_at]),
                    &strip_quotes(&dep[split_at..]),
                    "project.dependencies",
                    "pyproject.toml",
                );
            }
        }
    }
    package_name
}

fn framework_table(language: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    match language {
        "rust" => vec![
            ("axum", "axum", "web/server framework"),
            ("actix-web", "actix-web", "web/server framework"),
            ("rocket", "rocket", "web/server framework"),
            ("warp", "warp", "web/server framework"),
            ("tonic", "tonic", "grpc/server framework"),
            ("tokio", "tokio", "async runtime"),
            ("tauri", "tauri", "desktop app framework"),
            ("clap", "clap", "cli framework"),
            ("serde", "serde", "serialization framework"),
        ],
        "typescript" | "javascript" | "arkts" => vec![
            ("react", "react", "ui framework"),
            ("next", "next.js", "full-stack web framework"),
            ("nextjs", "next.js", "full-stack web framework"),
            ("vue", "vue", "ui framework"),
            ("svelte", "svelte", "ui framework"),
            ("vite", "vite", "frontend build tool"),
            ("express", "express", "web/server framework"),
            ("fastify", "fastify", "web/server framework"),
            ("koa", "koa", "web/server framework"),
            ("@nestjs/core", "nestjs", "server application framework"),
            ("electron", "electron", "desktop app framework"),
        ],
        "python" => vec![
            ("fastapi", "fastapi", "web/server framework"),
            ("flask", "flask", "web/server framework"),
            ("django", "django", "web/server framework"),
            ("typer", "typer", "cli framework"),
            ("click", "click", "cli framework"),
            ("pytest", "pytest", "test framework"),
        ],
        "c" | "cpp" => vec![
            ("cmake", "cmake", "build system"),
            ("vcpkg", "vcpkg", "dependency manager"),
            ("conan", "conan", "dependency manager"),
        ],
        _ => Vec::new(),
    }
}

fn framework_hints(
    language: &str,
    deps: &BTreeMap<String, Value>,
    manifests: &[Value],
) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut hints = Vec::new();
    for (needle, framework, role) in framework_table(language) {
        let matched_dep = deps.keys().find(|name| {
            let lower = name.to_ascii_lowercase();
            lower == needle || lower.contains(needle)
        });
        if let Some(dep) = matched_dep {
            if seen.insert(framework.to_string()) {
                hints.push(json!({
                    "framework": framework,
                    "dependency": dep,
                    "role": role,
                    "confidence": 0.88,
                    "reason": "dependency-detected",
                    "source": "manifest",
                    "staticOnly": true,
                }));
            }
        }
    }
    if matches!(language, "c" | "cpp") {
        for manifest in manifests {
            if let Some(path) = manifest.get("path").and_then(|v| v.as_str()) {
                let lower = path.to_ascii_lowercase();
                for (needle, framework, role) in framework_table(language) {
                    if lower.contains(needle) && seen.insert(framework.to_string()) {
                        hints.push(json!({
                            "framework": framework,
                            "dependency": needle,
                            "role": role,
                            "confidence": 0.7,
                            "reason": "manifest-file-detected",
                            "source": "manifest",
                            "staticOnly": true,
                        }));
                    }
                }
            }
        }
    }
    hints.truncate(12);
    hints
}

pub(crate) fn build_dependency_framework_digest(root: &Path, language: &str) -> Value {
    let language = normalize_language(language);
    let mut manifests = Vec::new();
    let mut deps: BTreeMap<String, Value> = BTreeMap::new();
    let mut package_name: Option<String> = None;
    let mut scripts = Vec::new();

    match language.as_str() {
        "rust" => {
            if let Some(content) = push_manifest(&mut manifests, root, "Cargo.toml") {
                package_name = parse_cargo_toml(&content, &mut deps);
            }
        }
        "typescript" | "javascript" | "arkts" => {
            if let Some(content) = push_manifest(&mut manifests, root, "package.json") {
                let (name, parsed_scripts) = parse_package_json(&content, &mut deps);
                package_name = name;
                scripts = parsed_scripts;
            }
            for file in [
                "oh-package.json5",
                "tsconfig.json",
                "vite.config.ts",
                "next.config.js",
            ] {
                let _ = push_manifest(&mut manifests, root, file);
            }
        }
        "python" => {
            if let Some(content) = push_manifest(&mut manifests, root, "pyproject.toml") {
                package_name = parse_pyproject(&content, &mut deps);
            }
            if let Some(content) = push_manifest(&mut manifests, root, "requirements.txt") {
                parse_requirements(&content, &mut deps, "requirements.txt");
            }
            let _ = push_manifest(&mut manifests, root, "setup.py");
        }
        "c" | "cpp" => {
            for file in [
                "CMakeLists.txt",
                "compile_commands.json",
                "vcpkg.json",
                "conanfile.txt",
                "conanfile.py",
            ] {
                let _ = push_manifest(&mut manifests, root, file);
            }
        }
        _ => {}
    }

    let top_dependencies = deps.values().take(40).cloned().collect::<Vec<_>>();
    let hints = framework_hints(&language, &deps, &manifests);
    json!({
        "schemaVersion": "codelattice.dependencyFrameworkDigest.v1",
        "language": language,
        "root": root.to_string_lossy(),
        "packageName": package_name,
        "manifestFiles": manifests,
        "dependencyCount": deps.len(),
        "topDependencies": top_dependencies,
        "frameworkHints": hints,
        "scripts": scripts,
        "generatedFrom": {
            "staticAnalysis": true,
            "manifestOnly": true,
            "targetCodeExecuted": false,
            "scriptsExecuted": false,
            "runtimeVerified": false
        },
        "confidence": {
            "level": if deps.is_empty() { "low" } else { "medium" },
            "reason": "Dependencies and framework hints are derived from manifests only."
        },
        "staticOnly": true,
        "targetCodeExecuted": false,
        "detailHint": "Use analyze --profile full or MCP project standard/deep for graph evidence; this digest only reads manifests."
    })
}

pub(crate) fn dependency_evidence_cards(digest: &Value, limit: usize) -> Vec<Value> {
    let mut cards = Vec::new();
    if let Some(deps) = digest.get("topDependencies").and_then(|v| v.as_array()) {
        for dep in deps.iter().take(limit) {
            cards.push(json!({
                "kind": "dependency",
                "name": dep["name"],
                "version": dep["version"],
                "manifest": dep["manifest"],
                "reason": "declared dependency in manifest",
                "source": "manifest",
                "staticOnly": true,
            }));
        }
    }
    if cards.len() < limit {
        if let Some(hints) = digest.get("frameworkHints").and_then(|v| v.as_array()) {
            for hint in hints.iter().take(limit - cards.len()) {
                cards.push(json!({
                    "kind": "framework_hint",
                    "framework": hint["framework"],
                    "dependency": hint["dependency"],
                    "role": hint["role"],
                    "reason": hint["reason"],
                    "source": "manifest",
                    "staticOnly": true,
                }));
            }
        }
    }
    cards
}

fn find_analysis_trace(value: &Value) -> Option<Value> {
    match value {
        Value::Object(obj) => {
            if let Some(trace) = obj.get("analysisTrace").filter(|v| !v.is_null()) {
                return Some(trace.clone());
            }
            if let Some(trace) = obj
                .get("warmTrace")
                .and_then(|w| w.get("analysisTrace"))
                .filter(|v| !v.is_null())
            {
                return Some(trace.clone());
            }
            for child in obj.values() {
                if let Some(trace) = find_analysis_trace(child) {
                    return Some(trace);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_analysis_trace),
        _ => None,
    }
}

pub(crate) fn build_runtime_trace_envelope(language: &str, result: Option<&Value>) -> Value {
    let language = normalize_language(language);
    let trace = result.and_then(find_analysis_trace);
    let detailed_available = trace.is_some();
    json!({
        "schemaVersion": "codelattice.languageRuntimeTrace.v1",
        "language": language,
        "available": detailed_available,
        "source": if detailed_available { "analysisTrace" } else { "not_available" },
        "stages": trace.unwrap_or_else(|| json!({})),
        "staticOnly": true,
        "targetCodeExecuted": false,
        "notes": if detailed_available {
            vec!["Detailed sub-stage timing captured by the language adapter."]
        } else {
            vec!["Detailed stage timing is not available for this response yet; runtimeCapabilities still describe adapter support."]
        },
    })
}
