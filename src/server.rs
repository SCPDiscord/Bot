use std::{env, net::SocketAddr, sync::Arc};

use color_eyre::Result;
use dashmap::DashMap;
use enum_map::Enum;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};

pub async fn create_server() -> Result<(
    Receiver<c2s::ServerEvent>,
    Arc<DashMap<SocketAddr, Sender<s2c::ClientEvent>>>,
)> {
    let binding_ip = env::var("HOST_IP").expect("Expected a HOST_IP in the environment");
    info!("Started listening in {}", binding_ip);
    let listener = TcpListener::bind(binding_ip).await?;
    let (tx, rx) = mpsc::channel::<c2s::ServerEvent>(100);
    let senders = Arc::new(DashMap::new());
    let senders_copy = senders.clone();

    tokio::spawn(async move {
        loop {
            let (socket, ip) = listener.accept().await.unwrap();
            debug!("Connected to {}", ip);
            let tx = tx.clone();
            let (bot_tx, bot_rx) = mpsc::channel(100);
            senders.insert(ip, bot_tx);

            tokio::spawn(async move {
                process(socket, ip, tx, bot_rx).await.unwrap();
            });
        }
    });

    Ok((rx, senders_copy))
}

async fn process(
    socket: TcpStream,
    ip: SocketAddr,
    tx: Sender<c2s::ServerEvent>,
    mut rx: Receiver<s2c::ClientEvent>,
) -> Result<()> {
    let mut stream = BufReader::new(socket);
    loop {
        tokio::select! {
            _ = stream.get_ref().readable() => {
                let Ok(length) = stream.read_u64().await else {
                    error!("Tried reading length of packet and failed to read it of {}", ip);
                    let event = s2c::ClientEvent::Drop(s2c::DropEvent {
                        reason: "Couldn't see length of packet in your message".to_string()
                    });
                    let vec = serde_json::to_vec(&event)?;
                    stream.write_u64(vec.len().try_into()?).await?;
                    stream.write(&vec).await?;
                    break;
                };

                let mut buffer = vec![0; length.try_into()?];
                stream.read_exact(&mut buffer).await?;
                let event: c2s::ServerEvent = serde_json::from_slice(&buffer)?;
                tx.send(event).await?;
            },
            Some(event) = rx.recv() => {
                let vec = serde_json::to_vec(&event)?;
                stream.write_u64(vec.len().try_into()?).await?;
                stream.write(&vec).await?;
                break;
            },
        }
    }

    Ok(())
}

// A packet will contain a u64 number in big-endian which will be the lenght of the message
// in bytes and a UTF-8 encoded JSON of `Event` in front of it

// Server to client packets
pub mod s2c {
    use serde::{Deserialize, Serialize};

    /// Events triggered in the plugin
    #[derive(Deserialize, Serialize, Debug)]
    pub enum ClientEvent {
        Command(CommandEvent),
        Drop(DropEvent),
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct CommandEvent {
        /// Unique ID given by bot to plugin to identify the command reply
        pub command_id: u64,
        pub command: String,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct DropEvent {
        pub reason: String,
    }
}

// Client to server packets
pub mod c2s {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug)]
    pub enum ServerEvent {
        Log(LogEvent),
        CommandReply(CommandReplyEvent),
        Status(StatusEvent),
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct CommandReplyEvent {
        /// Unique ID given by bot to plugin to identify the command reply
        pub command_id: u64,
        pub reply: super::Message,
        pub error: bool,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct LogEvent {
        pub message: super::Message,
        pub r#type: super::LogType,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct StatusEvent {
        /// What's the activity
        pub activity: Option<String>,
        /// https://docs.rs/serenity/0.11.6/serenity/model/user/enum.OnlineStatus.html
        pub status: serenity::model::user::OnlineStatus,
    }
}

/// If text, it will be formatted freely by the bot
#[derive(Deserialize, Serialize, Debug)]
pub enum Message {
    Text(String),
    /// https://docs.rs/serenity/0.11.6/serenity/model/prelude/struct.Embed.html
    Embed(serenity::model::prelude::Embed),
}

#[derive(Deserialize, Serialize, Debug, Enum)]
pub enum LogType {
    /// When a command gets executed
    Command,
    /// All stuff happening inside the server
    GameEvent,
    /// All stuff happening inside the server but with sensitive data
    GameEventSensitive,
    /// A banned player
    Ban,
    /// When a game report is sent
    Report,
    /// When a player gets disconnected specifically
    Disconnect,
}
