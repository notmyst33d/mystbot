// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

pub mod inline_audio;
pub mod inline_message_ext;
pub mod inline_query;

use clokwerk::{AsyncScheduler, Interval};
use dashmap::DashMap;
use futures::future::BoxFuture;
use grammers_client::{
    Client, Config, Update,
    client::bots::AuthorizationError,
    session::Session,
    types::{CallbackQuery, InlineQuery, InlineSend, Message, User},
};
use regex::Regex;
use std::{sync::Arc, time::Duration};

type Fut = BoxFuture<'static, ()>;
type MessageCallback<State> = fn(Context<State>, Message) -> Fut;
type CallbackQueryCallback<State> = fn(Context<State>, CallbackQuery) -> Fut;
type InlineQueryCallback<State> = fn(Context<State>, InlineQuery) -> Fut;
type InlineSendCallback<State> = fn(Context<State>, InlineSend) -> Fut;
type ContextCallback<State> = fn(Context<State>) -> Fut;

type ComposeModuleFunc<State> = fn(MystbotCore<State>, &mut AsyncScheduler) -> MystbotCore<State>;

struct CommandData<State> {
    regex: Regex,
    func: Box<MessageCallback<State>>,
}

#[derive(Clone)]
pub struct Context<State> {
    pub client: Client,
    pub state: State,
}

impl<State> Context<State> {
    pub fn new(client: Client, state: State) -> Self {
        Context { client, state }
    }
}

pub struct MystbotCore<State> {
    me: User,
    commands: DashMap<String, CommandData<State>>,
    client: Client,
    state: State,
    callback_query: Option<CallbackQueryCallback<State>>,
    inline_query: Option<InlineQueryCallback<State>>,
    inline_send: Option<InlineSendCallback<State>>,
}

impl<State: Send + Sync + Clone + 'static> MystbotCore<State> {
    pub async fn connect(
        bot_token: &str,
        api_id: i32,
        api_hash: &str,
        state: State,
    ) -> Result<(Client, Self), AuthorizationError> {
        let client = grammers_client::Client::connect(Config {
            session: Session::load_file_or_create("teobot.session")
                .expect("failed to load session"),
            api_id,
            api_hash: api_hash.to_owned(),
            params: Default::default(),
        })
        .await?;

        if !client.is_authorized().await? {
            client.bot_sign_in(bot_token).await?;
        }

        client.session().save_to_file("teobot.session")?;
        let me = client.get_me().await.unwrap();

        Ok((
            client.clone(),
            Self {
                me,
                commands: DashMap::new(),
                client,
                state,
                callback_query: None,
                inline_query: None,
                inline_send: None,
            },
        ))
    }

    /// Add new command to handler list
    pub fn add_command(&mut self, command: impl Into<String>, handler: MessageCallback<State>) {
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

    /// Set callback query handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn set_callback_query(&mut self, handler: CallbackQueryCallback<State>) {
        self.callback_query = Some(handler);
    }

    /// Set inline query handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn set_inline_query(&mut self, handler: InlineQueryCallback<State>) {
        self.inline_query = Some(handler);
    }

    /// Set inline send handler, there can be only one handler, if you call this function again with another handler it will replace the old one
    pub fn set_inline_send(&mut self, handler: InlineSendCallback<State>) {
        self.inline_send = Some(handler);
    }

    /// Composable module registration
    pub fn register(self, func: ComposeModuleFunc<State>, scheduler: &mut AsyncScheduler) -> Self {
        func(self, scheduler)
    }

    /// Schedule a function to run on intervals
    pub fn schedule_every(
        &self,
        scheduler: &mut AsyncScheduler,
        ival: Interval,
        func: ContextCallback<State>,
    ) {
        let context = Context::new(self.client.clone(), self.state.clone());
        scheduler.every(ival).run(move || {
            let context = context.clone();
            async move {
                let _ = func(context).await;
            }
        });
    }
}

/// Start bot
pub async fn run<S: Sync + Send + Clone + 'static>(
    app: Arc<MystbotCore<S>>,
    mut scheduler: AsyncScheduler,
) {
    tokio::spawn(async move {
        loop {
            scheduler.run_pending().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    loop {
        let app = app.clone();
        let context = Context::new(app.client.clone(), app.state.clone());

        match app.client.next_update().await.unwrap() {
            Update::NewMessage(message) => {
                tokio::spawn(async move {
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
                            (multi.func)(context.clone(), message.clone()).await;
                            break;
                        }
                    }
                });
            }
            Update::CallbackQuery(query) => {
                tokio::spawn(async move {
                    if let Some(func) = app.callback_query {
                        let _ = func(context.clone(), query).await;
                    }
                });
            }
            Update::InlineQuery(query) => {
                tokio::spawn(async move {
                    if let Some(func) = app.inline_query {
                        let _ = func(context.clone(), query).await;
                    }
                });
            }
            Update::InlineSend(send) => {
                tokio::spawn(async move {
                    if let Some(func) = app.inline_send {
                        let _ = func(context.clone(), send).await;
                    }
                });
            }
            _ => {}
        }
    }
}
