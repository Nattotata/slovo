//! HTTP API over the Slovo engine.
//!
//! The engine stays in `slovo-core`; this crate only exposes it over HTTP so
//! the browser UI and the desktop shell can talk to the same thing.
//!
//! Every handler formats its JSON with the shared types in
//! [`slovo_core::api`], which are also what the CLI emits with
//! `--format json`. The CLI and the API therefore cannot drift apart.

use std::net::SocketAddr;

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;

use slovo_core::api::{
    ApiError as ApiErrorBody, DatasetInfo, DatasetSummary, FreqResponse, Health, SearchResponse,
};
use slovo_core::query::By;
use slovo_core::search;
use slovo_service::Service;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8090;

/// An error rendered as `{"error": "..."}`.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<slovo_core::Error> for ApiError {
    fn from(error: slovo_core::Error) -> Self {
        use slovo_core::Error;
        let status = match &error {
            Error::DatasetNotFound(_) => StatusCode::NOT_FOUND,
            Error::Query(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

/// Run engine work off the async runtime: SQLite is blocking.
async fn blocking<T, F>(work: F) -> Result<T, ApiError>
where
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ApiError::internal(format!("worker panicked: {error}")))?
}

fn parse_by(value: &str) -> Result<By, ApiError> {
    match value {
        "word" => Ok(By::Word),
        "form" => Ok(By::Form),
        "lemma" => Ok(By::Lemma),
        "upos" => Ok(By::Upos),
        "xpos" => Ok(By::Xpos),
        other => Err(ApiError::bad_request(format!(
            "unknown `by` value `{other}`; expected one of word, form, lemma, upos, xpos"
        ))),
    }
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
        version: VERSION.to_string(),
    })
}

async fn datasets() -> Result<Json<Vec<DatasetSummary>>, ApiError> {
    let summaries = blocking(|| Ok(Service::new().list()?)).await?;
    Ok(Json(summaries))
}

async fn dataset_info(Path(name): Path<String>) -> Result<Json<DatasetInfo>, ApiError> {
    let info = blocking(move || Ok(Service::new().info(&name)?)).await?;
    Ok(Json(info))
}

#[derive(Debug, Deserialize)]
struct FreqParams {
    #[serde(default)]
    by: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

async fn get_freq(
    Path(name): Path<String>,
    Query(params): Query<FreqParams>,
) -> Result<Json<FreqResponse>, ApiError> {
    let by = parse_by(params.by.as_deref().unwrap_or("word"))?;
    let limit = params.limit.unwrap_or(20);

    let report = blocking(move || Ok(Service::new().frequency(&name, by, limit)?)).await?;

    Ok(Json(report))
}

#[derive(Debug, Deserialize)]
struct QueryParams {
    pattern: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    within: Option<u32>,
    #[serde(default)]
    min_freq: Option<u64>,
    #[serde(default)]
    case_sensitive: Option<bool>,
}

async fn get_query(
    Path(name): Path<String>,
    Query(params): Query<QueryParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    let options = search::SearchOptions {
        pattern: params.pattern,
        limit: params.limit.unwrap_or(20),
        case_sensitive: params.case_sensitive.unwrap_or(false),
        within: params.within.unwrap_or(5),
        min_freq: params.min_freq.unwrap_or(1),
    };

    let report = blocking(move || Ok(Service::new().query(&name, &options)?)).await?;

    Ok(Json(report))
}

/// The API routes, without serving them. Handy for tests.
pub fn router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/datasets", get(datasets))
        .route("/api/datasets/{name}", get(dataset_info))
        .route("/api/datasets/{name}/freq", get(get_freq))
        .route("/api/datasets/{name}/query", get(get_query))
        // Permissive so the Vite dev server can call it. Tighten before
        // exposing this beyond the machine.
        .layer(CorsLayer::permissive())
}

/// Serve the API until interrupted.
pub fn serve(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let address: SocketAddr = format!("{host}:{port}").parse()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(address).await?;
        println!("slovo server listening on http://{address}");
        if !address.ip().is_loopback() {
            eprintln!(
                "warning: bound to {}, and the API has no authentication",
                address.ip()
            );
        }
        axum::serve(listener, router()).await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
}
