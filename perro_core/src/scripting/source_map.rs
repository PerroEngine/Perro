// Source map for tracking line mappings from source scripts to generated Rust code
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Build a source map from a script and its generated code
/// This is a simplified version that uses function/statement order as approximate source lines
pub fn build_source_map_from_script(
    source_path: &str,
    identifier: &str,
    _source_code: &str,
    generated_code: &str,
    script: &crate::scripting::ast::Script,
) -> ScriptSourceMap {
    let mut builder = SourceMapBuilder::new(source_path.to_string(), identifier.to_string());
    
    // Count lines in generated code
    let generated_lines: Vec<&str> = generated_code.lines().collect();
    
    // Track identifier name mappings by recording all renamed variables and functions
    // Variables
    for var in &script.variables {
        let original_name = &var.name;
        let generated_name = crate::scripting::codegen::rename_variable(original_name, var.typ.as_ref());
        if generated_name != *original_name {
            builder.record_variable(original_name, &generated_name);
        }
    }
    
    // Functions
    for func in &script.functions {
        let original_name = &func.name;
        let generated_name = crate::scripting::codegen::rename_function(original_name);
        if generated_name != *original_name {
            builder.record_function(original_name, &generated_name);
        }
    }
    
    // Structs
    for struct_def in &script.structs {
        let original_name = &struct_def.name;
        let generated_name = crate::scripting::codegen::rename_struct(original_name);
        if generated_name != *original_name {
            builder.record_variable(original_name, &generated_name);
        }
    }
    
    // Map functions to approximate line ranges
    // Use function order as a proxy for source line numbers
    let mut current_source_line = 1u32;
    let mut current_generated_line = 1u32;
    
    // Find where functions start in generated code (look for "fn function_name")
    for (func_idx, func) in script.functions.iter().enumerate() {
        // Approximate source line: assume functions are roughly evenly spaced
        // This is a simplification - in a real implementation, we'd track line numbers during parsing
        let approx_source_line = current_source_line + (func_idx as u32 * 10); // Rough estimate
        
        // Find function in generated code using renamed function name
        let renamed_func_name = crate::scripting::codegen::rename_function(&func.name);
        let func_pattern = format!("fn {}", renamed_func_name);
        if let Some(gen_line) = generated_lines.iter().position(|line| line.contains(&func_pattern)) {
            let gen_start = gen_line as u32 + 1;
            
            // Estimate function end (look for closing brace)
            let mut gen_end = gen_start;
            let mut brace_count = 0;
            for (idx, line) in generated_lines.iter().enumerate().skip(gen_line) {
                brace_count += line.matches('{').count();
                brace_count -= line.matches('}').count();
                gen_end = idx as u32 + 1;
                if brace_count == 0 && idx > gen_line {
                    break;
                }
            }
            
            // Create a range mapping
            builder.start_range(approx_source_line);
            // Set generated line to where function starts
            for _ in 0..(gen_start - current_generated_line) {
                builder.increment_generated_line();
            }
            current_generated_line = gen_start;
            
            // End range at function end
            for _ in 0..(gen_end - current_generated_line) {
                builder.increment_generated_line();
            }
            current_generated_line = gen_end;
            builder.end_range();
            
            current_source_line = approx_source_line + 20; // Estimate
        }
    }
    
    builder.build()
}

/// Represents a range of lines in the source file mapped to a range in the generated file
/// Each LineRange typically corresponds to one function or major code block.
/// For example: source lines 31-50 (a function) might map to generated lines 80-120.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    /// Starting line number in source (1-indexed)
    #[serde(rename = "s_start")]
    pub source_start: u32,
    /// Ending line number in source (1-indexed, inclusive)
    #[serde(rename = "s_end")]
    pub source_end: u32,
    /// Starting line number in generated code (1-indexed)
    #[serde(rename = "g_start")]
    pub generated_start: u32,
    /// Ending line number in generated code (1-indexed, inclusive)
    #[serde(rename = "g_end")]
    pub generated_end: u32,
}

/// Source map for a single script file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSourceMap {
    /// Original source file path (in res:// format, e.g., "res://player.pup")
    #[serde(rename = "src")]
    pub source_path: String,
    /// Generated Rust file identifier
    #[serde(rename = "id")]
    pub generated_identifier: String,
    /// Line range mappings
    /// Each range maps a section of source code (e.g., a function) to generated code.
    /// Multiple ranges exist because each function/block in the source gets its own mapping.
    #[serde(rename = "lines")]
    pub line_ranges: Vec<LineRange>,
    /// Identifier name mappings: generated_name -> original_name
    /// Maps transpiled identifier names (variables and functions, e.g., "__t_myVar", "__t_myFunction") 
    /// back to original names (e.g., "myVar", "myFunction")
    #[serde(rename = "names")]
    pub identifier_names: HashMap<String, String>,
    /// Deprecated: kept for backwards compatibility, use identifier_names instead
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub variable_names: HashMap<String, String>,
}

