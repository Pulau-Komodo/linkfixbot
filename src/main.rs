use std::fs;

use discord_event_handler::DiscordEventHandler;
use serenity::all::GatewayIntents;

mod context_menu;
mod discord_event_handler;
mod fix_link;
mod reply_shortcuts;
mod slash_command;

#[tokio::main]
async fn main() {
	let discord_token = fs::read_to_string("./token.txt").expect("Could not read token file");

	let mut client = serenity::Client::builder(&discord_token, GatewayIntents::empty())
		.event_handler(DiscordEventHandler)
		.await
		.expect("Error creating Discord client");

	if let Err(why) = client.start().await {
		eprintln!("Error with client: {:?}", why);
	}
}
