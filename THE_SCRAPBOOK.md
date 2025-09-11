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
# Test assertions for validation
x = 42
test (x == 42)        # Passes silently
test (x > 50)         # Would fail with error

# Use in testing and validation
result = calculate_something()
test (result > 0)     # Ensure positive result
```

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
- `count(box)` - Get box size
- `fission(delimiter) <- string` - Split string (single delimiter)
- `fission(delimiters) <- string` - Split string (multiple delimiters in box)
- `fusion(delimiter) -> box` - Join strings
- `contains(box, item)` - Check if item exists in box
- `print value` - Output value
- `test condition` - Assert condition
- `result(function_call)` - Get function result

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
- `tcp_close(socket)`
- `tcp_listen(host, port) -> server`
- `tcp_accept(server) -> socket`

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
