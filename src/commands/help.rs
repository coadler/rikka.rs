use crate::error::CommandResult;
use crate::help::generate_help;
use crate::parse::matches_command;
use crate::rikka::Rikka;
use async_trait::async_trait;
use twilight_model::channel::Message;

use crate::help::CommandHelp;
use crate::rikka::Command;

pub struct Help;

const HELP_ALIAS: &[&'static str] = &["help"];

#[async_trait]
impl Command for Help {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        let mut cmd = CommandHelp::default();
        cmd.name = "help";
        vec![cmd]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        matches_command(bot, msg, HELP_ALIAS)?;
        let embed = generate_help(bot)?;

        bot.http
            .create_message(msg.channel_id)
            .embed(embed)?
            .await?;
        Ok(None)
    }
}
