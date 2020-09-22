use std::env;
use std::error::Error;

use rikka_rs::{commands, Rikka};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut r = Rikka::new(token).await?;

    r.register_prefix("rt.");
    r.register_command(commands::general::Say {});
    r.register_command(commands::general::Ping {});
    r.register_command(commands::general::Log {});
    r.register_command(commands::help::Help {});

    let r = leak(r);
    println!("start");
    r.start().await?;
    println!("end");
    Ok(())
}

fn leak<T>(cmd: T) -> &'static T {
    Box::leak(Box::new(cmd))
}
