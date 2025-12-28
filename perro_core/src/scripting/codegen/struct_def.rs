// StructDef code generation
use crate::ast::*;
use crate::scripting::ast::Type;
use std::fmt::Write as _;
use super::utils::rename_struct;

impl StructDef {
    pub fn to_rust_definition(&self, script: &Script) -> String {
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
        let renamed_struct_name = rename_struct(&self.name);
        writeln!(out, "impl std::fmt::Display for {} {{", renamed_struct_name).unwrap();
        writeln!(
            out,
            "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
        )
        .unwrap();
        writeln!(out, "        write!(f, \"{{{{ \")?;").unwrap();

        // --- print own fields ---
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
        let renamed_struct_name = rename_struct(&self.name);
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
}
