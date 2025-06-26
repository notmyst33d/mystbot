// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{InputMessage, InvocationError};

pub trait WithMessage {
    fn input_message(self) -> Option<InputMessage>;
}

pub enum Error {
    WithMessage(Box<InputMessage>),
    InvocationError(InvocationError),
    Other,
}

impl WithMessage for Error {
    fn input_message(self) -> Option<InputMessage> {
        match self {
            Self::WithMessage(m) => Some(*m),
            _ => None,
        }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::WithMessage(_) => write!(
                f,
                "Error::WithMessage(cannot display message because it doesnt derive Debug :()"
            ),
            Self::InvocationError(e) => write!(f, "Error::InvocationError({e})"),
            Self::Other => write!(f, "Error::Other"),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::WithMessage(_) => write!(
                f,
                "Error::WithMessage(cannot display message because it doesnt derive Debug :()"
            ),
            Self::InvocationError(e) => write!(f, "Error::InvocationError({e})"),
            Self::Other => write!(f, "Error::Other"),
        }
    }
}

impl std::error::Error for Error {}

impl From<InvocationError> for Error {
    fn from(value: InvocationError) -> Self {
        Self::InvocationError(value)
    }
}
