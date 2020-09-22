use crate::error::CommandResult;
use crate::parse::matches_command;
use crate::rikka::Rikka;
use async_trait::async_trait;
use twilight_model::channel::Message;

use crate::help::CommandHelp;
use crate::rikka::Command;

pub struct Ping;

const PING_ALIAS: &[&'static str] = &["ping"];

#[async_trait]
impl Command for Ping {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        let mut cmd = CommandHelp::default();
        cmd.name = "ping";
        vec![cmd]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        matches_command(bot, msg, PING_ALIAS)?;

        Ok(Some("Pong!".into()))
    }
}

pub struct Say;

const SAY_ALIAS: &[&'static str] = &["say"];

#[async_trait]
impl Command for Say {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        let mut cmd = CommandHelp::default();
        cmd.name = "say";
        vec![cmd]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        let args = matches_command(bot, msg, SAY_ALIAS)?;

        dbg!(msg);

        Ok(Some(format!("you said \"{}\"", args.as_str())))
    }
}

pub struct Log;

#[async_trait]
impl Command for Log {
    async fn receive(&self, _: &Rikka, _: &Message) -> CommandResult {
        Ok(None)
    }
}
