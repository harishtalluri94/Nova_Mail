use crate::jmap::*;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JMAP Session resource
/// Spec: https://jmap.io/spec-core.html#the-jmap-session-resource
pub async fn session(State(_state): State<AppState>) -> impl IntoResponse {
    let session = JmapSession {
        capabilities: Capabilities {
            core: CoreCapability {
                max_size_upload: 50_000_000,        // 50 MB
                max_concurrent_upload: 4,
                max_size_request: 10_000_000,       // 10 MB
                max_concurrent_requests: 4,
                max_calls_in_request: 16,
                max_objects_in_get: 500,
                max_objects_in_set: 500,
                collation_algorithms: vec!["i;ascii-casemap".to_string()],
            },
            mail: MailCapability {
                max_mailboxes_per_email: Some(100),
                max_mailbox_depth: Some(10),
                max_size_mailbox_name: 200,
                max_size_attachments_per_email: 50_000_000,
                email_query_sort_options: vec![
                    "receivedAt".to_string(),
                    "from".to_string(),
                    "to".to_string(),
                    "subject".to_string(),
                ],
                may_create_top_level_mailbox: true,
            },
        },
        accounts: vec![],
        primary_accounts: PrimaryAccounts {
            mail: "default".to_string(),
        },
        username: "user@example.com".to_string(),
        api_url: "/jmap".to_string(),
        download_url: "/jmap/download/{blob_id}".to_string(),
        upload_url: "/jmap/upload/{account_id}".to_string(),
        event_source_url: "/jmap/eventsource".to_string(),
        state: Uuid::new_v4().to_string(),
    };

    Json(session)
}

/// Main JMAP request handler
/// Spec: https://jmap.io/spec-core.html#the-request-object
pub async fn jmap_request(
    State(state): State<AppState>,
    Json(request): Json<JmapRequest>,
) -> Result<impl IntoResponse, JmapError> {
    tracing::debug!(
        "JMAP request with {} method calls",
        request.method_calls.len()
    );

    let mut responses = Vec::new();

    for method_call in request.method_calls {
        let response = match method_call.method.as_str() {
            "Email/query" => handle_email_query(&state, method_call).await?,
            "Email/get" => handle_email_get(&state, method_call).await?,
            "Email/set" => handle_email_set(&state, method_call).await?,
            "Mailbox/get" => handle_mailbox_get(&state, method_call).await?,
            "Mailbox/query" => handle_mailbox_query(&state, method_call).await?,
            "Mailbox/set" => handle_mailbox_set(&state, method_call).await?,
            _ => {
                return Err(JmapError::UnknownMethod(method_call.method.clone()));
            }
        };

        responses.push(response);
    }

    Ok(Json(JmapResponse {
        method_responses: responses,
        session_state: request.using.get("urn:ietf:params:jmap:core").cloned(),
    }))
}

