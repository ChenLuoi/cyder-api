use crate::controller::BaseError;
use crate::database::system_api_key::{SystemApiKey, UpdateSystemApiKeyData}; // Updated import
use crate::database::DbResult;
use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, // Router will be replaced by StateRouter
};
use serde::Deserialize;
use std::sync::Arc;
use crate::service::app_state::{create_state_router, AppState, StateRouter};
use crate::utils::HttpResult;
use chrono::Utc;
use cyder_tools::log::warn;

#[derive(Deserialize)]
struct InsertApiKeyRequest { // Renamed for clarity
    name: String,
    access_control_policy_id: Option<i64>,
    description: Option<String>,
    // is_enabled is handled by SystemApiKey::create (defaults to true)
}

#[derive(Deserialize)]
struct UpdateApiKeyRequest { // Renamed for clarity
    // api_key field is not updatable via UpdateSystemApiKeyData
    name: Option<String>,
    access_control_policy_id: Option<Option<i64>>, // Allow setting to null
    description: Option<Option<String>>,   // Allow setting to null
    is_enabled: Option<bool>,
}

#[derive(Deserialize)]
struct IssueTokenRequest {
    duration: Option<u64>, // in milliseconds
    end_at: Option<u64>,   // timestamp in milliseconds
    channel: Option<String>,
    uid: String,
    scope: Option<String>,
}

async fn insert_one(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<InsertApiKeyRequest>
) -> DbResult<HttpResult<SystemApiKey>> {
    let created_api_key = SystemApiKey::create(
        &payload.name,
        payload.description.as_deref(),
        payload.access_control_policy_id,
    )?;

    if let Err(e) = app_state.system_api_key_store.add(created_api_key.clone()) {
        warn!("Failed to add SystemApiKey id {} to store: {:?}", created_api_key.id, e);
    }

    Ok(HttpResult::new(created_api_key))
}

async fn delete_one(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<i64>
) -> DbResult<HttpResult<()>> {
    SystemApiKey::delete(id)?; // delete returns DbResult<usize>

    if let Err(e) = app_state.system_api_key_store.delete(id) {
        warn!("Failed to delete SystemApiKey id {} from store: {:?}", id, e);
    }

    Ok(HttpResult::new(()))
}

async fn list() -> DbResult<HttpResult<Vec<SystemApiKey>>> {
    let result = SystemApiKey::list_all()?;
    Ok(HttpResult::new(result))
}

async fn update_one(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateApiKeyRequest>,
) -> DbResult<HttpResult<SystemApiKey>> {
    let update_data = UpdateSystemApiKeyData {
        name: payload.name,
        description: payload.description,
        access_control_policy_id: payload.access_control_policy_id,
        is_enabled: payload.is_enabled,
    };
    let updated_api_key = SystemApiKey::update(id, &update_data)?;

    if let Err(e) = app_state.system_api_key_store.update(updated_api_key.clone()) {
        warn!("Failed to update SystemApiKey id {} in store: {:?}", updated_api_key.id, e);
    }

    Ok(HttpResult::new(updated_api_key))
}

async fn refresh_ref(
    State(app_state): State<Arc<AppState>>,
    Path(api_key_id): Path<i64>,
) -> DbResult<HttpResult<SystemApiKey>> {
    let updated_api_key = SystemApiKey::refresh_ref(api_key_id)?;

    if let Err(e) = app_state.system_api_key_store.update(updated_api_key.clone()) {
        warn!("Failed to update SystemApiKey id {} in store after refreshing ref: {:?}", updated_api_key.id, e);
    }

    Ok(HttpResult::new(updated_api_key))
}

async fn issue_token(
    State(app_state): State<Arc<AppState>>,
    Path(api_key_id): Path<i64>,
    Json(payload): Json<IssueTokenRequest>,
) -> DbResult<HttpResult<String>> {
    let api_key = SystemApiKey::get_by_id(api_key_id)?;

    if !api_key.is_enabled {
        return Err(BaseError::Unauthorized(Some(format!(
            "API key {} is disabled",
            api_key_id
        ))));
    }

    let key_ref = match api_key.ref_ {
        Some(ref_str) => ref_str,
        None => {
            let updated_api_key = SystemApiKey::refresh_ref(api_key_id)?;
            if let Err(e) = app_state.system_api_key_store.update(updated_api_key.clone()) {
                warn!("Failed to update SystemApiKey id {} in store after auto-refreshing ref: {:?}", updated_api_key.id, e);
            }
            // refresh_ref should always return a key with a ref.
            updated_api_key.ref_.ok_or_else(|| BaseError::DatabaseFatal(Some("Failed to generate and retrieve ref_".to_string())))?
        }
    };

    let now_ms = Utc::now().timestamp_millis() as u64;
    let exp_ms = if let Some(end_at) = payload.end_at {
        end_at
    } else {
        let duration = payload.duration.unwrap_or(86400 * 1000); // Default to 1 day
        now_ms + duration
    };

    let exp_s = exp_ms / 1000;

    let channel = payload.channel.unwrap_or_else(|| "cyder-api".to_string());

    let token = crate::utils::auth::issue_api_key_jwt(
        payload.uid,
        exp_s,
        channel,
        key_ref,
        payload.scope,
    );

    Ok(HttpResult::new(format!("jwt-{}", token)))
}

pub fn create_api_key_router() -> StateRouter {
    create_state_router().nest(
        "/system_api_key",
        create_state_router()
            .route("/", post(insert_one))
            .route("/{id}", delete(delete_one))
            .route("/{id}", put(update_one))
            .route("/list", get(list))
            .route("/{api_key_id}/issue", post(issue_token))
            .route("/{api_key_id}/refresh_ref", post(refresh_ref)),
    )
}
