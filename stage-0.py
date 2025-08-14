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
GROUPERS = {"(": "LP", ")": "RP"}

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

    def _peek(self) -> str:
        return self.s[self.i] if self.i < self.n else ""

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
                        dot = True
                        txt += self._adv()
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

# PICK(i1, i2, ...) <- box
@dataclass
class PickExpr:
    box: Any
    indices: List[Any]

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
        if t.kind == "ID" and t.lex == "PRINT":
            self._pop()
            expr = self.expr(0)
            return PrintStmt(expr)
        # TEST statement: evaluate expr and print TRUE/FALSE
        if t.kind == "ID" and t.lex == "TEST":
            self._pop()
            expr = self.expr(0)
            return TestStmt(expr)

        # PACK(v1, v2, ...) -> box
        if t.kind == "ID" and t.lex == "PACK":
            self._pop()                 # PACK
            self._expect("LP")          # (
            vals = self._parse_args()   # ) consumed inside
            if self._accept("ARROW_R"):
                box_expr = self.expr(0)
                return PackStmt(vals, box_expr)
            else:
                raise SyntaxError("Expected '->' after PACK(...)")

        # PLACE(i: v, j: w, ...) -> box
        if t.kind == "ID" and t.lex == "PLACE":
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
        if t.kind == "ID" and t.lex == "UNPACK":
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

        # PICK(i1, i2, ...) <- box  (non-contiguous selection)
        if t.kind == "ID" and t.lex == "PICK":
            self._pop()  # PICK
            self._expect("LP")
            idx_exprs: List[Any] = []
            if not self._accept("RP"):
                idx_exprs.append(self.expr(0))
                while self._accept("COMMA"):
                    idx_exprs.append(self.expr(0))
                self._expect("RP")
            if self._accept("ARROW_L"):
                box_e = self.expr(0)
                return PickExpr(box_e, idx_exprs)
            else:
                raise SyntaxError("Expected '<-' after PICK(...), use PICK(i, j) <- box")

        # COUNT(a, b, ...) <- box (if applicable)
        if t.kind == "ID" and t.lex == "COUNT":
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
            if t.lex == "TRUE":
                left = Bool(True)
            elif t.lex == "FALSE":
                left = Bool(False)
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
        else:
            raise SyntaxError(f"Unexpected token {t.kind} {t.lex} at {t.pos}")

        # Allow postfix arrows on certain calls to form expressions, e.g. UNPACK(..) <- box
        while True:
            nt = self._peek()
            if nt.kind == "ARROW_L":
                # read-from arrow, valid for UNPACK(..) and COUNT(..)
                # only if the current left is a Call of those names
                if isinstance(left, Call) and isinstance(left.func, Var) and left.func.name in ("UNPACK", "COUNT"):
                    self._pop()  # consume '<-'
                    box_e = self.expr(0)
                    if left.func.name == "UNPACK":
                        # left.args is [start] or [start,end]
                        start_e = left.args[0] if len(left.args) >= 1 else Num(0)
                        end_e = left.args[1] if len(left.args) >= 2 else None
                        left = UnpackExpr(box_e, start_e, end_e)
                    else:  # COUNT
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
        self.vars: Dict[str, Any] = {}
        # Built-in functions
        self.vars["OUTPUT"] = lambda v: self._builtin_output(v)
        self.vars["BOX"] = lambda: []
        self.vars["COUNT"] = self._builtin_count
        self.vars["__NOT__"] = lambda v: self._builtin_not(v)

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
        if name in self.vars:
            return self.vars[name]
        raise NameError(f"Undefined name: {name}")

    def set(self, name: str, val: Any) -> None:
        self.vars[name] = val

class Evaluator:
    def __init__(self, env: Env):
        self.env = env

    def eval_prog(self, items: List[Any]) -> Optional[Any]:
        last = None
        for node in items:
            last = self.eval(node)
        return last

    def eval(self, node: Any) -> Any:
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
            val = self.eval(node.expr)
            self.env.set(node.name, val)
            return val
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
        if isinstance(node, Call):
            fn = self.eval(node.func)
            args = [self.eval(arg) for arg in node.args]
            if callable(fn):
                return fn(*args)
            raise RuntimeError(f"{node.func} is not callable")

        if isinstance(node, PackStmt):
            box = self.eval(node.box)
            if not isinstance(box, list):
                raise TypeError("PACK: type")
            vals = [self.eval(v) for v in node.values]
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
            box = self.eval(node.box)
            idxs = [self.eval(e) for e in node.indices]
            # type checks for indices
            for i in idxs:
                if not isinstance(i, int):
                    raise TypeError("PICK: index type")
            if isinstance(box, list):
                # bounds check
                for i in idxs:
                    if i < 0 or i >= len(box):
                        raise IndexError("PICK: bounds")
                return [box[i] for i in idxs]
            if isinstance(box, str):
                for i in idxs:
                    if i < 0 or i >= len(box):
                        raise IndexError("PICK: bounds")
                return ''.join(box[i] for i in idxs)
            raise TypeError("PICK: type")

        raise RuntimeError(f"unknown node {node}")

# ----------------------------
# REPL / Runner
# ----------------------------

BANNER = "atoms-lang stage-0 | ops: + - * / = < > <= >= == !=  >|  |<  ! | bools: TRUE/FALSE | verbs: PACK(...) -> box, PLACE(...) -> box, UNPACK(a, b) <- box, PICK(i, ...) <- box | identifiers: UTF-8 | () grouping | newline-terminated"

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
    try:
        while True:
            line = input("» ")
            if line.strip() == ":quit":
                break
            buf += line + "\n"
            # execute on blank line
            if line.strip() == "":
                try:
                    run_source(buf, env)
                except Exception as e:
                    print(f"! {e}")
                buf = ""
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