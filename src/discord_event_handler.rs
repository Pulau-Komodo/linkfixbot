use itertools::Itertools;
use serenity::{
	all::{Command, Context, EventHandler, Interaction, Message, MessageUpdateEvent, Ready},
	async_trait,
};

use crate::{
	automatic, context_menu,
	fix_existing_message::{
		handle_bot_message_embed_generation, handle_user_message_embed_generation,
	},
	slash_command,
};

pub struct DiscordEventHandler;

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		let Interaction::Command(interaction) = interaction else {
			return;
		};
		match interaction.data.name.as_str() {
			"fix links" => context_menu::fix_links(&context, interaction).await,
			"fix" => slash_command::fix_links(&context, interaction).await,
			_ => (),
		}
	}
	async fn message(&self, context: Context, message: Message) {
		if !message.author.bot {
			automatic::fix_links(&context, &message).await;
		}
	}
	async fn message_update(
		&self,
		context: Context,
		_old: Option<Message>,
		_new: Option<Message>,
		event: MessageUpdateEvent,
	) {
		let Some(user) = &event.author else {
			return;
		};
		// println!(
		// 	"Edit event: {}, channel: {}, discord: {:?}, user: {:?}",
		// 	event.id,
		// 	event.channel_id,
		// 	event.guild_id,
		// 	event.author.as_ref().map(|a| a.id)
		// );
		if *user == **context.cache.current_user() {
			println!("Own message");
			handle_bot_message_embed_generation(&context, &event).await;
		} else if event
			.embeds
			.as_ref()
			.map(|embeds| !embeds.is_empty())
			.unwrap_or(false)
		{
			println!("Other user's message with embeds.");
			handle_user_message_embed_generation(&context, &event).await;
		}
	}
	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		maybe_register_commands(&context).await;
	}
}

/// Registers commands depending on the arguments passed to the executable.
async fn maybe_register_commands(context: &Context) {
	let (arg, arg2) = {
		let mut args = std::env::args();
		(args.nth(1), args.next())
	};
	if Some("register") == arg.as_deref() {
		let commands = vec![
			context_menu::create_command(),
			slash_command::create_command(),
		];
		if Some("global") == arg2.as_deref() {
			let resulting_commands = Command::set_global_commands(&context.http, commands.clone())
				.await
				.unwrap();
			let command_names = resulting_commands
				.into_iter()
				.map(|command| command.name)
				.join(", ");
			println!(
				"I now have the following global commands: {}",
				command_names
			);
			for guild in context.cache.guilds() {
				let _ = guild.set_commands(&context.http, Vec::new()).await.unwrap();
				println!("Cleared the commands from guild {}", guild.get());
			}
		} else {
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
