#!/usr/bin/env python3
# Stage-0: minimal atoms language (operators: + - * / = < >) with UTF-8 identifiers and PRINT.
# Goal: tokenize → parse → eval. No parentheses, no commas, no keywords besides PRINT.
# Statements are newline-terminated. Assignment uses '='. Comparisons `<` and `>` yield booleans.
# Identifiers accept full UTF-8 via Python's str.isidentifier(); numbers are decimal ints/floats.

from __future__ import annotations
from dataclasses import dataclass
from typing import Any, List, Optional, Tuple, Dict
import sys

# ----------------------------
# Tokenization
# ----------------------------

OPERATORS = {"+", "-", "*", "/", "=", "<", ">", "!=", "<=", ">=", "==", ">|", "|<", "!"}
GROUPERS = {"(": "LP", ")": "RP", "{": "LC", "}": "RC"}

@dataclass
class Tok:
    kind: str  # 'NUM','ID','OP','NL','EOF'
    lex: str
    val: Any = None
    pos: Tuple[int,int] = (0,0)  # (line, column)

class Lexer:
    def __init__(self, src: str):
        self.s = src
        self.i = 0
        self.line = 1
        self.col = 1
        self.n = len(src)

    def _peek(self, ahead: int = 0) -> str:
        i = self.i + ahead
        return self.s[i] if i < self.n else ""

    def _adv(self) -> str:
        ch = self._peek()
        self.i += 1
        if ch == "\n":
            self.line += 1
            self.col = 1
        else:
            self.col += 1
        return ch

    def tokens(self) -> List[Tok]:
        ts: List[Tok] = []
        while self.i < self.n:
            ch = self._peek()
            # skip spaces and tabs; keep newlines
            if ch in " \t\r":
                self._adv()
                continue
            if ch == "#":  # line comment
                while self._peek() not in ("", "\n"):
                    self._adv()
                continue
            if ch == "\n":
                ts.append(Tok("NL", "\n", pos=(self.line, self.col)))
                self._adv()
                continue

            # ARROW tokens
            nxt = self.s[self.i+1] if self.i+1 < self.n else ""
            if ch == "-" and nxt == ">":
                ts.append(Tok("ARROW_R", "->", pos=(self.line, self.col)))
                self._adv(); self._adv()
                continue
            if ch == "<" and nxt == "-":
                ts.append(Tok("ARROW_L", "<-", pos=(self.line, self.col)))
                self._adv(); self._adv()
                continue

            # :. (STEP) token: must be above colon and dot rules
            if ch == ":" and self._peek(1) == ".":
                ts.append(Tok("STEP", ":.", ":.", (self.line, self.col)))
                self._adv(); self._adv()
                continue

            # comma
            if ch == ",":
                ts.append(Tok("COMMA", ",", pos=(self.line, self.col)))
                self._adv()
                continue

            # colon
            if ch == ":":
                ts.append(Tok("COLON", ":", pos=(self.line, self.col)))
                self._adv()
                continue

            # string literal: "..." with escapes \n, \t, \\, \"
            if ch == '"':
                start_line, start_col = self.line, self.col
                self._adv()  # consume opening quote
                buf = []
                while True:
                    c = self._peek()
                    if c == "":
                        raise SyntaxError(f"Unterminated string at {start_line}:{start_col}")
                    if c == '"':
                        self._adv()  # closing quote
                        break
                    if c == "\\":
                        self._adv()
                        esc = self._peek()
                        if esc == "":
                            raise SyntaxError(f"Unterminated escape at {self.line}:{self.col}")
                        # simple escapes
                        mapping = {'n': '\n', 't': '\t', '"': '"', '\\': '\\'}
                        if esc in mapping:
                            buf.append(mapping[esc])
                            self._adv()
                        else:
                            # unknown escape: keep char as-is
                            buf.append(esc)
                            self._adv()
                        continue
                    # regular char
                    buf.append(self._adv())
                ts.append(Tok("STRING", '"' + ''.join(buf) + '"', ''.join(buf), (start_line, start_col)))
                continue

            # number (int/float)
            if ch.isdigit():
                start_line, start_col = self.line, self.col
                txt = self._adv()
                dot = False
                while True:
                    c = self._peek()
                    if c.isdigit():
                        txt += self._adv()
                    elif c == "." and not dot:
                        if self._peek(1).isdigit():
                            dot = True
                            txt += self._adv()
                        else:
                            break
                    else:
                        break
                val = float(txt) if "." in txt else int(txt)
                ts.append(Tok("NUM", txt, val, (start_line, start_col)))
                continue
            # two-char operators first
            nxt = self.s[self.i+1] if self.i+1 < self.n else ""
            pair = ch + nxt
            if pair in OPERATORS:
                ts.append(Tok("OP", pair, pair, (self.line, self.col)))
                self._adv(); self._adv()
                continue
            # groupers
            if ch in GROUPERS:
                ts.append(Tok(GROUPERS[ch], ch, ch, (self.line, self.col)))
                self._adv()
                continue
            # operator (single char)
            if ch in OPERATORS:
                ts.append(Tok("OP", ch, ch, (self.line, self.col)))
                self._adv()
                continue
            # identifier (UTF-8): use Python's incremental check
            if self._is_ident_start(ch):
                start_line, start_col = self.line, self.col
                txt = self._adv()
                while True:
                    c = self._peek()
                    if c and self._is_ident_part(txt + c):
                        txt += self._adv()
                    else:
                        break
                ts.append(Tok("ID", txt, txt, (start_line, start_col)))
                continue
            # unknown
            raise SyntaxError(f"Unexpected character {repr(ch)} at {self.line}:{self.col}")
        ts.append(Tok("EOF", "", pos=(self.line, self.col)))
        return ts

    @staticmethod
    def _is_ident_start(ch: str) -> bool:
        # conservative start: must be an identifier by itself and not a digit or operator
        return (ch not in OPERATORS) and (not ch.isdigit()) and (ch == ch if ch else False) and (ch.isidentifier() or ch == "_")

    @staticmethod
    def _is_ident_part(s: str) -> bool:
        # Python's isidentifier applies to the full string; grow-and-check approach
        return s.isidentifier()

