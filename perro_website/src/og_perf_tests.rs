// Baseline-compatible: copy this file and the module hook in main.rs.
use axum::response::IntoResponse;
use std::{hint::black_box, time::Instant};

#[tokio::test]
#[ignore = "manual release timing probe; run serially"]
async fn time_og_png_repeated_route() {
    async fn request() -> axum::body::Bytes {
        let response = crate::og_image(axum::extract::Path(String::from("features.png")))
            .await
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("PNG body")
    }
    let expected = request().await;
    let mut samples = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let output = black_box(request().await);
        samples.push(start.elapsed().as_nanos());
        assert_eq!(output, expected);
    }
    let checksum = expected.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    println!("{{\"case\":\"og_png_repeated_route\",\"operations_per_sample\":1,\"samples_ns\":{samples:?},\"output_bytes\":{},\"checksum\":{checksum}}}", expected.len());
}
