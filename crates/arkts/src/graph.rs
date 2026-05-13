//! ArkTS graph output — extends TypeScript graph with ArkTS-specific nodes/edges.
//!
//! Adds Component, StateProperty, BuildMethod, and UiCall node types
//! on top of the base TypeScript graph.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use gitnexus_typescript::graph::{TsGraphEdge, TsGraphOutput, TsNodeKind};

use crate::extractors::component::ArkTsComponent;

// ---------------------------------------------------------------------------
// ArkTS-specific node kinds (serialized as properties)
// ---------------------------------------------------------------------------

/// ArkTS-specific symbol kinds that extend the base TypeScript types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArkTsNodeKind {
    /// An ArkTS struct component (@Component / @Entry).
    Component,
    /// A state-decorated property (@State, @Local, @Prop, etc.).
    StateProperty,
    /// A build() method inside a component.
    BuildMethod,
}

/// Augment a TypeScript graph output with ArkTS-specific nodes.
pub fn augment_graph_with_arkts(
    base: &mut TsGraphOutput,
    components_by_file: &std::collections::BTreeMap<PathBuf, Vec<ArkTsComponent>>,
) {
    for (file, components) in components_by_file {
        let file_id = format!("file:{}", file.display());

        for comp in components {
            // Add component node
            let comp_id = format!("arkts:component:{}:{}", file.display(), comp.name);
            base.nodes.push(gitnexus_typescript::graph::TsGraphNode {
                id: comp_id.clone(),
                kind: TsNodeKind::Symbol,
                label: comp.name.clone(),
                properties: serde_json::json!({
                    "arktsKind": "component",
                    "fileId": file_id,
                    "decorators": comp.decorators,
                    "isEntry": comp.is_entry,
                    "startLine": comp.start_line,
                    "endLine": comp.end_line,
                }),
            });

            // Edge: file → component
            base.edges.push(TsGraphEdge {
                kind: gitnexus_typescript::graph::TsEdgeKind::Defines,
                source: Some(file_id.clone()),
                target: comp_id.clone(),
                properties: None,
            });

            // Add build method node if present
            if let Some(ref build) = comp.build_method {
                let build_id = format!(
                    "arkts:build:{}:{}:{}",
                    file.display(),
                    comp.name,
                    build.start_line
                );
                base.nodes.push(gitnexus_typescript::graph::TsGraphNode {
                    id: build_id.clone(),
                    kind: TsNodeKind::Symbol,
                    label: "build".to_string(),
                    properties: serde_json::json!({
                        "arktsKind": "buildMethod",
                        "fileId": file_id,
                        "uiCalls": build.ui_calls,
                        "startLine": build.start_line,
                        "endLine": build.end_line,
                    }),
                });

                // Edge: component → build method
                base.edges.push(TsGraphEdge {
                    kind: gitnexus_typescript::graph::TsEdgeKind::Defines,
                    source: Some(comp_id),
                    target: build_id,
                    properties: None,
                });
            }
        }
    }
}
