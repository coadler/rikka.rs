use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use async_trait::async_trait;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, TimeZone, Utc};
use foundationdb::Database;
use foundationdb::{FdbResult, TransactOption};
use futures::FutureExt;
use rusoto_core::{
    credential::{AwsCredentials, DefaultCredentialsProvider, ProvideAwsCredentials},
    ByteStream,
};
use rusoto_s3::{PutObjectRequest, S3Client, S3};
use rusoto_signature::region::Region;
use std::sync::Arc;
use twilight_cache_inmemory::model::CachedGuild;
use twilight_command_parser::Arguments;
use twilight_embed_builder::{
    image_source::ImageSource, EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder,
    EmbedFooterBuilder,
};
use twilight_mention::ParseMention;
use twilight_model::channel::{GuildChannel, Message};
use twilight_model::gateway::{
    event::Event,
    payload::{MessageDelete, MessageUpdate},
};
use twilight_model::id::{AttachmentId, ChannelId, GuildId, MessageId};
use twilight_model::user::User;

use super::adapter::Adapter;
use crate::error::{CommandError, CommandResult};
use crate::help::{CommandHelp, HelpSection};
use crate::parse::matches_command;
use crate::rikka::Command;
use crate::rikka::Rikka;

pub struct Logs {
    fdb: Database,

    s3_creds: AwsCredentials,
    s3_region: Region,
    s3: S3Client,

    pub nonce: String,
}

impl Logs {
    pub async fn new() -> Result<Logs> {
        let nonce = std::env::var("LOG_HASH_NONCE")?;

        let fdb = foundationdb::Database::default().expect("open fdb");
        let s3_creds = DefaultCredentialsProvider::new()?.credentials().await?;
        let s3_region = Region::Custom {
            name: "b2-usw".into(),
            endpoint: "s3.us-west-000.backblazeb2.com".into(),
        };
        let s3 = S3Client::new(s3_region.clone());

        Ok(Logs {
            fdb,
            s3_creds,
            s3_region,
            s3,
            nonce,
        })
    }
}

