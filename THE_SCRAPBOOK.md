# Scraps Language Manual

A concise, accurate reference to the Scraps language as implemented in this VM.

- Paradigm: expression‑first with newline‑terminated statements; control flow is statement‑based.
- Values: `Int`, `Float`, `Bool`, `Str` (Unicode), `Box` (list), `Function`, `None`.
- Purity: user function calls are pure by default (no caller env mutation). Use `rewire` + `result(...)` for intentional side effects.
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
- [I/O](#io)
- [Network I/O (TCP)](#network-io-tcp)
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
- [Modules](#modules)
- [Semantics & Errors](#semantics--errors)
- [Complete Examples](#complete-examples)
- [Quick Reference](#quick-reference)

---

## Syntax Overview

- Statements end at newline; blank lines allowed.
- Comments start with `#`.
- Grouping with `(` `)` for expressions and `{` `}` for blocks.
- Control flow keywords (`IF`, `WHILE`) are statements, not expressions.
- Arrows:
  - Read‑from: `<-` (read from right‑hand value)
  - Write‑to: `->` (write to right‑hand value)
- Keywords are case‑insensitive (e.g., `PACK` or `pack`).

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
Extract an element or slice (end‑exclusive) from a box or string.

```scraps
print unpack(0) <- x        # element at 0
print unpack(1, 3) <- x     # slice [1..3), e.g. [2, 3]
```

### PICK
Multi‑select within one layer (box or string). For strings, returns concatenated characters.

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

## I/O

File I/O with strings.

```scraps
WRITE("Hello") -> "tmp.txt"
content = READ <- "tmp.txt"
print content                 # Hello
```

## Network I/O (TCP)

Synchronous TCP sockets exposed as built-in functions. Works at top level and inside functions.

- `tcp_listen(port) -> listener`: Bind to `127.0.0.1:port` and return a listener handle.
- `tcp_accept(listener) -> connection`: Accept a single inbound connection and return a connection handle.
- `tcp_connect(host, port) -> connection`: Connect to a TCP server and return a connection handle.
- `tcp_send(connection, data) -> TRUE`: Send a string payload over a connection.
- `tcp_receive(connection, max_bytes) -> string`: Read up to `max_bytes` and return a string (UTF‑8).
- `tcp_close(connection_or_listener) -> TRUE`: Close a connection or a listener.

Notes:
- Blocking behavior: `tcp_accept` blocks until a client connects. For single-script demos, connect first, then accept.
- Timeouts: Connections have short read/write timeouts; `tcp_receive` returns an empty string on timeout.
- Encoding: `tcp_receive` expects UTF‑8. Invalid UTF‑8 returns an error.
- Scope: Built-ins (not opcodes). They compose like normal function calls and can be used inside `fn{}` bodies.

Examples

1) Local echo roundtrip (single script)

```scraps
port = 9091
tcp_listen(port) -> l
tcp_connect("127.0.0.1", port) -> c_client
tcp_accept(l) -> c_server

tcp_send(c_client, "ping") -> _
tcp_receive(c_server, 1024) -> srv_data
print srv_data              # ping

tcp_send(c_server, "pong") -> _
tcp_receive(c_client, 1024) -> cli_data
print cli_data              # pong

tcp_close(c_client) -> _
tcp_close(c_server) -> _
tcp_close(l) -> _
```

2) Simple server loop (single connection)

```scraps
tcp_listen(9092) -> l
print "waiting..."
tcp_accept(l) -> c
print "connected"

i = 0
WHILE (i < 3) {
  tcp_receive(c, 1024) -> msg
  print msg
  tcp_send(c, fusion("") -> ["ok:", msg]) -> _
  i = i + 1
}

tcp_close(c) -> _
tcp_close(l) -> _
```

3) Client function usage

```scraps
fn(use(host, port)) {
  tcp_connect(host, port) -> c
  tcp_send(c, "hello") -> _
  tcp_receive(c, 1024) -> reply
  tcp_close(c) -> _
  reply
} -> ping_once

print ping_once("127.0.0.1", 9092)
```

## Math Built-ins

The VM provides comprehensive mathematical functions. Trig functions use radians by default; degree variants are available.

Constants:
- `PI` (π), `TAU` (2π), `E` (e)

Scalars:
- `abs, sign, floor, ceil, round, trunc, frac`

Powers/roots:
- `sqrt, cbrt, pow, pow_int`

Trig (rad):
- `sin, cos, tan, asin, acos, atan, atan2`

Trig (deg):
- `sin_deg, cos_deg, tan_deg`

Exponentials/logs:
- `exp, ln, log10, log2`

Compare/clamp:
- `min, max, clamp, nearly_equal`

Division/modulo:
- `mod, div, divmod`

Geometry:
- `hypot, length` (Euclidean norm for numeric boxes)

Angle conversion:
- `deg, rad`

Aggregation:
- `sum, mean, dot`

Sequences:
- `linspace(start, end, n)`, `range(start, end, step)`

Examples:

```scraps
print PI                   # 3.14159...
print abs(-5)              # 5
print sqrt(16)             # 4
print sin(PI/2)            # ~1
print ln(E)                # 1
print divmod(17, 5)        # [3, 2]
print length([3, 4])       # 5
print linspace(0, 1, 5)    # [0, 0.25, 0.5, 0.75, 1]
```

## Control Flow

### IF / ELSE

```scraps
IF (1 < 2) {
  print 42
} ELSE {
  print 0
}
```

Notes:
- IF/ELSE are statements; they do not yield a value. Use prints or assignments within branches.
- Conditions must evaluate to `Bool`.

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
Prints a boolean expression’s result (`TRUE` or `FALSE`).

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

Semantics: the block runs with a mutable copy of `x`; after execution, the updated `x` is copied back to the caller’s env.

### Dynamic Symbol Binding (string)
Register a string literal as a live variable name.

```scraps
rewire "greet"
"greet" = "Hello"
print "greet"   # Hello (resolves symbol)
```

After rewiring, using the same string literal on the left‑hand side assigns to that variable; in print contexts, the rewired symbol resolves to the variable’s value.

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

## Modules

Ship a set of definitions as a module, then import them later.

- `ship("Name")`: snapshots the current environment (excluding built‑ins) under module Name.
- `ship(factory_fn)`: executes a zero‑arg function to build a module; exports are any new or changed definitions from its body; the module name is the function’s name.
- `import(name1, name2, ...) <- src`: loads from a source and injects only selected exports into the current environment; if exactly one selector is given and it matches/aliases the module name, imports the whole module under that name.
- `import("Name")`: import all exports from a previously shipped module in the current environment.
- `source("file.scraps")`: evaluate a file directly and return its last value (useful for ad‑hoc loading).

Import source resolution order for `src`:
1) `clanker.toml`: searched upward from CWD, or overridden by `IGNITE`. Resolves `src` as a key in `[modules]`. If `[paths] sources = "dir"` exists, paths are resolved relative to that directory.
2) `Scraps.toml`: fallback manifest; resolves keys similarly.
3) File path: if no manifest key matches, `src` is treated as a filesystem path and loaded.