# ----------------------------
# AST nodes
# ----------------------------

@dataclass
class Num:
    value: Any

# String literal node
@dataclass
class Str:
    value: str

# Boolean literal node
@dataclass
class Bool:
    value: bool

@dataclass
class Var:
    name: str

@dataclass
class Bin:
    op: str
    left: Any
    right: Any

@dataclass
class Assign:
    name: str
    expr: Any
    _rewire: bool = False

@dataclass
class PrintStmt:
    expr: Any

@dataclass
class TestStmt:
    expr: Any

@dataclass
class ExprStmt:
    expr: Any

@dataclass
class Call:
    func: Any
    args: list

# Function definition node
@dataclass
class FuncDef:
    name: str
    params: list
    body: Any

@dataclass
class PackStmt:
    values: List[Any]
    box: Any  # expression

@dataclass
class PlaceStmt:
    pairs: List[Tuple[Any, Any]]  # (index_expr, value_expr)
    box: Any

@dataclass
class UnpackExpr:
    box: Any
    start: Any
    end: Optional[Any]  # None means single element

@dataclass
class Step:
    index: Any

@dataclass
class PickExpr:
    box: Any
    indices: List[Any]  # mix of int and Step

# ----------- Blocks and control flow ----------
@dataclass
class Block:
    items: List[Any]  # statements inside { }

@dataclass
class IfExpr:
    cond: Any
    then_block: Any  # Block
    else_block: Any  # Block

@dataclass
class WhileStmt:
    cond: Any
    body: Any  # Block

# ----------------------------
# Parser (Pratt-style for precedence, no parentheses in MVP)
# Precedence: 3: * /   2: + -   1: < >
# ----------------------------

PRECEDENCE = {
    "*": 3, "/": 3,
    "+": 2, "-": 2,
    "<": 1, ">": 1, "<=": 1, ">=": 1, "==": 1, "!=": 1,
    ">|": 0, "|<": 0,
}

