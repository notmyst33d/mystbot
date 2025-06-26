// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use futures::TryStreamExt;
use lucida_api::{LucidaClient, Track};
use mystbot_core::client_wrapper::ClientWrapper;
use tokio::{fs::File, sync::mpsc, task};

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
) -> Option<(CachedFile, Option<CachedFile>)> {
    let workdir = tempfile::tempdir().ok()?;

    let lucida = LucidaClient::new();
    let get_audio = || async {
        let (dtx, mut drx) = mpsc::channel::<String>(16);
        {
            let tx = tx.clone();
            let track = track.clone();
            task::spawn(async move {
                while let Some(m) = drx.recv().await {
                    let _ = tx
                        .send(m.replace(
                            "{item}",
                            &format!("{} - {}", track.title, track.artists[0].name),
                        ))
                        .await;
                }
            });
        }

        let response = lucida
            .try_download_all_countries(&track.url, true, dtx)
            .await
            .ok()?;

        let mut stream = response.response.bytes_stream();
        let out = workdir.path().join(response.filename?);
        let mut file = File::create(&out).await.ok()?;
        while let Some(chunk) = stream.try_next().await.ok()? {
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await.ok()?;
        }

        upload_cached_file(
            client.clone(),
            &state.read().await.cache_dir,
            &out,
            &track.url,
            &response.content_type.unwrap_or("audio/flac".to_string()),
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

    let artwork = if let Some(artwork) = track.artwork() {
        let get_thumb = || async {
            let response = reqwest::get(&artwork).await.ok()?;
            let mut stream = response.bytes_stream();
            let out = workdir.path().join("cover.jpg");
            let mut file = File::create(&out).await.ok()?;
            while let Some(chunk) = stream.try_next().await.ok()? {
                tokio::io::copy(&mut chunk.as_ref(), &mut file).await.ok()?;
            }
            upload_cached_file(
                client,
                &state.read().await.cache_dir,
                &out,
                &artwork,
                "image/jpeg",
            )
            .await
        };
        if refresh_cache {
            get_thumb().await
        } else {
            match get_cached_file(&state.read().await.cache_dir, &artwork).await {
                Some(artwork) => Some(artwork),
                None => get_thumb().await,
            }
        }
    } else {
        None
    };

    Some((audio, artwork))
}
