// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use crate::{AppContext, fruityger_common::ModuleType, return_response};
use grammers_client::{
    button, reply_markup,
    types::{
        InlineQuery,
        inline::query::{Article, InlineResult},
    },
};
use mystbot_core::inline_audio::InlineAudio;

pub async fn run(context: AppContext, query: InlineQuery, args: Vec<String>) -> anyhow::Result<()> {
    if args.is_empty() {
        return_response!(query, "Введите запрос");
    }

    let (service, search_query) = if ["yandex", "qobuz", "hifi"].contains(&args[0].as_str()) {
        if args.len() < 2 {
            return_response!(query, "Введите запрос");
        }
        (ModuleType::try_from(args[0].as_str())?, args[1..].join(" "))
    } else {
        (ModuleType::HifiQobuz, args.join(" "))
    };

    let results = match service.as_str() {
        "yandex" => {
            if let Some(client) = &context.state.read().await.fruityger_clients.yandex {
                client.search(&search_query, 0).await
            } else {
                return_response!(query, "Сервис недоступен");
            }
        }
        "hifi" | "qobuz" => {
            if let Some(client) = &context.state.read().await.fruityger_clients.hifi {
                client.search(&search_query, 0).await
            } else {
                return_response!(query, "Сервис недоступен");
            }
        }
        _ => {
            return_response!(query, "Такого сервиса не существует");
        }
    };
    let Ok(results) = results else {
        return_response!(query, "Сервис недоступен");
    };

    let inline_results: Vec<_> = results
        .tracks
        .iter()
        .take(10)
        .map(|t| {
            InlineAudio::new("https://s.myst33d.ru/placeholder.mp3".to_string())
                .id(format!("fruityger|{}|{}", service.as_str(), t.id))
                .title(t.title.clone())
                .performer(t.artists[0].name.clone())
                .reply_markup(&reply_markup::inline(vec![vec![button::inline(
                    "Скачиваем...",
                    b"0",
                )]]))
        })
        .collect();

    for track in results.tracks.into_iter() {
        context
            .state
            .read()
            .await
            .fruityger_cache
            .insert(track.id.clone(), track);
    }

    let audio_query =
        mystbot_core::inline_query::InlineQuery::new(query.clone(), context.client.clone());
    audio_query.answer(inline_results).send().await?;

    Ok(())
}
