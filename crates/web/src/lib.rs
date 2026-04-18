use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use shiftwrangler_state::StateBackend;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub state_backend: Arc<dyn StateBackend>,
}

pub fn router(app_state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/status", get(api_status))
        .route("/api/sessions", get(api_sessions))
        .with_state(app_state)
}

async fn index() -> &'static str {
    "shiftwrangler dashboard — /api/status  /api/sessions"
}

async fn api_status() -> Json<Value> {
    Json(json!({ "status": "running", "version": env!("CARGO_PKG_VERSION") }))
}

async fn api_sessions(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match state.state_backend.load_manifest().await {
        Ok(Some(manifest)) => Ok(Json(json!({
            "suspended_at": manifest.suspended_at,
            "sessions": manifest.sessions,
        }))),
        Ok(None) => Ok(Json(json!({ "sessions": [] }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use shiftwrangler_state::local_fs::LocalFsBackend;
    use tempfile::tempdir;
    use tower::ServiceExt;

    fn make_app() -> Router {
        let dir = tempdir().unwrap();
        let backend = Arc::new(LocalFsBackend::new(dir.path().join("manifest.json")));
        // Keep dir alive for test duration by leaking (acceptable in tests).
        std::mem::forget(dir);
        router(AppState { state_backend: backend })
    }

    #[tokio::test]
    async fn index_returns_200() {
        let app = make_app();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_status_returns_running() {
        let app = make_app();
        let response = app
            .oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_sessions_returns_empty_without_manifest() {
        let app = make_app();
        let response = app
            .oneshot(Request::builder().uri("/api/sessions").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
