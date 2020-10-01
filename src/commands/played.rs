use crate::error::CommandResult;
use crate::parse::matches_command;
use crate::rikka::Rikka;
use anyhow::Result;
use anyhow::{Context, Error};
use async_trait::async_trait;
use chrono::Duration;
use chrono::Utc;
use rs_humanize::time;
use std::sync::Arc;
use twilight_embed_builder::{
    image_source::ImageSource, EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder,
    EmbedFooterBuilder,
};
use twilight_mention::ParseMention;
use twilight_model::channel::embed::Embed;
use twilight_model::channel::Message;
use twilight_model::id::UserId;
use twilight_model::user::User;

use crate::help::{CommandHelp, HelpSection};
use crate::rikka::Command;
use played_rs::Runner;

pub struct Played {
    c: &'static Runner,
}

const PLAYED_ALIAS: &[&'static str] = &["played"];

#[async_trait]
impl Command for Played {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        vec![CommandHelp {
            name: "played",
            section: HelpSection::Fun,
            ..Default::default()
        }]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        let mut args = matches_command(bot, msg, PLAYED_ALIAS)?;

        let uid = match args.next() {
            Some(uid) => {
                let parsed =
                    UserId::parse(uid).map_err(|e| Error::msg(format!("parse mention: {}", e)));

                match parsed {
                    Ok(uid) => uid,
                    Err(err) => UserId(uid.parse().map_err(|e| {
                        err.context(format!("parse id: {}", e)).context(format!(
                            "no parsing options left, expected mention or id. invalid option '{}'",
                            uid
                        ))
                    })?),
                }
            }
            None => msg.author.id,
        };

        let entries = self
            .c
            .read(uid.0.to_string())
            .await
            .context("read played entries")?;
        let user = bot.cache.user(uid).context("unknown user")?;

        if entries.games.len() == 0 {
            return Ok(Some(format!(
                "No entries found for {}#{}",
                &user.name, &user.discriminator
            )));
        }

        fn embed(user: &User, res: played_rs::Response) -> Result<Embed> {
            let mut games_str = String::new();
            for entry in &res.games {
                let dur = Duration::seconds(entry.dur as i64);
                games_str.push_str(&format!("• **{}** ", &entry.name));

                let hours = (dur.num_seconds() / 60) / 60;
                if hours > 0 {
                    games_str.push_str(&format!("{}h", hours))
                }

                let minutes = (dur.num_seconds() / 60) % 60;
                if minutes > 0 {
                    games_str.push_str(&format!("{}m", minutes))
                }

                let seconds = dur.num_seconds() % 60;
                if seconds > 0 {
                    games_str.push_str(&format!("{}s", seconds))
                }

                games_str.push_str("\n");
            }

            Ok(EmbedBuilder::new()
                .title(&user.name)?
                .description(format!(
                    "*First seen {}, last updated {}*",
                    time::format(res.first_seen),
                    time::format(res.last_updated),
                ))?
                .thumbnail(ImageSource::url(fmt_user_avatar(user))?)
                .timestamp(Utc::now().to_rfc3339())
                .field(EmbedFieldBuilder::new("Games", games_str)?)
                .color(0x79c879)?
                .build()?)
        }

        bot.http
            .create_message(msg.channel_id)
            .embed(embed(&user, entries)?)
            .context("set played embed")?
            .await
            .context("send played embed")?;

        Ok(None)
    }
}

impl Played {
    pub async fn new() -> Played {
        let fdb = foundationdb::Database::default().expect("open fdb");
        Played {
            c: Runner::new(fdb, ""),
        }
    }
}

fn fmt_user_avatar(usr: &User) -> String {
    let av = usr
        .avatar
        .as_ref()
        .map(|av| format!("{}/{}", usr.id, av))
        .unwrap_or((usr.discriminator.parse::<u16>().unwrap() % 5).to_string());
    let ext = if av.starts_with("a_") { "gif" } else { "png" };

    format!("https://cdn.discordapp.com/avatars/{}.{}", av, ext)
}
