// StructDef code generation
use super::utils::rename_struct;
use crate::ast::*;
use std::fmt::Write as _;

impl StructDef {
    /// Returns all fields in declaration order: base's fields (recursively) then own fields.
    fn flatten_fields(&self, script: &Script) -> Vec<(String, Type)> {
        let mut out = Vec::new();
        if let Some(ref base_name) = self.base {
            if let Some(base_def) = script.structs.iter().find(|s| s.name == *base_name) {
                out = base_def.flatten_fields(script);
            }
        }
        for f in &self.fields {
            out.push((f.name.clone(), f.typ.clone()));
        }
        out
    }

    pub fn to_rust_definition(&self, script: &Script) -> String {
        let mut out = String::with_capacity(1024);
        let flat_fields = self.flatten_fields(script);

        // === Struct Definition ===
        writeln!(
            out,
            "#[derive(Default, Debug, Clone, Serialize, Deserialize)]"
        )
        .unwrap();
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "pub struct {} {{", renamed_struct_name).unwrap();

        for (name, typ) in &flat_fields {
            writeln!(out, "    pub {}: {},", name, typ.to_rust_type()).unwrap();
        }

        writeln!(out, "}}\n").unwrap();

        // === Display Implementation ===
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "impl std::fmt::Display for {} {{", renamed_struct_name).unwrap();
        writeln!(
            out,
            "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
        )
        .unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();

        for (i, (name, _)) in flat_fields.iter().enumerate() {
            let sep = if i + 1 < flat_fields.len() { ", " } else { " " };
            writeln!(
                out,
                "        write!(f, \"{name}: {{:?}}{sep}\", self.{name})?;",
                name = name,
                sep = sep
            )
            .unwrap();
        }

        writeln!(out, "        write!(f, \"}}}}\")").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        // === Constructor Method ===
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "impl {} {{", renamed_struct_name).unwrap();
        write!(out, "    pub fn new(").unwrap();
        let param_list: Vec<String> = flat_fields
            .iter()
            .map(|(name, typ)| format!("{}: {}", name, typ.to_rust_type()))
            .collect();
        writeln!(out, "{}) -> Self {{", param_list.join(", ")).unwrap();
        write!(out, "        Self {{").unwrap();
        for (name, _) in &flat_fields {
            write!(out, " {}: {},", name, name).unwrap();
        }
        writeln!(out, " }}").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        // === Method Implementations ===
        if !self.methods.is_empty() {
            writeln!(out, "impl {} {{", renamed_struct_name).unwrap();
            for m in &self.methods {
                out.push_str(&m.to_rust_method(&self.name, script));
            }
            writeln!(out, "}}\n").unwrap();
        }

        out
    }

    pub fn to_rust_definition_for_module(&self) -> String {
        // For modules, we can use the same definition but without script context
        // Since modules don't have methods that need script context, we can simplify
        let mut out = String::with_capacity(1024);

        // === Struct Definition ===
        writeln!(
            out,
            "#[derive(Default, Debug, Clone, Serialize, Deserialize)]"
        )
        .unwrap();
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "pub struct {} {{", renamed_struct_name).unwrap();

        for field in &self.fields {
            writeln!(out, "    pub {}: {},", field.name, field.typ.to_rust_type()).unwrap();
        }

        writeln!(out, "}}\n").unwrap();

        // === Display Implementation ===
        writeln!(out, "impl std::fmt::Display for {} {{", renamed_struct_name).unwrap();
        writeln!(
            out,
            "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
        )
        .unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();

        for (i, field) in self.fields.iter().enumerate() {
            let sep = if i + 1 < self.fields.len() { ", " } else { " " };
            writeln!(
                out,
                "        write!(f, \"{name}: {{:?}}{sep}\", self.{name})?;",
                name = field.name,
                sep = sep
            )
            .unwrap();
        }

        writeln!(out, "        write!(f, \"}}}}\")").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        // === Constructor Method ===
        writeln!(out, "impl {} {{", renamed_struct_name).unwrap();
        write!(out, "    pub fn new(").unwrap();
        let mut param_list = Vec::new();
        for field in &self.fields {
            param_list.push(format!("{}: {}", field.name, field.typ.to_rust_type()));
        }
        writeln!(out, "{}) -> Self {{", param_list.join(", ")).unwrap();
        write!(out, "        Self {{").unwrap();
        for field in &self.fields {
            write!(out, " {}: {},", field.name, field.name).unwrap();
        }
        writeln!(out, " }}").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        out
    }
}
