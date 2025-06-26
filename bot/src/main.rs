// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

mod cached_file;
mod fruityger_common;
mod fruityger_inline_query;
mod fruityger_inline_send;
mod lucida_common;
mod lucida_inline_query;
mod lucida_inline_send;

use clokwerk::{AsyncScheduler, Interval};
use dashmap::DashMap;
use fruityger::{qobuz::Qobuz, yandex::Yandex};
use futures::FutureExt;
use grammers_client::{
    InputMessage, InvocationError, button, reply_markup,
    types::{
        Attribute, Chat, Message, PackedChat,
        inline::query::{Article, InlineResult},
    },
};
use lucida_api::{LucidaClient, LucidaService, Track};
use mystbot_core::{client_wrapper::ClientWrapper, error::Error};
use sqlx::{Pool, Sqlite, SqlitePool, prelude::FromRow};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{RwLock, mpsc},
    task,
};
use track24::{TrackResponse, TrackResponseInner};

use crate::cached_file::CachedFile;

#[macro_export]
macro_rules! sha1 {
    ($d:expr) => {{
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update($d);
        hex::encode(h.finalize())
    }};
}

fn boxed_message<S: AsRef<str>>(text: S) -> Error {
    Error::WithMessage(Box::new(InputMessage::text(text)))
}

type AppState = Arc<RwLock<State>>;

struct State {
    cache_dir: PathBuf,
    track_db: Pool<Sqlite>,
    lucida_cache: DashMap<String, Track>,
    fruityger_client: fruityger::Client,
    fruityger_cache: DashMap<String, fruityger::Track>,
    messages_processed: usize,
}

#[derive(Debug, FromRow)]
struct TrackEntry {
    packed: Vec<u8>,
    track_number: String,
}