async fn handle_email_query(
    state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    let account_id = call
        .arguments
        .get("accountId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JmapError::InvalidArguments("Missing accountId".to_string()))?;

    let limit = call
        .arguments
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .min(500) as i64;

    let position = call
        .arguments
        .get("position")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as i64;

    // Get filter criteria if provided
    let filter = call.arguments.get("filter");

    // Build query based on filters
    let (emails, total) = if let Some(filter_obj) = filter {
        // Handle filters like inMailbox, text search, etc.
        let mailbox_id = filter_obj.get("inMailbox").and_then(|v| v.as_str());

        if let Some(mailbox_id) = mailbox_id {
            // Query emails in specific mailbox
            let mailbox_uuid = Uuid::parse_str(mailbox_id)
                .map_err(|_| JmapError::InvalidArguments("Invalid mailbox ID".to_string()))?;

            let emails = sqlx::query!(
                r#"
                SELECT m.id
                FROM messages m
                WHERE m.mailbox_id = $1
                ORDER BY m.received_at DESC
                LIMIT $2 OFFSET $3
                "#,
                mailbox_uuid,
                limit,
                position
            )
            .fetch_all(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM messages WHERE mailbox_id = $1",
                mailbox_uuid
            )
            .fetch_one(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?
            .unwrap_or(0);

            (emails, total)
        } else {
            // No specific mailbox filter
            let emails = sqlx::query!(
                r#"
                SELECT m.id
                FROM messages m
                ORDER BY m.received_at DESC
                LIMIT $1 OFFSET $2
                "#,
                limit,
                position
            )
            .fetch_all(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?;

            let total = sqlx::query_scalar!("SELECT COUNT(*) FROM messages")
                .fetch_one(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?
                .unwrap_or(0);

            (emails, total)
        }
    } else {
        // No filter, return all emails
        let emails = sqlx::query!(
            r#"
            SELECT m.id
            FROM messages m
            ORDER BY m.received_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            position
        )
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| JmapError::Database(e.to_string()))?;

        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM messages")
            .fetch_one(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?
            .unwrap_or(0);

        (emails, total)
    };

    let email_ids: Vec<String> = emails.iter().map(|e| e.id.to_string()).collect();

    Ok(MethodResponse {
        method: "Email/query".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": account_id,
            "queryState": Uuid::new_v4().to_string(),
            "canCalculateChanges": false,
            "position": position,
            "ids": email_ids,
            "total": total,
            "limit": limit
        }),
    })
}

async fn handle_email_get(
    state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    let account_id = call
        .arguments
        .get("accountId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JmapError::InvalidArguments("Missing accountId".to_string()))?;

    let ids = call
        .arguments
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| JmapError::InvalidArguments("Missing ids array".to_string()))?;

    let properties = call
        .arguments
        .get("properties")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

    let mut emails = Vec::new();
    let mut not_found = Vec::new();

    for id_val in ids {
        let id_str = match id_val.as_str() {
            Some(s) => s,
            None => {
                not_found.push(id_val.clone());
                continue;
            }
        };

        let id = match Uuid::parse_str(id_str) {
            Ok(uuid) => uuid,
            Err(_) => {
                not_found.push(id_val.clone());
                continue;
            }
        };

        // Fetch email from database
        let email_result = sqlx::query!(
            r#"
            SELECT
                m.id, m.blob_id, m.thread_id, m.mailbox_id,
                m.size, m.received_at, m.subject,
                m.from_addr, m.from_name,
                m.to_addrs, m.has_attachment,
                m.is_seen, m.is_flagged, m.is_draft, m.is_answered
            FROM messages m
            WHERE m.id = $1
            "#,
            id
        )
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| JmapError::Database(e.to_string()))?;

        if let Some(email) = email_result {
            // Build email object based on requested properties
            let mut email_obj = serde_json::json!({
                "id": email.id.to_string(),
                "blobId": email.blob_id,
                "threadId": email.thread_id.map(|t| t.to_string()).unwrap_or_else(|| email.id.to_string()),
                "mailboxIds": {
                    email.mailbox_id.to_string(): true
                },
                "size": email.size.unwrap_or(0),
                "receivedAt": email.received_at.to_rfc3339(),
            });

            // Add keywords based on flags
            let mut keywords = serde_json::Map::new();
            if email.is_seen.unwrap_or(false) {
                keywords.insert("$seen".to_string(), serde_json::json!(true));
            }
            if email.is_flagged.unwrap_or(false) {
                keywords.insert("$flagged".to_string(), serde_json::json!(true));
            }
            if email.is_draft.unwrap_or(false) {
                keywords.insert("$draft".to_string(), serde_json::json!(true));
            }
            if email.is_answered.unwrap_or(false) {
                keywords.insert("$answered".to_string(), serde_json::json!(true));
            }
            email_obj["keywords"] = serde_json::Value::Object(keywords);

            // Add optional fields if requested (or if no properties filter)
            let should_include = |field: &str| {
                properties
                    .as_ref()
                    .map(|props| props.contains(&field.to_string()))
                    .unwrap_or(true)
            };

            if should_include("from") && email.from_addr.is_some() {
                email_obj["from"] = serde_json::json!([{
                    "name": email.from_name,
                    "email": email.from_addr
                }]);
            }

            if should_include("to") && email.to_addrs.is_some() {
                // Parse to_addrs (simplified - assumes comma-separated)
                let to_list: Vec<serde_json::Value> = email
                    .to_addrs
                    .unwrap()
                    .split(',')
                    .map(|addr| {
                        serde_json::json!({
                            "email": addr.trim()
                        })
                    })
                    .collect();
                email_obj["to"] = serde_json::json!(to_list);
            }

            if should_include("subject") {
                email_obj["subject"] = serde_json::json!(email.subject);
            }

            if should_include("hasAttachment") {
                email_obj["hasAttachment"] = serde_json::json!(email.has_attachment.unwrap_or(false));
            }

            // Add preview if requested (requires fetching blob)
            if should_include("preview") {
                // TODO: Fetch from blob storage and extract preview
                // For now, return empty preview
                email_obj["preview"] = serde_json::json!("");
            }

            emails.push(email_obj);
        } else {
            not_found.push(id_val.clone());
        }
    }

    Ok(MethodResponse {
        method: "Email/get".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": account_id,
            "state": Uuid::new_v4().to_string(),
            "list": emails,
            "notFound": not_found
        }),
    })
}

