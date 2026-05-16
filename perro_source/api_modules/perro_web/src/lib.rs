use std::sync::{Mutex, OnceLock};

pub mod storage;

fn normalize_href_impl(href: &str) -> String {
    let trimmed = href.trim();
    let path = trimmed.split(['?', '#']).next().unwrap_or("/").trim();
    let core = if path.is_empty() { "/" } else { path };
    let mut normalized = if core.starts_with('/') {
        core.to_string()
    } else {
        format!("/{core}")
    };
    if normalized.len() > "/index.html".len() && normalized.ends_with("/index.html") {
        normalized.truncate(normalized.len() - "/index.html".len());
    }
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    if normalized.is_empty() {
        normalized.push('/');
    }
    normalized
}

fn split_href_impl(href: &str) -> Vec<String> {
    let normalized = normalize_href_impl(href);
    if normalized == "/" {
        return Vec::new();
    }
    normalized
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_query_args_impl(href: &str) -> Vec<String> {
    let Some((_, tail)) = href.split_once('?') else {
        return Vec::new();
    };
    tail.split('#')
        .next()
        .unwrap_or("")
        .split('&')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn pending_route_slot() -> &'static Mutex<Option<String>> {
    static SLOT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(target_arch = "wasm32")]
fn router_init_slot() -> &'static OnceLock<()> {
    static INIT: OnceLock<()> = OnceLock::new();
    &INIT
}

#[cfg(target_arch = "wasm32")]
fn set_pending_route_change(href: &str) {
    if let Ok(mut slot) = pending_route_slot().lock() {
        *slot = Some(normalize_href_impl(href));
    }
}

pub fn normalize_href(href: &str) -> String {
    normalize_href_impl(href)
}

pub fn current_href() -> Option<String> {
    current_href_impl()
}

pub fn get_args() -> Option<Vec<String>> {
    get_args_impl()
}

pub fn push_route(href: &str) -> bool {
    push_route_impl(href)
}

pub fn pop_route() -> bool {
    pop_route_impl()
}

pub fn take_pending_route_change() -> Option<String> {
    pending_route_slot().lock().ok()?.take()
}

pub fn init_router() -> bool {
    init_router_impl()
}

pub fn split_href(href: &str) -> Vec<String> {
    split_href_impl(href)
}

pub fn split_query_args(href: &str) -> Vec<String> {
    split_query_args_impl(href)
}

#[cfg(not(target_arch = "wasm32"))]
fn current_href_impl() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn get_args_impl() -> Option<Vec<String>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn push_route_impl(_: &str) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn pop_route_impl() -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn init_router_impl() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn current_href_impl() -> Option<String> {
    let window = web_sys::window()?;
    let pathname = window.location().pathname().ok()?;
    Some(normalize_href_impl(&pathname))
}

#[cfg(target_arch = "wasm32")]
fn get_args_impl() -> Option<Vec<String>> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    Some(split_query_args_impl(&search))
}

#[cfg(target_arch = "wasm32")]
fn push_route_impl(href: &str) -> bool {
    use wasm_bindgen::JsValue;

    let normalized = normalize_href_impl(href);
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(history) = window.history() else {
        return false;
    };
    if history
        .push_state_with_url(&JsValue::NULL, "", Some(&normalized))
        .is_err()
    {
        return false;
    }
    set_pending_route_change(&normalized);
    true
}

#[cfg(target_arch = "wasm32")]
fn pop_route_impl() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(history) = window.history() else {
        return false;
    };
    history.back().is_ok()
}

#[cfg(target_arch = "wasm32")]
fn init_router_impl() -> bool {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return false;
    };

    let _ = router_init_slot().get_or_init(|| {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Some(current) = current_href_impl() {
                set_pending_route_change(&current);
            }
        }) as Box<dyn FnMut(_)>);
        let _ =
            window.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
        closure.forget();
    });

    true
}

#[cfg(test)]
mod tests {
    use super::{normalize_href, split_href, split_query_args};

    #[test]
    fn normalize_href_strips_query_hash_and_slash() {
        assert_eq!(normalize_href("/docs/?a=1#top"), "/docs");
        assert_eq!(normalize_href("docs"), "/docs");
        assert_eq!(normalize_href(""), "/");
        assert_eq!(normalize_href("/docs/index.html"), "/docs");
    }

    #[test]
    fn split_href_handles_root_and_segments() {
        assert_eq!(split_href("/"), Vec::<String>::new());
        assert_eq!(split_href("/docs/api"), vec!["docs", "api"]);
        assert_eq!(
            split_href("docs/api/?tab=1"),
            vec!["docs".to_string(), "api".to_string()]
        );
    }

    #[test]
    fn split_query_args_handles_empty_and_segments() {
        assert_eq!(split_query_args("/docs"), Vec::<String>::new());
        assert_eq!(split_query_args("/docs?a=1&b=2"), vec!["a=1", "b=2"]);
        assert_eq!(split_query_args("?arg1&arg2#top"), vec!["arg1", "arg2"]);
    }
}
