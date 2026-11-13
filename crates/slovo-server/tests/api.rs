//! Route tests.
//!
//! These deliberately avoid needing an installed dataset, so they are
//! independent of `SLOVO_DATA_DIR` and safe to run in parallel.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use slovo_core::api::{ApiError, Health};
use slovo_server::router;
use tower::ServiceExt;

async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
    let response = router()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, bytes.to_vec())
}

#[tokio::test]
async fn health_reports_ok() {
    let (status, body) = get("/api/health").await;
    assert_eq!(status, StatusCode::OK);

    let health: Health = serde_json::from_slice(&body).unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.version, slovo_server::VERSION);
}

#[tokio::test]
async fn unknown_dataset_is_a_404() {
    let (status, body) = get("/api/datasets/no-such-dataset").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let error: ApiError = serde_json::from_slice(&body).unwrap();
    assert!(
        error.error.contains("no-such-dataset"),
        "unhelpful message: {}",
        error.error
    );
}

#[tokio::test]
async fn a_bad_by_value_is_a_400() {
    // Validated before the database is touched, so no dataset is needed.
    let (status, body) = get("/api/datasets/whatever/freq?by=nonsense").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let error: ApiError = serde_json::from_slice(&body).unwrap();
    assert!(error.error.contains("nonsense"), "{}", error.error);
}

#[tokio::test]
async fn query_without_a_pattern_is_a_400() {
    let (status, _) = get("/api/datasets/whatever/query").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
