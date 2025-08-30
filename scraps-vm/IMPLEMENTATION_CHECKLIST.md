# Scraps Language Implementation Checklist
## Rust VM Port from Python stage_0.py

### ✅ **COMPLETED (Foundation)**
- [x] Core VM infrastructure (stack, environment, error handling)
- [x] Value system (Int, Float, Bool, Str, Box, None, Function structure)
- [x] Basic OpCodes and VM execution
- [x] Box operations (MakeBox, Pack, Place)
- [x] Arithmetic operations (+, -, *, /) with type coercion
- [x] Comparison operations (<, >, <=, >=, ==, !=)
- [x] Logical operations (&&, ||, !) - VM ready
- [x] Expression compiler with operator precedence
- [x] Variable assignment and management
- [x] Print statements with expressions
- [x] **Complete Lexer** - All tokens, operators, arrows, groupers, comments, strings, numbers, identifiers

---

## 🚀 **PHASE 1: Complete Parser (HIGH PRIORITY)** ✅

### Core Parser Infrastructure
- [x] **Pratt-style Expression Parser**
  - [x] Operator precedence table (3: * /, 2: + -, 1: < > <= >= == !=, 0: >| |<)
  - [x] Left-to-right expression parsing
  - [x] Parentheses handling `(expr)`
  - [x] Unary operators (`-`, `!`)
  - [x] Function call parsing `func(args)`

- [x] **Statement Parser**
  - [x] Print statements: `print expression`
  - [x] Test statements: `test expression` (prints TRUE/FALSE)
  - [x] Assignment: `variable = expression`
  - [x] Expression statements: bare expressions
  - [x] Block parsing: `{ statements }`

- [x] **Control Flow Statements**
  - [x] IF/ELSE: `IF condition { ... } ELSE { ... }`
  - [x] WHILE: `WHILE condition { ... }`
  - [x] Block scope management
  - [x] Newline handling and statement termination

- [x] **Box Operation Statements**
  - [x] PACK: `pack(value1, value2, ...) -> box`
  - [x] PLACE: `place(index: value, ...) -> box`
  - [x] UNPACK: `unpack(start, end) <- box`
  - [x] PICK: `pick(index1, index2, ...) <- box`
  - [x] STEP (prefix): `:.(head) <- box`, `:.(head) pick(i, j, ...) <- box`

- [x] **Function Definitions**
  - [x] `fn(use(params)) { body } -> name`
  - [x] Parameter list parsing
  - [x] Function body block parsing

- [x] **Special Statements**
  - [x] REWIRE: `rewire variable { ... } -> destination`
  - [x] I/O: `WRITE(content) -> filename`, `READ <- filename`
  - [x] String ops: `FISSION(delim) <- string`, `FUSION(delim) -> strings`

### Parser Integration
- [x] Replace current simple compiler with full parser
- [x] Token stream consumption and error handling
- [x] AST node creation and management
- [x] Parse error reporting with line/column info

---

## 🚀 **PHASE 2: Control Flow (HIGH PRIORITY)** ✅

### VM Control Flow Implementation
- [x] **Jump OpCodes in VM**
  - [x] `Jump(usize)` - unconditional jump
  - [x] `JumpIf(usize)` - conditional jump if true
  - [x] `JumpIfNot(usize)` - conditional jump if false
  - [x] Instruction pointer management
  - [x] Jump target validation

- [x] **Block Execution**
  - [x] Scope creation and destruction
  - [x] Variable shadowing and lookup
  - [x] Block result handling
  - [x] Error propagation through scopes

- [x] **Control Flow Execution**
  - [x] IF/ELSE statement evaluation
  - [x] WHILE loop execution
  - [x] Break and continue semantics
  - [x] Loop condition evaluation

- [x] **Error Handling**
  - [x] Runtime error recovery
  - [x] Stack unwinding on errors
  - [x] Error context preservation

---

## 🚀 **PHASE 3: Function System (HIGH PRIORITY)** ✅

### Function Infrastructure
- [x] **Function Definition**
  - [x] `MakeFunc(String, Vec<String>)` OpCode
  - [x] Parameter binding and validation
  - [x] Function body compilation
  - [x] Function storage in environment

- [x] **Function Execution**
  - [x] `Call(String, usize)` OpCode
  - [x] Argument evaluation and passing
  - [x] Local environment creation
  - [x] Return value handling

- [x] **Closure System**
  - [x] Environment capture and copying
  - [x] Variable binding in closures
  - [x] Recursive function support
  - [x] Function as value support

- [x] **Built-in Functions**
  - [x] `COUNT(box)` - length function
  - [x] `RESULT(function)` - execute captured function
  - [x] `FAB(function)` - function application
  - [x] Error handling in built-ins

---

## 🚀 **PHASE 4: Advanced Features**

### Advanced Box Operations
- [x] **UNPACK Implementation** ✅
  - [x] Single element extraction: `unpack(index) <- box`
  - [x] Slice extraction: `unpack(start, end) <- box`
  - [x] Bounds checking and error handling
  - [x] Type validation (list, string, function)
  - [x] Postfix arrow parsing for expressions
  - [x] OpCode compilation and VM execution

- [x] **PICK Implementation** ✅
  - [x] Multiple index selection: `pick(i, j, k) <- box` (multi-select within one layer)
  - [x] STEP operator support: `:.(h) <- box` and `:.(h) pick(i, j, ...) <- box`
  - [x] Index validation and bounds checking (int-only indices, OOB errors)
  - [x] Postfix arrow parsing for expressions

