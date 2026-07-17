use super::*;

pub(in super::super) fn load_runtime_scene_from_disk(
    path: &str,
) -> Result<(Scene, RuntimeSceneLoadStats), String> {
    #[cfg(feature = "profile")]
    let source_load_start = Instant::now();
    let bytes = load_asset(path).map_err(|err| format!("failed to load scene `{path}`: {err}"))?;
    #[cfg(feature = "profile")]
    let source_load = source_load_start.elapsed();

    let source = std::str::from_utf8(&bytes)
        .map_err(|err| format!("scene `{path}` is not valid UTF-8: {err}"))?;
    #[cfg(feature = "profile")]
    let parse_start = Instant::now();
    let mut scene = Parser::new(source)
        .try_parse_scene()
        .map_err(|err| format!("failed to parse scene `{path}`: {err}"))?;
    if let Some(mount_name) = parse_dlc_mount_name(path) {
        resolve_scene_dlc_self_paths(&mut scene, &mount_name);
    }
    #[cfg(feature = "profile")]
    let parse = parse_start.elapsed();
    #[cfg(feature = "profile")]
    let stats = RuntimeSceneLoadStats { source_load, parse };
    #[cfg(not(feature = "profile"))]
    let stats = RuntimeSceneLoadStats;
    Ok((scene, stats))
}

pub(super) fn parse_dlc_mount_name(path: &str) -> Option<String> {
    let rest = path.strip_prefix("dlc://")?;
    let (mount, _) = rest.split_once('/').unwrap_or((rest, ""));
    if mount.eq_ignore_ascii_case("self") || mount.is_empty() {
        return None;
    }
    Some(mount.to_string())
}

pub(super) fn resolve_scene_dlc_self_paths(scene: &mut Scene, mount_name: &str) {
    let prefix = "dlc://self/";
    let replacement = format!("dlc://{mount_name}/");
    let replacement_ref = replacement.as_str();
    for node in scene.nodes.to_mut() {
        if let Some(script) = node.script.as_ref()
            && script.starts_with(prefix)
        {
            let resolved = script.replacen(prefix, replacement_ref, 1);
            node.script = Some(Cow::Owned(resolved));
        }
        if let Some(root_of) = node.root_of.as_ref()
            && root_of.starts_with(prefix)
        {
            let resolved = root_of.replacen(prefix, replacement_ref, 1);
            node.root_of = Some(Cow::Owned(resolved));
        }
        resolve_scene_value_fields_dlc_self(node.script_vars.to_mut(), prefix, replacement_ref);
        resolve_scene_node_data_dlc_self(&mut node.data, prefix, replacement_ref);
    }
}

pub(super) fn resolve_scene_node_data_dlc_self(data: &mut SceneDefNodeData, prefix: &str, replacement: &str) {
    resolve_scene_value_fields_dlc_self(data.fields.to_mut(), prefix, replacement);
    if let Some(base) = data.base.as_mut()
        && let perro_scene::SceneNodeDataBase::Owned(base_data) = base
    {
        resolve_scene_node_data_dlc_self(base_data.as_mut(), prefix, replacement);
    }
}

pub(super) fn resolve_scene_value_fields_dlc_self(
    fields: &mut [SceneObjectField],
    prefix: &str,
    replacement: &str,
) {
    for (_, value) in fields {
        resolve_scene_value_dlc_self(value, prefix, replacement);
    }
}

