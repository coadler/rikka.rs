use anyhow::Result;
use foundationdb::api::FdbApiBuilder;
use std::env;

use rikka_rs::{commands, Rikka};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> Result<()> {
    let network_builder = FdbApiBuilder::default()
        .build()
        .expect("fdb api initialized");
    let (runner, cond) = network_builder.build().expect("fdb network runners");

    let net_thread = std::thread::spawn(move || {
        unsafe { runner.run() }.expect("failed to run");
    });

    // Wait for the foundationDB network thread to start
    let fdb_network = cond.wait();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut r = Rikka::new(token).await?;

    if env::var("PROD").is_ok() {
        println!("prod mode");
        r.register_prefix("r.");
    } else {
        r.register_prefix("rt.");
    }
    // r.register_command(commands::general::Say {});
    r.register_command(commands::general::Ping {});
    r.register_command(commands::help::Help {});
    r.register_command(commands::played::Played::new().await);
    r.register_command(commands::logs::Logs::new().await?);

    let r = leak(r);
    println!("start");
    r.start().await?;
    println!("end");

    fdb_network.stop().expect("stop network");
    net_thread.join().expect("join fdb thread");
    Ok(())
}

fn leak<T>(cmd: T) -> &'static T {
    Box::leak(Box::new(cmd))
}
