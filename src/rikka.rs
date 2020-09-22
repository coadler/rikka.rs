use anyhow::Result;
use async_trait::async_trait;
use tokio::stream::StreamExt;

use twilight_cache_inmemory::InMemoryCache;
use twilight_command_parser::{CommandParserConfig, Parser};
use twilight_gateway::{
    cluster::{Cluster, ShardScheme},
    Event,
};
use twilight_http::Client as HttpClient;
use twilight_model::{channel::Message, gateway::Intents};

use crate::error::{CommandError, CommandResult};
use crate::help::CommandHelp;

#[async_trait]
pub trait Command: Send + Sync {
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
        let cluster = Cluster::builder(&token)
            .shard_scheme(ShardScheme::Auto)
            // Use intents to only receive guild message events.
            .intents(Intents::GUILD_MESSAGES)
            .build()
            .await?;

        let cache = InMemoryCache::builder().build();

        Ok(Rikka {
            cmds: Vec::default(),

            cluster,
            http: HttpClient::new(&token),
            cache,
            prefix: "".into(),
            parser: Parser::new(CommandParserConfig::new()),
        })
    }

    pub async fn register_command<T: Command + 'static>(&mut self, cmd: T) {
        let cfg = self.parser.config_mut();

        for help in cmd.help(None).iter() {
            cfg.add_command(help.name, false);
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

        while let Some((_, event)) = events.next().await {
            self.cache.update(&event);
            let event = Box::new(event);

            if let Event::MessageCreate(msg) = *event.clone() {
                for cmd in self.cmds.iter() {
                    let msg = (**msg).clone();

                    tokio::spawn(async move {
                        let res = cmd.receive(self, &msg).await;
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
                            Err(err) => println!("command errored: {}", err),
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
