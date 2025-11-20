// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use crate::{AppContext, AppState};
use clokwerk::{AsyncScheduler, Interval};
use grammers_client::{InputMessage, types::PackedChat};
use mystbot_core::MystbotCore;
use sqlx::FromRow;
use track24::{TrackResponse, TrackResponseInner};

#[derive(Debug, FromRow)]
struct TrackEntry {
    packed: Vec<u8>,
    track_number: String,
}

async fn track_once(context: AppContext, track_number: &str, chat: PackedChat) {
    let mut t24client = track24::Client::new();
    let Ok(response) = t24client.track(track_number).await else {
        return;
    };

    let Ok(last_state) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_state FROM track_numbers WHERE chat_id = ? AND track_number = ?",
    )
    .bind(chat.id)
    .bind(track_number)
    .fetch_one(&context.state.read().await.track_db)
    .await
    else {
        return;
    };
    let last_state = if let Some(last_state) = last_state {
        let Ok(last_state) = serde_json::from_str::<track24::TrackResponse>(&last_state) else {
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
        return;
    };

    let Ok(_) = sqlx::query(
        "UPDATE track_numbers SET last_state = ? WHERE chat_id = ? AND track_number = ?",
    )
    .bind(data)
    .bind(chat.id)
    .bind(track_number)
    .execute(&context.state.read().await.track_db)
    .await
    else {
        return;
    };

    let Ok(_) = context
        .client
        .send_message(
            chat,
            InputMessage::markdown(format!(
                "**🔄 Новые обновления для трек номера {track_number}:**  \n{events}"
            )),
        )
        .await
    else {
        return;
    };
}

pub fn register(
    mut app: MystbotCore<AppState>,
    scheduler: &mut AsyncScheduler,
) -> MystbotCore<AppState> {
    app.add_command("track", |context, message| Box::pin(async move {
        let args: Vec<&str> = message.text().split(" ").skip(1).collect();
        if args.is_empty() {
            message.reply("Использование: /track (номер трек кода)").await.unwrap();
            return;
        }

        let Ok(entries) = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM track_numbers WHERE chat_id = ? AND track_number = ?",
        )
        .bind(message.chat().id())
        .bind(args[0])
        .fetch_one(&context.state.read().await.track_db)
        .await else {
            message.reply("Не удалось получить информацию о трек кодах").await.unwrap();
            return;
        };

        if entries > 0 {
            message.reply("Данный трек код уже добавлен").await.unwrap();
            return;
        }

        if sqlx::query(
            "INSERT INTO track_numbers (chat_id, packed, track_number, last_state) VALUES (?, ?, ?, NULL)",
        )
        .bind(message.chat().id())
        .bind(&message.chat().pack().to_bytes()[..])
        .bind(args[0])
        .fetch_all(&context.state.read().await.track_db)
        .await.is_err() {
            message.reply("Не удалось добавить трек код").await.unwrap();
            return;
        };

        message.reply("Трек код был успешно добавлен").await.unwrap();

        track_once(context, args[0], message.chat().pack()).await;
    }));

    app.add_command("untrack", |context, message| {
        Box::pin(async move {
            let args: Vec<&str> = message.text().split(" ").skip(1).collect();
            if args.is_empty() {
                message
                    .reply("Использование: /untrack (номер трек кода)")
                    .await
                    .unwrap();
                return;
            }

            if sqlx::query("DELETE FROM track_numbers WHERE chat_id = ? AND track_number = ?")
                .bind(message.chat().id())
                .bind(args[0])
                .execute(&context.state.read().await.track_db)
                .await
                .is_err()
            {
                message.reply("Не удалось удалить трек код").await.unwrap();
                return;
            }

            message.reply("Трек код был успешно удалён").await.unwrap();
        })
    });

    app.add_command("tracklist", |context, message| {
        Box::pin(async move {
            let Ok(entries) =
                sqlx::query_as::<_, TrackEntry>("SELECT * FROM track_numbers WHERE chat_id = ?")
                    .bind(message.chat().id())
                    .fetch_all(&context.state.read().await.track_db)
                    .await
            else {
                message
                    .reply("Не удалось получить информацию о трек кодах")
                    .await
                    .unwrap();
                return;
            };

            let track_numbers = entries
                .into_iter()
                .map(|v| format!("• `{}`  ", v.track_number))
                .collect::<Vec<_>>()
                .join("\n");

            message
                .reply(InputMessage::markdown(format!(
                    "**Список активных трек кодов:**  \n{track_numbers}"
                )))
                .await
                .unwrap();
        })
    });

    app.schedule_every(scheduler, Interval::Minutes(10), |context| {
        Box::pin(async move {
            let Ok(results) = sqlx::query_as::<_, TrackEntry>("SELECT * FROM track_numbers")
                .fetch_all(&context.state.read().await.track_db)
                .await
            else {
                return;
            };
            for entry in results {
                let Ok(chat) = PackedChat::from_bytes(&entry.packed) else {
                    continue;
                };
                track_once(context.clone(), &entry.track_number, chat).await;
            }
        })
    });

    app
}
