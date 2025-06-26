// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    grammers_tl_types,
    types::{Attribute, InlineSend},
};
use mystbot_core::{client_wrapper::ClientWrapper, error::Error};
use std::time::Duration;
use tokio::{sync::mpsc, task};

use crate::{AppState, cached_file::CachedFile, fruityger_common};

pub async fn run(
    client: ClientWrapper,
    send: InlineSend,
    state: AppState,
    args: Vec<String>,
) -> Result<(), Error> {
    let message_id = send.message_id().unwrap();
    let Some(track) = state
        .read()
        .await
        .fruityger_cache
        .view(&args[0], |_, v| v.clone())
    else {
        client
            .edit_inline_message(
                message_id,
                "Устаревшее сообщение",
                Some("Скачиваем...".to_string()),
                None,
            )
            .await?;
        return Ok(());
    };

    let (tx, mut rx) = mpsc::channel(16);
    {
        let message_id = message_id.clone();
        let client = client.clone();
        task::spawn(async move {
            while let Some(m) = rx.recv().await {
                let _ = client
                    .edit_inline_message(
                        message_id.clone(),
                        m,
                        Some("Скачиваем...".to_string()),
                        None,
                    )
                    .await;
            }
        });
    }

    let send = |audio: CachedFile, artwork: CachedFile| async {
        client
            .edit_inline_message(
                message_id.clone(),
                "",
                None,
                Some(grammers_tl_types::enums::InputMedia::UploadedDocument(
                    grammers_tl_types::types::InputMediaUploadedDocument {
                        nosound_video: false,
                        force_file: false,
                        spoiler: false,
                        file: audio.0.raw,
                        thumb: Some(artwork.0.raw),
                        mime_type: audio.1,
                        attributes: vec![
                            Attribute::Audio {
                                duration: Duration::from_millis(track.duration_ms as u64),
                                title: Some(track.title.clone()),
                                performer: Some(track.artists[0].name.clone()),
                            }
                            .into(),
                        ],
                        video_cover: None,
                        video_timestamp: None,
                        stickers: None,
                        ttl_seconds: None,
                    },
                )),
            )
            .await
    };

    let mut sent = false;
    for i in 0..2 {
        if let Some((audio, artwork)) = fruityger_common::download_track(
            client.clone(),
            state.clone(),
            track.clone(),
            tx.clone(),
            i == 1,
        )
        .await
        {
            if send(audio, artwork).await.unwrap_or_default() {
                sent = true;
                break;
            }
        };
    }

    if !sent {
        client
            .edit_inline_message(message_id.clone(), "Не удалось скачать трек", None, None)
            .await?;
    }

    Ok(())
}
