use crate::{quota, storage, AppState};
use anyhow::{Context, Result};
use nova_mime_tools::parse_message;
use redis::AsyncCommands;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

pub async fn run_server(addr: SocketAddr, state: AppState) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("LMTP server bound to {}", addr);

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, peer_addr, state).await {
                tracing::error!("Error handling connection from {}: {}", peer_addr, e);
            }
        });
    }
}

async fn handle_connection(socket: TcpStream, peer_addr: SocketAddr, state: AppState) -> Result<()> {
    tracing::info!("New LMTP connection from {}", peer_addr);

    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Send greeting
    writer
        .write_all(b"220 Nova Mail LMTP Service Ready\r\n")
        .await?;

    let mut session = LmtpSession::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            tracing::debug!("Client closed connection");
            break;
        }

        let response = handle_command(&line.trim(), &mut session, &state).await?;
        writer.write_all(response.as_bytes()).await?;

        if session.quit {
            break;
        }
    }

    tracing::info!("LMTP connection from {} closed", peer_addr);
    Ok(())
}

#[derive(Debug)]
struct LmtpSession {
    mail_from: Option<String>,
    rcpt_to: Vec<String>,
    data_buffer: Vec<u8>,
    in_data: bool,
    quit: bool,
}

impl LmtpSession {
    fn new() -> Self {
        Self {
            mail_from: None,
            rcpt_to: Vec::new(),
            data_buffer: Vec::new(),
            in_data: false,
            quit: false,
        }
    }

    fn reset(&mut self) {
        self.mail_from = None;
        self.rcpt_to.clear();
        self.data_buffer.clear();
        self.in_data = false;
    }
}

async fn handle_command(command: &str, session: &mut LmtpSession, state: &AppState) -> Result<String> {
    if session.in_data {
        return handle_data_line(command, session, state).await;
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Ok("500 Error: bad syntax\r\n".to_string());
    }

    let cmd = parts[0].to_uppercase();

    match cmd.as_str() {
        "LHLO" => {
            Ok(format!(
                "250-nova-mail.local\r\n\
                 250-PIPELINING\r\n\
                 250-ENHANCEDSTATUSCODES\r\n\
                 250-8BITMIME\r\n\
                 250 SIZE 104857600\r\n"
            ))
        }
        "MAIL" => handle_mail_from(command, session).await,
        "RCPT" => handle_rcpt_to(command, session, state).await,
        "DATA" => handle_data_start(session).await,
        "RSET" => {
            session.reset();
            Ok("250 2.0.0 OK\r\n".to_string())
        }
        "QUIT" => {
            session.quit = true;
            Ok("221 2.0.0 Bye\r\n".to_string())
        }
        "NOOP" => Ok("250 2.0.0 OK\r\n".to_string()),
        _ => Ok(format!("500 5.5.1 Command not recognized: {}\r\n", cmd)),
    }
}

async fn handle_mail_from(command: &str, session: &mut LmtpSession) -> Result<String> {
    // Parse MAIL FROM:<address>
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok("501 5.5.4 Syntax error in parameters\r\n".to_string());
    }

    let from_part = parts[1];
    let from_addr = extract_email_address(from_part)?;

    session.mail_from = Some(from_addr);
    Ok("250 2.1.0 OK\r\n".to_string())
}

async fn handle_rcpt_to(command: &str, session: &mut LmtpSession, state: &AppState) -> Result<String> {
    // Parse RCPT TO:<address>
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok("501 5.5.4 Syntax error in parameters\r\n".to_string());
    }

    let to_part = parts[1];
    let to_addr = extract_email_address(to_part)?;

    // Verify recipient exists and check quota
    match verify_recipient(&to_addr, state).await {
        Ok(user_id) => {
            match quota::check_quota(user_id, state).await {
                Ok(true) => {
                    session.rcpt_to.push(to_addr);
                    Ok("250 2.1.5 OK\r\n".to_string())
                }
                Ok(false) => Ok("452 4.2.2 Mailbox full\r\n".to_string()),
                Err(e) => {
                    tracing::error!("Quota check error: {}", e);
                    Ok("451 4.3.0 Temporary server error\r\n".to_string())
                }
            }
        }
        Err(_) => Ok("550 5.1.1 User unknown\r\n".to_string()),
    }
}

async fn handle_data_start(session: &mut LmtpSession) -> Result<String> {
    if session.mail_from.is_none() || session.rcpt_to.is_empty() {
        return Ok("503 5.5.1 Bad sequence of commands\r\n".to_string());
    }

    session.in_data = true;
    session.data_buffer.clear();
    Ok("354 Start mail input; end with <CRLF>.<CRLF>\r\n".to_string())
}