/// Complete source map for all scripts in a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMap {
    /// Maps script identifier to its source map
    pub scripts: HashMap<String, ScriptSourceMap>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
        }
    }

    /// Add a source map for a script
    pub fn add_script(&mut self, identifier: String, script_map: ScriptSourceMap) {
        self.scripts.insert(identifier, script_map);
    }

    /// Find the source line for a given generated line in a script
    pub fn find_source_line(&self, identifier: &str, generated_line: u32) -> Option<u32> {
        self.scripts.get(identifier)?.line_ranges.iter()
            .find(|range| generated_line >= range.generated_start && generated_line <= range.generated_end)
            .map(|range| {
                // Linear interpolation within the range
                let source_span = range.source_end - range.source_start;
                let generated_span = range.generated_end - range.generated_start;
                if generated_span == 0 {
                    range.source_start
                } else {
                    let offset = generated_line - range.generated_start;
                    range.source_start + (offset * source_span / generated_span)
                }
            })
    }

    /// Convert a generated identifier name back to original
    pub fn restore_variable_name(&self, identifier: &str, generated_name: &str) -> String {
        if let Some(script_map) = self.scripts.get(identifier) {
            // Try identifier_names first, then fall back to variable_names for backwards compatibility
            script_map.identifier_names.get(generated_name)
                .or_else(|| script_map.variable_names.get(generated_name))
                .cloned()
                .unwrap_or_else(|| {
                    // Try to strip __t_ prefix
                    if generated_name.starts_with("__t_") {
                        generated_name.strip_prefix("__t_").unwrap_or(generated_name).to_string()
                    } else if generated_name.ends_with("_id") {
                        // Try to restore _id suffix
                        generated_name.strip_suffix("_id").unwrap_or(generated_name).to_string()
                    } else {
                        generated_name.to_string()
                    }
                })
        } else {
            // Fallback: try to strip prefix/suffix
            if generated_name.starts_with("__t_") {
                generated_name.strip_prefix("__t_").unwrap_or(generated_name).to_string()
            } else if generated_name.ends_with("_id") {
                generated_name.strip_suffix("_id").unwrap_or(generated_name).to_string()
            } else {
                generated_name.to_string()
            }
        }
    }

    /// Convert an error message by replacing generated identifier names with original ones
    pub fn convert_error_message(&self, identifier: &str, error_msg: &str) -> String {
        let mut result = error_msg.to_string();
        
        if let Some(script_map) = self.scripts.get(identifier) {
            // Use identifier_names, with fallback to variable_names for backwards compatibility
            let name_map = if !script_map.identifier_names.is_empty() {
                &script_map.identifier_names
            } else {
                &script_map.variable_names
            };
            
            // Replace all occurrences of __t_ prefixed identifiers
            for (gen_name, orig_name) in name_map.iter() {
                // Replace whole word matches
                let pattern = format!(r"\b{}\b", regex::escape(gen_name));
                if let Ok(re) = regex::Regex::new(&pattern) {
                    result = re.replace_all(&result, orig_name.as_str()).to_string();
                }
            }
        }
        
        result
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for tracking source map during code generation
pub struct SourceMapBuilder {
    source_path: String,
    identifier: String,
    line_ranges: Vec<LineRange>,
    identifier_names: HashMap<String, String>,
    current_source_line: u32,
    current_generated_line: u32,
    current_range_start_source: Option<u32>,
    current_range_start_generated: Option<u32>,
}

impl SourceMapBuilder {
    pub fn new(source_path: String, identifier: String) -> Self {
        Self {
            source_path,
            identifier,
            line_ranges: Vec::new(),
            identifier_names: HashMap::new(),
            current_source_line: 1,
            current_generated_line: 1,
            current_range_start_source: None,
            current_range_start_generated: None,
        }
    }

    /// Record that we're starting a new range from a source line
    pub fn start_range(&mut self, source_line: u32) {
        self.current_source_line = source_line;
        self.current_range_start_source = Some(source_line);
        self.current_range_start_generated = Some(self.current_generated_line);
    }

    /// Record that we're ending the current range
    pub fn end_range(&mut self) {
        if let (Some(source_start), Some(generated_start)) = 
            (self.current_range_start_source, self.current_range_start_generated) 
        {
            let range = LineRange {
                source_start,
                source_end: self.current_source_line,
                generated_start,
                generated_end: self.current_generated_line,
            };
            self.line_ranges.push(range);
            self.current_range_start_source = None;
            self.current_range_start_generated = None;
        }
    }

    /// Increment generated line counter (call after each newline in generated code)
    pub fn increment_generated_line(&mut self) {
        self.current_generated_line += 1;
    }

    /// Record an identifier name mapping (variable or function)
    pub fn record_variable(&mut self, original_name: &str, generated_name: &str) {
        self.identifier_names.insert(generated_name.to_string(), original_name.to_string());
    }
    
    /// Record a function name mapping
    pub fn record_function(&mut self, original_name: &str, generated_name: &str) {
        self.identifier_names.insert(generated_name.to_string(), original_name.to_string());
    }

    /// Build the final source map
    pub fn build(self) -> ScriptSourceMap {
        ScriptSourceMap {
            source_path: self.source_path,
            generated_identifier: self.identifier,
            line_ranges: self.line_ranges,
            identifier_names: self.identifier_names,
            variable_names: HashMap::new(), // Empty for backwards compatibility
        }
    }

    /// Get current generated line number
    pub fn current_generated_line(&self) -> u32 {
        self.current_generated_line
    }
}

