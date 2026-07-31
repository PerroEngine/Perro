mod root_of_refs {
    use super::*;

    // Nested chain: outer imports mid.scn, mid.scn contains a node that
    // imports visual.scn whose root script refs its own children. Mirrors
    // UiManager -> character_editor -> clown_visual in a real game.
    #[test]
    fn nested_root_of_script_var_refs_resolve() {
        let outer = Parser::new(
            r#"
            [ui_host]
            root_of = "res://mid.scn"
            [Node3D/]
            [/ui_host]

            [ui_host_b]
            root_of = "res://mid.scn"
            [Node3D/]
            [/ui_host_b]
            "#,
        )
        .parse_scene();

        let mid = Parser::new(
            r#"
            $root = @Mid
            [Mid]
            [Node3D/]
            [/Mid]

            [EditorVisual]
            parent = @Mid
            root_of = "res://visual.scn"
            [Node3D/]
            [/EditorVisual]
            "#,
        )
        .parse_scene();

        let inner = Parser::new(
            r#"
            $root = @Visual
            [Visual]
            script = "res://visual.rs"
            script_vars = { part = @Part }
            [Node3D/]
            [/Visual]

            [Part]
            parent = @Visual
            [Node3D/]
            [/Part]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&outer, &|path| match path {
            "res://mid.scn" => Ok(std::rc::Rc::new(mid.clone())),
            "res://visual.scn" => Ok(std::rc::Rc::new(inner.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let hosts: Vec<u32> = prepared
            .nodes
            .iter()
            .filter(|pending| pending.key_name == "EditorVisual")
            .map(|pending| pending.key)
            .collect();
        assert_eq!(hosts.len(), 2, "one EditorVisual per mid instance");

        for host_key in hosts {
            let part_key = prepared
                .nodes
                .iter()
                .find(|pending| {
                    pending.key_name == "Part" && pending.parent_key == Some(host_key)
                })
                .unwrap_or_else(|| panic!("Part under host `{host_key}`"))
                .key;
            let script = prepared
                .scripts
                .iter()
                .find(|pending| pending.node_key == host_key)
                .unwrap_or_else(|| panic!("script on host `{host_key}`"));
            let part_var = script
                .scene_injected_vars
                .iter()
                .find(|(name, _)| name == "part")
                .unwrap_or_else(|| panic!("var `part` on host `{host_key}`"))
                .1
                .clone();
            assert_eq!(
                part_var,
                SceneValue::Key(format!("#{part_key}").into()),
                "host `{host_key}` must ref its own Part"
            );
        }
    }

    // Two hosts import the same inner scene; each host script's node-ref vars
    // must resolve to its OWN instance's children, not fall back to a global
    // name lookup that picks whichever instance loaded last.
    #[test]
    fn root_of_script_var_refs_resolve_per_instance() {
        let host = Parser::new(
            r#"
            [host_a]
            root_of = "res://visual.scn"
            [Node3D/]
            [/host_a]

            [host_b]
            root_of = "res://visual.scn"
            script_vars = { extra = @sibling }
            [Node3D/]
            [/host_b]

            [sibling]
            [Node3D/]
            [/sibling]
            "#,
        )
        .parse_scene();

        let inner = Parser::new(
            r#"
            $root = @Visual
            [Visual]
            script = "res://visual.rs"
            script_vars = { part = @Part }
            [Node3D/]
            [/Visual]

            [Part]
            parent = @Visual
            [Node3D/]
            [/Part]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://visual.scn" => Ok(std::rc::Rc::new(inner.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let node_key = |name: &str| {
            prepared
                .nodes
                .iter()
                .find(|pending| pending.key_name == name)
                .unwrap_or_else(|| panic!("node `{name}`"))
                .key
        };
        let part_key_under = |host_key: u32| {
            prepared
                .nodes
                .iter()
                .find(|pending| {
                    pending.key_name == "Part" && pending.parent_key == Some(host_key)
                })
                .unwrap_or_else(|| panic!("Part under host key `{host_key}`"))
                .key
        };
        let script_var = |host_name: &str, var: &str| {
            let script = prepared
                .scripts
                .iter()
                .find(|pending| pending.node_key_name == host_name)
                .unwrap_or_else(|| panic!("script on `{host_name}`"));
            script
                .scene_injected_vars
                .iter()
                .find(|(name, _)| name == var)
                .unwrap_or_else(|| panic!("var `{var}` on `{host_name}`"))
                .1
                .clone()
        };

        let part_a = part_key_under(node_key("host_a"));
        let part_b = part_key_under(node_key("host_b"));
        assert_ne!(part_a, part_b, "each instance must own a distinct Part");

        assert_eq!(
            script_var("host_a", "part"),
            SceneValue::Key(format!("#{part_a}").into())
        );
        assert_eq!(
            script_var("host_b", "part"),
            SceneValue::Key(format!("#{part_b}").into())
        );
        assert_eq!(
            script_var("host_b", "extra"),
            SceneValue::Key(format!("#{}", node_key("sibling")).into())
        );
    }
}
