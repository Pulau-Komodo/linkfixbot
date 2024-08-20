use itertools::Itertools;
use serenity::all::*;

use crate::{fix_link::find_and_fix, reply_shortcuts::ReplyShortcuts};

pub async fn fix_links(context: &Context, interaction: CommandInteraction) {
	let Some(content) = interaction.data.options.first().and_then(|option| option.value.as_str()) else {
		return;
	};
	let output = find_and_fix(content).join("\n");

	if output.is_empty() {
		let _ = interaction.ephemeral_reply(&context.http, "Found no links to fix. I only fix embed links for x.com, instagram.com and reddit.com, and unshort Youtube shorts links.").await;
		return;
	}

	let _ = interaction.public_reply(&context.http, output).await;
}

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix")
		.description(
			"Replace all relevant links with embed-friendly or non-Youtube shorts alternatives.",
		)
		.add_option(CreateCommandOption::new(
			CommandOptionType::String,
			"links",
			"All the links to replace.",
		).required(true))
}
