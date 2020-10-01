use thiserror::Error as EnumError;

pub type CommandResult = Result<Option<String>, CommandError>;

#[derive(EnumError, Debug)]
pub enum CommandError {
    #[error("command doesn't match")]
    NoMatch,
    #[error(transparent)]
    Generic(#[from] anyhow::Error),
}
