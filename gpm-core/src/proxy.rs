use crate::ollama::{OllamaApiResponse, OllamaMonitor};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    routing::any,
    Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct ProxyState {
    pub client: reqwest::Client,
    pub ollama_backend: String,
    pub ollama_monitor: Arc<OllamaMonitor>,
}

pub struct OllamaProxy {
    listen_port: u16,
    backend_url: String,
    ollama_monitor: Arc<OllamaMonitor>,
}

impl OllamaProxy {
    pub fn new(listen_port: u16, backend_url: String, ollama_monitor: Arc<OllamaMonitor>) -> Self {
        Self {
            listen_port,
            backend_url,
            ollama_monitor,
        }
    }

    pub async fn run(&self, mut shutdown_rx: broadcast::Receiver<()>) -> crate::Result<()> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| crate::GpmError::ProxyError(format!("Failed to create HTTP client: {}", e)))?;

        let state = ProxyState {
            client,
            ollama_backend: self.backend_url.clone(),
            ollama_monitor: Arc::clone(&self.ollama_monitor),
        };

        let app = Router::new()
            .route("/", any(proxy_handler))
            .route("/*path", any(proxy_handler))
            .with_state(state);

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], self.listen_port));
        let listener = tokio::net::TcpListener::bind(addr).await
            .map_err(|e| crate::GpmError::ProxyError(format!("Failed to bind to port {}: {}", self.listen_port, e)))?;

        info!("Ollama proxy listening on http://0.0.0.0:{}", self.listen_port);
        info!("Forwarding to backend: {}", self.backend_url);

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                info!("Ollama proxy shutting down");
            })
            .await
            .map_err(|e| crate::GpmError::ProxyError(format!("Proxy server error: {}", e)))?;

        Ok(())
    }
}

async fn proxy_handler(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Response<Body> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();
    let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();

    let backend_url = format!("{}{}{}", state.ollama_backend, path, query);

    debug!("Proxying {} {} -> {}", method, path, backend_url);

    let is_streaming_endpoint = path == "/api/generate" || path == "/api/chat";

    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Failed to read request body: {}", e)))
                .unwrap();
        }
    };

    let mut request_builder = state.client.request(method.clone(), &backend_url);

    for (name, value) in headers.iter() {
        if name != "host" && name != "content-length" {
            if let Ok(v) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), v);
            }
        }
    }

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.clone());
    }

    let response = match request_builder.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to Ollama: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Failed to connect to Ollama backend: {}", e)))
                .unwrap();
        }
    };

    let status = response.status();
    let resp_headers = response.headers().clone();

    if is_streaming_endpoint && status.is_success() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let model = extract_model_from_request(&body_bytes);

        debug!("Starting LLM session tracking: {} (model: {})", session_id, model);

        let ollama_monitor = Arc::clone(&state.ollama_monitor);
        let stream = response.bytes_stream();

        let tracked_stream = stream.map(move |chunk_result| {
            match &chunk_result {
                Ok(bytes) => {
                    if let Some(api_response) = parse_streaming_chunk(bytes) {
                        let monitor = ollama_monitor.clone();
                        let sid = session_id.clone();
                        let mdl = model.clone();
                        tokio::spawn(async move {
                            monitor.track_generation(sid, mdl, &api_response).await;
                        });
                    }
                }
                Err(e) => {
                    warn!("Stream chunk error: {}", e);
                }
            }
            chunk_result.map(|b| axum::body::Bytes::from(b.to_vec()))
        });

        let body = Body::from_stream(tracked_stream);

        let mut response_builder = Response::builder().status(status);
        for (name, value) in resp_headers.iter() {
            if name != "transfer-encoding" && name != "content-length" {
                response_builder = response_builder.header(name, value);
            }
        }
        response_builder = response_builder.header("transfer-encoding", "chunked");

        return response_builder.body(body).unwrap();
    }

    let body_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Failed to read response: {}", e)))
                .unwrap();
        }
    };

    let mut response_builder = Response::builder().status(status);
    for (name, value) in resp_headers.iter() {
        if name != "transfer-encoding" {
            response_builder = response_builder.header(name, value);
        }
    }

    response_builder.body(Body::from(body_bytes.to_vec())).unwrap()
}

fn extract_model_from_request(body: &Bytes) -> String {
    if body.is_empty() {
        return "unknown".to_string();
    }

    #[derive(serde::Deserialize)]
    struct RequestBody {
        model: Option<String>,
    }

    serde_json::from_slice::<RequestBody>(body)
        .ok()
        .and_then(|r| r.model)
        .unwrap_or_else(|| "unknown".to_string())
}

fn parse_streaming_chunk(bytes: &Bytes) -> Option<OllamaApiResponse> {
    let text = std::str::from_utf8(bytes).ok()?;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(response) = serde_json::from_str::<OllamaApiResponse>(trimmed) {
            return Some(response);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_model() {
        let body = Bytes::from(r#"{"model": "llama2", "prompt": "hello"}"#);
        assert_eq!(extract_model_from_request(&body), "llama2");

        let empty = Bytes::new();
        assert_eq!(extract_model_from_request(&empty), "unknown");
    }

    #[test]
    fn test_parse_streaming_chunk() {
        let chunk = Bytes::from(r#"{"model":"llama2","created_at":"2024-01-01T00:00:00Z","response":"Hi","done":false}"#);
        let parsed = parse_streaming_chunk(&chunk);
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap().model, "llama2");
    }
}
