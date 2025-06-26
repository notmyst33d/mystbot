// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use fruityger::{Metadata, Track};
use mystbot_core::client_wrapper::ClientWrapper;
use tokio::sync::mpsc;

use crate::{
    AppState,
    cached_file::{CachedFile, get_cached_file, upload_cached_file},
};

pub async fn download_track(
    client: ClientWrapper,
    state: AppState,
    track: Track,
    tx: mpsc::Sender<String>,
    refresh_cache: bool,
) -> Option<(CachedFile, CachedFile)> {
    let workdir = tempfile::tempdir().ok()?;
    let filename_temp = format!("{} - {}_temp", track.artists[0].name, track.title);
    let filename = format!("{} - {}", track.artists[0].name, track.title);

    let get_audio = || async {
        tx.send("Скачиваем аудио".to_string()).await.ok()?;

        let mut audio = state
            .read()
            .await
            .fruityger_client
            .download(workdir.path(), &filename_temp, &track.url)
            .await
            .ok()?;

        tx.send("Скачиваем обложку".to_string()).await.ok()?;

        let cover = state
            .read()
            .await
            .fruityger_client
            .download_cover(workdir.path(), "cover", &track.cover_url)
            .await
            .ok()?;

        tx.send("Добавляем метаданные".to_string()).await.ok()?;

        audio = state
            .read()
            .await
            .fruityger_client
            .remux(
                workdir.path(),
                &filename,
                audio,
                &cover.1,
                Metadata {
                    title: track.title.clone(),
                    artist: track.artists[0].name.clone(),
                    ..Default::default()
                },
            )
            .ok()?;

        tx.send("Загружаем файл".to_string()).await.ok()?;

        upload_cached_file(
            client.clone(),
            &state.read().await.cache_dir,
            &audio.1,
            &track.url,
            audio.0.mime_type(),
        )
        .await
    };

    let audio = if refresh_cache {
        get_audio().await?
    } else {
        match get_cached_file(&state.read().await.cache_dir, &track.url).await {
            Some(audio) => audio,
            None => get_audio().await?,
        }
    };

    let artwork = {
        let get_thumb = || async {
            let cover = state
                .read()
                .await
                .fruityger_client
                .download_cover(workdir.path(), "cover", &track.cover_url)
                .await
                .ok()?;
            upload_cached_file(
                client,
                &state.read().await.cache_dir,
                &cover.1,
                &track.cover_url,
                cover.0.mime_type(),
            )
            .await
        };
        if refresh_cache {
            get_thumb().await?
        } else {
            match get_cached_file(&state.read().await.cache_dir, &track.cover_url).await {
                Some(artwork) => artwork,
                None => get_thumb().await?,
            }
        }
    };

    Some((audio, artwork))
}
