use crate::{Scene, SceneNodeData, SceneNodeDataBase, SceneObjectField, SceneValue};
use std::{borrow::Cow, collections::HashSet};

pub const DEMO_EXCLUDE_TAG: &str = "demo_exclude";

pub fn filter_demo_scene(scene: &mut Scene, demo: bool) -> Result<(), String> {
    let mut removed = HashSet::new();
    for node in scene.nodes.to_mut().iter_mut() {
        let marked = node.tags.iter().any(|tag| tag == DEMO_EXCLUDE_TAG);
        node.tags.to_mut().retain(|tag| tag != DEMO_EXCLUDE_TAG);
        if demo && marked {
            removed.insert(node.key);
        }
    }
    if !demo || removed.is_empty() {
        return Ok(());
    }

    loop {
        let mut changed = false;
        for node in scene.nodes.iter() {
            if node.parent.is_some_and(|parent| removed.contains(&parent))
                && removed.insert(node.key)
            {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    if scene.root.is_some_and(|root| removed.contains(&root)) {
        return Err("demo filter removes scene root".to_string());
    }
    let removed_names = removed
        .iter()
        .filter_map(|key| scene.key_name(*key).map(str::to_string))
        .collect::<HashSet<_>>();
    for node in scene
        .nodes
        .iter()
        .filter(|node| !removed.contains(&node.key))
    {
        check_fields(node.script_vars.as_ref(), &removed_names)?;
        check_data(&node.data, &removed_names)?;
    }

    let kept = scene
        .nodes
        .iter()
        .filter(|node| !removed.contains(&node.key))
        .cloned()
        .map(|mut node| {
            node.children = Cow::Owned(
                node.children
                    .iter()
                    .copied()
                    .filter(|child| !removed.contains(child))
                    .collect(),
            );
            node
        })
        .collect::<Vec<_>>();
    scene.nodes = Cow::Owned(kept);
    Ok(())
}

fn check_data(data: &SceneNodeData, removed: &HashSet<String>) -> Result<(), String> {
    check_fields(data.fields.as_ref(), removed)?;
    if let Some(base) = data.base.as_ref() {
        match base {
            SceneNodeDataBase::Borrowed(data) => check_data(data, removed)?,
            SceneNodeDataBase::Owned(data) => check_data(data, removed)?,
        }
    }
    Ok(())
}

fn check_fields(fields: &[SceneObjectField], removed: &HashSet<String>) -> Result<(), String> {
    for (name, value) in fields {
        check_value(value, removed)
            .map_err(|target| format!("field `{name}` refs demo-excluded node `@{target}`"))?;
    }
    Ok(())
}

fn check_value(value: &SceneValue, removed: &HashSet<String>) -> Result<(), String> {
    match value {
        SceneValue::Key(key) if removed.contains(key.as_ref()) => Err(key.as_ref().to_string()),
        SceneValue::Object(fields) => {
            for (_, value) in fields.iter() {
                check_value(value, removed)?;
            }
            Ok(())
        }
        SceneValue::Array(values) => {
            for value in values.iter() {
                check_value(value, removed)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
