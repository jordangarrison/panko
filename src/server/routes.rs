//! HTTP routes for the web server.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;

use super::assets::{content_type, StaticAssets};
use super::templates::TemplateEngine;
use crate::parser::Session;

/// Shared application state.
pub struct AppState {
    pub session: Session,
    pub template_engine: TemplateEngine,
    /// Optional path to the source session file (for download).
    pub source_path: Option<PathBuf>,
}

/// Build the router with all routes.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(session_handler))
        .route("/download", get(download_handler))
        .route("/assets/*path", get(assets_handler))
        .with_state(state)
}

/// Handler for the main session view.
async fn session_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.template_engine.render_session(&state.session) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {}", e),
        )
            .into_response(),
    }
}

/// Handler for downloading the session JSONL file.
async fn download_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Get the source path
    let source_path = match &state.source_path {
        Some(path) => path,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(axum::body::Body::from(
                    "Session file not available for download",
                ))
                .unwrap()
        }
    };

    // Read the file contents
    let contents = match std::fs::read(source_path) {
        Ok(c) => c,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from(format!(
                    "Failed to read file: {}",
                    e
                )))
                .unwrap()
        }
    };

    // Get the filename for Content-Disposition
    let filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("session.jsonl");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/jsonl")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(contents))
        .unwrap()
}

/// Handler for static assets.
async fn assets_handler(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = content_type(&path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(axum::body::Body::from(file.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("Not found"))
            .unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use chrono::Utc;
    use tower::util::ServiceExt;

    use crate::parser::Block;

    fn create_test_state() -> Arc<AppState> {
        let mut session = Session::new("test-session", Utc::now());
        session.add_block(Block::user_prompt("Hello", Utc::now()));
        session.add_block(Block::assistant_response("Hi there!", Utc::now()));

        Arc::new(AppState {
            session,
            template_engine: TemplateEngine::default(),
            source_path: None,
        })
    }

    #[tokio::test]
    async fn test_session_handler_returns_html() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/html"));
    }

    #[tokio::test]
    async fn test_assets_handler_css() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/styles.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/css"));
    }

    #[tokio::test]
    async fn test_assets_handler_js() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/htmx.min.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("javascript"));
    }

    #[tokio::test]
    async fn test_assets_handler_not_found() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/nonexistent.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_download_handler_no_source_path() {
        // Create state without source_path
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 404 when no source_path is set
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_download_handler_with_source_path() {
        use tempfile::NamedTempFile;

        // Create a temporary file with some content
        let temp_file = NamedTempFile::new().unwrap();
        let content = b"{\"type\":\"test\"}\n";
        std::fs::write(temp_file.path(), content).unwrap();

        // Create state with source_path
        let mut session = Session::new("test-session", Utc::now());
        session.add_block(Block::user_prompt("Hello", Utc::now()));

        let state = Arc::new(AppState {
            session,
            template_engine: TemplateEngine::default(),
            source_path: Some(temp_file.path().to_path_buf()),
        });
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Check Content-Type header
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("jsonl"));

        // Check Content-Disposition header
        let disposition = response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(disposition.contains("attachment"));
        assert!(disposition.contains("filename="));
    }
}
