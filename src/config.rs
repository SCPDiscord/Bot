use enum_map::EnumMap;
use serde::{Deserialize, Serialize};
use serenity::{model::prelude::ChannelId, utils::{token::InvalidToken, validate_token}};

use crate::server::LogType;

#[derive(Deserialize, Serialize, Debug)]
pub struct BotConfig {
	pub server_id: BotId,
	pub channels: EnumMap<LogType, Option<ChannelId>>,
	pub token: Token,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BotId(String);

#[derive(Deserialize, Serialize, Debug)]
#[serde(try_from = "String")]
pub struct Token(String);

impl TryFrom<String> for Token {
    type Error = InvalidToken;

    fn try_from(value: String) -> Result<Self, Self::Error> {
		validate_token(&value).map(|_| Token(value))
    }
}
