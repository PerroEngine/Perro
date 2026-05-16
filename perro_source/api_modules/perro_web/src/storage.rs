use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageScope {
    Local,
    Session,
    Cookie,
}

pub fn load_bytes(scope: StorageScope, key: &str) -> io::Result<Option<Vec<u8>>> {
    load_bytes_impl(scope, key)
}

pub fn save_bytes(scope: StorageScope, key: &str, data: &[u8]) -> io::Result<()> {
    save_bytes_impl(scope, key, data)
}

pub fn remove(scope: StorageScope, key: &str) -> io::Result<()> {
    remove_impl(scope, key)
}

pub fn load_string(scope: StorageScope, key: &str) -> io::Result<Option<String>> {
    let Some(bytes) = load_bytes(scope, key)? else {
        return Ok(None);
    };
    let text =
        String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(Some(text))
}

pub fn save_string(scope: StorageScope, key: &str, text: &str) -> io::Result<()> {
    save_bytes(scope, key, text.as_bytes())
}

pub fn load_local_bytes(key: &str) -> io::Result<Option<Vec<u8>>> {
    load_bytes(StorageScope::Local, key)
}

pub fn save_local_bytes(key: &str, data: &[u8]) -> io::Result<()> {
    save_bytes(StorageScope::Local, key, data)
}

pub fn remove_local(key: &str) -> io::Result<()> {
    remove(StorageScope::Local, key)
}

pub fn load_session_bytes(key: &str) -> io::Result<Option<Vec<u8>>> {
    load_bytes(StorageScope::Session, key)
}

pub fn save_session_bytes(key: &str, data: &[u8]) -> io::Result<()> {
    save_bytes(StorageScope::Session, key, data)
}

pub fn remove_session(key: &str) -> io::Result<()> {
    remove(StorageScope::Session, key)
}

pub fn load_cookie_bytes(key: &str) -> io::Result<Option<Vec<u8>>> {
    load_bytes(StorageScope::Cookie, key)
}

pub fn save_cookie_bytes(key: &str, data: &[u8]) -> io::Result<()> {
    save_bytes(StorageScope::Cookie, key, data)
}

pub fn remove_cookie(key: &str) -> io::Result<()> {
    remove(StorageScope::Cookie, key)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_bytes_impl(_: StorageScope, _: &str) -> io::Result<Option<Vec<u8>>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "web storage requires wasm32 target",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_bytes_impl(_: StorageScope, _: &str, _: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "web storage requires wasm32 target",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_impl(_: StorageScope, _: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "web storage requires wasm32 target",
    ))
}

#[cfg(target_arch = "wasm32")]
fn encode_value(data: &[u8]) -> String {
    use base64::Engine as _;

    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(target_arch = "wasm32")]
fn decode_value(data: &str) -> io::Result<Vec<u8>> {
    use base64::Engine as _;

    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[cfg(target_arch = "wasm32")]
fn js_err(err: wasm_bindgen::JsValue) -> io::Error {
    io::Error::other(
        err.as_string()
            .unwrap_or_else(|| "web storage call fail".to_string()),
    )
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> io::Result<web_sys::Storage> {
    let window = web_sys::window()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "window !found"))?;
    let storage = window.local_storage().map_err(js_err)?;
    storage.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "localStorage !found"))
}

#[cfg(target_arch = "wasm32")]
fn session_storage() -> io::Result<web_sys::Storage> {
    let window = web_sys::window()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "window !found"))?;
    let storage = window.session_storage().map_err(js_err)?;
    storage.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "sessionStorage !found"))
}

#[cfg(target_arch = "wasm32")]
fn document() -> io::Result<web_sys::HtmlDocument> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "window !found"))?;
    window
        .document()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "document !found"))?
        .dyn_into::<web_sys::HtmlDocument>()
        .map_err(|_| io::Error::other("html document cast fail"))
}

#[cfg(target_arch = "wasm32")]
fn cookie_encode(text: &str) -> String {
    js_sys::encode_uri_component(text)
        .as_string()
        .unwrap_or_else(|| text.to_string())
}

#[cfg(target_arch = "wasm32")]
fn cookie_decode(text: &str) -> io::Result<String> {
    js_sys::decode_uri_component(text)
        .map_err(js_err)?
        .as_string()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "cookie decode fail"))
}

#[cfg(target_arch = "wasm32")]
fn load_cookie_value(key: &str) -> io::Result<Option<String>> {
    let all = document()?.cookie().map_err(js_err)?;
    for pair in all.split(';') {
        let trimmed = pair.trim();
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name == key {
            return cookie_decode(value).map(Some);
        }
    }
    Ok(None)
}

#[cfg(target_arch = "wasm32")]
fn save_cookie_value(key: &str, value: &str) -> io::Result<()> {
    let encoded = cookie_encode(value);
    document()?
        .set_cookie(&format!("{key}={encoded}; path=/; SameSite=Lax"))
        .map_err(js_err)
}

#[cfg(target_arch = "wasm32")]
fn remove_cookie_value(key: &str) -> io::Result<()> {
    document()?
        .set_cookie(&format!(
            "{key}=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT; SameSite=Lax"
        ))
        .map_err(js_err)
}

#[cfg(target_arch = "wasm32")]
fn load_bytes_impl(scope: StorageScope, key: &str) -> io::Result<Option<Vec<u8>>> {
    let encoded = match scope {
        StorageScope::Local => local_storage()?.get_item(key).map_err(js_err)?,
        StorageScope::Session => session_storage()?.get_item(key).map_err(js_err)?,
        StorageScope::Cookie => load_cookie_value(key)?,
    };
    encoded.as_deref().map(decode_value).transpose()
}

#[cfg(target_arch = "wasm32")]
fn save_bytes_impl(scope: StorageScope, key: &str, data: &[u8]) -> io::Result<()> {
    let encoded = encode_value(data);
    match scope {
        StorageScope::Local => local_storage()?.set_item(key, &encoded).map_err(js_err),
        StorageScope::Session => session_storage()?.set_item(key, &encoded).map_err(js_err),
        StorageScope::Cookie => save_cookie_value(key, &encoded),
    }
}

#[cfg(target_arch = "wasm32")]
fn remove_impl(scope: StorageScope, key: &str) -> io::Result<()> {
    match scope {
        StorageScope::Local => local_storage()?.remove_item(key).map_err(js_err),
        StorageScope::Session => session_storage()?.remove_item(key).map_err(js_err),
        StorageScope::Cookie => remove_cookie_value(key),
    }
}

#[cfg(test)]
mod tests {
    use super::StorageScope;

    #[test]
    fn scopes_stay_distinct() {
        assert_ne!(StorageScope::Local, StorageScope::Session);
        assert_ne!(StorageScope::Session, StorageScope::Cookie);
        assert_ne!(StorageScope::Local, StorageScope::Cookie);
    }
}