class Parser:
    def __init__(self, toks: List[Tok]):
        self.ts = toks
        self.i = 0

    def _peek(self) -> Tok:
        return self.ts[self.i]

    def _pop(self) -> Tok:
        t = self.ts[self.i]
        self.i += 1
        return t

    def _accept(self, kind: str, lex: Optional[str]=None) -> Optional[Tok]:
        t = self._peek()
        if t.kind == kind and (lex is None or t.lex == lex):
            self._pop()
            return t
        return None

    def _expect(self, kind: str, lex: Optional[str]=None) -> Tok:
        t = self._peek()
        if t.kind != kind or (lex is not None and t.lex != lex):
            raise SyntaxError(f"Expected {kind} {lex or ''} at {t.pos}, got {t.kind} {t.lex}")
        return self._pop()

    def _parse_args(self) -> List[Any]:
        args: List[Any] = []
        if self._accept("RP"):
            return args
        args.append(self.expr(0))
        while self._accept("COMMA"):
            args.append(self.expr(0))
        self._expect("RP")
        return args

    def _expect_id(self, name: str) -> Tok:
        t = self._expect("ID")
        if t.lex != name:
            raise SyntaxError(f'Expected identifier "{name}" at {t.pos}, got {t.lex}')
        return t

    def _block(self) -> Block:
        # assumes current token is LC
        self._expect("LC")
        # consume leading newlines
        while self._accept("NL"):
            pass
        items: List[Any] = []
        while True:
            t = self._peek()
            if t.kind == "RC":
                self._pop()
                break
            items.append(self.stmt())
            while self._accept("NL"):
                pass
        return Block(items)

    def parse(self) -> List[Any]:
        items: List[Any] = []
        # allow leading newlines
        while self._accept("NL"):
            pass
        while self._peek().kind != "EOF":
            items.append(self.stmt())
            # require newline or EOF to terminate
            while self._accept("NL"):
                pass
        return items

    def stmt(self) -> Any:
        t = self._peek()
        # PRINT statement (reserved word only for statement position)
        if t.kind == "ID" and t.lex in ("PRINT", "print"):
            self._pop()
            expr = self.expr(0)
            return PrintStmt(expr)
        # TEST statement: evaluate expr and print TRUE/FALSE
        if t.kind == "ID" and t.lex in ("TEST", "test"):
            self._pop()
            expr = self.expr(0)
            return TestStmt(expr)

        # IF cond { ... } ELSE { ... }  (expression that yields the last value of the taken block)
        if t.kind == "ID" and t.lex in ("IF", "if"):
            self._pop()
            cond_e = self.expr(0)
            then_b = self._block()
            # optional ELSE required for Stage-0 clarity
            self._expect_id("ELSE")
            else_b = self._block()
            return IfExpr(cond_e, then_b, else_b)

        # WHILE cond { ... }
        if t.kind == "ID" and t.lex in ("WHILE", "while"):
            self._pop()
            cond_e = self.expr(0)
            body_b = self._block()
            return WhileStmt(cond_e, body_b)

        # PACK(v1, v2, ...) -> box
        if t.kind == "ID" and t.lex in ("PACK", "pack"):
            self._pop()                 # PACK
            self._expect("LP")          # (
            vals = self._parse_args()   # ) consumed inside
            if self._accept("ARROW_R"):
                box_expr = self.expr(0)
                return PackStmt(vals, box_expr)
            else:
                raise SyntaxError("Expected '->' after PACK(...)")

        # PLACE(i: v, j: w, ...) -> box
        if t.kind == "ID" and t.lex in ("PLACE", "place"):
            self._pop()                 # PLACE
            self._expect("LP")
            pairs: List[Tuple[Any, Any]] = []
            if not self._accept("RP"):
                while True:
                    idx = self.expr(0)
                    self._expect("COLON")
                    val = self.expr(0)
                    pairs.append((idx, val))
                    if self._accept("COMMA"):
                        continue
                    self._expect("RP")
                    break
            if self._accept("ARROW_R"):
                box_expr = self.expr(0)
                return PlaceStmt(pairs, box_expr)
            else:
                raise SyntaxError("Expected '->' after PLACE(...)")

        # UNPACK(a, b, ...) <- box
        if t.kind == "ID" and t.lex in ("UNPACK", "unpack"):
            self._pop()  # UNPACK
            self._expect("LP")
            start_e = self.expr(0)
            end_e: Optional[Any] = None
            if self._accept("COMMA"):
                end_e = self.expr(0)
            self._expect("RP")
            if self._accept("ARROW_L"):
                box_e = self.expr(0)
                return UnpackExpr(box_e, start_e, end_e)
            else:
                # legacy: UNPACK(box: start, ...)
                raise SyntaxError("Expected '<-' after UNPACK(...), use UNPACK(a, b) <- box")

        # COUNT(a, b, ...) <- box (if applicable)
        if t.kind == "ID" and t.lex in ("COUNT", "count"):
            self._pop()  # COUNT
            self._expect("LP")
            start_e = self.expr(0)
            end_e: Optional[Any] = None
            if self._accept("COMMA"):
                end_e = self.expr(0)
            self._expect("RP")
            if self._accept("ARROW_L"):
                box_e = self.expr(0)
                # COUNT(a, b) <- box  is interpreted as UNPACK(box, a, b) and then len(...)
                # But in MVP, just return UnpackExpr and let evaluator handle
                return Call(Var("COUNT"), [UnpackExpr(box_e, start_e, end_e)])
            else:
                raise SyntaxError("Expected '<-' after COUNT(...), use COUNT(a, b) <- box")

        # FUNCTION DEFINITION: fn(use(a, b, c)) { ... }
        # Accept both "fn" and "FN"
        if t.kind == "ID" and t.lex in ("FN", "fn"):
            self._pop()  # FN/fn
            self._expect("LP")
            fn_name = "_anon"
            # parse use(a, b, c)
            use_tok = self._expect("ID")
            if use_tok.lex not in ("USE", "use"):
                raise SyntaxError(f"Expected 'use' in function parameter list, got {use_tok.lex}")
            self._expect("LP")
            param_names = []
            if not self._accept("RP"):
                first_param = self._expect("ID")
                param_names.append(first_param.lex)
                while self._accept("COMMA"):
                    next_param = self._expect("ID")
                    param_names.append(next_param.lex)
                self._expect("RP")
            # close fn(...)
            self._expect("RP")
            # function body block
            body = self._block()
            func = FuncDef(fn_name, param_names, body)
            if self._accept("ARROW_R"):
                dest = self._expect("ID").lex
                return Assign(dest, func)
            return func

        # REWIRE b = expr
        if t.kind == "ID" and t.lex in ("REWIRE", "rewire"):
            self._pop()  # consume 'rewire'
            name_tok = self._expect("ID")
            self._expect("OP", "=")
            expr = self.expr(0)
            node = Assign(name_tok.lex, expr)
            node._rewire = True  # mark as rewire
            return node

        # assignment: IDENT = expr
        if t.kind == "ID":
            # lookahead for '='
            if self.ts[self.i+1].kind == "OP" and self.ts[self.i+1].lex == "=":
                name = self._pop().lex
                self._expect("OP", "=")
                e = self.expr(0)
                return Assign(name, e)
        # otherwise expression statement
        e = self.expr(0)
        return ExprStmt(e)

    def expr(self, min_bp: int) -> Any:
        # prefix: number or identifier; allow unary minus for numbers/vars
        t = self._pop()
        if t.kind == "STRING":
            left: Any = Str(t.val)
        elif t.kind == "NUM":
            left = Num(t.val)
        elif t.kind == "ID":
            # Special handling for PICK(...) <- box as prefix expression
            if t.lex in ("PICK", "pick"):
                self._expect("LP")
                idx_exprs = []
                # Parse first index expression, then allow STEP-prefixed step expressions
                if not self._accept("RP"):
                    # First index
                    idx_exprs.append(self.expr(0))
                    # Now allow zero or more STEP-prefixed step expressions
                    while True:
                        if not self._accept("STEP"):
                            break
                        # Next must be an expression (variable, number, etc)
                        step_expr = self.expr(0)
                        idx_exprs.append(Step(step_expr))
                    self._expect("RP")
                if self._accept("ARROW_L"):
                    box_e = self.expr(0)
                    left = PickExpr(box_e, idx_exprs)
                else:
                    raise SyntaxError("Expected '<-' after PICK(...), use PICK(i, j) <- box")
            elif t.lex == "TRUE":
                left = Bool(True)
            elif t.lex == "FALSE":
                left = Bool(False)
            elif t.lex.lower() == "result":
                # support `result(x)` as special syntax for calling RESULT
                if self._accept("LP"):
                    arg = self.expr(0)
                    self._expect("RP")
                    left = Call(Var("result"), [arg])
                else:
                    left = Var(t.lex)
            else:
                left = Var(t.lex)
                # function call form: IDENT(...)
                while self._accept("LP"):
                    args = []
                    if not self._accept("RP"):
                        args.append(self.expr(0))
                        while self._accept("COMMA"):
                            args.append(self.expr(0))
                        self._expect("RP")
                    left = Call(left, args)
        elif t.kind == "OP" and t.lex == "-":
            # unary minus binds tighter than *; treat as 4
            rhs = self.expr(4)
            left = Bin("*", Num(-1), rhs)
        elif t.kind == "OP" and t.lex == "!":
            # logical NOT binds tightly (higher than comparisons)
            rhs = self.expr(4)
            left = Call(Var("__NOT__"), [rhs])
        elif t.kind == "LP":
            left = self.expr(0)
            self._expect("RP")
        elif t.kind == "LC":
            # already consumed LC in t; put it back by stepping index back by one and call _block()
            self.i -= 1
            left = self._block()
        else:
            raise SyntaxError(f"Unexpected token {t.kind} {t.lex} at {t.pos}")

        # Allow postfix arrows on certain calls to form expressions, e.g. UNPACK(..) <- box
        while True:
            nt = self._peek()
            if nt.kind == "ARROW_L":
                # read-from arrow, valid for UNPACK(..) and COUNT(..)
                # only if the current left is a Call of those names (case-insensitive)
                if (
                    isinstance(left, Call)
                    and isinstance(left.func, Var)
                    and left.func.name in ("UNPACK", "COUNT", "unpack", "count")
                ):
                    self._pop()  # consume '<-'
                    box_e = self.expr(0)
                    if left.func.name in ("UNPACK", "unpack"):
                        # left.args is [start] or [start,end]
                        start_e = left.args[0] if len(left.args) >= 1 else Num(0)
                        end_e = left.args[1] if len(left.args) >= 2 else None
                        left = UnpackExpr(box_e, start_e, end_e)
                    else:  # COUNT or count
                        # COUNT(...) <- box : interpret as COUNT( UNPACK(box, ...) ) when args present,
                        # or COUNT(box) when no args.
                        if len(left.args) == 0:
                            left = Call(Var("COUNT"), [box_e])
                        elif len(left.args) == 1:
                            left = Call(Var("COUNT"), [UnpackExpr(box_e, left.args[0], None)])
                        else:
                            left = Call(Var("COUNT"), [UnpackExpr(box_e, left.args[0], left.args[1])])
                    continue
                else:
                    break  # arrow not applicable here
            elif nt.kind == "ARROW_R":
                # write-to arrow, valid for PACK(..) and PLACE(..)
                if isinstance(left, Call) and isinstance(left.func, Var) and left.func.name in ("PACK", "PLACE"):
                    self._pop()  # consume '->'
                    box_e = self.expr(0)
                    if left.func.name == "PACK":
                        left = PackStmt(left.args, box_e)
                    else:  # PLACE
                        # PLACE takes pairs like i: v, represented during call as [Bin(":", i, v)] or we already split earlier.
                        # Here, left.args is a flat list of expressions in the original order (i, v, i, v, ...)
                        # But our current statement parser handled COLON pairing; in expression form we'll reuse the flat list
                        # by converting pairs [e0,e1,e2,e3,...] -> [(e0,e1),(e2,e3),...]
                        args = left.args
                        if len(args) % 2 != 0:
                            raise SyntaxError("PLACE: expected pairs i: v in arguments")
                        pairs = [(args[i], args[i+1]) for i in range(0, len(args), 2)]
                        left = PlaceStmt(pairs, box_e)
                    continue
                else:
                    break  # arrow not applicable here
            else:
                break

        while True:
            nt = self._peek()
            if nt.kind != "OP":
                break
            op = nt.lex
            bp = PRECEDENCE.get(op)
            if bp is None or bp < min_bp:
                break
            self._pop()
            right = self.expr(bp + 1)
            left = Bin(op, left, right)
        return left

