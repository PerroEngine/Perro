//! Nested `Variant` member walking shared by generated script glue.
//!
//! The compiler used to emit these three helpers verbatim into every generated
//! script even though none of them names a per-script type, so a project with N
//! scripts compiled ~N copies. Generated code calls them here instead.

use perro_ids::ScriptMemberID;
use perro_variant::Variant;

/// Find the member `var` inside `value`, matching on the dotted path hash.
///
/// `field_names` labels the lanes of an array-mode struct at this level;
/// deeper levels are object-keyed, so recursion passes an empty slice.
pub fn get_nested_by_hash(
    prefix: &str,
    value: Variant,
    var: ScriptMemberID,
    field_names: &[&str],
) -> Option<Variant> {
    if !has_members(&value, field_names) {
        return None;
    }
    get_nested(&mut member_path(prefix), value, var, field_names)
}

fn get_nested(
    path: &mut String,
    value: Variant,
    var: ScriptMemberID,
    field_names: &[&str],
) -> Option<Variant> {
    match value {
        Variant::Object(obj) => {
            for (key, child) in obj {
                let len = path.len();
                push_member(path, key.as_ref());
                let found = if ScriptMemberID::from_string(path) == var {
                    Some(child)
                } else {
                    get_nested(path, child, var, &[])
                };
                path.truncate(len);
                if let Some(found) = found {
                    return Some(found);
                }
            }
            None
        }
        Variant::Array(items) => {
            for (idx, child) in items.into_iter().enumerate() {
                let Some(key) = field_names.get(idx) else {
                    continue;
                };
                let len = path.len();
                push_member(path, key);
                let found = if ScriptMemberID::from_string(path) == var {
                    Some(child)
                } else {
                    get_nested(path, child, var, &[])
                };
                path.truncate(len);
                if let Some(found) = found {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Replace the member `var` inside `value` with `new_value`, taken on the
/// first hit. Returns whether anything was written.
pub fn set_nested_by_hash(
    prefix: &str,
    value: &mut Variant,
    var: ScriptMemberID,
    new_value: &mut Option<Variant>,
    field_names: &[&str],
) -> bool {
    if new_value.is_none() || !has_members(value, field_names) {
        return false;
    }
    set_nested(&mut member_path(prefix), value, var, new_value, field_names)
}

fn set_nested(
    path: &mut String,
    value: &mut Variant,
    var: ScriptMemberID,
    new_value: &mut Option<Variant>,
    field_names: &[&str],
) -> bool {
    match value {
        Variant::Object(obj) => {
            for (key, child) in obj {
                let len = path.len();
                push_member(path, key.as_ref());
                let changed = set_child(path, child, var, new_value);
                path.truncate(len);
                if changed {
                    return true;
                }
            }
            false
        }
        Variant::Array(items) => {
            for (idx, child) in items.iter_mut().enumerate() {
                let Some(key) = field_names.get(idx) else {
                    continue;
                };
                let len = path.len();
                push_member(path, key);
                let changed = set_child(path, child, var, new_value);
                path.truncate(len);
                if changed {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn set_child(
    path: &mut String,
    child: &mut Variant,
    var: ScriptMemberID,
    new_value: &mut Option<Variant>,
) -> bool {
    if ScriptMemberID::from_string(path) == var {
        let Some(new_value) = new_value.take() else {
            return false;
        };
        *child = new_value;
        true
    } else {
        set_nested(path, child, var, new_value, &[])
    }
}

/// Merge every key of an incoming object into `target` by member path.
/// Returns whether any member changed.
pub fn apply_nested_object(
    prefix: &str,
    target: &mut Variant,
    incoming: Variant,
    field_names: &[&str],
) -> bool {
    let Variant::Object(obj) = incoming else {
        return false;
    };
    if obj.is_empty() || !has_members(target, field_names) {
        return false;
    }
    let mut changed = false;
    let mut path = member_path(prefix);
    for (key, value) in obj {
        push_member(&mut path, key.as_ref());
        let member = ScriptMemberID::from_string(&path);
        path.truncate(prefix.len());
        let mut value = Some(value);
        changed |= set_nested(&mut path, target, member, &mut value, field_names);
    }
    changed
}

fn has_members(value: &Variant, field_names: &[&str]) -> bool {
    match value {
        Variant::Object(obj) => !obj.is_empty(),
        Variant::Array(items) => !items.is_empty() && !field_names.is_empty(),
        _ => false,
    }
}

// One reusable buffer per traversal; sibling/deep paths share its capacity.
fn member_path(prefix: &str) -> String {
    let mut path = String::with_capacity(prefix.len() + 32);
    path.push_str(prefix);
    path
}

fn push_member(path: &mut String, key: &str) {
    if !path.is_empty() {
        path.push('.');
    }
    path.push_str(key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn object(entries: &[(&str, Variant)]) -> Variant {
        let mut map = BTreeMap::<Arc<str>, Variant>::new();
        for (key, value) in entries {
            map.insert(Arc::from(*key), value.clone());
        }
        Variant::Object(map)
    }

    fn number(value: i64) -> Variant {
        Variant::from(value)
    }

    #[test]
    fn get_walks_object_and_array_levels() {
        let root = object(&[("inner", Variant::Array(vec![number(1), number(2)]))]);
        let found = get_nested_by_hash(
            "actors",
            root,
            ScriptMemberID::from_string("actors.inner"),
            &[],
        );
        assert!(matches!(found, Some(Variant::Array(_))));

        let array_root = Variant::Array(vec![number(7), number(8)]);
        let found = get_nested_by_hash(
            "actors",
            array_root,
            ScriptMemberID::from_string("actors.second"),
            &["first", "second"],
        );
        assert_eq!(found, Some(number(8)));
    }

    #[test]
    fn set_and_apply_write_nested_members() {
        let mut root = object(&[("count", number(1)), ("label", number(2))]);
        let mut value = Some(number(9));
        assert!(set_nested_by_hash(
            "leaf",
            &mut root,
            ScriptMemberID::from_string("leaf.count"),
            &mut value,
            &[],
        ));
        assert!(value.is_none());

        let incoming = object(&[("label", number(5))]);
        assert!(apply_nested_object("leaf", &mut root, incoming, &[]));
        assert_eq!(
            get_nested_by_hash("leaf", root, ScriptMemberID::from_string("leaf.label"), &[]),
            Some(number(5))
        );
    }

    #[test]
    fn missing_member_reports_no_change() {
        let mut root = object(&[("count", number(1))]);
        let mut value = Some(number(3));
        assert!(!set_nested_by_hash(
            "leaf",
            &mut root,
            ScriptMemberID::from_string("leaf.nope"),
            &mut value,
            &[],
        ));
        assert!(value.is_some());
        assert!(!apply_nested_object("leaf", &mut root, number(4), &[]));
    }

    #[test]
    fn path_buffer_restores_siblings_and_preserves_unicode_and_dots() {
        for prefix in ["", "root", "根.branch"] {
            let mut root = object(&[
                ("a", object(&[("deep", number(1))])),
                ("z.葉", object(&[("value", number(2))])),
            ]);
            let path = if prefix.is_empty() {
                "z.葉.value".to_string()
            } else {
                format!("{prefix}.z.葉.value")
            };
            let member = ScriptMemberID::from_string(&path);
            assert_eq!(
                get_nested_by_hash(prefix, root.clone(), member, &[]),
                Some(number(2))
            );
            let mut replacement = Some(number(9));
            assert!(set_nested_by_hash(
                prefix,
                &mut root,
                member,
                &mut replacement,
                &[]
            ));
            assert!(replacement.is_none());
            assert_eq!(
                get_nested_by_hash(prefix, root.clone(), member, &[]),
                Some(number(9))
            );

            assert!(apply_nested_object(
                prefix,
                &mut root,
                object(&[("z.葉.value", number(5))]),
                &[]
            ));
            assert_eq!(
                get_nested_by_hash(prefix, root, member, &[]),
                Some(number(5))
            );
        }
    }

    #[test]
    fn array_path_buffer_restores_after_child_miss() {
        let mut root = Variant::Array(vec![object(&[("x", number(1))]), number(2)]);
        let names = &["first", "second"];
        let member = ScriptMemberID::from_string("root.second");
        assert_eq!(
            get_nested_by_hash("root", root.clone(), member, names),
            Some(number(2))
        );
        assert!(apply_nested_object(
            "root",
            &mut root,
            object(&[("first.x", number(3)), ("second", number(4))]),
            names
        ));
        assert_eq!(
            get_nested_by_hash("root", root.clone(), member, names),
            Some(number(4))
        );
        assert_eq!(
            get_nested_by_hash(
                "root",
                root,
                ScriptMemberID::from_string("root.first.x"),
                names
            ),
            Some(number(3))
        );
    }
}
