# Scraps Language Manual

A concise, complete reference to the Scraps language as implemented in this VM.

- Paradigm: expression-first, newline-terminated statements, arrow verbs for data movement.
- Values: `Int`, `Float`, `Bool`, `Str` (Unicode), `Box` (list), `Function`, `None`.
- Purity: user function calls are pure by default (no env mutation). Use `rewire` + `result(...)` for intentional side effects.
- Identifiers: Unicode supported.

## Table of Contents
- [Syntax Overview](#syntax-overview)
- [Literals and Types](#literals-and-types)
- [Operators and Precedence](#operators-and-precedence)
- [Arrows and Verbs](#arrows-and-verbs)
  - [PACK](#pack)
  - [PLACE](#place)
  - [UNPACK](#unpack)
  - [PICK](#pick)
  - [STEP](#step)
  - [COUNT](#count)
  - [FISSION](#fission)
  - [FUSION](#fusion)
- [READ / WRITE](#read--write)
  - [Math Built-ins](#math-built-ins)
- [Control Flow](#control-flow)
  - [IF / ELSE](#if--else)
  - [WHILE](#while)
  - [TEST](#test)
- [Functions](#functions)
  - [Definition and Calls](#definition-and-calls)
  - [Purity and RESULT](#purity-and-result)
- [Rewire](#rewire)
  - [Mutable Operations (variable)](#mutable-operations-variable)
  - [Dynamic Symbol Binding (string)](#dynamic-symbol-binding-string)
- [String as Code](#string-as-code)
- [Semantics & Errors](#semantics--errors)
- [Complete Examples](#complete-examples)
- [Quick Reference](#quick-reference)

---

## Syntax Overview

- Statements end at newline; blank lines allowed.
- Comments start with `#`.
- Grouping with `(` `)` for expressions and `{` `}` for blocks.
- Arrows:
  - Read-from: `<-` (read from right-hand value)
  - Write-to: `->` (write to right-hand value)
- Case-insensitive keywords for verbs and control flow (e.g., `PACK` or `pack`).

## Literals and Types

- Integers: `0`, `42`, `-3`
- Floats: `3.14`, `0.0`
- Booleans: `TRUE`, `FALSE`
- Strings: `"hello"` (Unicode)
- Boxes: created with `box()` and populated with `pack`
- Functions: created via `fn(use(...)) { ... } -> name`

## Operators and Precedence

- Arithmetic: `+ - * /`
- Comparison: `< > <= >= == !=`
- Logical: `>|` (OR), `|<` (AND), `!` (NOT)
- Precedence (high → low): `* /` > `+ -` > comparisons > `>| |<`
- Grouping: `(expr)`

## Arrows and Verbs

Arrows bind verbs to their sources/targets.

### PACK
Append values to a box.

```scraps
x = box()
pack(1, 2, 3) -> x     # x = [1, 2, 3]
```

### PLACE
Update one or more indices in a box. Resizes if needed.

```scraps
place(1: 9, 3: 7) -> x  # x = [1, 9, 3, 7] (index 3 created)
```

### UNPACK
Extract an element or slice (end-exclusive) from a box or string.

```scraps
print unpack(0) <- x        # element at 0
print unpack(1, 3) <- x     # slice [1..3), e.g. [2, 3]
```

### PICK
Multi-select within one layer (box or string). For strings, returns concatenated characters.

```scraps
print pick(0, 2) <- x       # [x[0], x[2]]
```

### STEP
Scoped head selector; often combined with `PICK`.

```scraps
print :.(0) <- y            # same as unpack(0) <- y
print :.(0) pick(1, 2) <- y # pick within y[0]
```

### COUNT
Count length of boxes/strings or parameters of functions.

```scraps
print count(x)              # 3
print count(1, 3) <- x      # 2 (slice length)
```

### FISSION
Split a string by delimiter into a box of strings. Empty delimiter splits into characters.

```scraps
parts = fission(" ") <- "Hello World"  # ["Hello","World"]
chars = fission("") <- "hi!"          # ["h","i","!"]
```

### FUSION
Join a box of strings with a delimiter. (Strict: all elements must be strings.)

```scraps
print fusion("-") -> parts  # Hello-World
print fusion("") -> chars   # hi!
```

### READ / WRITE
File I/O with strings.

```scraps
WRITE("Hello") -> "tmp.txt"
content = READ <- "tmp.txt"
print content                 # Hello
```

### Math Built-ins

The Scraps VM provides comprehensive mathematical functions as built-in operations. All trigonometric functions use radians by default, with degree variants available.

#### Constants
Mathematical constants with full precision:
- `PI` - π (3.14159...)
- `TAU` - τ (2π, 6.28318...)
- `E` - e (2.71828...)

#### Scalar Operations
Basic scalar transformations:
- `abs(x)` - absolute value
- `sign(x)` - sign function (-1, 0, or 1)
- `floor(x)` - largest integer ≤ x
- `ceil(x)` - smallest integer ≥ x
- `round(x)` - nearest integer
- `trunc(x)` - truncate toward zero
- `frac(x)` - fractional part

#### Powers and Roots
Exponentiation and root functions:
- `sqrt(x)` - square root (√x)
- `cbrt(x)` - cube root (∛x)
- `pow(x, y)` - x raised to power y (x^y)
- `pow_int(x, n)` - x raised to integer power n (optimized)

#### Trigonometry (Radians)
Standard trigonometric functions:
- `sin(x)` - sine
- `cos(x)` - cosine
- `tan(x)` - tangent
- `asin(x)` - arcsine (inverse sine)
- `acos(x)` - arccosine (inverse cosine)
- `atan(x)` - arctangent (inverse tangent)
- `atan2(y, x)` - arctangent of y/x (handles quadrants)

#### Trigonometry (Degrees)
Degree-based variants for convenience:
- `sin_deg(x)` - sine of x degrees
- `cos_deg(x)` - cosine of x degrees
- `tan_deg(x)` - tangent of x degrees

#### Exponentials and Logarithms
Natural and base-specific logarithms:
- `exp(x)` - e raised to power x
- `ln(x)` - natural logarithm (base e)
- `log10(x)` - base-10 logarithm
- `log2(x)` - base-2 logarithm

#### Comparison and Clamping
Value comparison and range operations:
- `min(a, b)` - smaller of two values
- `max(a, b)` - larger of two values
- `clamp(x, lo, hi)` - constrain x to [lo, hi] range
- `nearly_equal(a, b, eps)` - approximate equality within epsilon

#### Division and Modulo
Integer division and remainder operations:
- `mod(a, b)` - remainder of a ÷ b
- `div(a, b)` - integer division (a ÷ b)
- `divmod(a, b)` - returns (quotient, remainder) as a box

#### Geometric Functions
Distance and angle calculations:
- `hypot(x, y)` - hypotenuse of right triangle (√(x² + y²))
- `length(a)` - magnitude of vector a (box of numbers)

#### Angle Conversion
Radian-degree conversions:
- `deg(x)` - convert radians to degrees
- `rad(x)` - convert degrees to radians

#### Aggregation Functions
Operations on collections of numbers:
- `sum(xs)` - sum of all values in box xs
- `mean(xs)` - arithmetic mean of values in box xs
- `dot(a, b)` - dot product of vectors a and b (boxes of numbers)

#### Sequence Generation
Create sequences of numbers:
- `linspace(start, end, n)` - n evenly spaced values from start to end
- `range(start, end, step)` - arithmetic sequence from start to end with step

Examples:

```scraps
# Constants
print PI                    # 3.14159...
print TAU                   # 6.28318...

# Basic operations
print abs(-5)              # 5
print sign(-3.14)          # -1
print floor(3.7)           # 3
print ceil(3.2)            # 4
print round(3.5)           # 4
print frac(3.7)            # 0.7

# Powers and roots
print sqrt(16)             # 4
print cbrt(27)             # 3
print pow(2, 8)            # 256
print pow_int(2, 10)       # 1024

# Trigonometry
print sin(PI/2)            # ~1
print cos(PI)              # -1
print atan2(1, 1)          # ~0.785 (π/4)
print sin_deg(90)          # 1
print cos_deg(180)         # -1

# Logarithms
print ln(E)                # 1
print log10(100)           # 2
print log2(8)              # 3

# Comparison and clamping
print min(5, 3)            # 3
print max(-1, 7)           # 7
print clamp(10, 0, 5)      # 5
print nearly_equal(3.14, PI, 0.01)  # TRUE

# Division and modulo
print mod(17, 5)           # 2
print div(17, 5)           # 3
print divmod(17, 5)        # [3, 2]

# Geometric
print hypot(3, 4)          # 5
print length([3, 4])       # 5

# Angle conversion
print deg(PI)              # 180
print rad(90)              # 1.5708...

# Aggregation
numbers = box()
pack(1, 2, 3, 4, 5) -> numbers
print sum(numbers)         # 15
print mean(numbers)        # 3
print dot([1, 2], [3, 4])  # 11

# Sequences
print linspace(0, 1, 5)    # [0, 0.25, 0.5, 0.75, 1]
print range(0, 10, 2)      # [0, 2, 4, 6, 8]
```

## Control Flow

### IF / ELSE

```scraps
print IF (1 < 2) { 42 } ELSE { 0 }    # 42
```

### WHILE

```scraps
i = 0
x = box()
WHILE (i < 3) {
  pack(i) -> x
  i = i + 1
}
print x    # [0, 1, 2]
```

### TEST
Prints `TRUE` or `FALSE` for boolean expression results.

```scraps
test (1 < 2)   # TRUE
```

## Functions

### Definition and Calls

```scraps
fn(use(a, b)) { a + b } -> add
print add(1, 2)      # 3
```

### Purity and RESULT

- Calls are pure by default (no mutation of caller’s environment).
- Use `RESULT(function)` to execute with captured env and to propagate rewire mutations (see below).

```scraps
fn(use(a, b)) { a + b } -> add
# If a,b exist in env, result(add) evaluates with those bindings
print result(add)
```

## Rewire

### Mutable Operations (variable)
Create a function that mutates a specific variable when executed via `result`.

```scraps
x = box()
rewire x {
  pack(42) -> x
  unpack(0) <- x
} -> f
print result(f)    # 42
print x            # [42]
```

- Semantics: the block runs with a mutable copy of `x`; after execution, the updated `x` is copied back to the caller’s env.
- Nested rewires are supported and memory-safe; updates are copied back in the order of `result(...)` calls.

### Dynamic Symbol Binding (string)
Register a string literal as a live variable name.

```scraps
rewire "greet"
"greet" = "Hello"
print "greet"   # Hello (resolves symbol)
```

- After rewiring, using the same string literal on the left-hand side assigns to that variable; in print contexts, the rewired symbol resolves to the variable’s value.

## String as Code

`print "..."` attempts to compile and evaluate the string as Scraps code in the current environment. If it fails to parse/compile, the raw string is printed.

- If the string literal is a rewired symbol, print resolves the symbol, and no code evaluation is attempted.

Examples:

```scraps
print "1 + 2"     # 3
rewire "msg"
"msg" = "Hi"
print "msg"       # Hi (resolves symbol, not evaluated)
```

## Semantics & Errors

- PICK: indices must be integers and within bounds; target must be box or string.
- UNPACK: start/end are integers; end is exclusive; bounds checked.
- FUSION: second argument must be a box of strings; all elements must be strings.
- READ/WRITE: filename must be a string; WRITE content must be a string.
- COUNT: accepts box, string, or function.
- Purity: only `rewire` + `result` can mutate the caller’s environment.

## Complete Examples

```scraps
# Boxes + selection
x = box()
pack(1, 2, 3) -> x

y = box()
pack(x, 4, 5, 6) -> y
print :.(0) <- y                 # [1, 2, 3]
print :.(0) pick(1, 2) <- y      # [2, 3]
print pick(0, 2) <- x            # [1, 3]

# Count + slicing
print count(x)                   # 3
print count(1, 3) <- x           # 2

# Strings + I/O
s = "Hello World Test"
parts = fission(" ") <- s
print parts                      # [Hello, World, Test]
print fusion("-") -> parts       # Hello-World-Test
WRITE(fusion("-") -> parts) -> "tmp.txt"
print READ <- "tmp.txt"

# Rewire (mutable)
x2 = box()
rewire x2 {
  pack(42) -> x2
  unpack(0) <- x2
} -> f
print result(f)                  # 42
print x2                         # [42]

# Rewire (symbol) + string as code
rewire "greet"
"greet" = "Hello"
print "greet"                    # Hello
print "1 + 2"                    # 3

# Nested rewire (box + fusion)
s2 = box()
rewire s2 {
  pack("Hello") -> s2
  rewire s2 {
    pack(" World") -> s2
  } -> inner
  result(inner)
} -> outer
print fusion("") -> s2           # Hello World
```

## Quick Reference

- Create box: `x = box()`
- Append: `pack(v1, v2, ...) -> x`
- Update: `place(i: v, ...) -> x`
- Extract: `unpack(i) <- x`, `unpack(i, j) <- x`
- Select: `pick(i, j, ...) <- x`
- Step: `:.(h) <- x`, `:.(h) pick(...) <- x`
- Count: `count(expr)` or `count(i[, j]) <- x`
- Split/Join: `fission(d) <- s`, `fusion(d) -> xs`
- I/O: `WRITE(content) -> "file"`, `READ <- "file"`
- If/Else: `IF cond { ... } ELSE { ... }`
- While: `WHILE cond { ... }`
- Test: `test expr`
- Define fn: `fn(use(a, b)) { body } -> name`
- Call: `name(args)` (pure)
- RESULT: `result(fn_value)` (propagates rewire target)
- Rewire variable: `rewire var { ... } -> dest`
- Rewire symbol: `rewire "name"`; then `"name" = expr`, `print "name"`
- String as code: `print "1 + 2"`

---

This manual documents all features supported by the VM, with examples and semantics to guide correct usage.
