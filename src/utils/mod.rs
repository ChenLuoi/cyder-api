use axum::{
    body::Bytes,
    response::{IntoResponse, Response},
    Json,
};
use cyder_tools::snow_flake::Snowflake;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::{json, Value};

pub mod auth;

#[derive(Debug, Serialize)]
pub struct HttpResult<T> {
    pub code: usize,
    pub data: T,
}

impl<T> HttpResult<T> {
    pub fn new(data: T) -> HttpResult<T> {
        HttpResult { code: 0, data }
    }
}

impl<T> IntoResponse for HttpResult<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

pub fn process_stream_options(data: &mut Value) {
    let is_stream = if let Some(stream) = data.get("stream") {
        stream.is_boolean() && stream.as_bool().unwrap_or(false) == true
    } else {
        false
    };
    if is_stream {
        if let Some(stream_options) = data.get_mut("stream_options") {
            if let Some(include_usage) = stream_options.get_mut("include_usage") {
                *include_usage = Value::Bool(true);
            } else {
                stream_options["include_usage"] = Value::Bool(true);
            }
        } else {
            data["stream_options"] = json!({ "include_usage": true });
        }
    }
}

pub fn remove_data_field(data: &mut Value, omit_fields: Option<&Vec<String>>) {
    if let Some(omit_fields) = omit_fields {
        if let Some(obj) = data.as_object_mut() {
            for field_key in omit_fields {
                obj.remove(field_key);
            }
        }
    }
}

pub fn split_chunks(input: Bytes) -> Vec<Bytes> {
    let mut result: Vec<Bytes> = Vec::new();
    let mut start = 0;

    while let Some(pos) = input[start..].iter().position(|&b| b == b'\n') {
        let end = start + pos;
        if start != end {
            result.push(input.slice(start..end));
        }
        start = end + 1; // Move past the newline character
    }

    // Add the last segment if there's any data left
    if start < input.len() {
        result.push(input.slice(start..));
    }

    result
}

pub static ID_GENERATOR: Lazy<Snowflake> = Lazy::new(|| Snowflake::new(1));
