// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use crate::{
    AppContext,
    audio_common::{self, DownloadedTrack},
    context_ext::ContextExt,
};
use fruityger::{Metadata, Track, format::Format};
use tokio::sync::mpsc;

#[derive(Clone)]
pub enum ModuleType {
    Yandex,
    HifiQobuz,
}

impl TryFrom<&str> for ModuleType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "hifi" | "qobuz" => Ok(ModuleType::HifiQobuz),
            "yandex" => Ok(ModuleType::Yandex),
            _ => Err(anyhow::anyhow!("cannot convert `{value}` to ModuleType")),
        }
    }
}

impl ModuleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModuleType::Yandex => "yandex",
            ModuleType::HifiQobuz => "hifi",
        }
    }
}

pub async fn download_track(
    context: AppContext,
    (track, module_type): (Track, ModuleType),
    tx: mpsc::Sender<String>,
    refresh_cache: bool,
) -> anyhow::Result<DownloadedTrack> {
    let workdir = tempfile::tempdir()?;
    let filename_temp = format!("{} - {}_temp", track.artists[0].name, track.title);
    let filename = format!("{} - {}", track.artists[0].name, track.title);

    audio_common::get_downloaded_track(
        context,
        refresh_cache,
        track.url.clone(),
        Some(track.cover_url.clone()),
        |context| async move {
            tx.send("Скачиваем аудио".to_string()).await?;

            let stream = match module_type {
                ModuleType::Yandex => {
                    context
                        .state
                        .read()
                        .await
                        .fruityger_clients
                        .yandex
                        .clone()
                        .ok_or(anyhow::anyhow!("module not active"))?
                        .get_stream(&track.id)
                        .await?
                }
                ModuleType::HifiQobuz => {
                    context
                        .state
                        .read()
                        .await
                        .fruityger_clients
                        .hifi
                        .clone()
                        .ok_or(anyhow::anyhow!("module not active"))?
                        .get_stream(&track.id)
                        .await?
                }
            };
            let format = stream.format.clone();

            let mut track_path =
                fruityger::save_audio_stream(stream, workdir.path(), &filename_temp).await?;

            tx.send("Скачиваем обложку".to_string()).await?;

            let cover_path = fruityger::save_cover(
                reqwest::get(&track.cover_url).await?,
                workdir.path(),
                "cover",
            )
            .await?;

            tx.send("Добавляем метаданные".to_string()).await?;

            track_path = fruityger::remux(
                workdir.path(),
                &track_path,
                Some(&cover_path.0),
                format.clone(),
                &filename,
                Metadata {
                    title: track.title.clone(),
                    artist: track.artists[0].name.clone(),
                    ..Default::default()
                },
            )?;

            tx.send("Загружаем файл".to_string()).await?;

            context
                .upload_cached_file(&track.url, &track_path, format.mime_type())
                .await
        },
    )
    .await
}
