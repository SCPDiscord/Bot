mod commands;
mod server;

use std::cell::OnceCell;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use color_eyre::Result;
use dashmap::DashMap;
use log::{error, info};
use serenity::async_trait;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::prelude::Activity;
use serenity::prelude::*;
use server::{create_server, ClientEvent, ServerEvent};
use tokio::sync::mpsc::{Receiver, Sender};

struct Handler {
    rx_server: Arc<Mutex<OnceCell<Receiver<ServerEvent>>>>,
    connected_servers: Arc<DashMap<SocketAddr, Sender<ClientEvent>>>,
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
            let rx = {
                let mut lock = rx_server.lock().unwrap();
                lock.take().unwrap()
            };
            loop {
                let Some(event) = rx.recv().await else { continue };
                match event {
                    ServerEvent::Status(status) => {
                        ctx.set_presence(
                            status.activity.map(|name| Activity::playing(name)),
                            status.status,
                        )
                        .await
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
    let handler = Handler {
        rx_server: Arc::new(Mutex::new(rx_server)),
        connected_servers,
    };

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(handler)
        .await
        .expect("Error creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    client.start().await?;
    Ok(())
}
