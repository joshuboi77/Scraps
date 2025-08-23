#!/usr/bin/env python3
# Integration module for enhanced error handling in stage-0.py
# This wraps existing functionality without modifying the original code

import sys
import os

# Import the existing stage-0 components
try:
    from stage_0 import Lexer, Parser, Evaluator, Env, run_source
except ImportError as e:
    print(f"Failed to import stage_0: {e}")
    print("Make sure stage_0.py exists in the current directory")
    sys.exit(1)

# Import our enhanced error handling
from errors import ErrorLogger, create_syntax_error, create_type_error, create_runtime_error

# ----------------------------
# Enhanced Lexer Wrapper
# ----------------------------

class EnhancedLexer:
    """Wrapper around existing Lexer with better error handling"""
    
    def __init__(self, source_code: str):
        self.source_code = source_code
        self.original_lexer = Lexer(source_code)
        self.error_logger = ErrorLogger(source_code)
        self.tokens = []
    
    def tokenize(self):
        """Tokenize with enhanced error handling"""
        try:
            self.tokens = self.original_lexer.tokens()
            return self.tokens
        except Exception as e:
            # Try to extract line/column info from the error
            line, col = self._extract_position_from_error(str(e))
            
            self.error_logger.log_error(create_syntax_error(
                message=f"Lexical error: {str(e)}",
                line=line,
                column=col,
                context="Tokenization phase",
                suggestion="Check for invalid characters or syntax"
            ))
            
            # Return empty tokens to allow parsing to continue
            return []
    
    def _extract_position_from_error(self, error_msg: str):
        """Try to extract line/column from error message"""
        try:
            # Look for patterns like "at 5:10" or "line 5, column 10"
            if "at" in error_msg and ":" in error_msg:
                parts = error_msg.split("at")[-1].strip().split(":")
                if len(parts) == 2:
                    return int(parts[0]), int(parts[1])
        except:
            pass
        
        # Default to first line if we can't parse the error
        return 1, 1

# ----------------------------
# Enhanced Parser Wrapper
# ----------------------------

class EnhancedParser:
    """Wrapper around existing Parser with better error handling"""
    
    def __init__(self, tokens, source_code: str = ""):
        self.tokens = tokens
        self.source_code = source_code
        self.original_parser = Parser(tokens)
        self.error_logger = ErrorLogger(source_code)
        self.ast_nodes = []
    
    def parse(self):
        """Parse with enhanced error handling"""
        try:
            self.ast_nodes = self.original_parser.parse()
            return self.ast_nodes
        except Exception as e:
            # Try to extract position info
            line, col = self._extract_position_from_error(str(e))
            
            self.error_logger.log_error(create_syntax_error(
                message=f"Parsing error: {str(e)}",
                line=line,
                column=col,
                context="Parsing phase",
                suggestion="Check syntax and ensure all statements are properly terminated"
            ))
            
            # Return empty AST to allow evaluation to continue
            return []
    
    def _extract_position_from_error(self, error_msg: str):
        """Try to extract line/column from error message"""
        try:
            if "at" in error_msg and ":" in error_msg:
                parts = error_msg.split("at")[-1].strip().split(":")
                if len(parts) == 2:
                    return int(parts[0]), int(parts[1])
        except:
            pass
        return 1, 1

# ----------------------------
# Enhanced Evaluator Wrapper
# ----------------------------

class EnhancedEvaluator:
    """Wrapper around existing Evaluator with better error handling"""
    
    def __init__(self, env: Env, source_code: str = ""):
        self.env = env
        self.source_code = source_code
        self.original_evaluator = Evaluator(env)
        self.error_logger = ErrorLogger(source_code)
    
    def eval_prog(self, nodes):
        """Evaluate program with enhanced error handling"""
        try:
            return self.original_evaluator.eval_prog(nodes)
        except Exception as e:
            # Try to extract position info
            line, col = self._extract_position_from_error(str(e))
            
            self.error_logger.log_error(create_runtime_error(
                message=f"Runtime error: {str(e)}",
                line=line,
                column=col,
                operation="Program execution",
                context="Evaluation phase",
                suggestion="Check variable definitions and operation types"
            ))
            
            # Re-raise the error so the program stops
            raise
    
    def _extract_position_from_error(self, error_msg: str):
        """Try to extract line/column from error message"""
        try:
            if "at" in error_msg and ":" in error_msg:
                parts = error_msg.split("at")[-1].strip().split(":")
                if len(parts) == 2:
                    return int(parts[0]), int(parts[1])
        except:
            pass
        return 1, 1

# ----------------------------
# Enhanced Main Function
# ----------------------------

def run_source_with_enhanced_errors(source_code: str, env=None):
    """Run Scraps source code with enhanced error handling"""
    if env is None:
        env = Env()
    
    all_errors = []
    
    # Phase 1: Lexical Analysis
    print("Phase 1: Lexical Analysis...")
    lexer = EnhancedLexer(source_code)
    tokens = lexer.tokenize()
    
    if lexer.error_logger.has_errors():
        all_errors.extend(lexer.error_logger.errors)
        print(f"Found {len(lexer.error_logger.errors)} lexical errors")
    else:
        print("Lexical analysis passed")
    
    # Phase 2: Parsing
    print("Phase 2: Parsing...")
    if tokens:
        parser = EnhancedParser(tokens, source_code)
        ast_nodes = parser.parse()
        
        if parser.error_logger.has_errors():
            all_errors.extend(parser.error_logger.errors)
            print(f"Found {len(parser.error_logger.errors)} parsing errors")
        else:
            print("Parsing passed")
    else:
        print("No tokens to parse")
        return False, None, all_errors
    
    # Phase 3: Evaluation
    print("Phase 3: Evaluation...")
    if ast_nodes:
        try:
            evaluator = EnhancedEvaluator(env, source_code)
            result = evaluator.eval_prog(ast_nodes)
            print("Evaluation completed")
            return True, result, all_errors
        except Exception as e:
            # Runtime errors are already logged by the evaluator
            return False, None, all_errors
    else:
        print("No AST nodes to evaluate")
        return False, None, all_errors

# ----------------------------
# Demo and Testing
# ----------------------------

def demo_integration():
    """Demonstrate the enhanced error handling integration"""
    print("Demo: Enhanced Error Handling Integration")
    print("=" * 50)
    
    # Test with some problematic Scraps code
    test_code = """
x = 5 + 'hello'
y = x * 2
PRINT y
"""
    
    print("Testing with problematic code:")
    print(test_code)
    print("-" * 30)
    
    # Run with enhanced error handling
    success, result, errors = run_source_with_enhanced_errors(test_code)
    
    if success:
        print(f"Result: {result}")
    else:
        print("Execution failed due to errors")
    
    print(f"\nTotal errors found: {len(errors)}")

if __name__ == "__main__":
    demo_integration()
