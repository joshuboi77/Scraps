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

Provided as built-in functions (radians for trig):

- Constants: `PI`, `TAU`, `E`
- Scalars: `abs(x)`, `sign(x)`, `floor(x)`, `ceil(x)`, `round(x)`, `trunc(x)`
- Powers/roots: `sqrt(x)`, `cbrt(x)`, `pow(x, y)`
- Trigonometry: `sin(x)`, `cos(x)`, `tan(x)`, `asin(x)`, `acos(x)`, `atan(x)`, `atan2(y, x)`
- Exponentials/logs: `exp(x)`, `ln(x)`, `log10(x)`, `log2(x)`
- Utilities: `min(a, b)`, `max(a, b)`, `clamp(x, lo, hi)`, `hypot(x, y)`, `deg(x)`, `rad(x)`
  - Extras: `mod(a, b)`, `div(a, b)`, `divmod(a, b)`, `pow_int(x, n)`, `frac(x)`, `nearly_equal(a, b, eps)`,
    `sum(xs)`, `mean(xs)`, `dot(a, b)`, `length(a)`, `sin_deg(x)`, `cos_deg(x)`, `tan_deg(x)`, `linspace(start, end, n)`, `range(start, end, step)`

Examples:

```scraps
print sqrt(9)        # 3
print sin(PI/2)     # ~1
print pow(2, 10)    # 1024
print clamp(5, 0, 3)# 3
print hypot(3, 4)   # 5
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