const LOGS_ALIAS: &[&'static str] = &["logs", "log"];

#[async_trait]
impl Command for Logs {
    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        vec![CommandHelp {
            name: "log",
            aliases: &["logs"],
            section: HelpSection::Moderation,
            ..Default::default()
        }]
    }

    async fn receive(&self, bot: &Rikka, msg: &Message) -> CommandResult {
        let mut args = matches_command(bot, msg, LOGS_ALIAS)?;
        const LOG_TYPES: [&'static str; 2] = ["message", "help"];

        Ok(match args.next() {
            Some("message") | Some("messages") => {
                self.handle_messages_command(bot, msg, args).await?
            }
            Some("help") | None => Some("help coming soon".to_owned()),
            Some(_) => Some(format!("Unknown option. Expected one of {:?}", LOG_TYPES)),
        })
    }

    async fn receive_raw(&self, bot: &Rikka, ev: &Event) -> Result<(), CommandError> {
        match ev {
            Event::MessageCreate(ref m) => self.store_message(bot, m).await?,
            Event::MessageUpdate(ref m) => self.log_update(bot, m).await?,
            Event::MessageDelete(ref m) => self.log_delete(bot, m).await?,
            _ => {}
        }

        Ok(())
    }
}

impl Logs {
    async fn handle_messages_command<'a>(
        &self,
        bot: &Rikka,
        msg: &Message,
        mut args: Arguments<'a>,
    ) -> CommandResult {
        const LOG_OPTIONS: [&str; 2] = ["enable", "disable"];

        if msg.author.id.0 != 105484726235607040 {
            return Ok(Some(
                "Only owners may use this command for now :)".to_owned(),
            ));
        }

        Ok(match args.next() {
            Some("enable") => self.handle_messages_enable_command(bot, msg, args).await?,
            Some("disable") => None,
            _ => Some(format!("Unknown option. Expected one of {:?}", LOG_OPTIONS)),
        })
    }
    async fn handle_messages_enable_command<'a>(
        &self,
        bot: &Rikka,
        msg: &Message,
        mut args: Arguments<'a>,
    ) -> CommandResult {
        let cid = match args.next().clone() {
            Some(cid) => ChannelId::parse(cid)
                .map_err(|e| Error::msg(format!("{}", e)))
                .context("parse channel id")?,
            None => msg.channel_id,
        };
        let ch = bot
            .cache
            .guild_channel(cid)
            .ok_or(anyhow!("channel not found in cache: {}", cid))?;
        let gid = msg
            .guild_id
            .ok_or(Error::msg("message didn't have guild id"))?;

        self.enable_messages(&gid, &cid).await?;

        Ok(Some(format!("Enabled message logs in {}", ch.name())))
    }

    async fn store_message(&self, bot: &Rikka, msg: &Message) -> Result<()> {
        if self
            .messages_enabled(&msg.guild_id.unwrap_or_default())
            .await?
            .is_none()
        {
            return Ok(());
        }

        for att in &msg.attachments {
            let res = reqwest::get(&att.proxy_url).await?;
            self.s3
                .put_object(PutObjectRequest {
                    bucket: "rikka-files".to_string(),
                    key: fmt_attachment_key(&msg.id, &att.id),
                    content_type: res
                        .headers()
                        .get("Content-Type")
                        .map(|k| k.to_str().unwrap().to_string()),
                    content_length: res.content_length().map(|i| i as i64),
                    body: Some(ByteStream::new(Adapter::new(res.bytes_stream()))),
                    ..Default::default()
                })
                .await?;
        }

        self.write_msg(msg).await?;
        Ok(())
    }

    async fn log_update(&self, bot: &Rikka, msg_u: &MessageUpdate) -> Result<()> {
        let cid = match self
            .messages_enabled(&msg_u.guild_id.unwrap_or_default())
            .await?
        {
            Some(id) => id,
            None => return Ok(()),
        };

        let msg = match self.get_message(&msg_u.id).await? {
            Some(msg) => msg,
            None => return Ok(()),
        };

        let new_content = msg_u.content.clone().unwrap_or(String::new());

        if msg.content == new_content {
            return Ok(());
        }

        let (chan, guild) = channel_and_guild(bot, cid)?;

        let embed = EmbedBuilder::new()
            .title("Message Update")?
            .thumbnail(ImageSource::url(fmt_user_avatar(&msg.author))?)
            .timestamp(Utc::now().to_rfc3339())
            .footer(
                EmbedFooterBuilder::new(&guild.name)?
                    .icon_url(ImageSource::url(fmt_guild_icon(&*guild))?),
            )
            .field(EmbedFieldBuilder::new(
                "User",
                format!(
                    "<@{}> {}#{} {}",
                    msg.author.id, msg.author.name, msg.author.discriminator, msg.author.id
                ),
            )?)
            .field(EmbedFieldBuilder::new(
                "User",
                format!("<#{}> {}", chan.id(), chan.id()),
            )?)
            .field(EmbedFieldBuilder::new("Old content", &msg.content)?)
            .field(EmbedFieldBuilder::new("New content", new_content)?);

        bot.http.create_message(cid).embed(embed.build()?)?.await?;

        Ok(())
    }

    async fn log_delete(&self, bot: &Rikka, msg_d: &MessageDelete) -> Result<()> {
        let cid = match self.messages_enabled(&msg_d.guild_id.unwrap()).await? {
            Some(id) => id,
            None => return Ok(()),
        };

        let msg = match self.get_message(&msg_d.id).await? {
            Some(msg) => msg,
            None => return Ok(()),
        };

        for a in &msg.attachments {
            bot.http
                .create_message(cid)
                .content(fmt_attachment_url(&msg_d.id, &a.id))?
                .await?;
        }

        let (chan, guild) = channel_and_guild(bot, cid)?;

        let mut embed = EmbedBuilder::new()
            .title("Message Deleted")?
            .thumbnail(ImageSource::url(fmt_user_avatar(&msg.author))?)
            .timestamp(Utc::now().to_rfc3339())
            .footer(
                EmbedFooterBuilder::new(&guild.name)?
                    .icon_url(ImageSource::url(fmt_guild_icon(&*guild))?),
            )
            .field(EmbedFieldBuilder::new(
                "User",
                format!(
                    "<@{}> {}#{} {}",
                    msg.author.id, msg.author.name, msg.author.discriminator, msg.author.id
                ),
            )?)
            .field(EmbedFieldBuilder::new(
                "User",
                format!("<#{}> {}", chan.id(), chan.id()),
            )?);

        if !msg.content.is_empty() {
            embed = embed.field(EmbedFieldBuilder::new("Deleted content", msg.content)?);
        }

        bot.http.create_message(cid).embed(embed.build()?)?.await?;

        Ok(())
    }

    async fn write_msg(&self, msg: &Message) -> Result<()> {
        #[inline]
        async fn exec(t: &foundationdb::Transaction, msg: &Message) -> FdbResult<()> {
            let msg_raw = serde_cbor::to_vec(msg).unwrap();
            t.set(fmt_msg_key(&msg.id).as_slice(), &msg_raw);
            Ok(())
        }

        self.fdb
            .transact_boxed(
                msg,
                |tx, msg| exec(tx, msg).boxed(),
                TransactOption::default(),
            )
            .await?;

        Ok(())
    }

    async fn enable_messages(&self, gid: &GuildId, cid: &ChannelId) -> Result<()> {
        #[inline]
        async fn exec(
            t: &foundationdb::Transaction,
            ids: &(&GuildId, &ChannelId),
        ) -> FdbResult<()> {
            let (gid, cid) = ids;
            let mut cid_raw = [0u8; 8];
            LittleEndian::write_u64(&mut cid_raw, cid.0);

            t.set(&fmt_messages_enabled_key(gid), &cid_raw);
            Ok(())
        }

        self.fdb
            .transact_boxed(
                (gid, cid),
                |tx, ids| exec(tx, ids).boxed(),
                TransactOption::default(),
            )
            .await?;

        Ok(())
    }

    async fn messages_enabled(&self, gid: &GuildId) -> Result<Option<ChannelId>> {
        #[inline]
        async fn exec(
            t: &foundationdb::Transaction,
            gid: &GuildId,
        ) -> FdbResult<Option<ChannelId>> {
            let ch = t.get(&fmt_messages_enabled_key(gid), true).await?;
            match ch {
                Some(cid) => Ok(Some(LittleEndian::read_u64(&cid).into())),
                None => Ok(None),
            }
        }

        let ch = self
            .fdb
            .transact_boxed(
                gid,
                |tx, gid| exec(tx, gid).boxed(),
                TransactOption::default(),
            )
            .await?;

        Ok(ch)
    }

    async fn get_message(&self, mid: &MessageId) -> Result<Option<Message>> {
        #[inline]
        async fn exec(
            t: &foundationdb::Transaction,
            mid: &MessageId,
        ) -> FdbResult<Option<Message>> {
            let msg = t.get(&fmt_msg_key(mid), true).await?;
            match msg {
                Some(msg) => Ok(serde_cbor::from_slice::<Message>(&msg).unwrap().into()),
                None => Ok(None),
            }
        }

        let ch = self
            .fdb
            .transact_boxed(
                mid,
                |tx, mid| exec(tx, mid).boxed(),
                TransactOption::default(),
            )
            .await?;

        Ok(ch)
    }
}

