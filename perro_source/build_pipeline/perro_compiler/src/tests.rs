#[cfg(test)]
mod tests {
    use super::{
        emit_web_route_html_files, generate_call_param_binding, generate_embedded_entry_files,
        generate_perro_assets, generate_project_static_modules, module_name_from_rel,
        module_short_name_from_rel, normalize_cargo_output_paths, reset_embedded_dir, sync_scripts,
        transpile_frontend_script, transpiled_exports_script_ctor, ProjectBuildOptions,
        ScriptMethodParam,
    };
    use perro_project::{
        ensure_project_layout, ensure_project_scaffold, ensure_project_toml,
        ensure_source_overrides, load_project_toml, load_routes_toml,
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
    fn normalizes_script_cargo_paths_to_project_relative_slashes() {
        let project = std::path::Path::new("D:/Game");
        let crate_dir = project.join(".perro/scripts");
        let input = "src\\scripts\\../../../../res/scripts/game_manager.rs:1929:68: error: bad\n";
        let out = normalize_cargo_output_paths(project, Some(&crate_dir), input);
        assert_eq!(
            out,
            "res/scripts/game_manager.rs:1929:68: error: bad\n"
        );
    }

    #[test]
    fn normalizes_nested_script_cargo_paths_to_project_relative_slashes() {
        let project = std::path::Path::new("D:/Game");
        let crate_dir = project.join(".perro/scripts");
        let input = " --> src\\scripts\\ai\\../../../../../res/scripts/ai/brain.rs:7:3\n";
        let out = normalize_cargo_output_paths(project, Some(&crate_dir), input);
        assert_eq!(out, " --> res/scripts/ai/brain.rs:7:3\n");
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
        assert!(transpiled.contains("perro_api::scripting::state_ref_unchecked::<AllVariantState>"));
        assert!(transpiled.contains("perro_api::scripting::state_mut_unchecked::<AllVariantState>"));
        assert!(transpiled.contains("value.into_parse::<NestedCombo>()"));
        assert!(transpiled.contains("fn __perro_set_nested_var"));
        assert_generated_script_compiles(source, &transpiled);
    }

    #[test]
    fn generated_project_crate_compiles_after_static_embed() {
        let root = unique_temp_dir("perro_compiler_project_crate_check");
        ensure_project_layout(&root).expect("layout");
        ensure_project_toml(&root, "Generated Compile").expect("project toml");
        ensure_project_scaffold(&root, "Generated Compile").expect("scaffold");
        create_static_embed_fixture(&root);
        ensure_source_overrides(&root).expect("source overrides");

        let cfg = load_project_toml(&root).expect("load project toml");
        reset_embedded_dir(&root).expect("reset embedded");
        sync_scripts(&root).expect("sync scripts");
        generate_project_static_modules(&root, &cfg).expect("generate static modules");
        perro_static_pipeline::write_static_mod_rs(&root).expect("write static mod");
        generate_embedded_entry_files(&root).expect("generate embedded main");
        generate_perro_assets(&root).expect("generate assets");
        assert_static_module_fixture_refs(&root);
        assert_generated_native_main_hides_windows_console(&root);

        assert_project_crate_checks(&root, ProjectBuildOptions::new(false, true));
    }

    #[test]
    fn web_route_emit_writes_multi_page_html_with_keywords_and_icon() {
        let root = unique_temp_dir("perro_web_route_emit");
        std::fs::create_dir_all(root.join("res").join("routes")).expect("routes dir");
        std::fs::create_dir_all(root.join("res").join("textures")).expect("textures dir");

        std::fs::write(
            root.join("project.toml"),
            r#"[project]
name = "Site"
main_scene = "res://routes/home.scn"
icon = "res://textures/icon.bmp"
startup_splash = "res://textures/icon.bmp"

[web]
title = "Perro Site"
description = "Global desc"
keywords = ["rust", "engine"]

[graphics]
virtual_resolution = "1280x720"
"#,
        )
        .expect("write project");
        std::fs::write(
            root.join("routes.toml"),
            r#"[[route]]
href = "/"
name = "home"
scene = "res://routes/home.scn"
title = "Home"
keywords = ["home"]

[[route]]
href = "/docs"
name = "docs"
scene = "res://routes/docs.scn"
title = "Docs"
description = "Docs page"
keywords = ["docs", "api"]
"#,
        )
        .expect("write routes");
        std::fs::write(root.join("res").join("textures").join("icon.bmp"), BMP_1X1)
            .expect("write icon");
        std::fs::write(root.join("res").join("textures").join("logo.bmp"), BMP_1X1)
            .expect("write logo");
        std::fs::write(
            root.join("res").join("routes").join("home.scn"),
            static_web_home_scene(),
        )
        .expect("write home scene");
        std::fs::write(
            root.join("res").join("routes").join("docs.scn"),
            static_web_docs_scene(),
        )
        .expect("write docs scene");

        let cfg = load_project_toml(&root).expect("load project");
        let routes = load_routes_toml(&root, &cfg).expect("load routes");
        let output = root.join(".output").join("web-static-test");
        std::fs::create_dir_all(&output).expect("mk output");
        std::fs::write(output.join("boot.js"), "console.log('boot');").expect("boot js");

        emit_web_route_html_files(&root, &output, &cfg, &routes).expect("emit route html");

        let home_html =
            std::fs::read_to_string(output.join("index.html")).expect("read home index");
        let docs_html =
            std::fs::read_to_string(output.join("docs").join("index.html")).expect("read docs");

        assert!(home_html.contains("Home Hero"));
        assert!(home_html.contains("href=\"/docs\""));
        assert!(home_html.contains("src=\"assets/textures/logo.bmp\""));
        assert!(home_html.contains("rel=\"icon\" href=\"assets/textures/icon.bmp\""));
        assert!(!home_html.contains("\\n"));

        assert!(docs_html.contains("Docs body"));
        assert!(docs_html.contains("name=\"keywords\" content=\"rust, engine, docs, api\""));
        assert!(docs_html.contains("name=\"description\" content=\"Docs page\""));
        assert!(docs_html.contains("rel=\"icon\" href=\"../assets/textures/icon.bmp\""));

        std::fs::remove_dir_all(&root).expect("cleanup");
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

    fn assert_project_crate_checks(project_root: &std::path::Path, options: ProjectBuildOptions) {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let project_crate = project_root.join(".perro").join("project");
        let target_dir = project_root.join("target");
        let mut cmd = std::process::Command::new(cargo);
        cmd.arg("check")
            .arg("--quiet")
            .env("CARGO_TARGET_DIR", &target_dir)
            .current_dir(&project_crate);
        if !options.console {
            cmd.env("RUSTFLAGS", "--cfg perro_no_console");
        }
        if options.profile {
            cmd.arg("--features").arg("profile");
        }
        let output = cmd.output().expect("run cargo check");

        if output.status.success() {
            let _ = std::fs::remove_dir_all(project_root);
            return;
        }

        panic!(
            "generated project crate failed cargo check in {}\nstdout:\n{}\nstderr:\n{}",
            project_crate.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn assert_generated_native_main_hides_windows_console(project_root: &std::path::Path) {
        let main_src = std::fs::read_to_string(
            project_root
                .join(".perro")
                .join("project")
                .join("src")
                .join("main.rs"),
        )
        .expect("read generated main");
        assert!(
            main_src.contains(
                "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]"
            ),
            "generated native binary main must hide the Windows console when perro_no_console is set"
        );
    }

    fn create_static_embed_fixture(root: &std::path::Path) {
        let res = root.join("res");
        std::fs::create_dir_all(res.join("textures")).expect("textures dir");
        std::fs::create_dir_all(res.join("materials")).expect("materials dir");
        std::fs::create_dir_all(res.join("ui")).expect("ui dir");
        std::fs::create_dir_all(res.join("tiles")).expect("tiles dir");
        std::fs::create_dir_all(res.join("particles")).expect("particles dir");
        std::fs::create_dir_all(res.join("animations")).expect("animations dir");
        std::fs::create_dir_all(res.join("models")).expect("models dir");
        std::fs::create_dir_all(res.join("rigs")).expect("rigs dir");
        std::fs::create_dir_all(res.join("shaders")).expect("shaders dir");
        std::fs::create_dir_all(res.join("audio")).expect("audio dir");

        std::fs::write(
            root.join("project.toml"),
            r#"[project]
name = "Generated Compile"
main_scene = "res://main.scn"
icon = "res://textures/pixel.bmp"
startup_splash = "res://textures/pixel.bmp"

[graphics]
virtual_resolution = "320x180"

[localization]
default_locale = "es"
"#,
        )
        .expect("write project.toml");
        std::fs::write(
            root.join("locale.csv"),
            "key,en,es\nmenu.start,Start,Iniciar\nmenu.quit,Quit,Salir\n",
        )
        .expect("write locale csv");
        std::fs::write(res.join("items.csv"), "key,value\nwood,12\nstone,4\n").expect("write csv");
        std::fs::write(res.join("textures").join("pixel.bmp"), BMP_1X1).expect("write bmp");
        std::fs::write(
            res.join("materials").join("fixture.pmat"),
            "type = \"standard\"\nbase_color_factor = (0.2, 0.4, 0.8, 1.0)\nroughness_factor = 0.35\n",
        )
        .expect("write pmat");
        std::fs::write(
            res.join("ui").join("panel.uistyle"),
            "fill = \"#223344FF\"\nstroke = \"#88AAFFFF\"\nradius = 0.15\n",
        )
        .expect("write uistyle");
        std::fs::write(
            res.join("tiles").join("fixture.ptileset"),
            "texture = \"res://textures/pixel.bmp\"\ntile_size = (16, 16)\ncolumns = 1\nrows = 1\n",
        )
        .expect("write ptileset");
        std::fs::write(
            res.join("particles").join("spark.ppart"),
            "preset = spiral\npreset_param_a = 4.0\npreset_param_b = 0.25\nlifetime_min = 0.2\nlifetime_max = 0.6\nx = sin(t * tau)\ny = t\nz = cos(t * tau)\n",
        )
        .expect("write ppart");
        std::fs::write(
            res.join("animations").join("idle.panim"),
            r#"[Animation]
name = "Idle"
fps = 30
[/Animation]

[Objects]
Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (0, 0, 0)
}
[/Frame0]
"#,
        )
        .expect("write panim");
        std::fs::write(
            res.join("animations").join("blend.panimtree"),
            r#"[AnimationTree]
name = "BlendTree"
[/AnimationTree]

[AnimationSlots]
Idle
[/AnimationSlots]

[Output]
input = @Idle
[/Output]
"#,
        )
        .expect("write panimtree");
        std::fs::write(res.join("models").join("triangle.pmesh"), pmesh_triangle())
            .expect("write pmesh");
        std::fs::write(
            res.join("rigs").join("root.pskel2d"),
            r#"[bone "Root"]
parent = -1
rest_pos = (0, 0)
rest_scale = (1, 1)
rest_rot_deg = 0
[/bone]
"#,
        )
        .expect("write pskel2d");
        std::fs::write(
            res.join("shaders").join("fixture.wgsl"),
            "@fragment\nfn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n",
        )
        .expect("write wgsl");
        std::fs::write(res.join("audio").join("beep.wav"), wav_silence()).expect("write wav");
        std::fs::write(res.join("main.scn"), fixture_scene()).expect("write scene");
    }

    fn assert_static_module_fixture_refs(root: &std::path::Path) {
        let static_dir = root
            .join(".perro")
            .join("project")
            .join("src")
            .join("static");
        let checks = [
            ("scenes.rs", "SCENE_HASH_0"),
            ("materials.rs", "MATERIAL_HASH_0"),
            ("ui_styles.rs", "UI_STYLE_HASH_0"),
            ("tilesets.rs", "TILESET_HASH_0"),
            ("particles.rs", "PARTICLE_HASH_0"),
            ("animations.rs", "ANIMATION_HASH_0"),
            ("animation_trees.rs", "ANIMATION_TREE_HASH_0"),
            ("meshes.rs", "MESH_HASH_0"),
            ("collision_trimeshes.rs", "COLLISION_TRIMESH_HASH_0"),
            ("skeletons.rs", "SKELETON_HASH_0"),
            ("textures.rs", "TEXTURE_HASH_0"),
            ("shaders.rs", "SHADER_HASH_0"),
            ("audios.rs", "AUDIO_HASH_0"),
            ("csvs.rs", "CSV_HASH_0"),
            ("localizations.rs", "menu.start"),
        ];
        for (file, needle) in checks {
            let src = std::fs::read_to_string(static_dir.join(file))
                .unwrap_or_else(|err| panic!("read {file}: {err}"));
            assert!(src.contains(needle), "missing `{needle}` in {file}");
        }
    }

    fn fixture_scene() -> &'static str {
        r#"$root = @main

[main]

[Node3D]
    position = (0, 0, 0)
[/Node3D]
[/main]

[mesh]
parent = $root

[MeshInstance3D]
    mesh = "res://models/triangle.pmesh"
    material = "res://materials/fixture.pmat"
    [Node3D]
        position = (0, 0, 0)
    [/Node3D]
[/MeshInstance3D]
[/mesh]

[collider]
parent = $root

[CollisionShape3D]
    trimesh = "res://models/triangle.pmesh"
    [Node3D]
        position = (0, 0, 0)
    [/Node3D]
[/CollisionShape3D]
[/collider]

[sprite]
parent = $root

[Sprite2D]
    texture = "res://textures/pixel.bmp"
    [Node2D]
        position = (0, 0)
    [/Node2D]
[/Sprite2D]
[/sprite]

[particles]
parent = $root

[ParticleEmitter3D]
    profile = "res://particles/spark.ppart"
    [Node3D]
        position = (0, 0, 0)
    [/Node3D]
[/ParticleEmitter3D]
[/particles]

[skeleton]
parent = $root

[Skeleton2D]
    skeleton = "res://rigs/root.pskel2d"
[/Skeleton2D]
[/skeleton]

"#
    }

    fn static_web_home_scene() -> &'static str {
        r#"$root = @page

[page]
[UiVBox]
[/UiVBox]
[/page]

[hero]
parent = page
[UiLabel]
    text = "Home Hero"
[/UiLabel]
[/hero]

[logo]
parent = page
[UiImage]
    texture = "res://textures/logo.bmp"
[/UiImage]
[/logo]

[cta]
parent = page
[UiButton]
    web = { href = "/docs" }
[/UiButton]
[/cta]

[cta_text]
parent = cta
[UiLabel]
    text = "Read Docs"
[/UiLabel]
[/cta_text]
"#
    }

    fn static_web_docs_scene() -> &'static str {
        r#"$root = @page

[page]
[UiVBox]
[/UiVBox]
[/page]

[body]
parent = page
[UiLabel]
    text = "Docs body"
[/UiLabel]
[/body]
"#
    }

    fn pmesh_triangle() -> Vec<u8> {
        let mut raw = Vec::new();
        for (pos, normal, uv) in [
            ([0.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0], [0.0f32, 0.0]),
            ([1.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0], [1.0f32, 0.0]),
            ([0.0f32, 1.0, 0.0], [0.0f32, 1.0, 0.0], [0.0f32, 1.0]),
        ] {
            for value in pos {
                raw.extend_from_slice(&value.to_le_bytes());
            }
            for value in normal {
                raw.extend_from_slice(&value.to_le_bytes());
            }
            for value in uv {
                raw.extend_from_slice(&value.to_le_bytes());
            }
        }
        for index in [0u32, 1, 2] {
            raw.extend_from_slice(&index.to_le_bytes());
        }
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&3u32.to_le_bytes());

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PMESH");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&((1u32 << 31) | 1 | 2).to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&raw);
        bytes
    }

    fn wav_silence() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes
    }

    const BMP_1X1: &[u8] = &[
        0x42, 0x4d, 58, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1,
        0, 24, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0,
        0, 0,
    ];

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

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), stamp))
    }
}
