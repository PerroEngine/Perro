use axum::body::Bytes;
use std::{
    collections::VecDeque,
    sync::{LazyLock, Mutex},
};

// Key by generated SVG rather than arbitrary request paths: unknown routes
// share the same fallback image. Bound both resident entries and cold raster
// concurrency; never hold a cache lock during raster work or an await.
const MAX_ENTRIES: usize = 64;
static CACHE: LazyLock<Mutex<VecDeque<(String, Bytes)>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));
static RASTER_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

fn cached(svg: &str) -> Option<Bytes> {
    CACHE
        .lock()
        .expect("social image cache lock")
        .iter()
        .find(|(key, _)| key == svg)
        .map(|(_, png)| png.clone())
}

pub(crate) async fn get_png(svg: String) -> Result<Bytes, String> {
    if let Some(png) = cached(&svg) {
        return Ok(png);
    }
    let _slot = RASTER_SLOTS
        .acquire()
        .await
        .map_err(|err| err.to_string())?;
    if let Some(png) = cached(&svg) {
        return Ok(png);
    }
    let source = svg.clone();
    let png = tokio::task::spawn_blocking(move || crate::svg_to_png(&source))
        .await
        .map_err(|err| err.to_string())??;
    let png = Bytes::from(png);
    let mut cache = CACHE.lock().expect("social image cache lock");
    if let Some((_, existing)) = cache.iter().find(|(key, _)| key == &svg) {
        return Ok(existing.clone());
    }
    if cache.len() == MAX_ENTRIES {
        cache.pop_front();
    }
    cache.push_back((svg, png.clone()));
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cached_png_keeps_pixels_dimensions_and_shared_storage() {
        let svg = crate::social_svg("Cache fixture", "Test", "Stable social image");
        let direct = crate::svg_to_png(&svg).expect("direct PNG");
        let first = get_png(svg.clone()).await.expect("cold PNG");
        let second = get_png(svg).await.expect("warm PNG");
        assert_eq!(first.as_ref(), direct.as_slice());
        assert_eq!(first.as_ptr(), second.as_ptr());
        assert_eq!(&first[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_be_bytes(first[16..20].try_into().expect("PNG width")),
            1200
        );
        assert_eq!(
            u32::from_be_bytes(first[20..24].try_into().expect("PNG height")),
            630
        );
    }
}
