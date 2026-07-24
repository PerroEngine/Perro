mod assets {
    use super::*;

    #[test]
    fn generated_scripts_manifest_binds_perro_jobs_patch() {
        let root = unique_temp_dir("perro_compiler_jobs_manifest");
        let scripts_crate = root.join(".perro").join("scripts");
        super::super::write_dlc_scripts_manifest(&root, "jobs_fixture", &scripts_crate)
            .expect("write scripts manifest");

        let manifest = std::fs::read_to_string(scripts_crate.join("Cargo.toml"))
            .expect("read scripts manifest");
        assert!(manifest.contains("perro_api = { path ="));
        assert!(manifest.contains("perro_jobs = { path ="));
        assert!(manifest.contains("perro-spec = [\"perro_api/spec\"]"));

        std::fs::remove_dir_all(root).expect("cleanup jobs manifest fixture");
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
    aspect_ratio = "16:9"
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
    fn web_export_paths_reject_root_escape() {
        let output = std::path::Path::new("web-output");
        for href in ["/../outside", "/dir\\outside", "/C:/outside"] {
            assert!(web_route_html_path(output, href).is_err(), "{href}");
        }
        for source in [
            "res://../outside.png",
            "res://dir\\outside.png",
            "res://C:/outside.png",
        ] {
            assert!(
                checked_res_relative_path(source, "test asset").is_err(),
                "{source}"
            );
        }
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
}