async fn track_once(client: ClientWrapper, state: AppState, track_number: &str, chat: PackedChat) {
    let mut t24client = track24::Client::new();
    let Ok(response) = t24client.track(track_number).await else {
        log::error!("Track24 API returned an error");
        return;
    };

    let Ok(last_state) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_state FROM track_numbers WHERE chat_id = ? AND track_number = ?",
    )
    .bind(chat.id)
    .bind(track_number)
    .fetch_one(&state.read().await.track_db)
    .await
    else {
        log::error!("Database returned an error");
        return;
    };
    let last_state = if let Some(last_state) = last_state {
        let Ok(last_state) = serde_json::from_str::<track24::TrackResponse>(&last_state) else {
            log::error!("Serde returned an error");
            return;
        };
        last_state
    } else {
        TrackResponse {
            data: TrackResponseInner { events: vec![] },
        }
    };

    let last_ids: Vec<_> = last_state
        .data
        .events
        .iter()
        .map(|v| v.id.clone())
        .collect();
    let diff: Vec<_> = response
        .data
        .events
        .iter()
        .filter(|v| !last_ids.contains(&v.id))
        .collect();

    if diff.is_empty() {
        return;
    }

    let events = diff
        .into_iter()
        .map(|v| {
            let mut place = "".to_string();
            if !v.operation_place_name.is_empty() {
                place = format!(" ({})", v.operation_place_name);
            }
            format!(
                "• {} {} - {}{}",
                v.operation_date_time, v.service_name, v.operation_attribute, place
            )
        })
        .collect::<Vec<_>>()
        .join("  \n");

    let Ok(data) = serde_json::to_string(&response) else {
        log::error!("Serde returned an error");
        return;
    };

    let Ok(_) = sqlx::query(
        "UPDATE track_numbers SET last_state = ? WHERE chat_id = ? AND track_number = ?",
    )
    .bind(data)
    .bind(chat.id)
    .bind(track_number)
    .execute(&state.read().await.track_db)
    .await
    else {
        log::error!("Cannot update database");
        return;
    };

    let Ok(_) = client
        .send_message(
            chat,
            InputMessage::markdown(format!(
                "**🔄 Новые обновления для трек номера {track_number}:**  \n{events}"
            )),
        )
        .await
    else {
        log::error!("Telegram returned an error");
        return;
    };
}

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let mut fruityger_client = fruityger::Client::new();
    fruityger_client.add_module(Yandex::new(
        std::env::var("YANDEX_MUSIC_TOKEN").expect("yandex music token not found"),
    ));
    fruityger_client.add_module(Qobuz::new(
        std::env::var("QOBUZ_TOKEN").expect("qobuz token not found"),
        std::env::var("QOBUZ_APP_ID").expect("qobuz app_id not found"),
        std::env::var("QOBUZ_APP_SECRET").expect("qobuz app_secret not found"),
    ));

    let cache_dir = PathBuf::from(std::env::var("CACHE_DIR").expect("CACHE_DIR not present"));
    if !cache_dir.exists() {
        panic!("CACHE_DIR doesnt exist");
    }

    let (client, mut app) = mystbot_core::MystbotCore::connect(
        &std::env::var("BOT_TOKEN").expect("bot token not found"),
        Arc::new(RwLock::new(State {
            cache_dir,
            track_db: SqlitePool::connect("sqlite://track.db")
                .await
                .expect("failed to open track db"),
            lucida_cache: DashMap::new(),
            fruityger_client,
            fruityger_cache: DashMap::new(),
            messages_processed: 0,
        })),
    )
    .await
    .expect("client initialization failed");

    app.add_after_command_hook(|_, _, state| {
        Box::pin(async move {
            state.write().await.messages_processed += 1;
            Ok(())
        })
    });

    app.add_inline_query(|client, query, state| {
        async move {
            let args: Vec<_> = query.text().split(" ").map(|s| s.to_string()).collect();
            if args.is_empty() {
                query
                    .answer([InlineResult::from(Article::new(
                        "Введите команду",
                        "Введите команду",
                    ))])
                    .send()
                    .await?;
                return Ok(());
            }
            match args[0].as_str() {
                "lucida" => {
                    lucida_inline_query::run(client, query, state, args[1..].to_vec()).await?
                }
                "music" => {
                    fruityger_inline_query::run(client, query, state, args[1..].to_vec()).await?
                }
                _ => {
                    query
                        .answer([InlineResult::from(Article::new(
                            "Неизвестная команда",
                            "Неизвестная команда",
                        ))])
                        .send()
                        .await?;
                }
            }
            Ok(())
        }
        .boxed()
    });

    app.add_inline_send(|client, send, state| {
        Box::pin(async move {
            let args: Vec<_> = send.result_id().split("|").map(|s| s.to_string()).collect();
            match args[0].as_str() {
                "lucida" => {
                    lucida_inline_send::run(client, send, state, args[1..].to_vec()).await?
                }
                "fruityger" => {
                    fruityger_inline_send::run(client, send, state, args[1..].to_vec()).await?
                }
                _ => {}
            }
            Ok(())
        })
    });

    app.add_callback_query(|client, query, state| {
        Box::pin(async move {
            let data = String::from_utf8(query.data().to_vec()).unwrap();
            let args: Vec<&str> = data.split("|").collect();
            match args[0] {
                "lucida" => {
                    if query.sender().id() != args[2].parse::<i64>().unwrap() {
                        query
                            .answer()
                            .text("Вы не можете взаимодействовать с чужим сообщением")
                            .send()
                            .await?;
                        return Ok(());
                    }

                    let Some(track) = state
                        .read()
                        .await
                        .lucida_cache
                        .view(args[1], |_, v| v.clone())
                    else {
                        query.answer().text("Устаревшее сообщение").send().await?;
                        return Ok(());
                    };

                    let query_message = query.load_message().await?;
                    let chat = query_message.chat();
                    let reply_message = query_message.get_reply().await?;
                    query.answer().send().await?;
                    query_message.delete().await?;

                    let status_message = client
                        .send_message(
                            &chat,
                            InputMessage::text("Скачиваем...")
                                .reply_to(reply_message.as_ref().map(|m| m.id())),
                        )
                        .await?;

                    let (tx, mut rx) = mpsc::channel(1);
                    {
                        let status_message = status_message.clone();
                        task::spawn(async move {
                            while let Some(m) = rx.recv().await {
                                status_message.edit(m).await.unwrap();
                            }
                        });
                    }

                    let send = {
                        |audio: CachedFile,
                         artwork: Option<CachedFile>,
                         chat: Chat,
                         track: Track,
                         client: ClientWrapper,
                         status_message: Message| async move {
                            let mut message = InputMessage::text("")
                                .document(audio.0)
                                .attribute(Attribute::Audio {
                                    duration: Duration::from_millis(track.duration_ms as u64),
                                    title: Some(track.title),
                                    performer: Some(track.artists[0].name.clone()),
                                })
                                .mime_type(&audio.1);
                            if let Some(artwork) = artwork {
                                message = message.thumbnail(artwork.0);
                            }
                            client.send_message(chat, message).await?;
                            status_message.delete().await?;
                            Ok::<bool, InvocationError>(true)
                        }
                    };

                    let mut sent = false;
                    for i in 0..2 {
                        if let Some((audio, artwork)) = lucida_common::download_track(
                            client.clone(),
                            state.clone(),
                            track.clone(),
                            tx.clone(),
                            i == 1,
                        )
                        .await
                        {
                            if send(
                                audio,
                                artwork,
                                chat.clone(),
                                track.clone(),
                                client.clone(),
                                status_message.clone(),
                            )
                            .await
                            .unwrap_or_default()
                            {
                                status_message.delete().await?;
                                sent = true;
                                break;
                            }
                        };
                    }

                    if !sent {
                        status_message.edit("Не удалось скачать трек").await?;
                    }
                }
                _ => query.answer().text("Неизвестный запрос").send().await?,
            };
            Ok(())
        })
    });

    app.add_command("start", |_, message, _| {
        Box::pin(async move {
            message.reply("Привет!").await?;
            Ok(())
        })
    });

    app.add_command("track", |client, message, state| Box::pin(async move {
        let args: Vec<&str> = message.text().split(" ").skip(1).collect();
        if args.is_empty() {
            return Err(Error::WithMessage(Box::new("Использование: /track (номер трек кода)".into())));
        }

        let entries: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_numbers WHERE chat_id = ? AND track_number = ?",
        )
        .bind(message.chat().id())
        .bind(args[0])
        .fetch_one(&state.read().await.track_db)
        .await
        .map_err(|e| {
            log::error!("{e}");
            boxed_message("Не удалось получить информацию о трек кодах")
        })?;

        if entries > 0 {
            return Err(boxed_message("Данный трек код уже добавлен"));
        }

        sqlx::query(
            "INSERT INTO track_numbers (chat_id, packed, track_number, last_state) VALUES (?, ?, ?, NULL)",
        )
        .bind(message.chat().id())
        .bind(&message.chat().pack().to_bytes()[..])
        .bind(args[0])
        .fetch_all(&state.read().await.track_db)
        .await
        .map_err(|e| {
            log::error!("{e}");
            boxed_message("Не удалось добавить трек код")
        })?;

        message.reply("Трек код был успешно добавлен").await?;

        track_once(client, state, args[0], message.chat().pack()).await;

        Ok(())
    }));

    app.add_command("untrack", |_, message, state| {
        Box::pin(async move {
            let args: Vec<&str> = message.text().split(" ").skip(1).collect();
            if args.is_empty() {
                return Err(boxed_message("Использование: /untrack (номер трек кода)"));
            }

            sqlx::query("DELETE FROM track_numbers WHERE chat_id = ? AND track_number = ?")
                .bind(message.chat().id())
                .bind(args[0])
                .execute(&state.read().await.track_db)
                .await
                .map_err(|e| {
                    log::error!("{e}");
                    boxed_message("Не удалось удалить трек код")
                })?;

            message.reply("Трек код был успешно удалён").await?;

            Ok(())
        })
    });

    app.add_command("tracklist", |_, message, state| {
        Box::pin(async move {
            let entries: Vec<TrackEntry> =
                sqlx::query_as("SELECT * FROM track_numbers WHERE chat_id = ?")
                    .bind(message.chat().id())
                    .fetch_all(&state.read().await.track_db)
                    .await
                    .map_err(|e| {
                        log::error!("{e}");
                        Error::WithMessage(Box::new(InputMessage::text(
                            "Не удалось получить информацию о трек кодах",
                        )))
                    })?;

            let track_numbers = entries
                .into_iter()
                .map(|v| format!("• `{}`  ", v.track_number))
                .collect::<Vec<_>>()
                .join("\n");

            message
                .reply(InputMessage::markdown(format!(
                    "**Список активных трек кодов:**  \n{track_numbers}"
                )))
                .await?;

            Ok(())
        })
    });

    app.add_command("ip", |_, message, _| {
        Box::pin(async move {
            if message.sender().unwrap().username() != Some("myst33d") {
                return Ok(());
            }
            message
                .reply(
                    reqwest::get("https://api.ipify.org")
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap(),
                )
                .await?;
            Ok(())
        })
    });

    app.add_command("info", |_, message, state| {
        Box::pin(async move {
            message
                .reply(InputMessage::markdown(format!(
                    "**Мистбот**\n\n**Статистика:**  \n• Обработано сообщений: **{}**",
                    state.read().await.messages_processed
                )))
                .await?;
            Ok(())
        })
    });

    app.add_command("lucida", |_, message, state| {
        Box::pin(async move {
            let usage = InputMessage::markdown("**Использование:**  \n• /lucida search (сервис) (запрос)\n\n**Доступные сервисы:**  \n• qobuz  \n• tidal  \n• soundcloud  \n• deezer  \n• amazon  \n• yandex",);
            let args: Vec<&str> = message.text().split(" ").skip(1).collect();
            if args.is_empty() {
                return Err(Error::WithMessage(Box::new(usage)));
            }

            let lucida = LucidaClient::new();
            match args[0] {
                "search" => {
                    if args.len() < 3 {
                        return Err(Error::WithMessage(Box::new(usage)));
                    }
                    let Ok(service) = LucidaService::try_from(args[1]) else {
                        return Err(boxed_message("Неизвестный сервис"));
                    };

                    let status_message = message.reply("Ищем...").await?;
                    let Ok(countries) = lucida.fetch_countries(service.clone()).await else {
                        status_message.edit("Сервис недоступен").await?;
                        return Ok(());
                    };
                    if countries.countries.is_empty() {
                        status_message.edit("Сервис недоступен").await?;
                        return Ok(());
                    }

                    let query = args[2..].join(" ");
                    let Ok(results) = lucida.fetch_search(service, &countries.countries[0].code, &query).await else {
                        status_message.edit("Сервис недоступен").await?;
                        return Ok(());
                    };
                    if results.results.tracks.is_empty() {
                        status_message.edit("Не найдено треков по данному запросу").await?;
                        return Ok(());
                    }
                    let buttons: Vec<Vec<_>> = results
                        .results
                        .tracks
                        .chunks(1)
                        .take(10)
                        .map(|c| c.iter().map(|t| button::inline(format!("{} - {}", t.artists[0].name.clone(), t.title.clone()), format!("lucida|{}|{}", &sha1!(&t.url)[..16], message.sender().unwrap().id()))).collect())
                        .collect();

                    for track in results.results.tracks.into_iter() {
                        state.read().await.lucida_cache.insert(sha1!(&track.url)[..16].to_string(), track);
                    }

                    status_message.edit(InputMessage::text("Выберите песню").reply_markup(&reply_markup::inline(buttons))).await?;
                },
                _ => {
                    return Err(boxed_message("Неизвестная команда"));
                }
            };
            Ok(())
        })
    });

    let mut scheduler = AsyncScheduler::new();
    {
        let client = client.clone();
        let state = app.state.clone();
        scheduler.every(Interval::Minutes(10)).run(move || {
            let client = client.clone();
            let state = state.clone();
            async move {
                let Ok(results) = sqlx::query_as::<_, TrackEntry>("SELECT * FROM track_numbers")
                    .fetch_all(&state.read().await.track_db)
                    .await
                else {
                    log::error!("Cannot access track database");
                    return;
                };
                for entry in results {
                    let Ok(chat) = PackedChat::from_bytes(&entry.packed) else {
                        log::error!("Cannot unpack chat");
                        continue;
                    };
                    track_once(client.clone(), state.clone(), &entry.track_number, chat).await;
                }
            }
        });
    }

    {
        let state = app.state.clone();
        scheduler.every(Interval::Minutes(5)).run(move || {
            let state = state.clone();
            async move {
                state.read().await.lucida_cache.clear();
            }
        });
    }

    task::spawn(async move {
        loop {
            scheduler.run_pending().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    mystbot_core::run(client, Arc::new(app)).await;
}
