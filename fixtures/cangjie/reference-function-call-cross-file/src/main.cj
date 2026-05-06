import mathpkg.ops.{add}

main(): Int64 {
    // Cross-file function call: add(1, 2) → USES edge to mathpkg/ops.cj add
    add(1, 2)
}
