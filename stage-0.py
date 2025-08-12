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

OPERATORS = {"+", "-", "*", "/", "=", "<", ">", "!=", "<=", ">="}
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
class ExprStmt:
    expr: Any

# ----------------------------
# Parser (Pratt-style for precedence, no parentheses in MVP)
# Precedence: 3: * /   2: + -   1: < >
# ----------------------------

PRECEDENCE = {
    "*": 3, "/": 3,
    "+": 2, "-": 2,
    "<": 1, ">": 1, "<=": 1, ">=": 1, "!=": 1,
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
        if t.kind == "NUM":
            left: Any = Num(t.val)
        elif t.kind == "ID":
            left = Var(t.lex)
        elif t.kind == "OP" and t.lex == "-":
            # unary minus binds tighter than *; treat as 4
            rhs = self.expr(4)
            left = Bin("*", Num(-1), rhs)
        elif t.kind == "LP":
            left = self.expr(0)
            self._expect("RP")
        else:
            raise SyntaxError(f"Unexpected token {t.kind} {t.lex} at {t.pos}")

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
        if isinstance(node, Var):
            return self.env.get(node.name)
        if isinstance(node, Bin):
            a = self.eval(node.left)
            b = self.eval(node.right)
            op = node.op
            if op == "+": return a + b
            if op == "-": return a - b
            if op == "*": return a * b
            if op == "/": return a / b
            if op == "<": return a < b
            if op == ">": return a > b
            if op == "<=": return a <= b
            if op == ">=": return a >= b
            if op == "!=": return a != b
            raise RuntimeError(f"unknown op {op}")
        if isinstance(node, Assign):
            val = self.eval(node.expr)
            self.env.set(node.name, val)
            return val
        if isinstance(node, PrintStmt):
            v = self.eval(node.expr)
            print(v)
            return v
        if isinstance(node, ExprStmt):
            return self.eval(node.expr)
        raise RuntimeError(f"unknown node {node}")

# ----------------------------
# REPL / Runner
# ----------------------------

BANNER = "atoms-lang stage-0 | ops: + - * / = < > <= >= != | identifiers: UTF-8 | () grouping | newline-terminated"

EXAMPLE = """
# examples:
x = 10
y = 3
PRINT (x + y) * 2
PRINT x * (y + 2)
PRINT x / y
PRINT x > y
PRINT x <= (y * 4)
PRINT (x + 1) != (y + 1)
""".strip()

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