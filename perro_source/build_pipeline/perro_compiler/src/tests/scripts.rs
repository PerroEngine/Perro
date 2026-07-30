mod scripts {
    use super::*;


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
        assert!(transpiled.contains("pub(crate) fn perro_create_script()"));
        assert!(transpiled.contains("extern \"C\" fn perro_create_script_dynamic()"));
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
        assert!(transpiled.contains("Box::new(())"));
    }


    #[test]
    fn methods_only_script_exports_ctor_with_implicit_script_and_empty_state() {
        let source = r#"
    use perro_api::prelude::*;

    methods!({
    pub fn ping(
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
        assert!(transpiled.contains("Box::new(())"));
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
        assert!(transpiled.contains("var!(\"person.name\")"));
        assert!(transpiled.contains("to_variant(&state.person.name)"));
        assert!(transpiled.contains("value.into_parse::<String>()"));
        assert!(transpiled.contains("state.person.name = v"));
        assert!(transpiled.contains("ScriptMemberID::from_string(full.as_str())"));
    }


    #[test]
    fn dlc_static_generators_keep_thread_local_pack_paths() {
        let root = unique_temp_dir("perro_compiler_dlc_static_paths");
        let dlc_root = root.join("dlcs").join("fixture");
        let static_dir = root.join("pack").join("src").join("static");
        let embedded_dir = root.join("pack").join("embedded");
        std::fs::create_dir_all(dlc_root.join("shaders")).expect("shader dir");
        std::fs::write(
            dlc_root.join("shaders").join("fixture.wgsl"),
            "@fragment\nfn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n",
        )
        .expect("write shader");
        std::fs::write(dlc_root.join("pass_through.bin"), b"raw").expect("write pass-through");

        perro_static_pipeline::set_static_pipeline_overrides(Some(
            perro_static_pipeline::StaticPipelineOverrides {
                res_dir: dlc_root.clone(),
                static_dir: static_dir.clone(),
                embedded_dir,
                asset_prefix: "dlc://fixture/".to_string(),
            },
        ));
        perro_static_pipeline::begin_static_asset_inventory();
        let result = generate_dlc_static_modules(&root, false);
        let inventory = perro_static_pipeline::take_static_asset_inventory()
            .expect("take canonical DLC inventory");
        perro_static_pipeline::set_static_pipeline_overrides(None);
        result.expect("generate dlc static modules");

        let shaders =
            std::fs::read_to_string(static_dir.join("shaders.rs")).expect("read dlc shaders");
        assert!(shaders.contains("dlc://fixture/shaders/fixture.wgsl"));
        assert_eq!(inventory.len(), 1);
        assert_eq!(
            inventory[0].kind,
            perro_asset_formats::dlc::DlcAssetKind::SHADER
        );
        assert_eq!(inventory[0].path, "dlc://fixture/shaders/fixture.wgsl");
        assert!(
            inventory
                .iter()
                .all(|record| record.kind != perro_asset_formats::dlc::DlcAssetKind::FILE),
            "pass-through files need archive emission inventory before FILE records are safe"
        );

        let pack_dir = root.join("pack");
        super::super::write_dlc_pack_lib(&root, "fixture", &dlc_root, &pack_dir, &inventory)
            .expect("write registry pack source");
        let pack_source = std::fs::read_to_string(pack_dir.join("src/lib.rs"))
            .expect("read registry pack source");
        assert!(pack_source.contains("perro_dlc_pack_registry_api"));
        assert!(pack_source.contains("DlcAssetKind::from_raw(12)"));
        assert!(pack_source.contains("dlc://fixture/shaders/fixture.wgsl"));
        assert!(pack_source.contains("registry_find_v1"));
        assert!(pack_source.contains("registry_lookup_bytes_v1"));
        assert!(!pack_source.contains("registry_len() -> usize {\n    0"));
        assert!(!root.join(".perro").join("project").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    // attr + field share 1 line: `#[default = 0] #[node_ref(T)] pub x: T,`.
    // scanner must strip attr groups, not skip the line (skipped fields drop
    // scene-injected node refs silently at runtime).
    #[test]
    fn state_fields_parse_with_inline_attrs() {
        let source = r#"
#[State]
struct InlineAttrState {
    #[default = NodeID::nil()] #[node_ref(MeshInstance3D)] pub hair_mesh: NodeID,
    #[default = 0] pub hair_type: i64,
    #[default = false] pub applied: bool,
    #[default = 0.0]
    pub voice_level: f32,
}
"#;
        let fields = super::super::parse_struct_fields(source, "InlineAttrState");
        let names: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
        assert_eq!(names, ["hair_mesh", "hair_type", "applied", "voice_level"]);
        assert!(fields.iter().all(|field| field.is_pub));
        assert_eq!(fields[0].ty, "NodeID");
    }

    // pub gate: non-pub members get zero glue, incl scene inject
    #[test]
    fn private_members_generate_no_glue() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct MixedState {
    #[default = 1.0]
    pub speed: f32,
    #[default = 0]
    frames: u32,
    }

    methods!({
    pub fn boost(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let _ = ctx.id;
    }

    fn tick_internal(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let _ = ctx.id;
    }
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://scripts/mixed_state.rs");
        // pub member glue present, incl scene arm (permissive wrapper)
        assert!(transpiled.contains(
            "__PERRO_VAR_SPEED => perro_api::variant::DeriveVariant::to_variant(&state.speed)"
        ));
        assert!(transpiled.contains("value.parse_scene::<f32>(resolver)"));
        assert_methods_emitted(&transpiled, &["boost"]);
        // private field: no consts, no arms anywhere (scene inject needs pub)
        assert!(!transpiled.contains("__PERRO_VAR_FRAMES"));
        assert!(!transpiled.contains("value.into_parse::<u32>()"));
        assert!(!transpiled.contains("value.parse_scene::<u32>(resolver)"));
        // private method: no const, no arm
        assert!(!transpiled.contains("__PERRO_METHOD_TICK_INTERNAL"));
    }

    #[test]
    fn pub_crate_members_expose_runtime_glue() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct CrateState {
    #[default = 0]
    pub(crate) hp: i32,
    }

    methods!({
    pub(crate) fn heal(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _amount: i32,
    ) {
        let _ = ctx.id;
    }
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://scripts/crate_state.rs");
        assert!(transpiled.contains(
            "__PERRO_VAR_HP => perro_api::variant::DeriveVariant::to_variant(&state.hp)"
        ));
        assert_methods_emitted(&transpiled, &["heal"]);
    }

    #[test]
    fn all_private_state_generates_no_glue() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct HiddenState {
    #[default = 0]
    frames: u32,
    }

    lifecycle!({});
    "#;

        let transpiled = transpile_frontend_script(source, "res://scripts/hidden_state.rs");
        assert!(!transpiled.contains("fn __perro_state_ref"));
        assert!(!transpiled.contains("fn __perro_state_mut"));
        assert!(!transpiled.contains("fn __perro_set_var_match"));
        assert!(!transpiled.contains("fn __perro_set_scene_var_match"));
        assert!(!transpiled.contains("fn __perro_get_nested_var"));
        assert!(!transpiled.contains("fn __perro_set_nested_var"));
        assert!(!transpiled.contains("fn __perro_set_nested_scene_var"));
        assert!(!transpiled.contains("__PERRO_VAR_FRAMES"));
    }

    // static scene analysis: arms only 4 observed injections; `_` fallback
    // kp runtime spawns (node_collection! vars) working 4 pub fields
    #[test]
    fn exact_scene_usage_prunes_scene_arms() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct TunedState {
    #[default = 1.0]
    pub speed: f32,
    #[default = 0]
    pub hp: i32,
    }
    "#;

        let mut roots = std::collections::HashSet::new();
        roots.insert("speed".to_string());
        let usage = SceneVarUsage::Exact {
            roots,
            paths: std::collections::HashSet::new(),
        };
        let transpiled = transpile_frontend_script_with_scene_vars(
            source,
            "res://scripts/tuned_state.rs",
            &usage,
        );
        // speed: scene arm; hp: get/set only
        assert!(transpiled.contains("value.parse_scene::<f32>(resolver)"));
        assert!(!transpiled.contains("value.parse_scene::<i32>(resolver)"));
        assert!(transpiled.contains("value.into_parse::<i32>()"));
        // runtime-spawn fallback: unmatched scene var -> strict pub set path
        assert!(transpiled.contains("__perro_set_var_match(state, var, value);"));
        assert!(transpiled.contains("__perro_set_nested_scene_var(state, var, value.clone(), resolver)"));
    }

    #[test]
    fn no_scene_usage_routes_inject_thru_set_var() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct SpawnedState {
    #[default = 0]
    pub charge: i32,
    }
    "#;

        let usage = SceneVarUsage::Exact {
            roots: std::collections::HashSet::new(),
            paths: std::collections::HashSet::new(),
        };
        let transpiled = transpile_frontend_script_with_scene_vars(
            source,
            "res://scripts/spawned_state.rs",
            &usage,
        );
        // no authored scene sets vars: no scene match fn at all,
        // apply_scene_injected_vars loops thru strict set path
        assert!(!transpiled.contains("fn __perro_set_scene_var_match"));
        assert!(!transpiled.contains("fn __perro_set_nested_scene_var"));
        assert!(transpiled.contains("__perro_set_var_match(state, var, value);"));
        assert!(transpiled.contains("let _ = resolver;"));
        assert!(transpiled.contains("value.into_parse::<i32>()"));
    }

    // private nested segment under pub root: runtime glue only 4 pub leaves
    #[test]
    fn private_nested_segment_drops_runtime_glue() {
        let source = r#"
    use perro_api::prelude::*;

    #[derive(Variant, Clone, Default)]
    pub struct Tuning {
    pub gain: f32,
    bias: f32,
    }

    #[State]
    pub struct NestedVisState {
    #[default = Tuning::default()]
    pub tuning: Tuning,
    }
    "#;

        let transpiled = transpile_frontend_script(source, "res://scripts/nested_vis.rs");
        // pub leaf gets runtime get arm
        assert!(transpiled.contains("to_variant(&state.tuning.gain)"));
        // private leaf: const + scene arm kp, no runtime get arm
        assert!(transpiled.contains("var!(\"tuning.bias\")"));
        let get_arm = "to_variant(&state.tuning.bias),";
        assert!(!transpiled.contains(get_arm));
    }

}
