package demo

// Single named import
import demo.math.add

// Grouped import
import demo.math.{sub, mul}

// Wildcard import (not resolved to specific symbols)
import demo.math.*

// Aliased import
import demo.math.Calculator as Calc

// Public import
public import demo.math.add

main(): Int64 {
    add(1, 2)
}
