use std::env;

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender}, io,
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
                process(socket, tx).await.unwrap();
            });
        }
    });

    Ok(rx)
}

async fn process(socket: TcpStream, tx: Sender<ServerEvent>) -> Result<()> {
	let mut msg = vec![0; 1024];
	let len = 0u64;
    loop {
		socket.readable().await?;

		// Try to read data, this may still fail with `WouldBlock`
        // if the readiness event is a false positive.
        match socket.try_read(&mut msg) {
            Ok(0) => break,
            Ok(n) => {
                if len == 0 && n >= 8 {

				}
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
	}

    Ok(())
}

// A packet will contain a u64 number which will be the lenght of the message
// in bytes and a UTF-8 encoded JSON of `Event` in front of it

/// Events triggered in the plugin
#[derive(Deserialize, Serialize, Debug)]
enum ClientEvent {
    Command(CommandEvent),
}

#[derive(Deserialize, Serialize, Debug)]
struct CommandEvent {
    /// Unique ID given by bot to plugin to identify the command reply
    command_id: u64,
    command: String,
}

/// Events triggered in the bot
#[derive(Deserialize, Serialize, Debug)]
enum ServerEvent {
    Log(LogEvent),
    CommandReply(CommandReplyEvent),
}

#[derive(Deserialize, Serialize, Debug)]
struct CommandReplyEvent {
    /// Unique ID given by bot to plugin to identify the command reply
    command_id: u64,
    reply: Message,
    error: bool,
}

#[derive(Deserialize, Serialize, Debug)]
struct LogEvent {
    message: Message,
    r#type: LogType,
}

/// If text, it will be formatted freely by the bot
#[derive(Deserialize, Serialize, Debug)]
enum Message {
    Text(String),
    Embed(serenity::model::prelude::Embed),
}

#[derive(Deserialize, Serialize, Debug)]
enum LogType {
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