async fn handle_data_line(line: &str, session: &mut LmtpSession, state: &AppState) -> Result<String> {
    // Check for end of data marker
    if line == "." {
        session.in_data = false;
        return process_message(session, state).await;
    }

    // Add line to buffer
    session.data_buffer.extend_from_slice(line.as_bytes());
    session.data_buffer.extend_from_slice(b"\r\n");

    Ok(String::new()) // No response during data collection
}

async fn process_message(session: &mut LmtpSession, state: &AppState) -> Result<String> {
    tracing::info!(
        "Processing message from {} to {} recipients",
        session.mail_from.as_ref().unwrap(),
        session.rcpt_to.len()
    );

    // Parse message
    let message = match parse_message(&session.data_buffer) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!("Failed to parse message: {}", e);
            return Ok("554 5.6.0 Message parsing failed\r\n".to_string());
        }
    };

    // Store blob
    let blob_id = match state.storage.store_blob(&session.data_buffer).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to store blob: {}", e);
            return Ok("451 4.3.0 Temporary storage error\r\n".to_string());
        }
    };

    // Process for each recipient
    let mut responses = Vec::new();

    for recipient in &session.rcpt_to {
        match deliver_to_recipient(recipient, &blob_id, &message, session, state).await {
            Ok(_) => {
                responses.push(format!("250 2.1.5 OK: queued as {}\r\n", blob_id));
            }
            Err(e) => {
                tracing::error!("Delivery error for {}: {}", recipient, e);
                responses.push("451 4.3.0 Temporary failure\r\n".to_string());
            }
        }
    }

    session.reset();
    Ok(responses.join(""))
}

async fn deliver_to_recipient(
    recipient: &str,
    blob_id: &str,
    message: &mail_parser::Message,
    session: &LmtpSession,
    state: &AppState,
) -> Result<()> {
    // Get user info
    let user = sqlx::query!(
        r#"
        SELECT id, tenant_id FROM users WHERE email = $1 AND is_active = true
        "#,
        recipient
    )
    .fetch_one(&state.db_pool)
    .await?;

    // Get inbox mailbox
    let mailbox = sqlx::query!(
        r#"
        SELECT id FROM mailboxes WHERE user_id = $1 AND role = 'inbox'
        "#,
        user.id
    )
    .fetch_one(&state.db_pool)
    .await?;

    let message_id = Uuid::new_v4();

    // Extract message metadata
    let subject = message.subject().unwrap_or("").to_string();
    let from_addr = session.mail_from.clone().unwrap_or_default();
    let received_at = chrono::Utc::now();
    let size_bytes = session.data_buffer.len() as i64;

    // Insert message metadata
    sqlx::query!(
        r#"
        INSERT INTO messages (
            id, tenant_id, user_id, mailbox_id, subject, from_addr,
            blob_id, size_bytes, received_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
        message_id,
        user.tenant_id,
        user.id,
        mailbox.id,
        subject,
        from_addr,
        blob_id,
        size_bytes,
        received_at
    )
    .execute(&state.db_pool)
    .await?;

    // Enqueue indexing job
    let index_job = serde_json::json!({
        "message_id": message_id,
        "tenant_id": user.tenant_id,
        "user_id": user.id,
        "blob_id": blob_id
    });

    let mut redis = state.redis.clone();
    redis
        .lpush::<_, _, ()>("nova:index:queue", index_job.to_string())
        .await?;

    // Enqueue delivery job
    let delivery_job = serde_json::json!({
        "message_id": message_id,
        "user_id": user.id,
        "tenant_id": user.tenant_id,
        "mailbox_id": mailbox.id,
        "blob_id": blob_id
    });

    redis
        .lpush::<_, _, ()>("nova:delivery:queue", delivery_job.to_string())
        .await?;

    tracing::info!("Delivered message {} to {}", message_id, recipient);

    Ok(())
}

async fn verify_recipient(email: &str, state: &AppState) -> Result<Uuid> {
    let user = sqlx::query!(
        r#"
        SELECT id FROM users WHERE email = $1 AND is_active = true
        "#,
        email
    )
    .fetch_one(&state.db_pool)
    .await
    .context("User not found")?;

    Ok(user.id)
}

fn extract_email_address(input: &str) -> Result<String> {
    // Extract email from formats like: <user@example.com> or user@example.com
    let cleaned = input.trim().trim_start_matches("FROM:").trim_start_matches("TO:");
    let addr = cleaned.trim_matches(&['<', '>'][..]);
    Ok(addr.to_string())
}
