// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    button, reply_markup,
    types::{
        InlineQuery,
        inline::query::{Article, InlineResult},
    },
};
use lucida_api::{LucidaClient, LucidaService};
use mystbot_core::inline_audio::InlineAudio;

use crate::{AppContext, return_response, sha1};

pub async fn run(context: AppContext, query: InlineQuery, args: Vec<String>) -> anyhow::Result<()> {
    if args.is_empty() {
        return_response!(query, "Введите запрос");
    }

    let lucida = LucidaClient::new();
    let (service, search_query) = if let Ok(service) = LucidaService::try_from(args[0].as_str()) {
        if args.len() < 2 {
            return_response!(query, "Введите запрос");
        }
        (service, args[1..].join(" "))
    } else {
        (LucidaService::Tidal, args.join(" "))
    };

    let Ok(countries) = lucida.fetch_countries(service.clone()).await else {
        return_response!(query, "Сервис недоступен");
    };
    if countries.countries.is_empty() {
        return_response!(query, "Сервис недоступен");
    }

    let Ok(results) = lucida
        .fetch_search(service, &countries.countries[0].code, &search_query)
        .await
    else {
        return_response!(query, "Сервис недоступен");
    };
    if results.results.tracks.is_empty() {
        return_response!(query, "Не найдено треков по данному запросу");
    }

    let inline_results: Vec<_> = results
        .results
        .tracks
        .iter()
        .take(10)
        .map(|t| {
            InlineAudio::new("https://s.myst33d.ru/placeholder.mp3".to_string())
                .id(format!("lucida|{}", &sha1!(&t.url.clone())[..16]))
                .title(t.title.clone())
                .performer(t.artists[0].name.clone())
                .reply_markup(&reply_markup::inline(vec![vec![button::inline(
                    "Скачиваем...",
                    b"0",
                )]]))
        })
        .collect();

    for track in results.results.tracks.into_iter() {
        context
            .state
            .read()
            .await
            .lucida_cache
            .insert(sha1!(&track.url)[..16].to_string(), track);
    }

    let audio_query =
        mystbot_core::inline_query::InlineQuery::new(query.clone(), context.client.clone());
    audio_query.answer(inline_results).send().await?;

    Ok(())
}