# ----------------------------
# Evaluator
# ----------------------------

class Env:
    def __init__(self):
        self.scopes: List[Dict[str, Any]] = [{}]
        # Set of protected names (built-ins that cannot be redefined)
        self._protected_names = {"box", "pack", "place", "unpack", "pick", "count", "print", "result"}
        # Built-ins in global scope (lowercase only)
        g = self.scopes[0]
        g["output"] = lambda v: self._builtin_output(v)
        g["box"] = lambda: []
        g["count"] = self._builtin_count
        g["__not__"] = lambda v: self._builtin_not(v)
        # Ensure "result" is present and not shadowed; no "RESULT" in builtins
        g["result"] = self._builtin_result
        g["pack"] = self._builtin_pack
        g["place"] = self._builtin_place
        g["unpack"] = self._builtin_unpack
        g["pick"] = self._builtin_pick
        g["fab"] = self._builtin_fab
        g["print"] = self._builtin_print

    # Stub definitions for built-in methods (for Pylance, etc.)
    def _builtin_pack(self, *args):
        raise NotImplementedError("_builtin_pack not yet implemented")

    def _builtin_place(self, *args):
        raise NotImplementedError("_builtin_place not yet implemented")

    def _builtin_unpack(self, *args):
        raise NotImplementedError("_builtin_unpack not yet implemented")

    def _builtin_pick(self, *args):
        raise NotImplementedError("_builtin_pick not yet implemented")

    def _builtin_print(self, *args):
        raise NotImplementedError("_builtin_print not yet implemented")
    def _builtin_fab(self, f):
        if isinstance(f, Func):
            args = [self.get(p) for p in f.params]
            return f(*args)
        raise TypeError("FAB: expects function")
    def _builtin_result(self, f):
        if isinstance(f, Func):
            # evaluate with captured param bindings from the closure environment
            args = [f.env.get(p) for p in f.params]
            return f(*args)
        raise TypeError("RESULT: expects function")

    def _fmt(self, v):
        if isinstance(v, bool):
            return "TRUE" if v else "FALSE"
        return v

    def _builtin_output(self, v):
        out = self._fmt(v)
        print(out)
        return v

    def _builtin_count(self, x):
        if isinstance(x, (list, str)):
            return len(x)
        raise TypeError("COUNT: type")

    def _builtin_not(self, v):
        if isinstance(v, bool):
            return not v
        raise TypeError("!: type")

    def get(self, name: str) -> Any:
        # No implicit case transformation or fallback to uppercase
        for scope in reversed(self.scopes):
            if name in scope:
                return scope[name]
        raise NameError(f"Undefined name: {name}")

    def set(self, name: str, val: Any) -> None:
        if name in self._protected_names:
            raise SyntaxError(f"Cannot redefine built-in: {name}")
        for scope in reversed(self.scopes):
            if name in scope:
                scope[name] = val
                return
        self.scopes[-1][name] = val

    def push(self):
        self.scopes.append({})

    def pop(self):
        self.scopes.pop()

