use super::Rikka;
use anyhow::Result;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, IntoStaticStr};
use twilight_embed_builder::{
    image_source::ImageSource, EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder,
};
use twilight_model::channel::embed::Embed;

#[derive(EnumIter, IntoStaticStr, AsRefStr, PartialEq, PartialOrd)]
pub enum HelpSection {
    General,
    Info,
    Moderation,
    Owner,
}

impl Default for HelpSection {
    fn default() -> Self {
        HelpSection::General
    }
}

#[derive(Default)]
pub struct CommandHelp {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub section: HelpSection,
    pub description: &'static str,
    pub usage: &'static str,
    pub detailed: &'static str,
    pub examples: &'static [&'static str],
}

pub fn generate_help<'a>(bot: &Rikka) -> Result<Embed> {
    let av_url = String::from("https://cdn.discordapp.com/avatars/319571495666057227/b14a77bf6f87d2ccc4a9c2d4e52cfe4b.webp?size=1024");
    let mut embed = EmbedBuilder::new()
        .author(
            EmbedAuthorBuilder::new()
                .name("Rikka v3 Command Help")?
                .url("https://github.com/coadler/rikka.rs")
                .icon_url(ImageSource::url(&av_url)?)
                .build(),
        )
        .thumbnail(ImageSource::url(av_url)?)
        .title("Join our server for more information")?
        .url("https://discord.gg/Na6knqq")
        .description(&format!(
            "Type `{}help [command]` for detailed usage information",
            &bot.prefix
        ))?;

    let mut buf = String::new();

    for sect in HelpSection::iter() {
        for cmd in &bot.cmds {
            for help in cmd.help(None) {
                if help.section == sect {
                    if buf.len() > 0 {
                        buf.push_str(", ");
                    }

                    buf.push_str(&format!("`{}`", help.name));
                }
            }
        }

        if buf.len() > 0 {
            embed = embed.field(EmbedFieldBuilder::new(sect.as_ref(), &buf)?);
            buf.clear();
        }
    }

    Ok(embed.build()?)
}
