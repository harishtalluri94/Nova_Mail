use anyhow::Result;
use mail_parser::{Message, MessageParser, MimeHeaders};

pub struct ExtractedContent {
    pub subject: String,
    pub from_addr: String,
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    pub body_text: String,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentInfo>,
}

pub struct AttachmentInfo {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
}

/// Extract searchable content from raw email bytes
pub fn extract_content(raw_email: &[u8]) -> Result<ExtractedContent> {
    let message = MessageParser::default()
        .parse(raw_email)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse email"))?;

    let subject = message
        .subject()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let from_addr = extract_from_address(&message);
    let to_addrs = extract_to_addresses(&message);
    let cc_addrs = extract_cc_addresses(&message);

    let body_text = message
        .text_body
        .first()
        .map(|b| b.to_string())
        .unwrap_or_default();

    let body_html = message.html_body.first().map(|b| b.to_string());

    let attachments = extract_attachments(&message);

    Ok(ExtractedContent {
        subject,
        from_addr,
        to_addrs,
        cc_addrs,
        body_text,
        body_html,
        attachments,
    })
}

fn extract_from_address(message: &Message) -> String {
    message
        .from()
        .and_then(|from| from.first())
        .and_then(|addr| addr.address())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn extract_to_addresses(message: &Message) -> Vec<String> {
    message
        .to()
        .map(|to| {
            to.iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_cc_addresses(message: &Message) -> Vec<String> {
    message
        .cc()
        .map(|cc| {
            cc.iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_attachments(message: &Message) -> Vec<AttachmentInfo> {
    let mut attachments = Vec::new();

    for i in 0..message.attachment_count() {
        if let Some(attachment) = message.attachment(i) {
            let filename = attachment
                .attachment_name()
                .unwrap_or("unknown")
                .to_string();

            let content_type = attachment
                .content_type()
                .map(|ct| ct.ctype())
                .unwrap_or("application/octet-stream")
                .to_string();

            let size = attachment.contents().len();

            attachments.push(AttachmentInfo {
                filename,
                content_type,
                size,
            });
        }
    }

    attachments
}

/// Extract text from PDF (stub - would use pdf-extract or similar)
pub fn extract_pdf_text(_pdf_bytes: &[u8]) -> Result<String> {
    // TODO: Implement PDF text extraction
    // Use pdf-extract, pdfium, or similar library
    Ok(String::new())
}

/// Extract text from Office documents (stub)
pub fn extract_office_text(_doc_bytes: &[u8], _content_type: &str) -> Result<String> {
    // TODO: Implement Office document text extraction
    // Use docx, xlsxwriter, or similar libraries
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_email() {
        let raw_email = b"From: sender@example.com\r
To: recipient@example.com\r
Subject: Test Email\r
\r
This is the body of the email.\r
";

        let content = extract_content(raw_email).unwrap();
        assert_eq!(content.subject, "Test Email");
        assert_eq!(content.from_addr, "sender@example.com");
        assert_eq!(content.to_addrs, vec!["recipient@example.com"]);
        assert!(content.body_text.contains("This is the body"));
    }
}