class Func:
    def __init__(self, name: str, params: list, body: Any, env: Env):
        self.name = name
        self.params = params
        self.body = body
        # Closure: capture a copy of the current environment
        self.env = Env()
        self.env.scopes = [scope.copy() for scope in env.scopes]

    def __call__(self, *args):
        # New local environment, with params bound
        new_env = Env()
        # Deep copy of all enclosing scopes for lookup (simulate closure)
        new_env.scopes = [scope.copy() for scope in self.env.scopes]
        if len(args) != len(self.params):
            raise TypeError(f"Function {self.name} expects {len(self.params)} arguments, got {len(args)}")
        # Bind parameters in innermost scope
        for k, v in zip(self.params, args):
            new_env.scopes[-1][k] = v
        print(f"[DEBUG] Entering function {self.name} with params {self.params} and args {args}")
        # Evaluate function body in this context
        evaluator = Evaluator(new_env)
        result = evaluator.eval(self.body)
        return result

    def __repr__(self):
        joined = ', '.join(f"{self.env.get(p)}{p}" for p in self.params)
        return f"fn(use({joined})) {{ ... }}"

    def as_box(self):
        return [f"{self.env.get(p)}{p}" for p in self.params] + [self.body]

last_box_context = None

class Evaluator:
    def __init__(self, env: Env):
        self.env = env

    def eval_prog(self, items: List[Any]) -> Optional[Any]:
        last = None
        for node in items:
            last = self.eval(node)
        return last

    def eval(self, node: Any) -> Any:
        print(f"[DEBUG] Evaluating node type: {type(node).__name__}")
        if isinstance(node, Num):
            return node.value
        if isinstance(node, Str):
            return node.value
        if isinstance(node, Bool):
            return node.value
        if isinstance(node, Var):
            return self.env.get(node.name)
        if isinstance(node, Bin):
            op = node.op
            a = self.eval(node.left)
            # short-circuit logical ops with strict booleans
            if op == ">|":  # OR
                if not isinstance(a, bool):
                    raise TypeError(">|: type")
                if a:
                    return True
                b = self.eval(node.right)
                if not isinstance(b, bool):
                    raise TypeError(">|: type")
                return a or b
            if op == "|<":  # AND
                if not isinstance(a, bool):
                    raise TypeError("|<: type")
                if not a:
                    return False
                b = self.eval(node.right)
                if not isinstance(b, bool):
                    raise TypeError("|<: type")
                return a and b
            # non-boolean binary ops evaluate RHS normally
            b = self.eval(node.right)
            if op == "+": return a + b
            if op == "-": return a - b
            if op == "*": return a * b
            if op == "/": return a / b
            if op == "<": return a < b
            if op == ">": return a > b
            if op == "<=": return a <= b
            if op == ">=": return a >= b
            if op == "==": return a == b
            if op == "!=": return a != b
            raise RuntimeError(f"unknown op {op}")
        if isinstance(node, Assign):
            value = self.eval(node.expr)
            if getattr(node, "_rewire", False):
                # New logic for rewired variables:
                if node.name in self.env.scopes[-1]:
                    self.env.scopes[-1][node.name] = value
                    print(f"[DEBUG] Rewired {node.name} = {value}")
                else:
                    raise RuntimeError(f"Cannot rewire undefined name: {node.name}")
            else:
                self.env.set(node.name, value)
                print(f"[DEBUG] Assign {node.name} = {value} (from expr: {node.expr})")
            return value
        if isinstance(node, PrintStmt):
            # If printing a string literal, treat its contents as a code snippet:
            # evaluate it in the *same* environment and print the resulting value.
            if isinstance(node.expr, Str):
                src = node.expr.value
                # Ensure the snippet ends with a newline so the parser sees a complete unit
                res = run_source(src + "\n", self.env)
                print(res)
                return res
            else:
                v = self.eval(node.expr)
                print(self.env._fmt(v))
                return v
        if isinstance(node, TestStmt):
            v = self.eval(node.expr)
            # Treat truthy strictly as boolean True/False
            out = "TRUE" if bool(v) else "FALSE"
            print(out)
            return out
        if isinstance(node, ExprStmt):
            return self.eval(node.expr)
        if isinstance(node, FuncDef):
            # Store function as Func object in the environment
            fn = Func(node.name, node.params, node.body, self.env)
            self.env.set(node.name, fn)  # ensure function is stored for external visibility
            print(f"[DEBUG] Defined function {node.name}({', '.join(node.params)})")
            return fn
        if isinstance(node, Call):
            fn = self.eval(node.func)
            # Evaluate arguments for debug print (but don't double-evaluate for call)
            debug_args = [self.eval(arg) for arg in node.args]
            print(f"[DEBUG] Calling function {fn} with args {debug_args}")
            # Actually pass already-evaluated args to fn
            # To avoid double evaluation, reuse debug_args
            args = debug_args
            if callable(fn):
                return fn(*args)
            raise RuntimeError(f"{node.func} is not callable")

        if isinstance(node, PackStmt):
            box = self.eval(node.box)
            if not isinstance(box, list):
                raise TypeError("PACK: type")
            vals = [self.eval(v) for v in node.values]
            print(f"[DEBUG] PACK -> {box} with {vals}")
            box.extend(vals)
            return box

        if isinstance(node, PlaceStmt):
            box = self.eval(node.box)
            if not isinstance(box, list):
                raise TypeError("PLACE: type")
            for idx_e, val_e in node.pairs:
                i = self.eval(idx_e)
                v = self.eval(val_e)
                if not isinstance(i, int) or i < 0 or i >= len(box):
                    raise IndexError("PLACE: bounds")
                box[i] = v
            return box

        if isinstance(node, UnpackExpr):
            box = self.eval(node.box)
            if isinstance(box, Func):
                box = box.as_box()
            i0 = self.eval(node.start)
            if node.end is None:
                if isinstance(box, list):
                    if not isinstance(i0, int) or i0 < 0 or i0 >= len(box):
                        raise IndexError("UNPACK: bounds")
                    return box[i0]
                if isinstance(box, str):
                    if not isinstance(i0, int) or i0 < 0 or i0 >= len(box):
                        raise IndexError("UNPACK: bounds")
                    return box[i0]
                raise TypeError("UNPACK: type")
            else:
                i1 = self.eval(node.end)
                if not (isinstance(i0, int) and isinstance(i1, int)):
                    raise TypeError("UNPACK: index type")
                if isinstance(box, list):
                    if i0 < 0 or i1 < i0 or i1 > len(box):
                        raise IndexError("UNPACK: bounds")
                    return box[i0:i1]
                if isinstance(box, str):
                    if i0 < 0 or i1 < i0 or i1 > len(box):
                        raise IndexError("UNPACK: bounds")
                    return box[i0:i1]
                raise TypeError("UNPACK: type")

        if isinstance(node, PickExpr):
            expr = node
            value = self.eval(expr.box)
            if isinstance(value, Func):
                value = value.as_box()
            if not isinstance(expr.indices, list) or len(expr.indices) == 0:
                raise TypeError("PICK: no indices provided")
            # The first index must not be a Step
            if isinstance(expr.indices[0], Step):
                raise TypeError("First pick index must be head, not step")
            index = self.eval(expr.indices[0])
            value = value[index]
            # For each subsequent index, must be Step
            for step in expr.indices[1:]:
                if not isinstance(step, Step):
                    raise TypeError("Subsequent pick indices must be steps")
                step_index = self.eval(step.index)
                value = value[step_index]
            return value

        # Helper for evaluating expressions in the context of PickExpr (no longer needed)

        if isinstance(node, Block):
            self.env.push()
            last = None
            for it in node.items:
                last = self.eval(it)
            self.env.pop()
            return last

        if isinstance(node, IfExpr):
            cond = self.eval(node.cond)
            if not isinstance(cond, bool):
                raise TypeError("IF: type")
            return self.eval(node.then_block) if cond else self.eval(node.else_block)

        if isinstance(node, WhileStmt):
            result = None
            while True:
                cond = self.eval(node.cond)
                if not isinstance(cond, bool):
                    raise TypeError("WHILE: type")
                if not cond:
                    break
                print(f"[DEBUG] WHILE iteration with i = {self.env.get('i')}")
                result = self.eval(node.body)
            return result

        raise RuntimeError(f"unknown node {node}")

