use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ComposeRequest {
    pub prompt: String,
    #[serde(default)]
    pub tone: Tone,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Professional,
    Casual,
    Friendly,
    Formal,
}

impl Default for Tone {
    fn default() -> Self {
        Tone::Professional
    }
}

#[derive(Debug, Serialize)]
pub struct ComposeResponse {
    pub content: String,
    pub confidence: f32,
}

#[derive(Debug, Deserialize)]
pub struct RewriteRequest {
    pub content: String,
    pub tone: Tone,
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RewriteResponse {
    pub rewritten: String,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    pub thread: Vec<EmailMessage>,
    #[serde(default = "default_max_words")]
    pub max_words: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EmailMessage {
    pub from: String,
    pub subject: String,
    pub body: String,
    pub timestamp: String,
}

fn default_max_words() -> usize {
    100
}

#[derive(Debug, Serialize)]
pub struct SummarizeResponse {
    pub summary: String,
    pub key_points: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractActionsRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ExtractActionsResponse {
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize)]
pub struct Action {
    pub action_type: String,
    pub description: String,
    pub due_date: Option<String>,
    pub priority: String,
}

/// Compose a new email from a prompt
pub async fn compose(
    State(state): State<AppState>,
    Json(request): Json<ComposeRequest>,
) -> Result<impl IntoResponse, AssistantError> {
    tracing::info!("Compose request: tone={:?}", request.tone);

    // Build prompt
    let system_prompt = build_compose_system_prompt(&request.tone);
    let user_prompt = if let Some(context) = &request.context {
        format!("Context: {}\n\nInstructions: {}", context, request.prompt)
    } else {
        request.prompt.clone()
    };

    // Generate response
    let content = state
        .model
        .generate(&system_prompt, &user_prompt, 512)
        .await?;

    Ok(Json(ComposeResponse {
        content,
        confidence: 0.85, // TODO: Calculate actual confidence
    }))
}

/// Rewrite existing text in a different tone
pub async fn rewrite(
    State(state): State<AppState>,
    Json(request): Json<RewriteRequest>,
) -> Result<impl IntoResponse, AssistantError> {
    tracing::info!("Rewrite request: tone={:?}", request.tone);

    let system_prompt = build_rewrite_system_prompt(&request.tone);
    let user_prompt = if let Some(instructions) = &request.instructions {
        format!(
            "Original text:\n{}\n\nAdditional instructions: {}",
            request.content, instructions
        )
    } else {
        format!("Original text:\n{}", request.content)
    };

    let rewritten = state
        .model
        .generate(&system_prompt, &user_prompt, 512)
        .await?;

    Ok(Json(RewriteResponse { rewritten }))
}

/// Summarize an email thread
pub async fn summarize(
    State(state): State<AppState>,
    Json(request): Json<SummarizeRequest>,
) -> Result<impl IntoResponse, AssistantError> {
    tracing::info!("Summarize request: {} messages", request.thread.len());

    // Build thread context
    let mut thread_text = String::new();
    for (i, msg) in request.thread.iter().enumerate() {
        thread_text.push_str(&format!(
            "Message {}: From {} at {}\nSubject: {}\n{}\n\n",
            i + 1,
            msg.from,
            msg.timestamp,
            msg.subject,
            msg.body
        ));
    }

    let system_prompt = format!(
        "You are an expert at summarizing email threads. \
         Provide a concise summary in {} words or less, \
         followed by 3-5 key points.",
        request.max_words
    );

    let user_prompt = format!("Please summarize this email thread:\n\n{}", thread_text);

    let response = state
        .model
        .generate(&system_prompt, &user_prompt, 256)
        .await?;

    // Parse response (summary + key points)
    // TODO: Improve parsing logic
    let lines: Vec<&str> = response.lines().collect();
    let summary = lines.first().unwrap_or(&"").to_string();
    let key_points = lines
        .iter()
        .skip(1)
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    Ok(Json(SummarizeResponse {
        summary,
        key_points,
    }))
}

/// Extract action items from email content
pub async fn extract_actions(
    State(state): State<AppState>,
    Json(request): Json<ExtractActionsRequest>,
) -> Result<impl IntoResponse, AssistantError> {
    tracing::info!("Extract actions request");

    let system_prompt = "You are an expert at extracting action items from emails. \
                         Identify tasks, deadlines, and priorities. \
                         Return results as JSON array with fields: action_type, description, due_date, priority.";

    let user_prompt = format!(
        "Extract action items from this email:\n\n{}",
        request.content
    );

    let response = state
        .model
        .generate(system_prompt, &user_prompt, 256)
        .await?;

    // TODO: Parse JSON response properly
    // For now, return mock actions
    let actions = vec![
        Action {
            action_type: "task".to_string(),
            description: "Example action extracted from email".to_string(),
            due_date: None,
            priority: "medium".to_string(),
        },
    ];

    Ok(Json(ExtractActionsResponse { actions }))
}

fn build_compose_system_prompt(tone: &Tone) -> String {
    let tone_desc = match tone {
        Tone::Professional => "professional and concise",
        Tone::Casual => "casual and friendly",
        Tone::Friendly => "warm and approachable",
        Tone::Formal => "formal and respectful",
    };

    format!(
        "You are an expert email writer. Compose a {} email based on the user's instructions. \
         Write only the email body, no subject line or signatures.",
        tone_desc
    )
}

fn build_rewrite_system_prompt(tone: &Tone) -> String {
    let tone_desc = match tone {
        Tone::Professional => "professional and concise",
        Tone::Casual => "casual and friendly",
        Tone::Friendly => "warm and approachable",
        Tone::Formal => "formal and respectful",
    };

    format!(
        "You are an expert editor. Rewrite the following text in a {} tone \
         while preserving the original meaning.",
        tone_desc
    )
}

#[derive(Debug, thiserror::Error)]
pub enum AssistantError {
    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

impl IntoResponse for AssistantError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AssistantError::ModelError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AI model error".to_string(),
            ),
            AssistantError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (
            status,
            Json(serde_json::json!({
                "error": message
            })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for AssistantError {
    fn from(err: anyhow::Error) -> Self {
        AssistantError::ModelError(err.to_string())
    }
}