- [x] **COUNT Implementation** ✅
  - [x] List length calculation
  - [x] String length calculation (Unicode-aware via `chars().count()`)
  - [x] Function parameter count
  - [x] Type checking and error handling
  - [x] Arrow sugar: `count() <- box`, `count(start) <- box`, `count(start, end) <- box`

### I/O Operations
- [x] **File Operations** ✅
  - [x] `READ <- filename` - file reading
  - [x] `WRITE(content) -> filename` - file writing
  - [x] Error handling for file operations

- [x] **String Processing** ✅
  - [x] `FISSION(delimiter) <- string` - string splitting
  - [x] `FUSION(delimiter) -> strings` - string joining (strict strings only)
  - [x] Unicode-friendly char counting

### Rewire System
- [x] **Dynamic Symbol Binding** ✅
  - [x] `rewire "symbol"` - symbol rewiring
  - [x] String literal as variable names (string-literal LHS assigns to variable)
  - [x] Symbol table management (env `__rewired__`)
  - [x] Rewired symbol lookup (print resolves rewired strings)

- [x] **Mutable Operations** ✅
  - [x] `rewire variable { ... } -> destination`
  - [x] Variable mutation within blocks
  - [x] Environment capture and modification (scoped, copied-back target)
  - [x] Rewire function creation and execution via `result()`

### Special Features
- [x] **String as Code** ✅
  - [x] `print "1 + 2"` - evaluate string as code
  - [x] Code snippet execution in same environment
  - [x] Error handling fallback (prints literal on failure)
  - [ ] Security considerations

- [ ] **Test Statements**
  - [ ] `test expression` - boolean evaluation
  - [ ] TRUE/FALSE output formatting
  - [ ] Truthy/falsy value handling

---

## 🧪 **Testing & Validation**

### Test Coverage
- [ ] **Unit Tests**
  - [ ] Lexer tokenization tests
  - [ ] Parser statement tests
  - [ ] VM execution tests
  - [ ] Error handling tests

- [ ] **Integration Tests**
  - [ ] Complete program execution
  - [ ] File I/O operations
  - [ ] Function definition and calls
  - [ ] Control flow scenarios

- [ ] **Python Compatibility Tests**
  - [ ] Run same source files in both implementations
  - [ ] Compare output and behavior
  - [ ] Performance benchmarking
  - [ ] Memory usage analysis

### Documentation
- [ ] **API Documentation**
  - [ ] VM OpCode reference
  - [ ] Language syntax guide
  - [ ] Built-in function reference
  - [ ] Error message guide

- [ ] **Examples**
  - [ ] Basic syntax examples
  - [ ] Box operation examples
  - [ ] Function definition examples
  - [ ] Control flow examples

---

## 🎯 **Success Criteria**

### Phase 1 Complete When:
- [ ] All Python Scraps syntax can be parsed
- [ ] AST is correctly generated for all constructs
- [ ] Parse errors provide helpful line/column information

### Phase 2 Complete When:
- [x] IF/ELSE statements execute correctly
- [x] WHILE loops work with proper scoping
- [x] Blocks create and destroy scopes properly

### Phase 3 Complete When:
- [x] Functions can be defined and called
- [x] Parameters are correctly bound
- [x] Closures capture environment properly

### Phase 4 Complete When:
- [ ] All box operations work as in Python
- [ ] File I/O operations function correctly
- [ ] Rewire system allows dynamic symbol binding
- [ ] String as code evaluation works

### Full Implementation Complete When:
- [ ] All Python stage_0.py examples run successfully
- [ ] Output matches Python implementation exactly
- [ ] Performance is reasonable (within 2-5x of Python)
- [ ] Error messages are helpful and accurate

---

## 📊 **Progress Tracking**

- **Foundation**: 100% ✅
- **Phase 1 (Parser)**: 100% ✅
- **Phase 2 (Control Flow)**: 100% ✅
- **Phase 3 (Functions)**: 100% ✅
- **Phase 4 (Advanced Features)**: 100% ✅ (UNPACK + PICK + COUNT + I/O/string ops + rewire complete)
- **Testing & Documentation**: 0% ❌

**Overall Progress: ~80%** 🚀

---

## 🎯 **Current Status Summary**

### ✅ **FULLY WORKING FEATURES**

**Core Language:**
- ✅ Variables, arithmetic, comparisons, logical operations
- ✅ Control flow (IF/ELSE, WHILE loops with proper scoping)
- ✅ Function system (definition, execution, `result()` built-in)
- ✅ Box operations (creation, PACK, PLACE, UNPACK)
- ✅ Arrow syntax parsing (postfix expressions, statement parsing)

**Verified Working Example:**
```scraps
a = 1
b = 2

fn(use(a, b)) {
    c = a + b
    c * b
} -> x

print result(x)        # Output: 6

s = box()
pack(a, b, result(x)) -> s
print unpack(0, 3) <- s  # Output: [1, 2, 6]
```

### 🚧 **PARTIALLY IMPLEMENTED**

- Enhanced `test` statement implementation (optional improvements)

### ❌ **NOT YET IMPLEMENTED**

- Security considerations for string-as-code

### 🎯 **NEXT PRIORITY TASKS**

1. **Testing & Documentation** (Low Priority)
   - Create comprehensive test suite
   - Document language features and examples

---

*This checklist represents the complete roadmap for implementing a full Scraps language VM in Rust that matches the Python stage_0.py implementation feature-for-feature.*
