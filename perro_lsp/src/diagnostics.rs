use tower_lsp::lsp_types::*;
use perro_core::scripting::ast::Script;
use perro_core::scripting::lang::pup::parser::PupParser;
use perro_core::nodes::ui::parser::FurParser;
use perro_core::fur_ast::FurNode;
use crate::types::ParsedDocument;

/// Generate diagnostics for a PUP file
pub fn diagnose_pup(source: &str, uri: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    // Parse the script
    let mut parser = PupParser::new(source);
    parser.set_source_file(uri.to_string());
    
    match parser.parse_script() {
        Ok(script) => {
            // Run validation checks that would normally happen during codegen
            diagnostics.extend(validate_script(&script, source, uri));
        }
        Err(err) => {
            // Parse error - try to extract position from error message
            // For incomplete scripts, be more lenient - only show error if it's not just incomplete syntax
            let error_lower = err.to_lowercase();
            
            // Don't show errors for obviously incomplete code (user is still typing)
            // These are common when the user is in the middle of typing
            if error_lower.contains("expected") && (
                error_lower.contains("lparen") || 
                error_lower.contains("rparen") ||
                error_lower.contains("extends") ||
                error_lower.contains("identifier")
            ) {
                // Likely incomplete code - only show if it's a clear syntax error
                // For now, we'll still show it but with a less severe message
            }
            
            let (line, col) = extract_error_position(&err, source);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { 
                        line: line as u32, 
                        character: col as u32 
                    },
                    end: Position { 
                        line: line as u32, 
                        character: (col + 1).max(1) as u32 
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("parse_error".to_string())),
                code_description: None,
                source: Some("perro".to_string()),
                message: err,
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }
    
    diagnostics
}

/// Try to extract error position from error message
/// This is a fallback - ideally the parser would return structured errors
fn extract_error_position(err: &str, source: &str) -> (usize, usize) {
    // Simple heuristic: look for common error patterns
    // In a real implementation, you'd want the parser to return structured errors
    (0, 0)
}

/// Generate diagnostics for a FUR file
pub fn diagnose_fur(source: &str, uri: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    match FurParser::new(source) {
        Ok(mut parser) => {
            match parser.parse() {
                Ok(_ast) => {
                    // FUR validation can be added here
                    // For now, if it parses, it's valid
                }
                Err(err) => {
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 0 },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(NumberOrString::String("parse_error".to_string())),
                        code_description: None,
                        source: Some("perro".to_string()),
                        message: err,
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }
        Err(err) => {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("parse_error".to_string())),
                code_description: None,
                source: Some("perro".to_string()),
                message: err,
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }
    
    diagnostics
}

/// Validate a parsed script using codegen validation logic
fn validate_script(script: &Script, source: &str, uri: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    // Check for lifecycle method calls (these are not allowed)
    for func in &script.functions {
        if func.is_lifecycle_method {
            // Check if this lifecycle method is being called anywhere
            // This would require traversing all function bodies
            // For now, we'll add a note that lifecycle methods exist
        }
        
        // Validate function bodies
        for stmt in &func.body {
            validate_statement(stmt, script, &mut diagnostics, source, uri);
        }
    }
    
    // Validate variable types
    for var in &script.variables {
        if let Some(ref value) = var.value {
            // Check if the value type matches the declared type
            if let Some(ref declared_type) = var.typ {
                // Type checking logic here
                // This would use the same inference logic from codegen
            }
        }
    }
    
    diagnostics
}

fn validate_statement(
    stmt: &perro_core::scripting::ast::Stmt,
    script: &Script,
    diagnostics: &mut Vec<Diagnostic>,
    source: &str,
    uri: &str,
) {
    use perro_core::scripting::ast::{Stmt, Expr};
    
    match stmt {
        Stmt::Expr(typed_expr) => {
            validate_expression(&typed_expr.expr, script, diagnostics, source, uri);
        }
        Stmt::VariableDecl(var) => {
            if let Some(ref value) = var.value {
                validate_expression(&value.expr, script, diagnostics, source, uri);
            }
        }
        Stmt::Assign(_, expr) | Stmt::AssignOp(_, _, expr) => {
            validate_expression(&expr.expr, script, diagnostics, source, uri);
        }
        Stmt::If { condition, then_body, else_body, .. } => {
            validate_expression(&condition.expr, script, diagnostics, source, uri);
            for stmt in then_body {
                validate_statement(stmt, script, diagnostics, source, uri);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    validate_statement(stmt, script, diagnostics, source, uri);
                }
            }
        }
        Stmt::For { iterable, body, .. } => {
            validate_expression(&iterable.expr, script, diagnostics, source, uri);
            for stmt in body {
                validate_statement(stmt, script, diagnostics, source, uri);
            }
        }
        Stmt::ForTraditional { init, condition, increment, body, .. } => {
            if let Some(init) = init {
                validate_statement(init, script, diagnostics, source, uri);
            }
            if let Some(condition) = condition {
                validate_expression(&condition.expr, script, diagnostics, source, uri);
            }
            if let Some(increment) = increment {
                validate_statement(increment, script, diagnostics, source, uri);
            }
            for stmt in body {
                validate_statement(stmt, script, diagnostics, source, uri);
            }
        }
        _ => {}
    }
}

fn validate_expression(
    expr: &perro_core::scripting::ast::Expr,
    script: &Script,
    diagnostics: &mut Vec<Diagnostic>,
    source: &str,
    uri: &str,
) {
    use perro_core::scripting::ast::{Expr, TypedExpr};
    
    // Helper to get position from a TypedExpr's span
    let get_position = |span: &Option<perro_core::scripting::source_span::SourceSpan>| -> Position {
        span.as_ref().map(|s| {
            // LSP uses 0-indexed lines, SourceSpan uses 1-indexed
            Position {
                line: (s.line.saturating_sub(1)) as u32,
                character: (s.column.saturating_sub(1)) as u32,
            }
        }).unwrap_or_else(|| Position { line: 0, character: 0 })
    };
    
    match expr {
        Expr::Call(target, args) => {
            // Check if calling a lifecycle method
            if let Expr::Ident(func_name) = target.as_ref() {
                if let Some(func) = script.functions.iter().find(|f| f.name == *func_name) {
                    if func.is_lifecycle_method {
                        // Try to get position from the call expression
                        // In a real implementation, you'd track this during parsing
                        let pos = get_position(&None); // Would need to track this in TypedExpr
                        diagnostics.push(Diagnostic {
                            range: Range {
                                start: pos,
                                end: Position {
                                    line: pos.line,
                                    character: pos.character + func_name.len() as u32,
                                },
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::String("lifecycle_call".to_string())),
                            code_description: None,
                            source: Some("perro".to_string()),
                            message: format!("Cannot call lifecycle method '{}' - lifecycle methods (defined with 'on {}()') are not callable", func_name, func_name),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
            
            validate_expression(target, script, diagnostics, source, uri);
            for arg in args {
                validate_expression(arg, script, diagnostics, source, uri);
            }
        }
        Expr::MemberAccess(base, _) => {
            validate_expression(base, script, diagnostics, source, uri);
        }
        Expr::BinaryOp(left, _, right) => {
            validate_expression(left, script, diagnostics, source, uri);
            validate_expression(right, script, diagnostics, source, uri);
        }
        _ => {}
    }
}