async fn handle_email_set(
    state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    let account_id = call
        .arguments
        .get("accountId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JmapError::InvalidArguments("Missing accountId".to_string()))?;

    let old_state = Uuid::new_v4().to_string();
    let new_state = Uuid::new_v4().to_string();

    let mut created = serde_json::Map::new();
    let mut updated = serde_json::Map::new();
    let mut destroyed = Vec::new();
    let mut not_created = serde_json::Map::new();
    let mut not_updated = serde_json::Map::new();
    let mut not_destroyed = serde_json::Map::new();

    // Handle updates (flag changes, mailbox moves)
    if let Some(update_obj) = call.arguments.get("update").and_then(|v| v.as_object()) {
        for (email_id, changes) in update_obj {
            let id = match Uuid::parse_str(email_id) {
                Ok(uuid) => uuid,
                Err(_) => {
                    not_updated.insert(
                        email_id.clone(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid email ID"
                        }),
                    );
                    continue;
                }
            };

            // Handle keyword changes (flags)
            if let Some(keywords) = changes.get("keywords").and_then(|v| v.as_object()) {
                let is_seen = keywords.contains_key("$seen");
                let is_flagged = keywords.contains_key("$flagged");
                let is_draft = keywords.contains_key("$draft");
                let is_answered = keywords.contains_key("$answered");

                sqlx::query!(
                    r#"
                    UPDATE messages
                    SET is_seen = $1, is_flagged = $2, is_draft = $3, is_answered = $4,
                        updated_at = NOW()
                    WHERE id = $5
                    "#,
                    is_seen,
                    is_flagged,
                    is_draft,
                    is_answered,
                    id
                )
                .execute(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;

                updated.insert(email_id.clone(), serde_json::json!(null));
            }

            // Handle mailbox changes (move)
            if let Some(mailbox_ids) = changes.get("mailboxIds").and_then(|v| v.as_object()) {
                if let Some((new_mailbox_id, _)) = mailbox_ids.iter().next() {
                    let mailbox_uuid = Uuid::parse_str(new_mailbox_id).map_err(|_| {
                        JmapError::InvalidArguments("Invalid mailbox ID".to_string())
                    })?;

                    sqlx::query!(
                        r#"
                        UPDATE messages
                        SET mailbox_id = $1, updated_at = NOW()
                        WHERE id = $2
                        "#,
                        mailbox_uuid,
                        id
                    )
                    .execute(&state.db_pool)
                    .await
                    .map_err(|e| JmapError::Database(e.to_string()))?;

                    updated.insert(email_id.clone(), serde_json::json!(null));
                }
            }
        }
    }

    // Handle destroys (soft delete)
    if let Some(destroy_arr) = call.arguments.get("destroy").and_then(|v| v.as_array()) {
        for id_val in destroy_arr {
            let id_str = match id_val.as_str() {
                Some(s) => s,
                None => {
                    not_destroyed.insert(
                        id_val.to_string(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid email ID"
                        }),
                    );
                    continue;
                }
            };

            let id = match Uuid::parse_str(id_str) {
                Ok(uuid) => uuid,
                Err(_) => {
                    not_destroyed.insert(
                        id_str.to_string(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid email ID"
                        }),
                    );
                    continue;
                }
            };

            // Soft delete: mark as deleted
            let result = sqlx::query!(
                r#"
                UPDATE messages
                SET deleted_at = NOW()
                WHERE id = $1 AND deleted_at IS NULL
                "#,
                id
            )
            .execute(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?;

            if result.rows_affected() > 0 {
                destroyed.push(serde_json::json!(id_str));
            } else {
                not_destroyed.insert(
                    id_str.to_string(),
                    serde_json::json!({
                        "type": "notFound",
                        "description": "Email not found"
                    }),
                );
            }
        }
    }

    let mut result = serde_json::json!({
        "accountId": account_id,
        "oldState": old_state,
        "newState": new_state,
        "created": created,
        "updated": updated,
        "destroyed": destroyed,
    });

    if !not_created.is_empty() {
        result["notCreated"] = serde_json::Value::Object(not_created);
    }
    if !not_updated.is_empty() {
        result["notUpdated"] = serde_json::Value::Object(not_updated);
    }
    if !not_destroyed.is_empty() {
        result["notDestroyed"] = serde_json::Value::Object(not_destroyed);
    }

    Ok(MethodResponse {
        method: "Email/set".to_string(),
        call_id: call.call_id,
        result,
    })
}

