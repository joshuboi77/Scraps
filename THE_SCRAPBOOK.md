# 📚 The Scraps Language Scrapbook

**A comprehensive manual for the Scraps programming language**

> **Navigation Tip**: Use GitHub's table of contents (📋 icon) or Ctrl+F to quickly find what you need!

---

## 🚀 Quick Start

### Language Overview
- **Paradigm**: Expression-first with newline-terminated statements
- **Values**: `Int`, `Float`, `Bool`, `Str`, `Box` (list), `Function`, `None`
- **Purity**: Functions are pure by default, use `rewire` for side effects
- **Unicode**: Full Unicode support for identifiers and strings

### Hello World
```scraps
print "Hello, World!"
x = 42
y = x + 17
print y
```

---

## 📋 Table of Contents

### 🎯 **Core Language**
- [**Syntax & Basics**](#-syntax--basics) - Variables, expressions, comments
- [**Data Types**](#-data-types) - Numbers, strings, booleans, boxes
- [**Operators**](#-operators) - Arithmetic, comparison, logical
- [**Control Flow**](#-control-flow) - IF/ELSE, WHILE, TEST

### 🔧 **Advanced Features**
- [**Functions**](#-functions) - Definition, calls, purity
- [**Arrows & Data Flow**](#-arrows--data-flow) - PACK, UNPACK, data manipulation
- [**Rewire System**](#-rewire-system) - Side effects and mutations

### 🌐 **Built-in Functions**
- [**Core-8 Hardware Functions**](#-core-8-hardware-functions) - Low-level system access
- [**Character Classification**](#-character-classification) - Text analysis
- [**Number Parsing**](#-number-parsing) - String to number conversion
- [**String Manipulation**](#-string-manipulation) - Text processing
- [**Math Functions**](#-math-functions) - Mathematical operations
- [**I/O Functions**](#-io-functions) - Input/output operations
- [**Network Functions**](#-network-functions) - TCP networking

### 📖 **Reference**
- [**Complete Examples**](#-complete-examples) - Real-world code samples
- [**Function Reference**](#-function-reference) - Alphabetical function list
- [**Error Handling**](#-error-handling) - Common errors and solutions
- [**Performance Notes**](#-performance-notes) - Optimization tips

---

# 🎯 Core Language

## 📝 Syntax & Basics

### Variables and Assignment
```scraps
# Variable assignment
x = 42
name = "Alice"
is_ready = TRUE

# Expressions
result = x + 17
message = "Hello, " + name
```

### Comments
```scraps
# Single line comment
x = 42  # End of line comment

# Multi-line comments not supported
# Use multiple single-line comments instead
```

### Expression Evaluation
```scraps
# Expressions are evaluated left to right with standard precedence
result = 2 + 3 * 4    # 14 (not 20)
grouped = (2 + 3) * 4 # 20

# Newlines terminate statements
x = 42
y = 17
```

---

## 🏗️ Data Types

### Numbers
```scraps
# Integers
age = 25
negative = -42
large = 999999

# Floating point
pi = 3.14159
temperature = -12.5
scientific = 1.23e-4
```

### Strings
```scraps
# String literals
greeting = "Hello, World!"
empty = ""
with_quotes = "She said \"Hello\""

# Unicode support
unicode_text = "🚀 Scraps Language 🎯"
chinese = "你好世界"
```

### Booleans
```scraps
# Boolean values
is_true = TRUE
is_false = FALSE

# Boolean expressions
result = (x > 5) |< (y < 10)  # Logical AND
```

### Boxes (Lists)
```scraps
# Create empty box
my_box = box()

# Add items
pack(42) -> my_box
pack("hello") -> my_box
pack(TRUE) -> my_box

# Access items
first_item = unpack(0) <- my_box
second_item = unpack(1) <- my_box

# Get size
size = count(my_box)
```

### None
```scraps
# None represents absence of value
empty_var = None
result = some_function()  # May return None
```

---

## ⚡ Operators

### Arithmetic Operators
```scraps
# Basic arithmetic
sum = 5 + 3        # 8
difference = 5 - 3 # 2
product = 5 * 3    # 15
quotient = 5 / 3   # 1.666...
remainder = 5 mod 3 # 2

# Unary operators
negative = -42
positive = +17
```

### Comparison Operators
```scraps
# Equality
equal = (5 == 5)      # TRUE
not_equal = (5 != 3)  # TRUE

# Ordering
less = (3 < 5)        # TRUE
less_equal = (3 <= 3) # TRUE
greater = (5 > 3)     # TRUE
greater_equal = (5 >= 5) # TRUE
```

### Logical Operators
```scraps
# Logical AND
both_true = TRUE |< TRUE   # TRUE
mixed = TRUE |< FALSE      # FALSE
```

### Operator Precedence
1. Unary (`-`, `+`)
2. Multiplication, Division, Modulo (`*`, `/`, `mod`)
3. Addition, Subtraction (`+`, `-`)
4. Comparisons (`<`, `<=`, `>`, `>=`)
5. Equality (`==`, `!=`)
6. Logical AND (`|<`)

---

## 🔄 Control Flow

### IF/ELSE Statements
```scraps
# Simple IF
IF (x > 0) {
  print "Positive number"
} ELSE {
  print "Zero or negative"
}

# Nested conditions
IF (score >= 90) {
  grade = "A"
} ELSE {
  IF (score >= 80) {
    grade = "B"
  } ELSE {
    IF (score >= 70) {
      grade = "C"
    } ELSE {
      grade = "F"
    }
  }
}

# Note: ELSE is always required
```

### WHILE Loops
```scraps
# Basic loop
counter = 0
WHILE (counter < 5) {
  print counter
  counter = counter + 1
}

# Loop with condition
found = FALSE
i = 0
WHILE ((i < count(items)) |< (found == FALSE)) {
  IF (unpack(i) <- items == target) {
    found = TRUE
  } ELSE {
    i = i + 1
  }
}
```

### TEST Assertions
```scraps
# Test assertions for validation (halts on failure)
x = 42
test (x == 42)        # OK
test (x > 50)         # Fails: halts with error pointing to line

# Use in testing and validation
result = calculate_something()
test (result > 0)     # Halts with clear error if FALSE or non-boolean
```
Behavior:
- Evaluates the expression; if it is `TRUE`, execution continues.
- If `FALSE`, execution halts with: `TEST failed: condition evaluated to FALSE (at file:line)`.
- If the expression is not boolean, execution halts with: `TEST failed: non-boolean value: <value>` and context.

---

## 📦 Module System

### Project Structure with clanker.toml
```toml
# clanker.toml - Project manifest file
[modules]
# Define module mappings
lexer = "lexer.scraps"
parser = "parser.scraps" 
main = "main.scraps"

[paths]
# Source directory
sources = "void"
```

### IMPORT - Loading Modules
```scraps
# Import module by name (from clanker.toml)
import("lexer")

# Import with alias
import("lex") <- "lexer"

# After import, module's global variables and functions become available
print count(KEYWORDS)  # Access KEYWORDS from lexer module

# Import multiple modules
import("lexer")
import("codegen")
import("parser")
```

### SHIP - Exporting Module Definitions
```scraps
# Create module factory function
fn(use()) {
  # Define module contents
  MODULE_VERSION = "1.0.0"
  
  fn(use(x)) {
    x * 2
  } -> double_value
  
  # Ship exports all definitions created in this function
  ship_result = ship()
  ship_result
} -> my_module

# Execute to create and export the module
result(my_module)  # Returns "Module 'my_module' shipped with N exports"
```

### RENAME - Renaming Modules
```scraps
# Rename a module in the module system
result1 = rename("old_module_name", "new_module_name")
print result1  # "OK" if successful

# After renaming, import with new name
import("new_module_name")
```

### REWIRE_SYMBOL - Dynamic Symbol Binding
```scraps
# Mark symbols as rewirable for dynamic binding
rewire_symbol("dynamic_var")    # Returns None
rewire_symbol("dynamic_func")   # Returns None

# Now these symbols can be used with rewire statements
fn(use()) {
  rewire dynamic_var = "Hello World!"
  rewire dynamic_func = some_function
  dynamic_var
} -> setup_dynamics
```

### Module Development
```scraps
# my_module.scraps - Define functions and variables
CONSTANTS = box()
pack("VALUE1", "VALUE2") -> CONSTANTS

fn(use(x, y)) {
  x + y
} -> add_function

# Use ship() for formal exports
fn(use()) {
  # Reference definitions to export them
  CONSTANTS
  add_function
  ship()  # Export all definitions created in this scope
} -> my_module_factory

result(my_module_factory)  # Execute to ship the module
```

**Use Cases**: Code organization, reusable libraries, modular compilation, dependency management

**Performance**: Module loading ~2-5ms, import resolution ~500ns per symbol

---

# 🔧 Advanced Features

## ⚙️ Functions

### Function Definition
```scraps
# Basic function
fn(use(x, y)) {
  sum = x + y
  sum
} -> add_numbers

# Function with multiple operations
fn(use(name, age)) {
  greeting = "Hello, " + name
  info = "You are " + age + " years old"
  result = greeting + ". " + info
  result
} -> create_greeting
```

### Function Calls
```scraps
# Call function
result = add_numbers(5, 3)
message = create_greeting("Alice", "25")

# Functions are expressions
total = add_numbers(10, add_numbers(5, 3))
```

### Pure Functions
```scraps
# Functions are pure by default - no side effects
fn(use(x)) {
  # This cannot modify variables outside the function
  local_var = x * 2
  local_var
} -> double_value

# To access function result, use result()
print result(double_value(5))  # Prints 10
```

---

## ➡️ Arrows & Data Flow

### PACK - Adding to Boxes
```scraps
# Pack single items
my_box = box()
pack(42) -> my_box
pack("hello") -> my_box

# Pack multiple items
pack(1, 2, 3) -> my_box
```

### UNPACK - Extracting from Boxes
```scraps
# Unpack by index
first = unpack(0) <- my_box
second = unpack(1) <- my_box

# Unpack with bounds checking
IF (count(my_box) > 2) {
  third = unpack(2) <- my_box
} ELSE {
  third = None
}
```

### PICK - Multi-Select from Boxes
```scraps
# Pick single element (same as unpack)
my_box = box()
pack("apple", "banana", "cherry", "date") -> my_box
second = pick(1) <- my_box  # "banana"

# Pick multiple elements by indices
selected = pick(0, 2, 3) <- my_box  # ["apple", "cherry", "date"]

# Pick from strings
text = "hello"
char = pick(1) <- text        # "e"
chars = pick(1, 2, 4) <- text # "elo"
```

### PLACE - Setting Values at Indices
```scraps
# Place value at specific index
my_box = box()
pack("apple", "banana", "cherry") -> my_box

# Replace existing value
place(1:"ORANGE") -> my_box  # ["apple", "ORANGE", "cherry"]

# Place beyond current size (auto-resizes with None)
place(5:"GRAPE") -> my_box   # ["apple", "ORANGE", "cherry", None, None, "GRAPE"]

# Multiple placements
place(0:"FIRST", 2:"THIRD") -> my_box
```

### String Operations
```scraps
# FISSION - Split string into characters
text = "hello"
chars = fission("") <- text
# chars = ["h", "e", "l", "l", "o"]

# FUSION - Join characters into string
chars = box()
pack("h", "e", "l", "l", "o") -> chars
text = fusion("") -> chars
# text = "hello"

# Split by single delimiter
sentence = "apple,banana,cherry"
fruits = fission(",") <- sentence
# fruits = ["apple", "banana", "cherry"]

# Split by multiple delimiters (NEW!)
source = "x = 42 + 17\nprint x"
delims = box()
pack(" ") -> delims
pack("\n") -> delims
tokens = fission(delims) <- source
# tokens = ["x", "=", "42", "+", "17", "print", "x"]

# CONTAINS - Check if item exists in box
keywords = box()
pack("print") -> keywords
pack("if") -> keywords
pack("while") -> keywords

result = contains(keywords, "print")  # TRUE
result = contains(keywords, "xyz")    # FALSE
```

### COUNT - Getting Size
```scraps
# Count items in box
my_box = box()
pack(1, 2, 3) -> my_box
size = count(my_box)  # 3

# Count characters in string
text = "hello"
length = count(fission("") <- text)  # 5
```

---

## 🔄 Rewire System

### Variable Rewiring
```scraps
# Rewire allows controlled side effects
counter = 0

fn(use()) {
  rewire counter = counter + 1
  counter
} -> increment

# Call and get result
new_value = result(increment())
print new_value  # 1
```

### Dynamic Symbol Binding
```scraps
# Rewire with string for dynamic binding
var_name = "dynamic_var"
rewire var_name = "Hello, World!"

# This creates a variable named "dynamic_var"
print dynamic_var  # "Hello, World!"
```

---

# 🌐 Built-in Functions

## 🔧 Core-8 Hardware Functions

Low-level hardware access functions for systems programming:

### Memory Operations
```scraps
# Read from memory
value = mem_load(address, width)  # width: 1, 2, 4, or 8 bytes

# Write to memory
mem_store(address, value, width)

# Memory barrier for synchronization
mem_fence()

# Atomic compare-and-swap
old_value = mem_cmpxchg(address, expected, new_value)
```

### CPU Control
```scraps
# Disable interrupts
old_state = int_disable()

# Enable interrupts
int_enable(old_state)

# Halt CPU until interrupt
cpu_halt()
```

### Timing
```scraps
# Get current time counter
current_time = time_counter()

# Get timer frequency
frequency = time_freq()

# Calculate elapsed time
start = time_counter()
# ... do work ...
end = time_counter()
elapsed_ns = (end - start) * 1000000000 / time_freq()
```

**Use Cases**: Device drivers, operating systems, real-time systems, hardware control

**Performance**: Memory operations <100ns, timing functions ~200-400ns

---

## 🔤 Character Classification

Functions for analyzing individual characters:

### Basic Character Types
```scraps
# Check character types
is_digit("5")     # TRUE
is_digit("a")     # FALSE

is_alpha("A")     # TRUE
is_alpha("5")     # FALSE

is_space(" ")     # TRUE
is_space("a")     # FALSE

is_alnum("A")     # TRUE (alphanumeric)
is_alnum("!")     # FALSE
```

### Extended Character Classification
```scraps
# Operators: +, -, *, /, %, =, !, <, >, &, |, ^, ~
is_operator("+")     # TRUE
is_operator("=")     # TRUE
is_operator("a")     # FALSE

# Punctuation: (, ), {, }, [, ], ,, ;, :, ., ?, ", ', `
is_punctuation("(")  # TRUE
is_punctuation(".")  # TRUE
is_punctuation("a")  # FALSE

# Symbols: @, #, $, \, _
is_symbol("@")       # TRUE
is_symbol("_")       # TRUE
is_symbol("a")       # FALSE

# Get comprehensive category
get_char_category("a")  # "alpha"
get_char_category("5")  # "digit"
get_char_category("+")  # "operator"
get_char_category("(")  # "punctuation"
get_char_category("@")  # "symbol"
get_char_category(" ")  # "space"
get_char_category("€")  # "other"
```

### Character Codes
```scraps
# Get character code
code = char_code("A")  # 65

# Create character from code
char = char_from_code(65)  # "A"
```

**Use Cases**: Lexical analysis, input validation, text processing, parser construction, tokenization

**Performance**: ~583ns per operation, can process ~1.7M characters per second

---

## 🔢 Number Parsing

Convert strings to numbers with automatic whitespace handling:

```scraps
# Parse integers
num = parse_int("42")      # 42
neg = parse_int("-17")     # -17
spaces = parse_int("  123  ")  # 123 (auto-trim)

# Parse floating point
pi = parse_float("3.14159")     # 3.14159
sci = parse_float("1.23e-4")    # 0.000123 (scientific notation)
zero = parse_float("0.0")       # 0.0

# Error handling
# parse_int("abc") -> Runtime error with descriptive message
```

**Use Cases**: Configuration parsing, user input processing, data file reading, calculator implementation

**Performance**: ~1250ns per operation, can parse ~800K numbers per second

---

## 📝 String Manipulation

Comprehensive text processing functions:

### Case Conversion
```scraps
upper = to_upper("hello")    # "HELLO"
lower = to_lower("WORLD")    # "world"
mixed = to_upper("Hello!")   # "HELLO!"
```

### Substring Operations
```scraps
# Extract substring
text = "Hello, World!"
greeting = substring(text, 0, 5)    # "Hello"
world = substring(text, 7, 12)      # "World"

# Find substring
pos = index_of("Hello World", "World")  # 6
not_found = index_of("abc", "xyz")      # -1

# Check prefix/suffix
starts = starts_with("Hello", "He")     # TRUE
ends = ends_with("World", "ld")         # TRUE
```

**Use Cases**: Lexical analysis, template processing, configuration parsing, text search and replace

**Performance**: ~833ns per operation, excellent for text processing

---

## 📊 Math Functions

Comprehensive mathematical operations:

### Constants
```scraps
PI    # 3.14159...
TAU   # 6.28318... (2π)
E     # 2.71828...
```

### Basic Functions
```scraps
# Absolute value and sign
abs(-5)    # 5
sign(-3)   # -1
sign(0)    # 0
sign(7)    # 1

# Min/max
min(5, 3)  # 3
max(5, 3)  # 5

# Power and roots
pow(2, 3)  # 8
sqrt(16)   # 4
```

### Trigonometry (radians)
```scraps
sin(PI / 2)    # 1.0
cos(0)         # 1.0
tan(PI / 4)    # 1.0

# Inverse functions
asin(1)        # PI/2
acos(1)        # 0
atan(1)        # PI/4
atan2(1, 1)    # PI/4
```

### Logarithms
```scraps
ln(E)      # 1.0 (natural log)
log10(100) # 2.0 (base 10)
log2(8)    # 3.0 (base 2)
```

### Rounding
```scraps
floor(3.7)   # 3
ceil(3.2)    # 4
round(3.5)   # 4
trunc(3.9)   # 3
```

---

## 💾 I/O Functions

### Basic Output
```scraps
# Print values
print "Hello"
print 42
print TRUE

# Print expressions
print (5 + 3)
print ("Result: " + result)
```

---

## 🌐 Network Functions

TCP networking capabilities:

### Client Operations
```scraps
# Connect to server
socket = tcp_connect("127.0.0.1", 8080)

# Send data
tcp_send(socket, "Hello, Server!")

# Receive data
response = tcp_receive(socket, 1024)

# Close connection
tcp_close(socket)
```

### Server Operations
```scraps
# Listen for connections
server = tcp_listen("0.0.0.0", 8080)

# Accept client
client = tcp_accept(server)

# Handle client communication
message = tcp_receive(client, 1024)
tcp_send(client, "Echo: " + message)

# Cleanup
tcp_close(client)
tcp_close(server)
```

**Use Cases**: Web servers, API clients, distributed systems, network protocols

---

# 📖 Reference

## 📚 Complete Examples

### Simple Calculator
```scraps
# Calculator with basic operations
fn(use(a, b, op)) {
  IF (op == "+") {
    a + b
  } ELSE {
    IF (op == "-") {
      a - b
    } ELSE {
      IF (op == "*") {
        a * b
      } ELSE {
        IF (op == "/") {
          a / b
        } ELSE {
          "Unknown operation"
        }
      }
    }
  }
} -> calculate

# Test calculator
result1 = calculate(10, 5, "+")  # 15
result2 = calculate(10, 5, "*")  # 50
print result1
print result2
```

### Text Processing
```scraps
# Word counter
fn(use(text)) {
  words = fission(" ") <- text
  count(words)
} -> count_words

# Clean and process text
fn(use(text)) {
  # Convert to lowercase
  clean = to_lower(text)
  
  # Count words
  word_count = count_words(clean)
  
  # Return info
  result = box()
  pack("text", clean) -> result
  pack("words", word_count) -> result
  result
} -> process_text

# Test text processing
sample = "Hello World! This is a Test."
info = process_text(sample)
print info
```

### Simple Lexer
```scraps
# Basic token types
TOKEN_NUMBER = "NUMBER"
TOKEN_WORD = "WORD"
TOKEN_SPACE = "SPACE"

# Tokenize simple text
fn(use(source)) {
  tokens = box()
  chars = fission("") <- source
  i = 0
  
  WHILE (i < count(chars)) {
    char = unpack(i) <- chars
    
    IF (is_digit(char)) {
      # Collect number
      number = ""
      WHILE ((i < count(chars)) |< (is_digit(unpack(i) <- chars))) {
        number = number + unpack(i) <- chars
        i = i + 1
      }
      
      token = box()
      pack("type", TOKEN_NUMBER) -> token
      pack("value", number) -> token
      pack(token) -> tokens
    } ELSE {
      IF (is_alpha(char)) {
        # Collect word
        word = ""
        WHILE ((i < count(chars)) |< (is_alnum(unpack(i) <- chars))) {
          word = word + unpack(i) <- chars
          i = i + 1
        }
        
        token = box()
        pack("type", TOKEN_WORD) -> token
        pack("value", word) -> token
        pack(token) -> tokens
      } ELSE {
        IF (is_space(char)) {
          # Skip whitespace
          i = i + 1
        } ELSE {
          # Unknown character
          i = i + 1
        }
      }
    }
  }
  
  tokens
} -> simple_lexer

# Test lexer
source = "hello 123 world 456"
tokens = simple_lexer(source)
print tokens
```

---

## 📋 Function Reference

### Core Language Functions
- `box()` - Create empty box
- `pack(items...) -> box` - Add items to box
- `unpack(index) <- box` - Get item from box
- `pick(indices...) <- box` - Select multiple elements from box or string
- `place(index:value, ...) -> box` - Set values at specific indices in box
- `count(box)` - Get box size
- `fission(delimiter) <- string` - Split string (single delimiter)
- `fission(delimiters) <- string` - Split string (multiple delimiters in box)
- `fusion(delimiter) -> box` - Join strings
- `contains(box, item)` - Check if item exists in box
- `print value` - Output value
- `test condition` - Assert condition
- `result(function_call)` - Get function result
- `import(module_name)` - Load and import module from clanker.toml
- `import(alias) <- module` - Import module with alias
- `ship()` - Export all definitions created in current function scope
- `rename(from, to)` - Rename a module in the module system
- `rewire_symbol(symbol_name)` - Mark symbol as rewirable for dynamic binding

### Core-8 Hardware Functions
- `mem_load(ptr, width) -> int` - Read memory
- `mem_store(ptr, val, width)` - Write memory  
- `mem_fence()` - Memory barrier
- `mem_cmpxchg(ptr, expect, val) -> int` - Atomic compare-swap
- `int_disable() -> int` - Disable interrupts
- `int_enable(state)` - Enable interrupts
- `cpu_halt()` - Halt until interrupt
- `time_counter() -> int` - Get time counter
- `time_freq() -> int` - Get timer frequency

### Character Classification
- `is_digit(char) -> bool` - Check if digit
- `is_alpha(char) -> bool` - Check if letter
- `is_space(char) -> bool` - Check if whitespace
- `is_alnum(char) -> bool` - Check if alphanumeric
- `char_code(char) -> int` - Get character code
- `char_from_code(int) -> char` - Create character
- `is_operator(char) -> bool` - Operator characters (+-*/%=!<>&|^~)
- `is_punctuation(char) -> bool` - Punctuation ((){}[],;:.?"'`)
- `is_symbol(char) -> bool` - Symbols (@ # $ \ _)
- `get_char_category(char) -> string` - One of: alpha, digit, space, operator, punctuation, symbol, other

### Number Parsing
- `parse_int(string) -> int` - Parse integer
- `parse_float(string) -> float` - Parse floating point

### String Manipulation
- `to_upper(string) -> string` - Convert to uppercase
- `to_lower(string) -> string` - Convert to lowercase
- `substring(string, start, end) -> string` - Extract substring
- `index_of(string, substring) -> int` - Find substring
- `starts_with(string, prefix) -> bool` - Check prefix
- `ends_with(string, suffix) -> bool` - Check suffix

### Math Functions
- `abs(x)`, `sign(x)`, `min(x, y)`, `max(x, y)`
- `pow(x, y)`, `sqrt(x)`, `floor(x)`, `ceil(x)`, `round(x)`
- `sin(x)`, `cos(x)`, `tan(x)`, `asin(x)`, `acos(x)`, `atan(x)`
- `ln(x)`, `log10(x)`, `log2(x)`

### Network Functions
- `tcp_connect(host, port) -> socket`
- `tcp_send(socket, data)`
- `tcp_receive(socket, size) -> string`
- `tcp_try_receive(socket, size) -> string` — Non-blocking; empty string if none
- `tcp_close(socket_or_listener)`
- `tcp_listen(port) -> listener`
- `tcp_accept(listener) -> socket`

### HTTP & JSON
- `http_get(url[, headers]) -> [status:int, headers:[[k,v]], body:string]`
- `http_post(url, data[, headers]) -> [status, headers, body]`
- `http_put(url, data[, headers]) -> [status, headers, body]`
- `http_delete(url[, headers]) -> [status, headers, body]`
- `json_encode(value) -> string`
- `json_decode(string) -> value`
- Headers format: `[["Header-Name", "Value"], ...]`

### Encoding
- `base64_encode(string) -> string`
- `base64_decode(string) -> string`
- `url_encode(string) -> string`
- `url_decode(string) -> string`

### DNS
- `dns_resolve(hostname) -> ip_string`

### Events
- `event_register(socket, event_types) -> event_id` (event_types: string or box of strings: `read|write|accept|connect`)
- `event_unregister(event_id) -> bool`
- `event_poll() -> [[socket_id, event], ...]`
- `event_wait(timeout_ms|-1) -> [[socket_id, event], ...]`
- `event_wait_any(socket_list[, event_types][, timeout_ms|-1]) -> [socket_id, event] | None`

### UDP
- `udp_bind(address) -> socket` (e.g., "0.0.0.0:9999")
- `udp_send(socket, data, target_addr) -> bool`
- `udp_receive(socket, max_bytes) -> [data, source_addr]`
- `udp_try_receive(socket, max_bytes) -> [data, source_addr] | None`
- `udp_close(socket) -> bool`
- `udp_join_multicast(socket, multicast_addr[, interface_addr]) -> bool`
- `udp_leave_multicast(socket, multicast_addr[, interface_addr]) -> bool`
- `udp_set_multicast_ttl(socket, ttl:int) -> bool`
- `udp_set_multicast_loopback(socket, enabled:bool) -> bool`
- `udp_set_broadcast(socket, enabled:bool) -> bool`
- `udp_send_broadcast(socket, data, port) -> bytes_sent:int`
- `udp_send_multicast(socket, data, multicast_addr, port) -> bytes_sent:int`
- `udp_is_multicast(address) -> bool`
- `udp_is_broadcast(address) -> bool`

### TLS
- `tls_connect(host, port) -> tls_connection`
- `tls_listen(port) -> tls_listener`
- `tls_accept(tls_listener) -> tls_connection`
- `tls_send(tls_connection, data) -> bool`
- `tls_receive(tls_connection, max_bytes) -> string`
- `tls_try_receive(tls_connection, max_bytes) -> string | None`
- `tls_close(tls_connection|tls_listener) -> bool`

### WebSockets
- `ws_connect(url) -> websocket`
- `ws_send(websocket, text) -> bool`
- `ws_receive(websocket) -> string`
- `ws_try_receive(websocket) -> string` (empty string if none)
- `ws_send_binary(websocket, string_bytes) -> bool`
- `ws_receive_bytes(websocket) -> [byte:int, ...]`
- `ws_try_receive_bytes(websocket) -> [byte:int, ...]` (empty if none)
- `ws_close(websocket) -> bool`

### Raw Sockets & Packets
- `raw_socket_create(protocol:int) -> raw_socket | "Error: ..."`
- `raw_socket_set_header_included(raw_socket, included:bool) -> bool`
- `raw_socket_send(raw_socket, data:string_bytes, target_addr) -> bytes_sent:int`
- `raw_socket_receive(raw_socket, max_bytes) -> [data:string_bytes, source_addr]`
- `raw_socket_close(raw_socket) -> bool`
- `raw_socket_info(raw_socket) -> [protocol:int, protocol_name, header_included:bool]`
- `packet_build_icmp_echo(id:int, sequence:int, data:string_bytes) -> packet_bytes:string`
- `packet_build_ipv4_header(source, dest, protocol:int, data_len:int) -> header_bytes:string`
- `packet_calculate_checksum(data:string_bytes) -> checksum:int`

### Network Interfaces
- `get_interfaces() -> [name, ...]`
- `get_interface_info(name) -> [name, type, is_up, is_loopback, is_multicast, mtu, addresses:[[ip, netmask, broadcast|"None"], ...], mac]`
- `get_interface_stats(name) -> [name, is_up, mtu, address_count, has_ipv4, has_ipv6]`
- `get_primary_interface() -> name | None`
- `get_loopback_interface() -> name | None`
- `get_interfaces_by_type(type) -> [name, ...]` where type ∈ {ethernet, wireless, loopback, tunnel, virtual, unknown}
- `get_up_interfaces() -> [name, ...]`
- `get_interface_by_ip(ip) -> name | None`
- `get_best_interface(dest_host_or_ip) -> name | None`

### IPv6 Helpers & Dual Stack
- `ipv6_get_dual_stack_mode() -> mode:string`
- `ipv6_set_dual_stack_mode(mode:string) -> bool`
- `ipv6_resolve_dual_stack(host) -> [preferred_addr, preferred_family]`
- `ipv6_parse_address(address) -> [canonical, is_link_local, is_site_local, is_unique_local, is_multicast, is_loopback, is_unspecified, is_global] | Error`
- `ipv6_get_multicast_address(group) -> [address, scope_id:int, is_multicast:bool]`
- `ipv6_is_ipv6_address(address) -> bool`
- `ipv6_is_ipv4_address(address) -> bool`
- `ipv6_get_address_info(address) -> [canonical, scope_id:int, is_link_local, is_site_local, is_unique_local, is_multicast, is_loopback, is_unspecified, is_global]`
- `ipv6_get_config() -> [mode, ipv4_enabled:bool, ipv6_enabled:bool]`
- `ipv6_create_dual_stack_socket(host, port, prefer_ipv6:bool) -> [socket_addr, family, port]`

### Proxy Manager
- `proxy_set_global(protocol, proxy_type, host, port[, username][, password]) -> bool`
- `proxy_set_specific(protocol, target_host, target_port, proxy_type, proxy_host, proxy_port[, username][, password]) -> bool`
- `proxy_set_default(proxy_type, host, port[, username][, password]) -> bool`
- `proxy_remove_global(protocol) -> bool`
- `proxy_remove_specific(protocol, target_host, target_port) -> bool`
- `proxy_add_bypass(host_or_domain) -> bool`
- `proxy_remove_bypass(host_or_domain) -> bool`
- `proxy_get_bypass_list() -> [host_or_domain, ...]`
- `proxy_get_info(protocol[, target_host][, target_port]) -> [proxy_type, host, port, username, source] | None`
- `proxy_clear_all() -> bool`
- `proxy_stats() -> [global_count:int, specific_count:int, has_default:bool, bypass_count:int]`

### Timeout Manager
- `timeout_set_global(protocol, connect_ms, read_ms, write_ms) -> bool`
- `timeout_set_specific(protocol, host, port, connect_ms, read_ms, write_ms) -> bool`
- `timeout_get_info(protocol) -> [connect_ms, read_ms, write_ms, source] | None`
- `timeout_remove(protocol[, host][, port]) -> bool`
- `timeout_clear() -> bool`
- `timeout_summary() -> [global_count:int, specific_count:int, default_connect_ms:int, default_read_ms:int, default_write_ms:int]`

### Connection Pool
- `pool_configure(max_connections:int, max_idle_seconds:int, max_lifetime_seconds:int) -> bool`
- `pool_clear() -> bool`
- `pool_stats() -> [total:int, idle:int, active:int]`

### File I/O & Eval
- `read(filename) -> string`
- `write(content, filename) -> string`
- `source(filename) -> value`

### Math Extras
- `deg(rad) -> degrees`, `rad(deg) -> radians`
- `sin_deg(x)`, `cos_deg(x)`, `tan_deg(x)` (x in degrees)
- `atan2(y, x)`, `hypot(a, b)`, `trunc(x)`, `frac(x)`
- `mod(a, b)`, `div(a:int, b:int)`, `divmod(a:int, b:int) -> [q, r]`
- `pow_int(base, exponent:int)`
- `nearly_equal(a, b, eps)`
- `clamp(x, lo, hi)`

### Vector Math
- `length([x, y, ...]) -> float`
- `dot([a...], [b...]) -> float`
- `sum([x...]) -> float`, `mean([x...]) -> float`
- `linspace(start, end, n:int) -> [floats...]`
- `range(start, end, step) -> [ints|floats]`

---

## ⚠️ Error Handling

### Common Errors

**Syntax Errors:**
- Missing `ELSE` in `IF` statement
- Unmatched parentheses or braces
- Invalid operators (use `|<` instead of `&&`)

**Runtime Errors:**
- Index out of bounds in `unpack`
- Invalid arguments to functions
- Division by zero
- Type mismatches

**Function Errors:**
- Wrong number of arguments
- Invalid argument types
- Calling undefined functions

### Error Messages
Scraps provides descriptive error messages:
```
Runtime error: UNPACK: index 5 out of bounds (box has 3 elements)
Runtime error: PARSE_INT: 'abc123' is not a valid integer
Runtime error: SUBSTRING start index must be <= end index
```

### Debugging Tips
1. **Use `print` statements** to trace execution
2. **Test functions independently** before combining
3. **Check box sizes** before unpacking
4. **Validate inputs** in functions
5. **Use `test` assertions** to catch errors early

---

## ⚡ Performance Notes

### Function Call Overhead
- **Character classification**: ~583ns per operation
- **Number parsing**: ~1250ns per operation  
- **String manipulation**: ~833ns per operation
- **Core-8 operations**: <100ns to 500ns per operation

### Optimization Tips
1. **Minimize box operations** in tight loops
2. **Cache function results** when possible
3. **Use character classification** instead of string comparisons
4. **Batch string operations** when processing large texts
5. **Prefer direct arithmetic** over function calls for simple math

### Memory Usage
- Boxes grow dynamically - no need to pre-allocate
- Strings are immutable - operations create new strings
- Functions are lightweight - no significant overhead

---

## 🎯 Language Design Philosophy

### Expression-First Design
Everything that can be an expression, is an expression. This makes the language more composable and functional in nature.

### Purity by Default
Functions are pure by default, requiring explicit `rewire` for side effects. This makes code more predictable and easier to reason about.

### Unicode Support
Full Unicode support throughout the language, from identifiers to string processing.

### Simplicity
The language aims to be simple and consistent, with minimal special cases and exceptions.

---

**This completes the Scraps Language Scrapbook. For the latest updates and examples, check the test files and compiler implementation.**
