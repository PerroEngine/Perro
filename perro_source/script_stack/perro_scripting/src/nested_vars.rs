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
    match value {
        Variant::Object(obj) => {
            for (key, child) in obj {
                let full = join_member(prefix, key.as_ref());
                if ScriptMemberID::from_string(full.as_str()) == var {
                    return Some(child);
                }
                if let Some(found) = get_nested_by_hash(full.as_str(), child, var, &[]) {
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
                let full = join_member(prefix, key);
                if ScriptMemberID::from_string(full.as_str()) == var {
                    return Some(child);
                }
                if let Some(found) = get_nested_by_hash(full.as_str(), child, var, &[]) {
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
    match value {
        Variant::Object(obj) => {
            for (key, child) in obj {
                let full = join_member(prefix, key.as_ref());
                if ScriptMemberID::from_string(full.as_str()) == var {
                    let Some(new_value) = new_value.take() else {
                        return false;
                    };
                    *child = new_value;
                    return true;
                }
                if set_nested_by_hash(full.as_str(), child, var, new_value, &[]) {
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
                let full = join_member(prefix, key);
                if ScriptMemberID::from_string(full.as_str()) == var {
                    let Some(new_value) = new_value.take() else {
                        return false;
                    };
                    *child = new_value;
                    return true;
                }
                if set_nested_by_hash(full.as_str(), child, var, new_value, &[]) {
                    return true;
                }
            }
            false
        }
        _ => false,
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
    let mut changed = false;
    for (key, value) in obj {
        let full = join_member(prefix, key.as_ref());
        let mut value = Some(value);
        changed |= set_nested_by_hash(
            prefix,
            target,
            ScriptMemberID::from_string(full.as_str()),
            &mut value,
            field_names,
        );
    }
    changed
}

fn join_member(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
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
        let root = object(&[(
            "inner",
            Variant::Array(vec![number(1), number(2)]),
        )]);
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
            get_nested_by_hash(
                "leaf",
                root,
                ScriptMemberID::from_string("leaf.label"),
                &[]
            ),
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
}
