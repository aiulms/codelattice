//! ArkTS state property extraction.
//!
//! ArkTS uses decorators like @State, @Prop, @Link, @Local, etc. to
//! declare reactive state properties on struct components.

/// An ArkTS state-decorated property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkTsStateProperty {
    /// The property name.
    pub name: String,
    /// The decorator applied (e.g., "@State", "@Local", "@Prop").
    pub decorator: String,
    /// 1-based line number.
    pub line: usize,
    /// Type annotation (if present).
    pub type_name: Option<String>,
}

/// Known ArkTS state decorators.
pub const STATE_DECORATORS: &[&str] = &[
    "@State",
    "@Prop",
    "@Link",
    "@Local",
    "@StorageLink",
    "@StorageProp",
    "@LocalStorageLink",
    "@LocalStorageProp",
    "@Provide",
    "@Consume",
    "@Watch",
    "@ObservedV2",
    "@Trace",
    "@Param",
    "@Event",
    "@Monitor",
];

/// Check if a decorator name is a known state decorator.
pub fn is_state_decorator(decorator: &str) -> bool {
    STATE_DECORATORS.contains(&decorator)
}
