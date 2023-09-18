mod commands;
mod server;

use std::cell::OnceCell;
use std::collections::VecDeque;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::Result;
use dashmap::DashMap;
use enum_map::enum_map;
use log::{error, info};
use serenity::async_trait;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::prelude::{Activity, ActivityType};
use serenity::prelude::*;
use server::{create_server, s2c, c2s};
use tokio::sync::{self, mpsc::Receiver};

struct CommandExecutionWaiter;
impl TypeMapKey for CommandExecutionWaiter {
    type Value = Arc<DashMap<u64, sync::oneshot::Sender<c2s::CommandReplyEvent>>>;
}

struct ServerConnection;
impl TypeMapKey for ServerConnection {
    type Value = Arc<DashMap<SocketAddr, sync::mpsc::Sender<s2c::ClientEvent>>>;
}

struct Handler {
    rx_server: Arc<Mutex<OnceCell<Receiver<c2s::ServerEvent>>>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            info!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                // "ping" => commands::ping::run(&command.data.options),
                // "id" => commands::id::run(&command.data.options),
                // "attachmentinput" => commands::attachmentinput::run(&command.data.options),
                _ => "not implemented :(".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                error!("Cannot respond to slash command: {}", why);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId(
            env::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let commands = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands
            // .create_application_command(|command| commands::ping::register(command))
            // .create_application_command(|command| commands::id::register(command))
            // .create_application_command(|command| commands::welcome::register(command))
            // .create_application_command(|command| commands::numberinput::register(command))
            // .create_application_command(|command| commands::attachmentinput::register(command))
        })
        .await;

        info!(
            "I now have the following guild slash commands: {:#?}",
            commands
        );

        let guild_command = Command::create_global_application_command(&ctx.http, |command| {
            commands::execute::register(command)
        })
        .await;

        info!(
            "I created the following global slash command: {:#?}",
            guild_command
        );

        let rx_server = self.rx_server.clone();
        tokio::spawn(async move {
            let mut rx = {
                let mut lock = rx_server.lock().unwrap();
                lock.take().unwrap()
            };
            let mut log_deqs = enum_map! {
                _ => VecDeque::new()
            };
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        match event {
                            c2s::ServerEvent::Status(status) => {
                                ctx.set_presence(
                                    status.activity.map(|name| {
                                        let mut activity = Activity::playing("~");
                                        activity.state = Some(name);
                                        activity.kind = ActivityType::Custom;
                                        activity
                                    }),
                                    status.status,
                                )
                                .await
                            },
                            c2s::ServerEvent::Log(log) => {
                                log_deqs[log.r#type].push_back(log.message)
                            },
                            c2s::ServerEvent::CommandReply(_) => todo!("Haven't done command execution yet")
                        }
                    },
                    _ = interval.tick() => {
                        for (log_type, deq) in log_deqs.iter_mut() {
							todo!("Get log channel")
                        }
                    }
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    pretty_env_logger::init();
    if let Err(_) = dotenvy::dotenv() {
        error!("Couldn't find .env file, using environment variables directly.")
    }
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a DISCORD_TOKEN in the environment");

    let (rx_server, connected_servers) = create_server().await?;
    let cell = OnceCell::new();
    cell.set(rx_server).expect("Couldn't set Receiver");
    let handler = Handler {
        rx_server: Arc::new(Mutex::new(cell)),
    };

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(handler)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;

        data.insert::<ServerConnection>(connected_servers);
        data.insert::<CommandExecutionWaiter>(Arc::new(DashMap::new()));
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    client.start().await?;
    Ok(())
}
