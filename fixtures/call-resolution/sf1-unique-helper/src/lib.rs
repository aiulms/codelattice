// static-analysis-only: not compilable in Rust (helper in mod a not visible at crate level)
// Tests same-file heuristic resolving a unique-name Function across inline module boundary
mod a {
    pub fn helper() -> i32 { 42 }
}
fn main_fn() -> i32 {
    helper()
}
