use serde_json::{json, Value};
use std::path::Path;

pub(crate) fn normalize_facade_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "ts" => "typescript".to_string(),
        "js" => "javascript".to_string(),
        "py" => "python".to_string(),
        "c++" => "cpp".to_string(),
        "" => "auto".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn facade_language_runtime_capabilities(language: &str) -> Value {
    let lang = normalize_facade_language(language);
    let (in_process, call_edges, delta_overlay, trace_available) = match lang.as_str() {
        "rust" => (true, true, true, true),
        "typescript" | "javascript" | "arkts" => (true, true, false, false),
        "python" | "c" | "cpp" | "cangjie" => (true, true, false, false),
        "shell" => (true, false, false, false),
        _ => (false, false, false, false),
    };
    json!({
        "schemaVersion": "codelattice.languageRuntimeCapabilities.v1",
        "language": lang,
        "inProcessAnalysis": in_process,
        "cliFallbackUsed": false,
        "supportsDeltaOverlay": delta_overlay,
        "supportsCallEdges": call_edges,
        "supportsPersistentCache": true,
        "traceAvailable": trace_available,
        "staticOnly": true
    })
}

pub(crate) struct FacadeRequestContext {
    tool: String,
    mode: String,
    original_root: String,
    effective_root: String,
    requested_language: String,
    effective_language: String,
    compact: bool,
    root_router: Value,
}

impl FacadeRequestContext {
    pub(crate) fn new(
        tool: &str,
        mode: &str,
        original_root: &str,
        effective_root: &str,
        requested_language: &str,
        effective_language: &str,
        compact: bool,
        root_router: Value,
    ) -> Self {
        Self {
            tool: tool.to_string(),
            mode: mode.to_string(),
            original_root: original_root.to_string(),
            effective_root: effective_root.to_string(),
            requested_language: requested_language.to_string(),
            effective_language: effective_language.to_string(),
            compact,
            root_router,
        }
    }

    pub(crate) fn unrouted(
        tool: &str,
        mode: &str,
        root: &str,
        requested_language: &str,
        effective_language: &str,
        compact: bool,
    ) -> Self {
        Self::new(
            tool,
            mode,
            root,
            root,
            requested_language,
            effective_language,
            compact,
            json!({
                "schemaVersion": "codelattice.rootRouter.v1",
                "routed": false,
                "tool": tool,
                "mode": mode,
                "originalRoot": root,
                "selectedRoot": root,
                "selectedLanguage": effective_language,
                "confidence": "n/a",
                "reason": "root already targets a single project or no workspace routing was needed"
            }),
        )
    }

    pub(crate) fn from_routed_tool_result(inner: &Value, router: &Value) -> Self {
        let tool = inner["tool"].as_str().unwrap_or("codelattice_unknown");
        let mode = inner["mode"].as_str().unwrap_or("unknown");
        let effective_root = inner["root"].as_str().unwrap_or("");
        let effective_language = inner["language"].as_str().unwrap_or("auto");
        let original_root = router["originalRoot"].as_str().unwrap_or(effective_root);
        let requested_language = router["requestedLanguage"].as_str().unwrap_or("auto");
        let compact = inner["compact"].as_bool().unwrap_or(false);
        Self::new(
            tool,
            mode,
            original_root,
            effective_root,
            requested_language,
            effective_language,
            compact,
            router.clone(),
        )
    }

    pub(crate) fn root_router_is_routed(&self) -> bool {
        self.root_router
            .get("routed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn root_router(&self) -> &Value {
        &self.root_router
    }

    pub(crate) fn to_json(&self) -> Value {
        let canonical_effective_root = if self.effective_root.is_empty() {
            Value::Null
        } else {
            Path::new(&self.effective_root)
                .canonicalize()
                .ok()
                .map(|path| json!(path.to_string_lossy().to_string()))
                .unwrap_or_else(|| json!(self.effective_root))
        };
        json!({
            "schemaVersion": "codelattice.facadeRequest.v1",
            "tool": self.tool,
            "mode": self.mode,
            "originalRoot": self.original_root,
            "effectiveRoot": self.effective_root,
            "canonicalEffectiveRoot": canonical_effective_root,
            "requestedLanguage": self.requested_language,
            "effectiveLanguage": self.effective_language,
            "compact": self.compact,
            "rootRouter": self.root_router,
            "cacheKeyScope": {
                "root": self.effective_root,
                "language": self.effective_language
            }
        })
    }
}

pub(crate) fn attach_facade_request_context(out: &mut Value, context: &FacadeRequestContext) {
    if let Some(obj) = out.as_object_mut() {
        obj.insert("requestContext".to_string(), context.to_json());
        if context.root_router_is_routed() && !obj.contains_key("rootRouter") {
            obj.insert("rootRouter".to_string(), context.root_router().clone());
        }
    }
}
