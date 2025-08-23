#!/usr/bin/env python3
# Enhanced error handling for Scraps language
# Rust-style error reporting with clear, professional formatting

from __future__ import annotations
from dataclasses import dataclass
from typing import Any, List, Optional, Tuple
from enum import Enum, auto
import traceback

# ----------------------------
# Error Types
# ----------------------------

class ErrorLevel(Enum):
    INFO = auto()
    WARNING = auto()
    ERROR = auto()

@dataclass
class ScrapsError(Exception):
    """Base error class for Scraps language"""
    message: str
    level: ErrorLevel = ErrorLevel.ERROR
    line: int = 0
    column: int = 0
    context: str = ""
    suggestion: str = ""
    
    def __str__(self) -> str:
        return f"{self.message} at line {self.line}, column {self.column}"

@dataclass
class SyntaxError(ScrapsError):
    """Syntax-related errors"""
    unexpected_token: str = ""
    expected_tokens: List[str] = None
    
    def __post_init__(self):
        if not self.message and self.unexpected_token:
            if self.expected_tokens:
                expected = ", ".join(f"'{t}'" for t in self.expected_tokens)
                self.message = f"Unexpected '{self.unexpected_token}', expected {expected}"
            else:
                self.message = f"Unexpected '{self.unexpected_token}'"
        self.level = ErrorLevel.ERROR

@dataclass
class TypeError(ScrapsError):
    """Type-related errors"""
    expected_type: str = ""
    actual_type: str = ""
    
    def __post_init__(self):
        if not self.message and self.expected_type and self.actual_type:
            self.message = f"Type mismatch: expected {self.expected_type}, got {self.actual_type}"
        self.level = ErrorLevel.ERROR

@dataclass
class RuntimeError(ScrapsError):
    """Runtime execution errors"""
    operation: str = ""
    
    def __post_init__(self):
        if not self.message and self.operation:
            self.message = f"Runtime error in operation: {self.operation}"
        self.level = ErrorLevel.ERROR

# ----------------------------
# Error Logger
# ----------------------------

class ErrorLogger:
    """Rust-style error logging with clear, professional formatting"""
    
    def __init__(self, source_code: str = ""):
        self.source_lines = source_code.split('\n') if source_code else []
        self.errors: List[ScrapsError] = []
        self.warnings: List[ScrapsError] = []
    
    def log_error(self, error: ScrapsError):
        """Log an error"""
        self.errors.append(error)
        self._print_error(error)
    
    def log_warning(self, warning: ScrapsError):
        """Log a warning"""
        warning.level = ErrorLevel.WARNING
        self.warnings.append(warning)
        self._print_warning(warning)
    
    def _print_error(self, error: ScrapsError):
        """Print an error in Rust-style format"""
        print(f"\nerror: {error.message}")
        print(f"  --> line {error.line}, column {error.column}")
        
        # Show source line if available
        if 0 < error.line <= len(self.source_lines):
            source_line = self.source_lines[error.line - 1]
            print(f"  |")
            print(f"{error.line:>3} | {source_line}")
            print(f"  |")
            
            # Show pointer to error location
            if error.column > 0:
                pointer = " " * (error.column - 1) + "^"
                print(f"  | {pointer}")
                print(f"  |")
        
        # Show context if available
        if error.context:
            print(f"  = note: {error.context}")
        
        # Show suggestion if available
        if error.suggestion:
            print(f"  help: {error.suggestion}")
        
        print()  # Empty line for readability
    
    def _print_warning(self, warning: ScrapsError):
        """Print a warning in Rust-style format"""
        print(f"\nwarning: {warning.message}")
        print(f"  --> line {warning.line}, column {warning.column}")
        
        if warning.suggestion:
            print(f"  help: {warning.suggestion}")
        
        print()
    
    def get_summary(self) -> str:
        """Get a summary of all errors and warnings"""
        if not self.errors and not self.warnings:
            return "No errors or warnings found"
        
        summary = []
        if self.errors:
            summary.append(f"{len(self.errors)} error(s)")
        if self.warnings:
            summary.append(f"{len(self.warnings)} warning(s)")
        
        return " | ".join(summary)
    
    def print_summary(self):
        """Print the error/warning summary"""
        print(f"\n{self.get_summary()}")
    
    def has_errors(self) -> bool:
        """Check if there are any errors"""
        return len(self.errors) > 0
    
    def clear(self):
        """Clear all logged errors and warnings"""
        self.errors.clear()
        self.warnings.clear()

# ----------------------------
# Error Helpers
# ----------------------------

def create_syntax_error(message: str, line: int, column: int, 
                       unexpected: str = "", expected: List[str] = None,
                       context: str = "", suggestion: str = "") -> SyntaxError:
    """Create a syntax error with all relevant information"""
    return SyntaxError(
        message=message,
        line=line,
        column=column,
        unexpected_token=unexpected,
        expected_tokens=expected or [],
        context=context,
        suggestion=suggestion
    )

def create_type_error(message: str, line: int, column: int,
                     expected_type: str = "", actual_type: str = "",
                     context: str = "", suggestion: str = "") -> TypeError:
    """Create a type error with all relevant information"""
    return TypeError(
        message=message,
        line=line,
        column=column,
        expected_type=expected_type,
        actual_type=actual_type,
        context=context,
        suggestion=suggestion
    )

def create_runtime_error(message: str, line: int, column: int,
                        operation: str = "", context: str = "",
                        suggestion: str = "") -> RuntimeError:
    """Create a runtime error with all relevant information"""
    return RuntimeError(
        message=message,
        line=line,
        column=column,
        operation=operation,
        context=context,
        suggestion=suggestion
    )

# ----------------------------
# Simple Usage Examples
# ----------------------------

def demo_error_logging():
    """Demonstrate the Rust-style error logging system"""
    print("Demo: Rust-style Error Logging")
    print("=" * 40)
    
    # Create a logger
    logger = ErrorLogger("x = 5 + 'hello'\ny = x * 2")
    
    # Log some example errors
    logger.log_error(create_syntax_error(
        "Cannot add integer and string",
        line=1, column=7,
        unexpected="+",
        expected=["*", "/"],
        context="Arithmetic operation",
        suggestion="Convert string to number or use string concatenation"
    ))
    
    logger.log_warning(create_type_error(
        "Variable 'x' may not be numeric",
        line=2, column=5,
        expected_type="number",
        actual_type="unknown",
        context="Multiplication operation",
        suggestion="Ensure 'x' is assigned a numeric value first"
    ))
    
    # Show summary
    logger.print_summary()

if __name__ == "__main__":
    demo_error_logging()
