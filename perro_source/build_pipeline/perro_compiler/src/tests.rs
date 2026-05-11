#[cfg(test)]
mod tests {
    use super::{
        ScriptMethodParam, generate_call_param_binding, module_name_from_rel,
        module_short_name_from_rel, transpile_frontend_script, transpiled_exports_script_ctor,
    };

    fn assert_methods_emitted(transpiled: &str, expected_method_names: &[&str]) {
        assert!(
            transpiled.contains("match method {"),
            "expected generated call_method match"
        );
        assert!(
            !transpiled.contains("let _ = (method, ctx, params);"),
            "unexpected empty call_method stub generated"
        );
        for method_name in expected_method_names {
            let const_name = format!("__PERRO_METHOD_{}", method_name.to_ascii_uppercase());
            assert!(
                transpiled.contains(&const_name),
                "missing method const for {method_name}"
            );
            let arm = format!("{const_name} =>");
            assert!(
                transpiled.contains(&arm),
                "missing call_method arm for {method_name}"
            );
        }
    }

    #[test]
    fn transpiles_controller_methods_into_call_method_arms() {
        let source = r#"
use perro_api::prelude::*;

#[State]
pub struct ArcherControllerState {
    #[default = false]
    pub enabled: bool,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_player_index(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _player_index: i32,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
});
"#;

        let transpiled = transpile_frontend_script(source, "res://tests/controller.rs");
        assert_methods_emitted(
            &transpiled,
            &["bind_agent", "set_player_index", "set_turn_enabled"],
        );
    }

    #[test]
    fn transpiles_ai_methods_into_call_method_arms() {
        let source = r#"
use perro_api::prelude::*;

#[derive(Variant, Clone, Copy)]
pub struct AgentRef {
    pub agent_id: NodeID,
}

impl Default for AgentRef {
    fn default() -> Self {
        Self {
            agent_id: NodeID::nil(),
        }
    }
}

#[derive(Variant, Clone, Copy)]
pub struct AimPlan {
    pub has_plan: bool,
}

impl Default for AimPlan {
    fn default() -> Self {
        Self { has_plan: false }
    }
}

#[State]
pub struct ArcherAiBrainState {
    #[default = false]
    pub enabled: bool,
    #[default = AgentRef::default()]
    pub agent_ref: AgentRef,
    #[default = AimPlan::default()]
    pub plan: AimPlan,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}

    fn set_ai_skill(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _skill: f32,
    ) {}

    fn reset_plan(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});
"#;

        let transpiled = transpile_frontend_script(source, "res://tests/ai.rs");
        assert_methods_emitted(
            &transpiled,
            &[
                "bind_agent",
                "set_turn_enabled",
                "set_ai_skill",
                "reset_plan",
            ],
        );
    }

    #[test]
    fn transpiles_methods_even_with_braces_in_strings_comments_and_raw_strings() {
        let source = r###"
use perro_api::prelude::*;

#[State]
pub struct WeirdState {
    #[default = false]
    pub enabled: bool,
}

lifecycle!({
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        // comment with misleading delimiters: methods!({ fn nope( ) { } });
        let _a = "format-like braces {x} and parens (y) should not affect parser";
        let _b = r#"raw string with fake delimiters: methods!({ fn fake() {} })"#;
        let _c = br##"byte raw with nested hashes and braces { } ) ("##;
        let _d = "emoji \u{1F3F9} and braces {{{}}}";
    }
});

