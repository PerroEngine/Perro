// Caches for codegen (script members only; type inference cache was removed for determinism)
use std::cell::RefCell;

thread_local! {
    pub(crate) static SCRIPT_MEMBERS_CACHE: RefCell<Option<(usize, std::collections::HashSet<String>)>> = RefCell::new(None);
}

/// No-op: type inference cache was removed so codegen is deterministic (same input → same output).
pub(crate) fn clear_type_cache() {}

pub(crate) fn clear_script_members_cache() {
    SCRIPT_MEMBERS_CACHE.with(|cache| *cache.borrow_mut() = None);
}

#[allow(dead_code)]
pub(crate) fn get_cached_script_members(key: usize) -> Option<std::collections::HashSet<String>> {
    SCRIPT_MEMBERS_CACHE.with(|cache| {
        cache.borrow().as_ref().and_then(|(k, members)| {
            if *k == key {
                Some(members.clone())
            } else {
                None
            }
        })
    })
}

#[allow(dead_code)]
pub(crate) fn set_cached_script_members(key: usize, members: std::collections::HashSet<String>) {
    SCRIPT_MEMBERS_CACHE.with(|cache| {
        *cache.borrow_mut() = Some((key, members));
    });
}
