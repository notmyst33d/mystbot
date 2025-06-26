// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{grammers_tl_types, reply_markup::ReplyMarkup};
use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::SystemTime,
};

pub struct InlineAudio {
    id: Option<String>,
    url: String,
    text: Option<String>,
    thumbnail: Option<String>,
    mime: Option<String>,
    title: Option<String>,
    performer: Option<String>,
    duration: Option<i32>,
    reply_markup: Option<grammers_tl_types::enums::ReplyMarkup>,
}

static LAST_ID: AtomicI64 = AtomicI64::new(0);

fn generate_random_id() -> i64 {
    if LAST_ID.load(Ordering::SeqCst) == 0 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_nanos() as i64;

        LAST_ID
            .compare_exchange(0, now, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap();
    }

    LAST_ID.fetch_add(1, Ordering::SeqCst)
}

impl InlineAudio {
    pub const fn new(url: String) -> Self {
        Self {
            id: None,
            url,
            text: None,
            thumbnail: None,
            mime: None,
            title: None,
            performer: None,
            duration: None,
            reply_markup: None,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn thumbnail(mut self, thumbnail: impl Into<String>) -> Self {
        self.thumbnail = Some(thumbnail.into());
        self
    }

    pub fn thumbnail_option(mut self, thumbnail: Option<impl Into<String>>) -> Self {
        self.thumbnail = thumbnail.map(|s| s.into());
        self
    }

    pub fn mime(mut self, mime: impl Into<String>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn performer(mut self, performer: impl Into<String>) -> Self {
        self.performer = Some(performer.into());
        self
    }

    pub const fn duration(mut self, duration: i32) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn reply_markup<RM: ReplyMarkup>(mut self, markup: &RM) -> Self {
        self.reply_markup = Some(markup.to_reply_markup().raw);
        self
    }
}

impl From<InlineAudio> for grammers_tl_types::enums::InputBotInlineResult {
    fn from(value: InlineAudio) -> Self {
        Self::Result(grammers_tl_types::types::InputBotInlineResult {
            id: value.id.unwrap_or_else(|| generate_random_id().to_string()),
            r#type: "audio".to_string(),
            title: value.title.clone(),
            description: value.performer.clone(),
            url: None,
            thumb: value.thumbnail.map(|u| {
                grammers_tl_types::enums::InputWebDocument::Document(
                    grammers_tl_types::types::InputWebDocument {
                        url: u,
                        size: 0,
                        mime_type: "image/jpeg".to_string(),
                        attributes: vec![],
                    },
                )
            }),
            content: Some(grammers_tl_types::enums::InputWebDocument::Document(
                grammers_tl_types::types::InputWebDocument {
                    url: value.url,
                    size: 0,
                    mime_type: value.mime.unwrap_or_else(|| "audio/mpeg".to_string()),
                    attributes: vec![grammers_tl_types::enums::DocumentAttribute::Audio(
                        grammers_tl_types::types::DocumentAttributeAudio {
                            voice: false,
                            duration: value.duration.unwrap_or(0),
                            title: value.title,
                            performer: value.performer,
                            waveform: None,
                        },
                    )],
                },
            )),
            send_message: grammers_tl_types::enums::InputBotInlineMessage::MediaAuto(
                grammers_tl_types::types::InputBotInlineMessageMediaAuto {
                    invert_media: false,
                    message: value.text.unwrap_or_default(),
                    entities: None,
                    reply_markup: value.reply_markup,
                },
            ),
        })
    }
}
