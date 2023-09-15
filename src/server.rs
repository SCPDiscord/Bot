use std::{env, net::SocketAddr};

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};

async fn create_server() -> Result<Receiver<ServerEvent>> {
    let binding_ip = env::var("HOST_IP").expect("Expected a HOST_IP in the environment");
    println!("Started listening in {}", binding_ip);
    let listener = TcpListener::bind(binding_ip).await?;
    let (tx, rx) = mpsc::channel::<ServerEvent>(100);

    tokio::spawn(async move {
        loop {
            let (socket, ip) = listener.accept().await.unwrap();
            println!("Connected to {}", ip);
            let tx = tx.clone();

            tokio::spawn(async move {
                process(socket, ip, tx).await.unwrap();
            });
        }
    });

    Ok(rx)
}

async fn process(socket: TcpStream, ip: SocketAddr, tx: Sender<ServerEvent>) -> Result<()> {
    let mut stream = BufReader::new(socket);
    loop {
        stream.get_ref().readable().await?;
        let Ok(length) = stream.read_u64().await else {
			eprintln!("Tried reading length of packet and failed to read it of {}", ip);
			let event = ClientEvent::Drop(DropEvent {
				reason: "Couldn't see length of packet in your message".to_string()
			});
			let vec = serde_json::to_vec(&event)?;
			stream.write_u64(vec.len().try_into()?).await?;
			stream.write(&vec).await?;
			break;
		};

        let mut buffer = vec![0; length.try_into()?];
        stream.read_exact(&mut buffer).await?;
        let event: ServerEvent = serde_json::from_slice(&buffer)?;
        tx.send(event).await?;
    }

    Ok(())
}

// A packet will contain a u64 number in big-endian which will be the lenght of the message
// in bytes and a UTF-8 encoded JSON of `Event` in front of it

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

/// Events triggered in the bot
#[derive(Deserialize, Serialize, Debug)]
pub enum ServerEvent {
    Log(LogEvent),
    CommandReply(CommandReplyEvent),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CommandReplyEvent {
    /// Unique ID given by bot to plugin to identify the command reply
    pub command_id: u64,
    pub reply: Message,
    pub error: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LogEvent {
    pub message: Message,
    pub r#type: LogType,
}

/// If text, it will be formatted freely by the bot
#[derive(Deserialize, Serialize, Debug)]
pub enum Message {
    Text(String),
    Embed(serenity::model::prelude::Embed),
}

#[derive(Deserialize, Serialize, Debug)]
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