Examples:

```scraps
# Function‑based module
fn(use()) {
  fn(use(x)) { x * 2 } -> double
  PI = 3.14159
} -> create_math_module
ship(create_math_module)
import(create_math_module)
print double(10)   # 20

# Snapshot current env as a module
fn(use(x)) { x + 10 } -> add10
ANSWER = 42
ship("Utils")
ANSWER = 0
import("Utils")
print ANSWER       # 42

# Selective import from a manifest key or file
import(double) <- create_math_module
print double(7)    # 14

# Source a file directly (no manifest)
print source("lib/math.scraps")
```

## Semantics & Errors

- PICK: indices must be integers and within bounds; target must be box or string.
- UNPACK: start/end are integers; end is exclusive; bounds checked.
- FUSION: second argument must be a box of strings; all elements must be strings.
- READ/WRITE: filename must be a string; WRITE content must be a string.
- COUNT: accepts box, string, or function.
- Control flow: `IF` and `WHILE` are statements (not expressions).
- Purity: only `rewire` + `result` can mutate the caller’s environment.
- PLACE: indices must be integers; resizing fills with `None`.

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
- Network I/O: `tcp_listen(p)->l`, `tcp_accept(l)->c`, `tcp_connect(h,p)->c`, `tcp_send(c,s)`, `tcp_receive(c,n)`, `tcp_close(x)`
- If/Else: `IF cond { ... } ELSE { ... }`
- While: `WHILE cond { ... }`
- Test: `test expr`
- Define fn: `fn(use(a, b)) { body } -> name`
- Call: `name(args)` (pure)
- RESULT: `result(fn_value)` (propagates rewire target)
- Rewire variable: `rewire var { ... } -> dest`
- Rewire symbol: `rewire "name"`; then `"name" = expr`, `print "name"`
- String as code: `print "1 + 2"`
- Modules: `ship("Name")`, `ship(factory)`, `import(name,...) <- src`, `import("Name")`, `source("file")`

---

This manual documents features supported by the VM, with examples and semantics to guide correct usage. (Note: a `rename` helper is intentionally omitted here.)
