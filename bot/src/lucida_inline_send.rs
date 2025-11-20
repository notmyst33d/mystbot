// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use crate::{AppContext, audio_common, lucida_common};
use grammers_client::types::InlineSend;
use mystbot_core::inline_message_ext::InlineMessageExt;

pub async fn run(context: AppContext, send: InlineSend, args: Vec<String>) -> anyhow::Result<()> {
    let message_id = send.message_id().unwrap();

    let Some(track) = context
        .state
        .read()
        .await
        .lucida_cache
        .get(&args[0])
        .map(|v| v.value().clone())
    else {
        context
            .client
            .edit_inline_message_ext(
                message_id,
                "Устаревшее сообщение",
                Some("Скачиваем..."),
                None,
            )
            .await?;
        return Ok(());
    };

    audio_common::retry_send_inline_with_progress(
        context.clone(),
        message_id.clone(),
        track.title.clone(),
        track.artists[0].name.clone(),
        track.duration_ms as u64,
        track.clone(),
        lucida_common::download_track,
    )
    .await
}
