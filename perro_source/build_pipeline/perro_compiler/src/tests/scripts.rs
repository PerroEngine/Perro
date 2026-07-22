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

}
