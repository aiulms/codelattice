// static-analysis-only: not compilable in Rust (compute in mod inner not visible at crate level)
// Tests same-file heuristic resolving a unique-name Function across inline module boundary
mod inner {
    pub fn compute() -> i32 { 1 }
}
fn main_fn() -> i32 {
    compute()
}