methods!({
    fn alpha(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn beta(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
});
"###;

        let transpiled = transpile_frontend_script(source, "res://tests/weird.rs");
        assert_methods_emitted(&transpiled, &["alpha", "beta"]);
    }

    #[test]
    fn typed_param_binding_uses_first_for_zero_index() {
        let first = generate_call_param_binding(
            0,
            &ScriptMethodParam {
                name: "sport".to_string(),
                ty: "Sport".to_string(),
            },
        )
        .expect("custom param should generate binding");
        let second = generate_call_param_binding(
            1,
            &ScriptMethodParam {
                name: "enabled".to_string(),
                ty: "bool".to_string(),
            },
        )
        .expect("bool param should generate binding");

        assert!(first.contains("match params.first()"));
        assert!(!first.contains("params.get(0)"));
        assert!(second.contains("match params.get(1)"));
    }

    #[test]
    fn bare_module_has_no_exported_ctor() {
        let source = r#"
pub const SCALE: f32 = 2.0;

pub enum Team {
    Red,
    Blue,
}

pub fn mix(a: f32, b: f32) -> f32 {
    (a + b) * SCALE
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/util.rs");
        assert!(
            !transpiled_exports_script_ctor(&transpiled),
            "bare modules should not register as script constructors"
        );
    }

    #[test]
    fn state_script_exports_ctor() {
        let source = r#"
use perro_api::prelude::*;

#[State]
pub struct StateOnly {
    #[default = 1.0]
    pub speed: f32,
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/state_only.rs");
        assert!(
            transpiled_exports_script_ctor(&transpiled),
            "state-backed scripts should register constructors"
        );
    }

    #[test]
    fn lifecycle_only_script_exports_ctor_with_empty_state() {
        let source = r#"
use perro_api::prelude::*;

lifecycle!({
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let _ = ctx.id;
    }
});
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/lifecycle_only.rs");
        assert!(
            transpiled_exports_script_ctor(&transpiled),
            "lifecycle-only scripts should register constructors"
        );
        assert!(transpiled.contains("Box::new(<() as Default>::default())"));
    }

    #[test]
    fn methods_only_script_exports_ctor_with_implicit_script_and_empty_state() {
        let source = r#"
use perro_api::prelude::*;

methods!({
    fn ping(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let _ = ctx.id;
    }
});
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/methods_only.rs");
        assert!(
            transpiled_exports_script_ctor(&transpiled),
            "methods-only scripts should register constructors"
        );
        assert!(transpiled.contains("struct Script;"));
        assert!(transpiled.contains("Box::new(<() as Default>::default())"));
        assert_methods_emitted(&transpiled, &["ping"]);
    }

    #[test]
    fn transpiled_state_includes_nested_var_helpers() {
        let source = r#"
use perro_api::prelude::*;

#[derive(Variant, Clone)]
pub struct Person {
    pub name: String,
}

#[State]
pub struct NestedState {
    #[default = Person { name: String::new() }]
    pub person: Person,
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/nested_state.rs");
        assert!(transpiled.contains("fn __perro_state_ref"));
        assert!(transpiled.contains("fn __perro_state_mut"));
        assert!(!transpiled.contains("unsafe fn __perro_state_ref"));
        assert!(!transpiled.contains("unsafe fn __perro_state_mut"));
        assert!(!transpiled.contains("__perro_checked_state_ref"));
        assert!(!transpiled.contains("__perro_checked_state_mut"));
        assert!(!transpiled.contains("std::any::TypeId::of"));
        assert!(transpiled.contains("let state = __perro_state_ref(state)"));
        assert!(transpiled.contains("let state = __perro_state_mut(state)"));
        assert!(transpiled.contains("perro_api::scripting::state_ref_unchecked::<NestedState>"));
        assert!(transpiled.contains("perro_api::scripting::state_mut_unchecked::<NestedState>"));
        assert!(transpiled.contains("__perro_get_nested_var"));
        assert!(transpiled.contains("__perro_set_nested_var"));
        assert!(transpiled.contains("ScriptMemberID::from_string(full.as_str())"));
    }

    #[test]
    fn generated_state_all_variant_types_compiles() {
        let source = r#"
use perro_api::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
pub struct CustomLeaf {
    pub count: i32,
    pub label: String,
    pub pos: Vector3,
}

impl Default for CustomLeaf {
    fn default() -> Self {
        Self {
            count: 1,
            label: String::from("leaf"),
            pos: Vector3::new(1.0, 2.0, 3.0),
        }
    }
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "u16")]
pub enum CustomMode {
    Idle,
    Tuple(CustomLeaf, f32),
    Named { leaf: CustomLeaf, active: bool },
}

impl Default for CustomMode {
    fn default() -> Self {
        Self::Named {
            leaf: CustomLeaf::default(),
            active: true,
        }
    }
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
pub struct NestedCombo {
    pub optional_leaf: Option<CustomLeaf>,
    pub leaves: Vec<CustomLeaf>,
    pub scores: BTreeMap<Arc<str>, i32>,
    pub modes: Vec<CustomMode>,
}

impl Default for NestedCombo {
    fn default() -> Self {
        let mut scores = BTreeMap::<Arc<str>, i32>::new();
        scores.insert(Arc::<str>::from("focus"), 7);
        Self {
            optional_leaf: Some(CustomLeaf::default()),
            leaves: vec![CustomLeaf::default()],
            scores,
            modes: vec![CustomMode::Idle],
        }
    }
}

#[State]
pub struct AllVariantState {
    #[default = true]
    pub bool_value: bool,
    #[default = -1_i8]
    pub i8_value: i8,
    #[default = -2_i16]
    pub i16_value: i16,
    #[default = -3_i32]
    pub i32_value: i32,
    #[default = -4_i64]
    pub i64_value: i64,
    #[default = -5_i128]
    pub i128_value: i128,
    #[default = -6_isize]
    pub isize_value: isize,
    #[default = 1_u8]
    pub u8_value: u8,
    #[default = 2_u16]
    pub u16_value: u16,
    #[default = 3_u32]
    pub u32_value: u32,
    #[default = 4_u64]
    pub u64_value: u64,
    #[default = 5_u128]
    pub u128_value: u128,
    #[default = 6_usize]
    pub usize_value: usize,
    #[default = 1.25_f32]
    pub f32_value: f32,
    #[default = 2.5_f64]
    pub f64_value: f64,
    #[default = String::from("owned")]
    pub string_value: String,
    #[default = Arc::<str>::from("shared")]
    pub arc_str_value: Arc<str>,
    #[default = Variant::from(vec![1_u8, 2_u8, 3_u8])]
    pub raw_variant_value: Variant,
    #[default = NodeID::nil()]
    pub node_id: NodeID,
    #[default = TextureID::nil()]
    pub texture_id: TextureID,
    #[default = MaterialID::nil()]
    pub material_id: MaterialID,
    #[default = MeshID::nil()]
    pub mesh_id: MeshID,
    #[default = AnimationID::nil()]
    pub animation_id: AnimationID,
    #[default = LightID::nil()]
    pub light_id: LightID,
    #[default = UIElementID::nil()]
    pub ui_element_id: UIElementID,
    #[default = SignalID::nil()]
    pub signal_id: SignalID,
    #[default = AudioBusID::nil()]
    pub audio_bus_id: AudioBusID,
    #[default = TagID::nil()]
    pub tag_id: TagID,
    #[default = PreloadedSceneID::nil()]
    pub preloaded_scene_id: PreloadedSceneID,
    #[default = Vector2::new(1.0, 2.0)]
    pub vec2_value: Vector2,
    #[default = Vector3::new(1.0, 2.0, 3.0)]
    pub vec3_value: Vector3,
    #[default = Quaternion::default()]
    pub quat_value: Quaternion,
    #[default = Transform2D::default()]
    pub transform_2d_value: Transform2D,
    #[default = Transform3D::default()]
    pub transform_3d_value: Transform3D,
    #[default = PostProcessSet::default()]
    pub post_process_value: PostProcessSet,
    #[default = VisualAccessibilitySettings::new()]
    pub visual_accessibility_value: VisualAccessibilitySettings,
    #[default = Some(CustomLeaf::default())]
    pub option_custom: Option<CustomLeaf>,
    #[default = Vec::new()]
    pub vec_i32: Vec<i32>,
    #[default = vec![CustomLeaf::default()]]
    pub vec_custom: Vec<CustomLeaf>,
    #[default = BTreeMap::new()]
    pub map_i32: BTreeMap<Arc<str>, i32>,
    #[default = BTreeMap::new()]
    pub map_custom: BTreeMap<Arc<str>, CustomLeaf>,
    #[default = CustomLeaf::default()]
    pub custom_leaf: CustomLeaf,
    #[default = CustomMode::default()]
    pub custom_mode: CustomMode,
    #[default = NestedCombo::default()]
    pub nested_combo: NestedCombo,
}

methods!({
    fn accept_combo(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        combo: NestedCombo,
        mode: CustomMode,
        node: NodeID,
        raw: Variant,
    ) -> Variant {
        let _ = (ctx.id, combo, mode, node);
        raw
    }
});

lifecycle!({});
"#;

        let transpiled = transpile_frontend_script(source, "all_variant_types.rs");
        assert!(transpiled.contains(
            "fn set_var(&self, state: &mut dyn std::any::Any, var: ScriptMemberID, value: Variant)"
        ));
        assert!(!transpiled.contains("fn set_var_owned"));
        assert!(transpiled.contains("fn __perro_state_ref"));
        assert!(transpiled.contains("fn __perro_state_mut"));
        assert!(!transpiled.contains("unsafe fn __perro_state_ref"));
        assert!(!transpiled.contains("unsafe fn __perro_state_mut"));
        assert!(!transpiled.contains("std::any::TypeId::of"));
        assert!(
            transpiled.contains("perro_api::scripting::state_ref_unchecked::<AllVariantState>")
        );
        assert!(
            transpiled.contains("perro_api::scripting::state_mut_unchecked::<AllVariantState>")
        );
        assert!(transpiled.contains("value.into_parse::<NestedCombo>()"));
        assert!(transpiled.contains("fn __perro_set_nested_var"));
        assert_generated_script_compiles(source, &transpiled);
    }

    #[test]
    fn module_short_name_drops_rs_suffix() {
        assert_eq!(
            module_name_from_rel("scripts/personality_module.rs"),
            "scripts_personality_module_rs"
        );
        assert_eq!(
            module_short_name_from_rel("scripts/personality_module.rs"),
            "scripts_personality_module"
        );
    }

    fn assert_generated_script_compiles(source: &str, transpiled: &str) {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("workspace root")
            .to_path_buf();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!(
            "perro_compiler_all_variant_types_{}_{}",
            std::process::id(),
            stamp
        ));
        let src_dir = tmp.join("src");
        std::fs::create_dir_all(&src_dir).expect("create temp src dir");

        std::fs::write(src_dir.join("all_variant_types.rs"), source).expect("write source script");
        std::fs::write(
            src_dir.join("lib.rs"),
            format!("pub type RuntimeScriptApi = perro_runtime::RuntimeScriptApi;\n{transpiled}"),
        )
        .expect("write generated lib");

        let perro_api = toml_path(&workspace_root.join("perro_source/api_modules/perro_api"));
        let perro_runtime =
            toml_path(&workspace_root.join("perro_source/runtime_project/perro_runtime"));
        std::fs::write(
            tmp.join("Cargo.toml"),
            format!(
                r#"[package]
name = "perro_compiler_generated_check"
version = "0.0.0"
edition = "2024"

[dependencies]
perro_api = {{ path = "{perro_api}" }}
perro_runtime = {{ path = "{perro_runtime}" }}
"#
            ),
        )
        .expect("write temp Cargo.toml");

        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = std::process::Command::new(cargo)
            .arg("check")
            .arg("--quiet")
            .current_dir(&tmp)
            .output()
            .expect("run cargo check");

        if output.status.success() {
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }

        panic!(
            "generated script failed cargo check in {}\nstdout:\n{}\nstderr:\n{}",
            tmp.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn toml_path(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }
}
