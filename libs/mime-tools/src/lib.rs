use mail_parser::MessageParser;
use thiserror::Error;

pub mod builder;
pub mod parser;
pub mod sanitize;

#[derive(Error, Debug)]
pub enum MimeError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid MIME structure: {0}")]
    InvalidStructure(String),

    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),

    #[error("Size limit exceeded")]
    SizeLimitExceeded,
}

pub type Result<T> = std::result::Result<T, MimeError>;

/// Parse a raw email message
pub fn parse_message(raw: &[u8]) -> Result<mail_parser::Message> {
    MessageParser::default()
        .parse(raw)
        .ok_or_else(|| MimeError::Parse("Failed to parse message".to_string()))
}

/// Extract text content from a message
pub fn extract_text(message: &mail_parser::Message) -> String {
    message
        .text_body(0)
        .map(|b| b.to_string())
        .unwrap_or_default()
}

/// Extract HTML content from a message
pub fn extract_html(message: &mail_parser::Message) -> Option<String> {
    message.html_body(0).map(|b| b.to_string())
}

/// Check if message has attachments
pub fn has_attachments(message: &mail_parser::Message) -> bool {
    message.attachment_count() > 0
}

/// Get total size of message
pub fn message_size(raw: &[u8]) -> usize {
    raw.len()
}
