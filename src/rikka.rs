use anyhow::Result;
use async_trait::async_trait;
use tokio::stream::StreamExt;

use twilight_cache_inmemory::{EventType, InMemoryCache};
use twilight_command_parser::{CommandParserConfig, Parser};
use twilight_gateway::cluster::{Cluster, ShardScheme};
use twilight_gateway::Event;
use twilight_http::Client as HttpClient;
use twilight_model::gateway::payload::request_guild_members::RequestGuildMembersBuilder;
use twilight_model::{channel::Message, gateway::Intents};

use crate::error::{CommandError, CommandResult};
use crate::help::CommandHelp;

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;

    fn help(&self, _: Option<&Message>) -> Vec<CommandHelp> {
        Vec::default()
    }

    async fn receive(&self, _: &Rikka, _: &Message) -> CommandResult {
        Ok(None)
    }

    async fn receive_raw(&self, _: &Rikka, _: &Event) -> Result<(), CommandError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct Rikka {
    pub(crate) cmds: Vec<&'static dyn Command>,

    pub(crate) cluster: Cluster,
    pub(crate) http: HttpClient,
    pub(crate) cache: InMemoryCache,

    pub(crate) prefix: String,
    pub(crate) parser: Parser<'static>,
}

impl Rikka {
    pub async fn new(token: String) -> Result<Self> {
        let cluster = Cluster::builder(
            &token,
            Intents::all() - Intents::GUILD_PRESENCES - Intents::GUILD_VOICE_STATES,
        )
        .shard_scheme(ShardScheme::Auto)
        .build()
        .await?;

        let cache = InMemoryCache::builder()
            .event_types(EventType::all() - EventType::PRESENCE_UPDATE)
            .build();

        Ok(Rikka {
            cmds: Vec::default(),

            cluster,
            http: HttpClient::new(&token),
            cache,
            prefix: "".into(),
            parser: Parser::new(CommandParserConfig::new()),
        })
    }

    pub fn register_command<T: Command + 'static>(&mut self, cmd: T) {
        let cfg = self.parser.config_mut();

        for help in cmd.help(None).iter() {
            cfg.add_command(help.name, false);
            for alias in help.aliases {
                println!("{}", *alias);
                cfg.add_command(*alias, false);
            }
        }

        self.cmds.push(leak(cmd));
    }

    pub fn register_prefix(&mut self, pre: impl Into<String>) {
        let pre = pre.into();
        self.prefix = pre.clone();
        self.parser.config_mut().add_prefix(pre.clone());
    }

    pub async fn start(&'static self) -> Result<()> {
        tokio::spawn(async move {
            println!("booting up shards...");
            self.cluster.up().await
        });

        let mut events = self.cluster.events();

        while let Some((shard, event)) = events.next().await {
            self.cache.update(&event);
            let event = Box::new(event);

            if event.kind().name().is_none() {
                dbg!(&event);
            }

            if let Event::GuildCreate(guild) = *event.clone() {
                tokio::spawn(async move {
                    let shard = self.cluster.shard(shard).unwrap();
                    shard
                        .command(&RequestGuildMembersBuilder::new(guild.id).query("", None))
                        .await
                        .map_err(|e| println!("request guild members: {}", e))
                        .ok();
                });
            };

            if let Event::MessageCreate(msg) = *event.clone() {
                for cmd in self.cmds.iter() {
                    let msg = (**msg).clone();

                    tokio::spawn(async move {
                        println!("send to cmd: {}", cmd.name());
                        let res = cmd.receive(self, &msg).await;
                        println!("end cmd: {}", cmd.name());
                        match res {
                            Ok(Some(res)) => {
                                self.http
                                    .create_message(msg.channel_id)
                                    .content(res)
                                    .unwrap()
                                    .await
                                    .map_err(|err| println!("respond to command: {}", err))
                                    .ok();
                            }
                            Ok(None) => {}
                            Err(CommandError::NoMatch) => {}
                            Err(err) => {
                                println!("command errored: {:?}", err);
                                self.http
                                    .create_message(msg.channel_id)
                                    .allowed_mentions()
                                    .build()
                                    .content(format!("```An error occured: {:?}```", err))
                                    .unwrap()
                                    .await
                                    .ok();
                            }
                        }
                    });
                }
            };

            for cmd in self.cmds.iter() {
                let event = event.clone();

                tokio::spawn(async move {
                    if let Err(err) = cmd.receive_raw(self, &event).await {
                        println!("raw event errored: {}", err)
                    }
                });
            }
        }

        Ok(())
    }

    // fn generate_help(&self) -> CreateMessage {
    //     let help = super::help::generate_help(&self.cmds);
    //     help
    // }
}

fn leak<T: Command>(cmd: T) -> &'static T {
    Box::leak(Box::new(cmd))
}

unsafe impl Send for Rikka {}
unsafe impl Sync for Rikka {}