async fn handle_mailbox_get(
    state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    let account_id = call
        .arguments
        .get("accountId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JmapError::InvalidArguments("Missing accountId".to_string()))?;

    let ids = call.arguments.get("ids").and_then(|v| v.as_array());

    let mut mailboxes = Vec::new();
    let mut not_found = Vec::new();

    if let Some(ids_arr) = ids {
        // Fetch specific mailboxes by ID
        for id_val in ids_arr {
            let id_str = match id_val.as_str() {
                Some(s) => s,
                None => {
                    not_found.push(id_val.clone());
                    continue;
                }
            };

            let id = match Uuid::parse_str(id_str) {
                Ok(uuid) => uuid,
                Err(_) => {
                    not_found.push(id_val.clone());
                    continue;
                }
            };

            let mailbox_result = sqlx::query!(
                r#"
                SELECT
                    m.id, m.name, m.parent_id, m.role, m.sort_order,
                    m.is_subscribed,
                    COUNT(DISTINCT msg.id) FILTER (WHERE msg.deleted_at IS NULL) as total_emails,
                    COUNT(DISTINCT msg.id) FILTER (WHERE msg.deleted_at IS NULL AND NOT msg.is_seen) as unread_emails
                FROM mailboxes m
                LEFT JOIN messages msg ON msg.mailbox_id = m.id
                WHERE m.id = $1
                GROUP BY m.id
                "#,
                id
            )
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?;

            if let Some(mailbox) = mailbox_result {
                mailboxes.push(serde_json::json!({
                    "id": mailbox.id.to_string(),
                    "name": mailbox.name,
                    "parentId": mailbox.parent_id.map(|p| p.to_string()),
                    "role": mailbox.role,
                    "sortOrder": mailbox.sort_order,
                    "totalEmails": mailbox.total_emails.unwrap_or(0),
                    "unreadEmails": mailbox.unread_emails.unwrap_or(0),
                    "totalThreads": 0, // TODO: Implement thread counting
                    "unreadThreads": 0,
                    "myRights": {
                        "mayReadItems": true,
                        "mayAddItems": true,
                        "mayRemoveItems": true,
                        "maySetSeen": true,
                        "maySetKeywords": true,
                        "mayCreateChild": true,
                        "mayRename": true,
                        "mayDelete": mailbox.role.is_none(), // Can't delete system mailboxes
                        "maySubmit": true
                    },
                    "isSubscribed": mailbox.is_subscribed.unwrap_or(true)
                }));
            } else {
                not_found.push(id_val.clone());
            }
        }
    } else {
        // No IDs specified, return all mailboxes
        let all_mailboxes = sqlx::query!(
            r#"
            SELECT
                m.id, m.name, m.parent_id, m.role, m.sort_order,
                m.is_subscribed,
                COUNT(DISTINCT msg.id) FILTER (WHERE msg.deleted_at IS NULL) as total_emails,
                COUNT(DISTINCT msg.id) FILTER (WHERE msg.deleted_at IS NULL AND NOT msg.is_seen) as unread_emails
            FROM mailboxes m
            LEFT JOIN messages msg ON msg.mailbox_id = m.id
            GROUP BY m.id
            ORDER BY m.sort_order, m.name
            "#
        )
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| JmapError::Database(e.to_string()))?;

        for mailbox in all_mailboxes {
            mailboxes.push(serde_json::json!({
                "id": mailbox.id.to_string(),
                "name": mailbox.name,
                "parentId": mailbox.parent_id.map(|p| p.to_string()),
                "role": mailbox.role,
                "sortOrder": mailbox.sort_order,
                "totalEmails": mailbox.total_emails.unwrap_or(0),
                "unreadEmails": mailbox.unread_emails.unwrap_or(0),
                "totalThreads": 0,
                "unreadThreads": 0,
                "myRights": {
                    "mayReadItems": true,
                    "mayAddItems": true,
                    "mayRemoveItems": true,
                    "maySetSeen": true,
                    "maySetKeywords": true,
                    "mayCreateChild": true,
                    "mayRename": true,
                    "mayDelete": mailbox.role.is_none(),
                    "maySubmit": true
                },
                "isSubscribed": mailbox.is_subscribed.unwrap_or(true)
            }));
        }
    }

    Ok(MethodResponse {
        method: "Mailbox/get".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": account_id,
            "state": Uuid::new_v4().to_string(),
            "list": mailboxes,
            "notFound": not_found
        }),
    })
}

