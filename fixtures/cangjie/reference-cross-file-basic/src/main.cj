// Import Point class from mathpkg.ops
import mathpkg.ops.{Point}

main(): Int64 {
    // Type annotation using imported Point → should produce cross-file USES edge
    let p: Point = Point(1, 2)
    p.x
}