# ----------------------------
# REPL / Runner
# ----------------------------

BANNER = "scraps-lang stage-0 | ops: + - * / = < > <= >= == !=  >|  |<  ! | bools: TRUE/FALSE | control: { }, IF/ELSE, WHILE | verbs: PACK(...) -> box, PLACE(...) -> box, UNPACK(a, b) <- box, PICK(i, ...) <- box | identifiers: UTF-8 | () grouping | newline-terminated"

EXAMPLE = '''
# examples:
box = BOX()
PACK(1, 2, 3) -> box
PRINT box             # [1, 2, 3]
PRINT COUNT(box)      # 3
PRINT (UNPACK(1) <- box)  # 2
PRINT (UNPACK(1, 3) <- box)  # [2, 3]
PRINT PICK(0, 2) <- box      # [1, 3]
PRINT PICK(2, 2, 0) <- box    # [3, 3, 1]
PLACE(1: 9, 0: 7) -> box
PRINT box             # [7, 9, 3]
PRINT TRUE            # TRUE
TEST (1 < 2)          # TRUE
# String literal and PRINT as code:
# PRINT "1 + 2"        # prints 3
# PRINT "PRINT 4 + 5"  # prints 9, then None

# Logic operator demo:
PRINT (TRUE >| (PACK(99) -> box))   # TRUE  (PACK not executed)
PRINT (FALSE |< (UNPACK(0) <- box)) # FALSE (UNPACK not executed)
PRINT (!TRUE)                       # FALSE
PRINT (TRUE |< FALSE)               # FALSE
PRINT (FALSE >| (1 < 2))            # TRUE

# Control flow demo:
i = 0
WHILE (i < 3) {
  PACK(i) -> box
  i = i + 1
}
PRINT box            # [1, 2, 3, 0, 1, 2] if run after earlier PACKs; or [0,1,2] fresh
PRINT (IF TRUE { 42 } ELSE { 0 })   # 42
'''.strip()

