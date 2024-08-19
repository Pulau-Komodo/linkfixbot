use itertools::Itertools;
use serenity::{
	all::{Context, EventHandler, Interaction, Ready},
	async_trait,
};

use crate::fix_link::{create_command, fix_link};

pub struct DiscordEventHandler;

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		let Interaction::Command(interaction) = interaction else {
			return;
		};
		match interaction.data.name.as_str() {
			"fix link" => fix_link(&context, interaction).await,
			"blah" => (),
			_ => (),
		}
	}
	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		let commands = vec![create_command()];
		if Some("register") == arg.as_deref() {
			for guild in context.cache.guilds() {
				let commands = guild
					.set_commands(&context.http, commands.clone())
					.await
					.unwrap();
				let command_names = commands.into_iter().map(|command| command.name).join(", ");
				println!(
					"I now have the following commands in guild {}: {}",
					guild.get(),
					command_names
				);
			}
		}
	}
}