use super::Rikka;
use crate::error::CommandError;
use twilight_command_parser::{Arguments, Command};
use twilight_model::channel::Message;

pub fn matches_command<'a>(
    bot: &'a Rikka,
    msg: &'a Message,
    cmds: &[&'static str],
) -> Result<Arguments<'a>, CommandError> {
    if msg.author.bot {
        return Err(CommandError::NoMatch);
    }

    let found = bot
        .parser
        .parse(&msg.content)
        .ok_or(CommandError::NoMatch)?;

    match found {
        Command {
            name, arguments, ..
        } if cmds.contains(&name) => Ok(arguments),
        _ => Err(CommandError::NoMatch),
    }
}