fn channel_and_guild(bot: &Rikka, cid: ChannelId) -> Result<(Arc<GuildChannel>, Arc<CachedGuild>)> {
    let chan = bot
        .cache
        .guild_channel(cid)
        .ok_or(anyhow!("channel not found in cache: {}", cid))?;
    let gid = chan.guild_id().ok_or(Error::msg(format!(
        "channel doesn't contain guild id: {}",
        cid
    )))?;
    let guild = bot
        .cache
        .guild(gid)
        .ok_or(anyhow!("guild not found in cache: {}", gid))?;

    Ok((chan, guild))
}

use foundationdb::tuple;

const SUBSPACE_PREFIX: &[u8] = b"logs";

enum Subspace {
    MessagesEnabled = 1,
    MessageLog = 2,
}

fn fmt_messages_enabled_key(id: &GuildId) -> Vec<u8> {
    tuple::Subspace::all()
        .subspace(&SUBSPACE_PREFIX)
        .subspace(&(Subspace::MessagesEnabled as u16))
        .pack(&id.0)
}

fn fmt_msg_key(mid: &MessageId) -> Vec<u8> {
    tuple::Subspace::all()
        .subspace(&SUBSPACE_PREFIX)
        .subspace(&(Subspace::MessageLog as u16))
        .pack(&mid.0)
}

fn fmt_attachment_key(mid: &MessageId, aid: &AttachmentId) -> String {
    format!("{}/{}", mid, aid)
}

fn fmt_attachment_url(mid: &MessageId, aid: &AttachmentId) -> String {
    format!("https://files.rikka.xyz/{}", fmt_attachment_key(mid, aid))
}

use std::convert::TryInto;

fn id_to_timestamp(id: u64) -> DateTime<Utc> {
    let ms = ((id >> 22) + 1420070400000) as i64;
    let s: i64 = ms / 1000;
    let ns: u32 = ((ms % 1000) * 1e6 as i64).try_into().unwrap();

    Utc.timestamp(s, ns)
}

fn fmt_guild_icon(g: &CachedGuild) -> String {
    let icon = g.icon.as_ref().map(|i| i.to_owned()).unwrap_or("".into());
    let ext = if icon.starts_with("a_") { "gif" } else { "png" };
    format!("https://cdn.discordapp.com/icons/{}/{}.{}", g.id, icon, ext)
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
