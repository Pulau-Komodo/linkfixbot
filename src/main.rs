use std::fs;

use discord_event_handler::DiscordEventHandler;
use fix_existing_message::{FutureEmbedRemovals, FutureEmbedRemovalsTypeMap};
use serenity::all::GatewayIntents;

use crate::fix_link::LinkFixer;

mod automatic;
mod context_menu;
mod discord_event_handler;
mod fix_existing_message;
mod fix_link;
mod reply_shortcuts;
mod slash_command;
mod strings;
mod util;

#[tokio::main]
async fn main() {
	let link_fixer = LinkFixer::from_config();

	let discord_token = fs::read_to_string("./token.txt").expect("Could not read token file");

	let mut client = serenity::Client::builder(
		&discord_token,
		GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT,
	)
	.event_handler(DiscordEventHandler::new(link_fixer))
	.await
	.expect("Error creating Discord client");

	client
		.data
		.write()
		.await
		.insert::<FutureEmbedRemovalsTypeMap>(FutureEmbedRemovals::new());

	if let Err(why) = client.start().await {
		eprintln!("Error with client: {:?}", why);
	}
}
