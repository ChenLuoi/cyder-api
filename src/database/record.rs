use crate::{
    controller::BaseError, db_execute, db_object,
    utils::ID_GENERATOR,
};

use super::{get_connection, DbResult, ListResult};
use chrono::Utc;
use diesel::prelude::{Insertable, Queryable, ExpressionMethods};
use serde::{Deserialize, Serialize};
use diesel::dsl::sql;
use diesel::sql_types::BigInt;

db_object! {
    #[derive(Debug, Serialize, Queryable, Insertable, Selectable)]
    #[diesel(table_name = record)]
    pub struct Record {
        pub id: i64,
        pub api_key_id: i64,
        pub provider_id: i64,
        pub model_id: Option<i64>,
        pub model_name: String,
        pub real_model_name: String,
        pub prompt_tokens: i32,
        pub prompt_cache_tokens: i32,
        pub prompt_audio_tokens: i32,
        pub completion_tokens: i32,
        pub reasoning_tokens: i32,
        pub first_token_time: Option<i32>,
        pub response_time: i32,
        pub is_stream: bool,
        pub request_at: i64,
        pub created_at: i64,
        pub updated_at: i64,
    }
}

pub struct TimeInfo {
    pub start_time: i64,
    pub first_token_time: Option<i32>,
    pub response_time: i32,
}

pub struct UsageInfo {
    pub prompt_tokens: i32,
    pub prompt_cache_tokens: i32,
    pub prompt_audio_tokens: i32,
    pub completion_tokens: i32,
    pub reasoning_tokens: i32,
}

pub struct ModelInfo {
    pub provider_id: i64,
    pub model_id: Option<i64>,
    pub model_name: String,
    pub real_model_name: String,
}

impl Record {
    pub fn new(
        api_key_id: i64,
        model_info: &ModelInfo,
        usage_info: Option<&UsageInfo>,
        time_info: &TimeInfo,
        is_stream: bool,
    ) -> Self {
        let (
            prompt_tokens,
            completion_tokens,
            prompt_cache_tokens,
            prompt_audio_tokens,
            reasoning_tokens,
        ) = match usage_info {
            Some(usage) => (
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.prompt_cache_tokens,
                usage.prompt_audio_tokens,
                usage.reasoning_tokens,
            ),
            None => (0, 0, 0, 0, 0),
        };
        let now = Utc::now().timestamp_millis();

        Self {
            id: ID_GENERATOR.generate_id(),
            api_key_id,
            provider_id: model_info.provider_id,
            model_id: model_info.model_id,
            model_name: model_info.model_name.clone(),
            real_model_name: model_info.real_model_name.clone(),
            prompt_tokens,
            completion_tokens,
            prompt_cache_tokens,
            prompt_audio_tokens,
            reasoning_tokens,
            first_token_time: time_info.first_token_time,
            response_time: time_info.response_time,
            is_stream: is_stream,
            request_at: time_info.start_time,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert_one(record: &Record) -> DbResult<()> {
        let conn = &mut get_connection();

        db_execute!(conn, {
            diesel::insert_into(record::table)
                .values(RecordDb::to_db(&record))
                .execute(conn)
                .map_err(|_| BaseError::DatabaseFatal(None))?;
            Ok(())
        })
    }

    pub fn list(payload: RecordQueryPayload) -> DbResult<ListResult<Record>> {
        let conn = &mut get_connection();
        let page_size = payload.page_size.unwrap_or(10);
        let page = payload.page.unwrap_or(1);
        let offset = (page - 1) * page_size;

        db_execute!(conn, {
            let provider_id_filter = payload.provider_id;
            let model_id_filter = payload.model_id;
            let model_name_filter = payload.model_name;
            let api_key_id_filter = payload.api_key_id;

            let mut query = record::table.into_boxed();

            if let Some(provider_id_filter) = provider_id_filter {
                query = query.filter(record::dsl::provider_id.eq(provider_id_filter));
            }

            if let Some(model_id_filter) = model_id_filter {
                query = query.filter(record::dsl::model_id.eq(model_id_filter));
            }

            if let Some(model_name_filter) = model_name_filter.as_ref() {
                let pattern = format!("%{}%", model_name_filter);
                query = query.filter(record::dsl::model_name.like(pattern));
            }

            if let Some(api_key_id_filter) = api_key_id_filter {
                query = query.filter(record::dsl::api_key_id.eq(api_key_id_filter));
            }

            // Build a separate query for counting
            let mut count_query = record::table.into_boxed();

            if let Some(provider_id_filter) = provider_id_filter {
                count_query = count_query.filter(record::dsl::provider_id.eq(provider_id_filter));
            }

            if let Some(model_id_filter) = model_id_filter {
                count_query = count_query.filter(record::dsl::model_id.eq(model_id_filter));
            }

            if let Some(model_name_filter) = model_name_filter.as_ref() {
                let pattern = format!("%{}%", model_name_filter);
                count_query = count_query.filter(record::dsl::model_name.like(pattern));
            }

            if let Some(api_key_id_filter) = api_key_id_filter {
                count_query = count_query.filter(record::dsl::api_key_id.eq(api_key_id_filter));
            }

            let total = count_query
                .select(diesel::dsl::count(record::dsl::id))
                .first::<i64>(conn)
                .map_err(|_| BaseError::DatabaseFatal(None))?;

            let list = query
                .order(record::dsl::request_at.desc())
                .limit(page_size)
                .offset(offset)
                .load::<RecordDb>(conn)
                .map_err(|_| BaseError::DatabaseFatal(None))?;

            let list = list
                .into_iter()
                .map(|db| db.from_db())
                .collect::<Vec<Record>>();
            Ok(ListResult {
                total,
                list,
                page,
                page_size,
            })
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct RecordQueryPayload {
    pub provider_id: Option<i64>,
    pub model_id: Option<i64>,
    pub model_name: Option<String>,
    pub api_key_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
