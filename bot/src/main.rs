// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

mod audio_common;
mod context_ext;
mod fruityger_common;
mod fruityger_inline_query;
mod fruityger_inline_send;
mod lucida_common;
mod lucida_inline_query;
mod lucida_inline_send;
mod modules;

use clokwerk::AsyncScheduler;
use dashmap::DashMap;
use grammers_client::types::{
    inline::query::{Article, InlineResult},
    media::Uploaded,
};
use mystbot_core::Context;
use serde::Deserialize;
use sqlx::{Pool, Sqlite, SqlitePool};
use std::sync::Arc;
use tokio::{fs, sync::RwLock};

#[macro_export]
macro_rules! return_response {
    ($q:ident, $msg:literal) => {
        $q.answer([InlineResult::from(Article::new($msg, $msg))])
            .send()
            .await?;
        return Ok(());
    };
}

#[macro_export]
macro_rules! return_unit_response {
    ($q:ident, $msg:literal) => {
        let _ = $q
            .answer([InlineResult::from(Article::new($msg, $msg))])
            .send()
            .await;
        return;
    };
}

#[macro_export]
macro_rules! sha1 {
    ($d:expr) => {{
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update($d);
        hex::encode(h.finalize())
    }};
}

type AppState = Arc<RwLock<State>>;
type AppContext = Context<AppState>;

#[derive(Clone)]
struct CachedFile(Uploaded, String);

#[derive(Default)]
struct FruitygerClients {
    yandex: Option<fruityger::yandex::Yandex>,
    hifi: Option<fruityger::hifi::Hifi>,
}

struct State {
    track_db: Pool<Sqlite>,
    file_cache: DashMap<String, CachedFile>,
    lucida_cache: DashMap<String, lucida_api::Track>,
    fruityger_cache: DashMap<String, fruityger::Track>,
    fruityger_clients: FruitygerClients,
}

#[derive(Deserialize)]
struct Config {
    token: String,
    api_id: i32,
    api_hash: String,
    fruityger: FruitygerConfig,
}

#[derive(Deserialize)]
struct FruitygerConfig {
    hifi: Option<fruityger::hifi::Config>,
    yandex: Option<fruityger::yandex::Config>,
}

#[tokio::main]
async fn main() {
    let config: Config = toml::from_slice(
        &fs::read(std::env::var("CONFIG").unwrap_or("config.toml".to_owned()))
            .await
            .unwrap(),
    )
    .unwrap();
    let mut fruityger_clients = FruitygerClients::default();

    if let Some(config) = config.fruityger.hifi {
        fruityger_clients.hifi = Some(fruityger::hifi::Hifi::new(config))
    }

    if let Some(config) = config.fruityger.yandex {
        fruityger_clients.yandex = Some(fruityger::yandex::Yandex::new(config))
    }

    let (_, mut app) = mystbot_core::MystbotCore::connect(
        &config.token,
        config.api_id,
        &config.api_hash,
        Arc::new(RwLock::new(State {
            track_db: SqlitePool::connect("sqlite://track.db")
                .await
                .expect("failed to open track db"),
            file_cache: DashMap::new(),
            lucida_cache: DashMap::new(),
            fruityger_cache: DashMap::new(),
            fruityger_clients,
        })),
    )
    .await
    .expect("client initialization failed");

    app.set_inline_query(|context, query| {
        Box::pin(async move {
            let args: Vec<_> = query.text().split(" ").map(|s| s.to_string()).collect();
            if args.is_empty() {
                return_unit_response!(query, "Введите команду");
            }
            let _ = match args[0].as_str() {
                "lucida" => lucida_inline_query::run(context, query, args[1..].to_vec()).await,
                "music" => fruityger_inline_query::run(context, query, args[1..].to_vec()).await,
                _ => {
                    return_unit_response!(query, "Неизвестная команда");
                }
            };
        })
    });

    app.set_inline_send(|context, send| {
        Box::pin(async move {
            let args: Vec<_> = send.result_id().split("|").map(|s| s.to_string()).collect();
            let _ = match args[0].as_str() {
                "lucida" => lucida_inline_send::run(context, send, args[1..].to_vec()).await,
                "fruityger" => fruityger_inline_send::run(context, send, args[1..].to_vec()).await,
                _ => Ok(()),
            };
        })
    });

    app.add_command("start", |_, message| {
        Box::pin(async move {
            message.reply("Привет!").await.unwrap();
        })
    });

    let mut scheduler = AsyncScheduler::new();
    mystbot_core::run(
        Arc::new(app.register(modules::track::register, &mut scheduler)),
        scheduler,
    )
    .await;
}
