// Type inference cache for performance optimization
use std::cell::RefCell;
use std::collections::HashMap;
use crate::scripting::ast::{Expr, Type};

thread_local! {
    static TYPE_CACHE: RefCell<HashMap<usize, Option<Type>>> = RefCell::new(HashMap::new());
    pub(crate) static SCRIPT_MEMBERS_CACHE: RefCell<Option<(usize, std::collections::HashSet<String>)>> = RefCell::new(None);
}

pub(crate) fn expr_cache_key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

pub(crate) fn clear_type_cache() {
    TYPE_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn clear_script_members_cache() {
    SCRIPT_MEMBERS_CACHE.with(|cache| *cache.borrow_mut() = None);
}

pub(crate) fn get_cached_type(key: usize) -> Option<Option<Type>> {
    TYPE_CACHE.with(|cache| cache.borrow().get(&key).cloned())
}

pub(crate) fn set_cached_type(key: usize, typ: Option<Type>) {
    TYPE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, typ);
    });
}

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

pub(crate) fn set_cached_script_members(key: usize, members: std::collections::HashSet<String>) {
    SCRIPT_MEMBERS_CACHE.with(|cache| {
        *cache.borrow_mut() = Some((key, members));
    });
}

