use itertools::Itertools;
use serenity::all::*;

use crate::{fix_link::find_and_fix, reply_shortcuts::ReplyShortcuts, strings::ERROR_NONE_FOUND};

pub async fn fix_links(context: &Context, interaction: CommandInteraction) {
	let Some(content) = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str())
	else {
		return;
	};
	let output = find_and_fix(content).map(|fix| fix.fixed).join("\n");

	if output.is_empty() {
		let _ = interaction
			.ephemeral_reply(&context.http, ERROR_NONE_FOUND)
			.await;
		return;
	}

	let _ = interaction.public_reply(&context.http, output).await;
}

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix")
		.description(
			"Replace all relevant links with alternatives that are embed-friendly or non-Youtube shorts, or had their tracking bloat stripped.",
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"links",
				"All the links to replace.",
			)
			.required(true),
		)
		.contexts(vec![
			InteractionContext::Guild,
			InteractionContext::BotDm,
			InteractionContext::PrivateChannel,
		])
}