def run_source(src: str, env: Optional[Env]=None) -> Any:
    env = env or Env()
    # preload nothing except allowing users to assign PRINT if they want; here PRINT is a statement, not a variable
    lex = Lexer(src)
    toks = lex.tokens()
    parser = Parser(toks)
    prog = parser.parse()
    ev = Evaluator(env)
    return ev.eval_prog(prog)




def repl() -> None:
    print(BANNER)
    env = Env()
    buf = ""

    def flush_block():
        nonlocal buf
        if buf.strip() == "":
            buf = ""
            return
        try:
            run_source(buf, env)
        except Exception as e:
            print(f"! {e}")
        buf = ""

    try:
        while True:
            # Print prompt without letting input() manage newlines; this prevents extra blank lines
            sys.stdout.write("» ")
            sys.stdout.flush()
            line = sys.stdin.readline()
            if not line:
                break
            line = line[:-1] if line.endswith("\n") else line
            if line.strip() == ":quit":
                break

            # Empty line triggers execution of the buffered block; no brace logic
            if line.strip() == "":
                flush_block()
                continue

            # Otherwise, accumulate into the buffer as-is
            buf += line + "\n"

    except (EOFError, KeyboardInterrupt):
        print()

if __name__ == "__main__":
    if len(sys.argv) == 1:
        repl()
    else:
        if sys.argv[1] == "--example":
            print(EXAMPLE)
            sys.exit(0)
        with open(sys.argv[1], "r", encoding="utf-8") as f:
            src = f.read()
        run_source(src)
    def _builtin_fab(self, f):
        if isinstance(f, Func):
            args = [self.get(p) for p in f.params]
            return f(*args)
        raise TypeError("FAB: expects function")
    # Built-in implementations for lowercase vocabulary
    def _builtin_pack(self, *args):
        # Expects last arg is the box
        if len(args) < 1:
            raise TypeError("pack: expects at least one argument (box)")
        *vals, box = args
        if not isinstance(box, list):
            raise TypeError("pack: last argument must be a box (list)")
        box.extend(vals)
        return box

    def _builtin_place(self, *args):
        # expects pairs of (idx, val), then box
        if len(args) < 1:
            raise TypeError("place: expects at least one argument (box)")
        box = args[-1]
        pairs = args[:-1]
        if not isinstance(box, list):
            raise TypeError("place: last argument must be a box (list)")
        if len(pairs) % 2 != 0:
            raise TypeError("place: expects pairs of (idx, value)")
        for i in range(0, len(pairs), 2):
            idx = pairs[i]
            val = pairs[i+1]
            if not isinstance(idx, int) or idx < 0 or idx >= len(box):
                raise IndexError("place: index out of bounds")
            box[idx] = val
        return box

    def _builtin_unpack(self, *args):
        # expects box, start, [end]
        if len(args) < 2:
            raise TypeError("unpack: expects at least 2 arguments (box, start)")
        box = args[0]
        start = args[1]
        end = args[2] if len(args) > 2 else None
        if isinstance(box, Func):
            box = box.as_box()
        if end is None:
            if isinstance(box, list) or isinstance(box, str):
                if not isinstance(start, int) or start < 0 or start >= len(box):
                    raise IndexError("unpack: index out of bounds")
                return box[start]
            raise TypeError("unpack: box type")
        else:
            if not (isinstance(start, int) and isinstance(end, int)):
                raise TypeError("unpack: index type")
            if isinstance(box, list) or isinstance(box, str):
                if start < 0 or end < start or end > len(box):
                    raise IndexError("unpack: slice out of bounds")
                return box[start:end]
            raise TypeError("unpack: box type")

    def _builtin_pick(self, *args):
        # expects box, i1, i2, ...
        if len(args) < 2:
            raise TypeError("pick: expects at least box and one index")
        box = args[0]
        idxs = args[1:]
        if isinstance(box, Func):
            box = box.as_box()
        for idx in idxs:
            if not isinstance(idx, int):
                raise TypeError("pick: index type")
        if isinstance(box, list):
            for idx in idxs:
                if idx < 0 or idx >= len(box):
                    raise IndexError("pick: index out of bounds")
            return [box[idx] for idx in idxs]
        if isinstance(box, str):
            for idx in idxs:
                if idx < 0 or idx >= len(box):
                    raise IndexError("pick: index out of bounds")
            return ''.join(box[idx] for idx in idxs)
        raise TypeError("pick: box type")

    def _builtin_print(self, v):
        print(self._fmt(v))
        return v