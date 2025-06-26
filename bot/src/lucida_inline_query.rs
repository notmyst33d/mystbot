// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    button, reply_markup,
    types::{
        InlineQuery,
        inline::query::{Article, InlineResult},
    },
};
use itertools::Itertools;
use lucida_api::{LucidaClient, LucidaService};
use mystbot_core::{client_wrapper::ClientWrapper, error::Error, inline_audio::InlineAudio};

use crate::{AppState, sha1};

pub async fn run(
    client: ClientWrapper,
    query: InlineQuery,
    state: AppState,
    args: Vec<String>,
) -> Result<(), Error> {
    if args.is_empty() {
        query
            .answer([InlineResult::from(Article::new(
                "Введите запрос",
                "Введите запрос",
            ))])
            .send()
            .await?;
        return Ok(());
    }

    let lucida = LucidaClient::new();
    let (service, search_query) = if let Ok(service) = LucidaService::try_from(args[0].as_str()) {
        if args.len() < 2 {
            query
                .answer([InlineResult::from(Article::new(
                    "Введите запрос",
                    "Введите запрос",
                ))])
                .send()
                .await?;
            return Ok(());
        }
        (service, args[1..].join(" "))
    } else {
        (LucidaService::Tidal, args.join(" "))
    };

    let Ok(countries) = lucida.fetch_countries(service.clone()).await else {
        query
            .answer([InlineResult::from(Article::new(
                "Сервис недоступен",
                "Сервис недоступен",
            ))])
            .send()
            .await?;
        return Ok(());
    };
    if countries.countries.is_empty() {
        query
            .answer([InlineResult::from(Article::new(
                "Сервис недоступен",
                "Сервис недоступен",
            ))])
            .send()
            .await?;
        return Ok(());
    }

    let Ok(results) = lucida
        .fetch_search(service, &countries.countries[0].code, &search_query)
        .await
    else {
        query
            .answer([InlineResult::from(Article::new(
                "Сервис недоступен",
                "Сервис недоступен",
            ))])
            .send()
            .await?;
        return Ok(());
    };
    if results.results.tracks.is_empty() {
        query
            .answer([InlineResult::from(Article::new(
                "Не найдено треков по данному запросу",
                "Не найдено треков по данному запросу",
            ))])
            .send()
            .await?;
        return Ok(());
    }

    let inline_results: Vec<_> = results
        .results
        .tracks
        .iter()
        .take(10)
        .map(|t| {
            (
                sha1!(t.url.clone())[..16].to_string(),
                InlineAudio::new("https://s.myst33d.ru/placeholder.mp3".to_string())
                    .id(format!("lucida|{}", &sha1!(&t.url.clone())[..16]))
                    .title(t.title.clone())
                    .performer(t.artists[0].name.clone())
                    .reply_markup(&reply_markup::inline(vec![vec![button::inline(
                        "Скачиваем...",
                        b"0",
                    )]])),
            )
        })
        .unique_by(|t| t.0.clone())
        .map(|t| t.1)
        .collect();

    for track in results.results.tracks.into_iter() {
        state
            .read()
            .await
            .lucida_cache
            .insert(sha1!(&track.url)[..16].to_string(), track);
    }

    let audio_query = mystbot_core::inline_query::InlineQuery::new(query.clone(), client.0);
    audio_query.answer(inline_results).send().await?;

    Ok(())
}
