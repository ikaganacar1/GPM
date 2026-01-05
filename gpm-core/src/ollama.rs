use crate::error::{GpmError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSession {
    pub id: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub tokens_per_second: f64,
    pub time_to_first_token_ms: Option<u64>,
    pub time_per_output_token_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaApiResponse {
    pub model: String,
    pub created_at: String,
    pub response: Option<String>,
    pub done: bool,
    #[serde(default)]
    pub eval_count: Option<u64>,
    #[serde(default)]
    pub eval_duration: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub prompt_eval_duration: Option<u64>,
}

#[derive(Debug, Clone)]
struct SessionTracker {
    session_id: String,
    model: String,
    start_time: chrono::DateTime<chrono::Utc>,
    first_token_time: Option<chrono::DateTime<chrono::Utc>>,
    prompt_tokens: u64,
    completion_tokens: u64,
    prompt_eval_duration_ns: u64,
    eval_duration_ns: u64,
}

pub struct OllamaMonitor {
    client: Client,
    api_url: String,
    active_sessions: Arc<RwLock<HashMap<String, SessionTracker>>>,
    completed_sessions: Arc<RwLock<Vec<LlmSession>>>,
}

impl OllamaMonitor {
    pub fn new(api_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap(),
            api_url,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            completed_sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn is_ollama_running(&self) -> bool {
        match self.client
            .get(format!("{}/api/tags", self.api_url))
            .send()
            .await
        {
            Ok(resp) => {
                let is_ok = resp.status().is_success();
                if is_ok {
                    debug!("Ollama API is reachable");
                } else {
                    debug!("Ollama API returned non-success status: {}", resp.status());
                }
                is_ok
            }
            Err(e) => {
                debug!("Ollama API not reachable: {}", e);
                false
            }
        }
    }

    pub async fn get_running_models(&self) -> Result<Vec<String>> {
        let response = self.client
            .get(format!("{}/api/ps", self.api_url))
            .send()
            .await
            .map_err(|e| GpmError::OllamaError(format!("Failed to query running models: {}", e)))?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        #[derive(Deserialize)]
        struct ProcessList {
            models: Vec<ModelInfo>,
        }

        #[derive(Deserialize)]
        struct ModelInfo {
            name: String,
        }

        let process_list: ProcessList = response
            .json()
            .await
            .map_err(|e| GpmError::OllamaError(format!("Failed to parse response: {}", e)))?;

        Ok(process_list.models.into_iter().map(|m| m.name).collect())
    }

    pub async fn track_generation(
        &self,
        session_id: String,
        model: String,
        response: &OllamaApiResponse,
    ) {
        let now = chrono::Utc::now();
        let mut sessions = self.active_sessions.write().await;

        let tracker = sessions.entry(session_id.clone()).or_insert_with(|| {
            SessionTracker {
                session_id: session_id.clone(),
                model: model.clone(),
                start_time: now,
                first_token_time: None,
                prompt_tokens: 0,
                completion_tokens: 0,
                prompt_eval_duration_ns: 0,
                eval_duration_ns: 0,
            }
        });

        if tracker.first_token_time.is_none() && response.response.is_some() {
            tracker.first_token_time = Some(now);
        }

        if let Some(count) = response.prompt_eval_count {
            tracker.prompt_tokens = count;
        }

        if let Some(count) = response.eval_count {
            tracker.completion_tokens = count;
        }

        if let Some(duration) = response.prompt_eval_duration {
            tracker.prompt_eval_duration_ns = duration;
        }

        if let Some(duration) = response.eval_duration {
            tracker.eval_duration_ns = duration;
        }

        if response.done {
            let tracker_clone = tracker.clone();
            let session = self.finalize_session(&tracker_clone);
            drop(sessions);

            let mut completed = self.completed_sessions.write().await;
            completed.push(session);

            info!(
                "Completed LLM session: model={} tokens={} tps={:.2}",
                model,
                tracker_clone.prompt_tokens + tracker_clone.completion_tokens,
                tracker_clone.completion_tokens as f64 * 1e9 / tracker_clone.eval_duration_ns as f64
            );
        }
    }

    fn finalize_session(&self, tracker: &SessionTracker) -> LlmSession {
        let end_time = chrono::Utc::now();
        let total_tokens = tracker.prompt_tokens + tracker.completion_tokens;

        let tokens_per_second = if tracker.eval_duration_ns > 0 {
            tracker.completion_tokens as f64 * 1e9 / tracker.eval_duration_ns as f64
        } else {
            0.0
        };

        let time_to_first_token_ms = tracker.first_token_time.map(|t| {
            (t - tracker.start_time).num_milliseconds() as u64
        });

        let time_per_output_token_ms = if tracker.completion_tokens > 0 && tracker.eval_duration_ns > 0 {
            Some(tracker.eval_duration_ns as f64 / 1e6 / tracker.completion_tokens as f64)
        } else {
            None
        };

        LlmSession {
            id: tracker.session_id.clone(),
            start_time: tracker.start_time,
            end_time: Some(end_time),
            model: tracker.model.clone(),
            prompt_tokens: tracker.prompt_tokens,
            completion_tokens: tracker.completion_tokens,
            total_tokens,
            tokens_per_second,
            time_to_first_token_ms,
            time_per_output_token_ms,
        }
    }

    pub async fn get_completed_sessions(&self) -> Vec<LlmSession> {
        self.completed_sessions.read().await.clone()
    }

    pub async fn clear_completed_sessions(&self) {
        self.completed_sessions.write().await.clear();
    }

    pub async fn check_and_track_logs(&self) -> Result<()> {
        if !self.is_ollama_running().await {
            return Ok(());
        }

        let models = self.get_running_models().await?;

        if !models.is_empty() {
            debug!("Active Ollama models: {:?}", models);
        }

        Ok(())
    }
}

pub fn parse_ollama_log_line(line: &str) -> Option<OllamaApiResponse> {
    if !line.contains("generate") && !line.contains("chat") {
        return None;
    }

    serde_json::from_str(line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_tracking() {
        let monitor = OllamaMonitor::new("http://localhost:11434".to_string());

        let response1 = OllamaApiResponse {
            model: "llama2".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            response: Some("Hello".to_string()),
            done: false,
            eval_count: Some(1),
            eval_duration: Some(100_000_000),
            prompt_eval_count: Some(10),
            prompt_eval_duration: Some(50_000_000),
        };

        monitor.track_generation(
            "session1".to_string(),
            "llama2".to_string(),
            &response1,
        ).await;

        let response2 = OllamaApiResponse {
            model: "llama2".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            response: Some(" world!".to_string()),
            done: true,
            eval_count: Some(3),
            eval_duration: Some(300_000_000),
            prompt_eval_count: Some(10),
            prompt_eval_duration: Some(50_000_000),
        };

        monitor.track_generation(
            "session1".to_string(),
            "llama2".to_string(),
            &response2,
        ).await;

        let completed = monitor.get_completed_sessions().await;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].model, "llama2");
        assert_eq!(completed[0].completion_tokens, 3);
    }
}