async fn handle_mailbox_query(
    _state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    // TODO: Implement Mailbox/query

    Ok(MethodResponse {
        method: "Mailbox/query".to_string(),
        call_id: call.call_id,
        result: serde_json::json!({
            "accountId": call.arguments.get("accountId"),
            "queryState": Uuid::new_v4().to_string(),
            "canCalculateChanges": false,
            "position": 0,
            "ids": [],
            "total": 0
        }),
    })
}

async fn handle_mailbox_set(
    state: &AppState,
    call: MethodCall,
) -> Result<MethodResponse, JmapError> {
    let account_id = call
        .arguments
        .get("accountId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JmapError::InvalidArguments("Missing accountId".to_string()))?;

    let old_state = Uuid::new_v4().to_string();
    let new_state = Uuid::new_v4().to_string();

    let mut created = serde_json::Map::new();
    let mut updated = serde_json::Map::new();
    let mut destroyed = Vec::new();
    let mut not_created = serde_json::Map::new();
    let mut not_updated = serde_json::Map::new();
    let mut not_destroyed = serde_json::Map::new();

    // Handle creates
    if let Some(create_obj) = call.arguments.get("create").and_then(|v| v.as_object()) {
        for (creation_id, mailbox_data) in create_obj {
            let name = match mailbox_data.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    not_created.insert(
                        creation_id.clone(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Missing name"
                        }),
                    );
                    continue;
                }
            };

            let parent_id = mailbox_data
                .get("parentId")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());

            let role = mailbox_data.get("role").and_then(|v| v.as_str());

            let sort_order = mailbox_data
                .get("sortOrder")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            // Get user_id from account
            // TODO: Get actual user_id from authenticated session
            // For now, use a placeholder
            let user_id = Uuid::new_v4(); // This should come from auth

            let mailbox_id = sqlx::query_scalar!(
                r#"
                INSERT INTO mailboxes (user_id, name, parent_id, role, sort_order, is_subscribed)
                VALUES ($1, $2, $3, $4, $5, true)
                RETURNING id
                "#,
                user_id,
                name,
                parent_id,
                role,
                sort_order
            )
            .fetch_one(&state.db_pool)
            .await
            .map_err(|e| JmapError::Database(e.to_string()))?;

            created.insert(
                creation_id.clone(),
                serde_json::json!({
                    "id": mailbox_id.to_string()
                }),
            );
        }
    }

    // Handle updates
    if let Some(update_obj) = call.arguments.get("update").and_then(|v| v.as_object()) {
        for (mailbox_id, changes) in update_obj {
            let id = match Uuid::parse_str(mailbox_id) {
                Ok(uuid) => uuid,
                Err(_) => {
                    not_updated.insert(
                        mailbox_id.clone(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid mailbox ID"
                        }),
                    );
                    continue;
                }
            };

            // Build dynamic update query based on what fields are provided
            let name = changes.get("name").and_then(|v| v.as_str());
            let parent_id = changes
                .get("parentId")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let sort_order = changes.get("sortOrder").and_then(|v| v.as_i64()).map(|v| v as i32);
            let is_subscribed = changes.get("isSubscribed").and_then(|v| v.as_bool());

            if let Some(name_val) = name {
                sqlx::query!(
                    "UPDATE mailboxes SET name = $1, updated_at = NOW() WHERE id = $2",
                    name_val,
                    id
                )
                .execute(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;
            }

            if parent_id.is_some() || changes.get("parentId").is_some() {
                sqlx::query!(
                    "UPDATE mailboxes SET parent_id = $1, updated_at = NOW() WHERE id = $2",
                    parent_id,
                    id
                )
                .execute(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;
            }

            if let Some(sort_val) = sort_order {
                sqlx::query!(
                    "UPDATE mailboxes SET sort_order = $1, updated_at = NOW() WHERE id = $2",
                    sort_val,
                    id
                )
                .execute(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;
            }

            if let Some(subscribed_val) = is_subscribed {
                sqlx::query!(
                    "UPDATE mailboxes SET is_subscribed = $1, updated_at = NOW() WHERE id = $2",
                    subscribed_val,
                    id
                )
                .execute(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;
            }

            updated.insert(mailbox_id.clone(), serde_json::json!(null));
        }
    }

    // Handle destroys
    if let Some(destroy_arr) = call.arguments.get("destroy").and_then(|v| v.as_array()) {
        for id_val in destroy_arr {
            let id_str = match id_val.as_str() {
                Some(s) => s,
                None => {
                    not_destroyed.insert(
                        id_val.to_string(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid mailbox ID"
                        }),
                    );
                    continue;
                }
            };

            let id = match Uuid::parse_str(id_str) {
                Ok(uuid) => uuid,
                Err(_) => {
                    not_destroyed.insert(
                        id_str.to_string(),
                        serde_json::json!({
                            "type": "invalidProperties",
                            "description": "Invalid mailbox ID"
                        }),
                    );
                    continue;
                }
            };

            // Check if mailbox is a system mailbox (has a role)
            let mailbox = sqlx::query!("SELECT role FROM mailboxes WHERE id = $1", id)
                .fetch_optional(&state.db_pool)
                .await
                .map_err(|e| JmapError::Database(e.to_string()))?;

            if let Some(mb) = mailbox {
                if mb.role.is_some() {
                    not_destroyed.insert(
                        id_str.to_string(),
                        serde_json::json!({
                            "type": "mailboxHasRole",
                            "description": "Cannot delete system mailbox"
                        }),
                    );
                    continue;
                }

                // Delete the mailbox
                sqlx::query!("DELETE FROM mailboxes WHERE id = $1", id)
                    .execute(&state.db_pool)
                    .await
                    .map_err(|e| JmapError::Database(e.to_string()))?;

                destroyed.push(serde_json::json!(id_str));
            } else {
                not_destroyed.insert(
                    id_str.to_string(),
                    serde_json::json!({
                        "type": "notFound",
                        "description": "Mailbox not found"
                    }),
                );
            }
        }
    }

    let mut result = serde_json::json!({
        "accountId": account_id,
        "oldState": old_state,
        "newState": new_state,
        "created": created,
        "updated": updated,
        "destroyed": destroyed,
    });

    if !not_created.is_empty() {
        result["notCreated"] = serde_json::Value::Object(not_created);
    }
    if !not_updated.is_empty() {
        result["notUpdated"] = serde_json::Value::Object(not_updated);
    }
    if !not_destroyed.is_empty() {
        result["notDestroyed"] = serde_json::Value::Object(not_destroyed);
    }

    Ok(MethodResponse {
        method: "Mailbox/set".to_string(),
        call_id: call.call_id,
        result,
    })
}

/// JMAP Upload endpoint
pub async fn upload(
    State(_state): State<AppState>,
    Path(_account_id): Path<String>,
    _body: bytes::Bytes,
) -> Result<impl IntoResponse, JmapError> {
    // TODO: Implement blob upload
    // Store in object storage, return blob ID

    Ok(Json(serde_json::json!({
        "accountId": "default",
        "blobId": Uuid::new_v4().to_string(),
        "type": "application/octet-stream",
        "size": 0
    })))
}

/// JMAP Download endpoint
pub async fn download(
    State(_state): State<AppState>,
    Path(_blob_id): Path<String>,
) -> Result<impl IntoResponse, JmapError> {
    // TODO: Implement blob download
    // Fetch from object storage

    Ok((StatusCode::NOT_FOUND, "Blob not found"))
}

#[derive(Debug, thiserror::Error)]
pub enum JmapError {
    #[error("Unknown method: {0}")]
    UnknownMethod(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for JmapError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_type, description) = match self {
            JmapError::UnknownMethod(method) => (
                StatusCode::BAD_REQUEST,
                "unknownMethod",
                format!("Unknown method: {}", method),
            ),
            JmapError::InvalidArguments(msg) => (
                StatusCode::BAD_REQUEST,
                "invalidArguments",
                msg,
            ),
            JmapError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "serverFail",
                "Internal server error".to_string(),
            ),
            JmapError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "notFound",
                msg,
            ),
        };

        (
            status,
            Json(serde_json::json!({
                "type": error_type,
                "description": description
            })),
        )
            .into_response()
    }
}
