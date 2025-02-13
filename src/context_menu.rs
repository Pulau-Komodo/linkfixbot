use itertools::Itertools;
use serenity::all::*;

use crate::{fix_link::find_and_fix, reply_shortcuts::ReplyShortcuts};

pub async fn fix_links(context: &Context, interaction: CommandInteraction) {
	let Some(message) = interaction.data.resolved.messages.values().next() else {
		eprintln!("Did not find a message for some reason.");
		let _ = interaction
			.ephemeral_reply(&context.http, "Did not receive the message.")
			.await;
		return;
	};
	//println!("Parsing content: \"{}\"", message.content);
	let content = &message.content;
	let output = find_and_fix(content).join("\n");

	if output.is_empty() {
		let _ = interaction.ephemeral_reply(&context.http, "Found no links to fix. I only fix embed links for x.com, instagram.com and reddit.com, and unshort Youtube shorts links.").await;
		return;
	}

	let _ = interaction.public_reply(&context.http, output).await;
}

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix links")
		.description("")
		.kind(CommandType::Message)
		.contexts(vec![
			InteractionContext::Guild,
			InteractionContext::BotDm,
			InteractionContext::PrivateChannel,
		])
}
