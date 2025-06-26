// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use std::ops::Deref;

use grammers_client::{
    InvocationError, button, grammers_tl_types,
    reply_markup::{self, ReplyMarkup},
};

#[derive(Clone)]
pub struct ClientWrapper(pub grammers_client::Client);

impl Deref for ClientWrapper {
    type Target = grammers_client::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClientWrapper {
    pub fn client(self) -> grammers_client::Client {
        self.0
    }

    pub async fn edit_inline_message(
        &self,
        id: grammers_tl_types::enums::InputBotInlineMessageId,
        text: impl Into<String>,
        button_text: Option<String>,
        media: Option<grammers_tl_types::enums::InputMedia>,
    ) -> Result<bool, InvocationError> {
        self.0
            .invoke(
                &grammers_tl_types::functions::messages::EditInlineBotMessage {
                    id,
                    message: Some(text.into()),
                    media,
                    entities: None,
                    no_webpage: true,
                    reply_markup: button_text.map(|t| {
                        reply_markup::inline(vec![vec![button::inline(t, b"0")]])
                            .to_reply_markup()
                            .raw
                    }),
                    invert_media: false,
                },
            )
            .await
    }
}
