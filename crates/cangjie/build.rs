fn main() {
    #[cfg(feature = "tree-sitter-cangjie")]
    {
        cc::Build::new()
            .include("vendor/tree-sitter-cangjie/src")
            .file("vendor/tree-sitter-cangjie/src/parser.c")
            .file("vendor/tree-sitter-cangjie/src/scanner.c")
            .compile("tree-sitter-cangjie");
    }
}
