// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    Client, InvocationError, button,
    grammers_tl_types::{
        self,
        enums::{InputBotInlineMessageId, InputMedia},
    },
    reply_markup::{self, ReplyMarkup},
};

pub trait InlineMessageExt {
    fn edit_inline_message_ext<S: AsRef<str> + Send>(
        &self,
        id: InputBotInlineMessageId,
        text: S,
        button_text: Option<S>,
        media: Option<InputMedia>,
    ) -> impl Future<Output = Result<bool, InvocationError>> + Send;
}

impl InlineMessageExt for Client {
    async fn edit_inline_message_ext<S: AsRef<str> + Send>(
        &self,
        id: InputBotInlineMessageId,
        text: S,
        button_text: Option<S>,
        media: Option<InputMedia>,
    ) -> Result<bool, InvocationError> {
        self.invoke(
            &grammers_tl_types::functions::messages::EditInlineBotMessage {
                id,
                message: Some(text.as_ref().to_owned()),
                media,
                entities: None,
                no_webpage: true,
                reply_markup: button_text.map(|t| {
                    reply_markup::inline(vec![vec![button::inline(t.as_ref(), b"0")]])
                        .to_reply_markup()
                        .raw
                }),
                invert_media: false,
            },
        )
        .await
    }
}
