use thiserror::Error as EnumError;

pub type CommandResult = Result<Option<String>, CommandError>;

#[derive(EnumError, Debug)]
pub enum CommandError {
    #[error("command doesn't match")]
    NoMatch,
    #[error("{0:?}")]
    Library(&'static str),
    #[error(transparent)]
    TwilightHttp(#[from] twilight_http::Error),
    #[error(transparent)]
    CreateMessage(
        #[from] twilight_http::request::channel::message::create_message::CreateMessageError,
    ),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
