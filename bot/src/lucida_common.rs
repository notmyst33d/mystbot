// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use crate::{
    AppContext,
    audio_common::{self, DownloadedTrack},
    context_ext::ContextExt,
};
use lucida_api::{LucidaClient, Track};
use tokio::sync::mpsc;

pub async fn download_track(
    context: AppContext,
    track: Track,
    tx: mpsc::Sender<String>,
    refresh_cache: bool,
) -> anyhow::Result<DownloadedTrack> {
    let workdir = tempfile::tempdir()?;
    let lucida = LucidaClient::new();

    audio_common::get_downloaded_track(
        context,
        refresh_cache,
        track.url.clone(),
        track.artwork(),
        |context| async move {
            let response = lucida
                .try_download_all_countries(&track.url, true, tx)
                .await?;
            let out = workdir
                .path()
                .join(response.filename.unwrap_or("audio.flac".to_owned()));
            fruityger::save(response.response, &out).await?;
            context
                .upload_cached_file(
                    &track.url,
                    &out,
                    &response.content_type.unwrap_or("audio/flac".to_owned()),
                )
                .await
        },
    )
    .await
}
