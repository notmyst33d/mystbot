// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{InvocationError, grammers_tl_types};

/// ## InlineQuery
/// Custom inline query
pub struct InlineQuery {
    query: grammers_client::types::InlineQuery,
    client: grammers_client::Client,
}

impl InlineQuery {
    pub const fn new(
        query: grammers_client::types::InlineQuery,
        client: grammers_client::Client,
    ) -> Self {
        Self { query, client }
    }

    pub fn answer(
        self,
        results: impl IntoIterator<Item = impl Into<grammers_tl_types::enums::InputBotInlineResult>>,
    ) -> Answer {
        Answer {
            request: grammers_tl_types::functions::messages::SetInlineBotResults {
                gallery: false,
                private: true,
                query_id: self.query.query_id(),
                results: results.into_iter().map(Into::into).collect(),
                cache_time: 0,
                next_offset: None,
                switch_pm: None,
                switch_webview: None,
            },
            client: self.client,
        }
    }
}

pub struct Answer {
    request: grammers_tl_types::functions::messages::SetInlineBotResults,
    client: grammers_client::Client,
}

impl Answer {
    pub async fn send(self) -> Result<(), InvocationError> {
        self.client.invoke(&self.request).await?;
        Ok(())
    }
}
