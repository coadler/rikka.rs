use crate::error::CommandResult;
use crate::parse::matches_command;
use crate::rikka::Rikka;
use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use twilight_model::channel::Message;

use crate::help::CommandHelp;
use crate::rikka::Command;

pub struct Ping;

const PING_ALIAS: &[&'static str] = &["ping"];

#[async_trait]
impl Command for Ping {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        vec![CommandHelp {
            name: "ping",
            ..Default::default()
        }]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        matches_command(bot, msg, PING_ALIAS)?;

        let start = Utc::now();
        let msg = bot
            .http
            .create_message(msg.channel_id)
            .content("Pong!")
            .context("add content")?
            .await
            .context("send message")?;

        bot.http
            .update_message(msg.channel_id, msg.id)
            .content(format!(
                "Pong! - `{}ms`",
                Utc::now().signed_duration_since(start).num_milliseconds()
            ))
            .context("set content")?
            .await
            .context("update message")?;

        Ok(None)
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
