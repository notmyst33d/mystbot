// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

pub mod client_wrapper;
pub mod error;
pub mod inline_audio;
pub mod inline_query;

use dashmap::DashMap;
use futures::future::BoxFuture;
use grammers_client::{
    Config, Update,
    client::bots::AuthorizationError,
    session::Session,
    types::{CallbackQuery, InlineQuery, InlineSend, Message, User},
};
use log::debug;
use regex::Regex;
use std::sync::Arc;
use tokio::task;

use crate::{
    client_wrapper::ClientWrapper,
    error::{Error, WithMessage},
};

type Fut = BoxFuture<'static, Result<(), Error>>;
type FutBool = BoxFuture<'static, Result<bool, Error>>;

struct CommandData<State> {
    regex: Regex,
    func: Box<fn(ClientWrapper, Message, State) -> Fut>,
}

pub struct MystbotCore<State: Send + Sync> {
    me: User,
    commands: DashMap<String, CommandData<State>>,
    pub state: State,
    callback_query: Option<fn(ClientWrapper, CallbackQuery, State) -> Fut>,
    inline_query: Option<fn(ClientWrapper, InlineQuery, State) -> Fut>,
    inline_send: Option<fn(ClientWrapper, InlineSend, State) -> Fut>,
    before_command_hook: Option<fn(ClientWrapper, Message, State) -> FutBool>,
    after_command_hook: Option<fn(ClientWrapper, Message, State) -> Fut>,
}

impl<State: Send + Sync + 'static> MystbotCore<State> {
    pub async fn connect(
        bot_token: &str,
        state: State,
    ) -> Result<(ClientWrapper, Self), AuthorizationError> {
        let client = grammers_client::Client::connect(Config {
            session: Session::load_file_or_create("teobot.session")
                .expect("failed to load session"),
            api_id: std::env::var("API_ID")
                .expect("api_id not found")
                .parse()
                .expect("failed to parse api_id"),
            api_hash: std::env::var("API_HASH")
                .expect("api_hash not found")
                .to_string(),
            params: Default::default(),
        })
        .await?;

        if !client.is_authorized().await? {
            client.bot_sign_in(bot_token).await?;
        }

        client.session().save_to_file("teobot.session")?;
        let me = client.get_me().await.unwrap();

        Ok((
            ClientWrapper(client),
            Self {
                me,
                state,
                commands: DashMap::new(),
                callback_query: None,
                inline_query: None,
                inline_send: None,
                before_command_hook: None,
                after_command_hook: None,
            },
        ))
    }

    /// Add new command to handler list
    pub fn add_command(
        &mut self,
        command: impl Into<String>,
        handler: fn(ClientWrapper, Message, State) -> Fut,
    ) {
        let command = command.into();
        self.commands.insert(
            command.clone(),
            CommandData {
                regex: Regex::new(&format!(
                    r"^(\/{})(@|)({}|)( |$)",
                    command,
                    self.me.username().unwrap(),
                ))
                .unwrap(),
                func: Box::new(handler),
            },
        );
    }

    /// Add callback query handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn add_callback_query(&mut self, handler: fn(ClientWrapper, CallbackQuery, State) -> Fut) {
        self.callback_query = Some(handler);
    }

    /// Add inline query handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn add_inline_query(&mut self, handler: fn(ClientWrapper, InlineQuery, State) -> Fut) {
        self.inline_query = Some(handler);
    }

    /// Add inline send handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn add_inline_send(&mut self, handler: fn(ClientWrapper, InlineSend, State) -> Fut) {
        self.inline_send = Some(handler);
    }

    /// Add handler which executes after the command handler
    pub fn add_after_command_hook(&mut self, handler: fn(ClientWrapper, Message, State) -> Fut) {
        self.after_command_hook = Some(handler);
    }

    /// Add handler which executes before the command handler, the return value indicates if the command handler should run or not
    pub fn add_before_command_hook(
        &mut self,
        handler: fn(ClientWrapper, Message, State) -> FutBool,
    ) {
        self.before_command_hook = Some(handler);
    }
}

/// Start bot
pub async fn run<S: Sync + Send + Clone + 'static>(
    client: ClientWrapper,
    app: Arc<MystbotCore<S>>,
) {
    loop {
        let client = client.clone();
        let app = app.clone();
        match client.next_update().await.unwrap() {
            Update::NewMessage(message) => {
                debug!("new command thread");
                task::spawn(async move {
                    if message.outgoing() {
                        return;
                    }
                    for multi in app.commands.iter() {
                        let Some(caps) = multi.regex.captures(message.text()) else {
                            continue;
                        };
                        if caps[2].is_empty() || (!caps[2].is_empty() && !caps[3].is_empty()) {
                            if caps[1][1..] != *multi.key() {
                                continue;
                            }
                            if let Some(func) = app.before_command_hook {
                                if !func(client.clone(), message.clone(), app.state.clone())
                                    .await
                                    .unwrap_or_default()
                                {
                                    continue;
                                }
                            }
                            match (multi.func)(client.clone(), message.clone(), app.state.clone())
                                .await
                            {
                                Ok(_) => {
                                    if let Some(func) = app.after_command_hook {
                                        let _ =
                                            func(client.clone(), message, app.state.clone()).await;
                                    }
                                }
                                Err(e) => {
                                    debug!("handler error: {e}");
                                    if let Some(m) = e.input_message() {
                                        message.reply(m).await.unwrap();
                                    }
                                }
                            };
                            break;
                        }
                    }
                });
            }
            Update::CallbackQuery(query) => {
                debug!("new callback query thread");
                task::spawn(async move {
                    if let Some(func) = app.callback_query {
                        match func(client, query, app.state.clone()).await {
                            Ok(_) => {}
                            Err(e) => {
                                debug!("handler error: {e}");
                            }
                        };
                    }
                });
            }
            Update::InlineQuery(query) => {
                debug!("new inline query thread");
                task::spawn(async move {
                    if let Some(func) = app.inline_query {
                        match func(client, query, app.state.clone()).await {
                            Ok(_) => {}
                            Err(e) => {
                                debug!("handler error: {e}");
                            }
                        };
                    }
                });
            }
            Update::InlineSend(send) => {
                debug!("new inline send thread");
                task::spawn(async move {
                    if let Some(func) = app.inline_send {
                        match func(client, send, app.state.clone()).await {
                            Ok(_) => {}
                            Err(e) => {
                                debug!("handler error: {e}");
                            }
                        };
                    }
                });
            }
            _ => {}
        }
    }
}
