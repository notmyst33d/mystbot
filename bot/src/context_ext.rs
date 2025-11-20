// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    grammers_tl_types::{self, enums::InputBotInlineMessageId},
    types::Attribute,
};
use mystbot_core::inline_message_ext::InlineMessageExt;

use crate::{AppContext, CachedFile, audio_common::DownloadedTrack};
use std::{path::Path, time::Duration};

pub trait ContextExt {
    fn get_cached_file(&self, url: &str) -> impl Future<Output = Option<CachedFile>>;

    fn upload_cached_file(
        &self,
        url: &str,
        path: &Path,
        content_type: &str,
    ) -> impl Future<Output = anyhow::Result<CachedFile>>;

    fn send_downloaded_track(
        &self,
        downloaded_track: DownloadedTrack,
        message_id: InputBotInlineMessageId,
        title: String,
        artist: String,
        duration_ms: u64,
    ) -> impl Future<Output = anyhow::Result<bool>>;
}

impl ContextExt for AppContext {
    async fn get_cached_file(&self, url: &str) -> Option<CachedFile> {
        Some(self.state.read().await.file_cache.get(url)?.value().clone())
    }

    async fn upload_cached_file(
        &self,
        url: &str,
        path: &Path,
        content_type: &str,
    ) -> anyhow::Result<CachedFile> {
        let cached_file = CachedFile(
            self.client.upload_file(path).await?,
            content_type.to_owned(),
        );
        self.state
            .read()
            .await
            .file_cache
            .insert(url.to_owned(), cached_file.clone());
        Ok(cached_file)
    }

    async fn send_downloaded_track(
        &self,
        downloaded_track: DownloadedTrack,
        message_id: InputBotInlineMessageId,
        title: String,
        artist: String,
        duration_ms: u64,
    ) -> anyhow::Result<bool> {
        Ok(self
            .client
            .edit_inline_message_ext(
                message_id,
                "",
                None,
                Some(grammers_tl_types::enums::InputMedia::UploadedDocument(
                    grammers_tl_types::types::InputMediaUploadedDocument {
                        nosound_video: false,
                        force_file: false,
                        spoiler: false,
                        file: downloaded_track.track_file.0.raw,
                        thumb: downloaded_track.cover_file.map(|t| t.0.raw),
                        mime_type: downloaded_track.track_file.1,
                        attributes: vec![
                            Attribute::Audio {
                                duration: Duration::from_millis(duration_ms),
                                title: Some(title),
                                performer: Some(artist),
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
            .await?)
    }
}
