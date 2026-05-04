// static-analysis-only: Foo is not visible at crate level from inside mod inner
mod inner {
    use crate::Foo;
}
struct Foo;