pub(super) fn resolve_scene_value_dlc_self(value: &mut SceneValue, prefix: &str, replacement: &str) {
    match value {
        SceneValue::Str(v) if v.as_ref().starts_with(prefix) => {
            *v = Cow::Owned(v.replacen(prefix, replacement, 1));
        }
        SceneValue::Object(fields) => {
            for (_, item) in fields.to_mut() {
                resolve_scene_value_dlc_self(item, prefix, replacement);
            }
        }
        SceneValue::Array(values) => {
            for item in values.to_mut() {
                resolve_scene_value_dlc_self(item, prefix, replacement);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(in super::super) fn prepare_scene_with_loader(
    scene: &Scene,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
) -> Result<PreparedScene, String> {
    prepare_scene_with_loader_and_styles(scene, load_scene, None)
}

pub(in super::super) fn prepare_scene_with_loader_and_styles(
    scene: &Scene,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    let mut include_stack = HashSet::new();
    prepare_scene_with_stack(
        scene,
        &mut include_stack,
        load_scene,
        static_ui_style_lookup,
    )
}

pub(super) fn prepare_scene_with_stack(
    scene: &Scene,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    if scene.nodes.iter().all(|entry| entry.root_of.is_none()) {
        let mut prepared = prepare_scene_parallel(scene, static_ui_style_lookup)?;
        ensure_default_ray_light_3d(&mut prepared);
        return Ok(prepared);
    }

    let mut prepared_nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();
    let mut next_key = scene
        .nodes
        .iter()
        .map(|node| node.key.as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let key_map = HashMap::new();

    let mut ctx = PrepareSceneCtx {
        prepared_nodes: &mut prepared_nodes,
        scripts: &mut scripts,
        next_key: &mut next_key,
        include_stack,
        load_scene,
        static_ui_style_lookup,
        scratch: ScenePrepareScratch::default(),
    };

    for entry in scene.nodes.as_ref() {
        push_entry_prepared(scene, entry, None, &key_map, &mut ctx)?;
    }

    let mut prepared = PreparedScene {
        root_key: scene.root.map(|key| key.as_u32()),
        nodes: prepared_nodes,
        scripts,
    };
    ensure_default_ray_light_3d(&mut prepared);
    Ok(prepared)
}

pub(super) struct PreparedEntry {
    node: PendingNode,
    script: Option<PendingScript>,
}

pub(super) fn prepare_scene_parallel(
    scene: &Scene,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    if scene.nodes.iter().all(|entry| entry.script.is_none()) {
        let nodes = scene
            .nodes
            .as_ref()
            .par_iter()
            .with_min_len(256)
            .map_init(ScenePrepareScratch::default, |scratch, entry| {
                prepare_node_no_root(scene, entry, static_ui_style_lookup, scratch)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut prepared = PreparedScene {
            root_key: scene.root.map(|key| key.as_u32()),
            nodes,
            scripts: Vec::new(),
        };
        ensure_default_ray_light_3d(&mut prepared);
        return Ok(prepared);
    }

    let entries = scene
        .nodes
        .as_ref()
        .par_iter()
        .with_min_len(256)
        .map_init(ScenePrepareScratch::default, |scratch, entry| {
            prepare_entry_no_root(scene, entry, static_ui_style_lookup, scratch)
        })
        .collect::<Vec<_>>();

    let mut prepared_nodes = Vec::with_capacity(entries.len());
    let mut scripts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if let Some(script) = entry.script {
            scripts.push(script);
        }
        prepared_nodes.push(entry.node);
    }

    let mut prepared = PreparedScene {
        root_key: scene.root.map(|key| key.as_u32()),
        nodes: prepared_nodes,
        scripts,
    };
    ensure_default_ray_light_3d(&mut prepared);
    Ok(prepared)
}

pub(super) fn ensure_default_ray_light_3d(prepared: &mut PreparedScene) {
    if !prepared
        .nodes
        .iter()
        .any(|node| node.node.node_type().is_a(NodeType::Node3D))
    {
        return;
    }
    if prepared
        .nodes
        .iter()
        .any(|node| matches!(node.node.data, SceneNodeData::RayLight3D(_)))
    {
        return;
    }
    let key = prepared
        .nodes
        .iter()
        .map(|node| node.key)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut node = SceneNode::new(SceneNodeData::RayLight3D(RayLight3D::new()));
    node.name = Cow::Borrowed("__perro_default_ray_light");
    prepared.nodes.push(PendingNode {
        key,
        key_name: "__perro_default_ray_light".to_string(),
        parent_key: None,
        node,
        animation_source: None,
        animation_tree_source: None,
        animation_tree_animations: Vec::new(),
        texture_source: None,
        decal_texture_sources: [None, None, None],
        mesh_source: None,
        material_surfaces: Vec::new(),
        skeleton_source: None,
        bone_pose_overrides: Vec::new(),
        mesh_skeleton_target: None,
        bone_attachment_skeleton_target: None,
        ik_target_skeleton_target: None,
        physics_bone_chain_skeleton_target: None,
        camera_stream_target: None,
        joint_body_links: Vec::new(),
        animation_bindings: Vec::new(),
        locale_text_bindings: Vec::new(),
    });
}

pub(super) fn prepare_entry_no_root(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<PreparedEntry, String> {
    let node = prepare_node_no_root(scene, entry, static_ui_style_lookup, scratch)?;
    let key_map = HashMap::new();
    let script = entry.script.as_ref().map(|script| {
        let script_path_hash = string_to_u64(script.as_ref());
        let script_mount = parse_dlc_mount_name(script.as_ref());
        PendingScript {
            node_key: node.key,
            #[cfg(test)]
            node_key_name: node.key_name.clone(),
            script_path_hash,
            script_mount,
            scene_injected_vars: entry
                .script_vars
                .iter()
                .map(|(k, v)| (k.to_string(), remap_scene_value_keys(v, scene, &key_map)))
                .collect(),
        }
    });

    Ok(PreparedEntry { node, script })
}

pub(super) fn prepare_node_no_root(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<PendingNode, String> {
    let key = entry.key.as_u32();
    let key_name = scene.key_name_or_id(entry.key).into_owned();
    let parent_key = entry.parent.map(|p| p.as_u32());

    let (
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        camera_stream_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ) = scene_node_from_entry(entry, static_ui_style_lookup, scratch)?;

    Ok(PendingNode {
        key,
        key_name,
        parent_key,
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations: animation_tree_animations
            .into_iter()
            .map(
                |(source, bindings, speed, paused, playback_type)| PendingAnimationTreeAnimation {
                    source,
                    bindings: bindings
                        .into_iter()
                        .filter_map(|(object, target)| {
                            scene_key_by_name(scene, target.as_str())
                                .map(|target| (object, target.as_u32()))
                        })
                        .collect(),
                    speed,
                    paused,
                    playback_type,
                },
            )
            .collect(),
        texture_source,
        decal_texture_sources: extract_decal_texture_sources(&entry.data),
        mesh_source,
        material_surfaces,
        skeleton_source,
        bone_pose_overrides: extract_bone_pose_overrides(&entry.data),
        mesh_skeleton_target: mesh_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        bone_attachment_skeleton_target: bone_attachment_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        ik_target_skeleton_target: ik_target_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        physics_bone_chain_skeleton_target: physics_bone_chain_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        camera_stream_target: camera_stream_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        joint_body_links: joint_body_targets
            .into_iter()
            .filter_map(|(field, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| PendingJointBodyLink {
                    field,
                    target_key: target.as_u32(),
                })
            })
            .collect(),
        animation_bindings: animation_bindings
            .into_iter()
            .filter_map(|(object, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| (object, target.as_u32()))
            })
            .collect(),
        locale_text_bindings,
    })
}

pub(super) fn push_entry_prepared(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    key_override: Option<u32>,
    key_map: &HashMap<SceneKey, u32>,
    ctx: &mut PrepareSceneCtx<'_>,
) -> Result<(), String> {
    let key = key_override.unwrap_or_else(|| remap_key(entry.key, key_map));
    let key_name = scene.key_name_or_id(entry.key).into_owned();
    let parent_key = entry.parent.map(|p| remap_key(p, key_map));
    let mut merged_root_entry = None;

    let root_of_source = entry.root_of.as_ref().map(|v| v.as_ref().to_string());
    if let Some(root_of_path) = root_of_source.as_ref() {
        if ctx.include_stack.contains(root_of_path) {
            return Err(format!(
                "root_of cycle detected while loading `{}` for host `{}`",
                root_of_path, key_name
            ));
        }
        ctx.include_stack.insert(root_of_path.clone());
        let root_merge_result = (|| {
            let import_scene = (ctx.load_scene)(root_of_path.as_str())?;
            let import_root = import_scene
                .root
                .ok_or_else(|| format!("root_of scene `{}` has no $root", root_of_path))?;
            let import_root_node = import_scene
                .nodes
                .iter()
                .find(|node| node.key == import_root)
                .ok_or_else(|| {
                    format!(
                        "root_of scene `{}` root key `{}` was not found in node list",
                        root_of_path,
                        import_scene.key_name_or_id(import_root)
                    )
                })?;
            let merged = merge_root_host_entry(entry, import_root_node);
            expand_import_children_into_host(
                key,
                root_of_path.as_str(),
                import_scene.as_ref(),
                &import_root,
                ctx,
            )?;
            Ok::<SceneDefNodeEntry, String>(merged)
        })();
        ctx.include_stack.remove(root_of_path);
        merged_root_entry = Some(root_merge_result?);
    }

    let entry = merged_root_entry.as_ref().unwrap_or(entry);

    let (
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        camera_stream_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ) = scene_node_from_entry(entry, ctx.static_ui_style_lookup, &mut ctx.scratch)?;

    #[cfg(test)]
    let test_node_key_name = key_name.clone();

    ctx.prepared_nodes.push(PendingNode {
        key,
        key_name,
        parent_key,
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations: animation_tree_animations
            .into_iter()
            .map(
                |(source, bindings, speed, paused, playback_type)| PendingAnimationTreeAnimation {
                    source,
                    bindings: bindings
                        .into_iter()
                        .filter_map(|(object, target)| {
                            scene_key_by_name(scene, target.as_str())
                                .map(|target| (object, remap_key(target, key_map)))
                        })
                        .collect(),
                    speed,
                    paused,
                    playback_type,
                },
            )
            .collect(),
        texture_source,
        decal_texture_sources: extract_decal_texture_sources(&entry.data),
        mesh_source,
        material_surfaces,
        skeleton_source,
        bone_pose_overrides: extract_bone_pose_overrides(&entry.data),
        mesh_skeleton_target: mesh_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        bone_attachment_skeleton_target: bone_attachment_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        ik_target_skeleton_target: ik_target_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        physics_bone_chain_skeleton_target: physics_bone_chain_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        camera_stream_target: camera_stream_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        joint_body_links: joint_body_targets
            .into_iter()
            .filter_map(|(field, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| PendingJointBodyLink {
                    field,
                    target_key: remap_key(target, key_map),
                })
            })
            .collect(),
        animation_bindings: animation_bindings
            .into_iter()
            .filter_map(|(object, target)| {
                scene_key_by_name(scene, target.as_str())
                    .map(|target| (object, remap_key(target, key_map)))
            })
            .collect(),
        locale_text_bindings,
    });

    if let Some(script) = entry.script.as_ref() {
        let script_path_hash = string_to_u64(script.as_ref());
        let script_mount = entry
            .script
            .as_ref()
            .and_then(|path| parse_dlc_mount_name(path.as_ref()));
        ctx.scripts.push(PendingScript {
            node_key: key,
            #[cfg(test)]
            node_key_name: test_node_key_name,
            script_path_hash,
            script_mount,
            scene_injected_vars: entry
                .script_vars
                .iter()
                .map(|(k, v)| (k.to_string(), remap_scene_value_keys(v, scene, key_map)))
                .collect(),
        });
    }

    Ok(())
}
