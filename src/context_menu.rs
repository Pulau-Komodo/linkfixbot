use itertools::Itertools;
use serenity::{
	all::*,
	futures::future::{self, OptionFuture},
};

use crate::{fix_link::find_and_fix, reply_shortcuts::ReplyShortcuts, strings::ERROR_NONE_FOUND};

fn can_react(permissions: &Option<Permissions>) -> bool {
	permissions
		.map(|perm| perm.add_reactions())
		.unwrap_or(false)
}

fn can_suppress_embeds(permissions: &Option<Permissions>) -> bool {
	permissions
		.map(|perm| perm.manage_messages())
		.unwrap_or(false)
}

fn take_interacted_message(interaction: &mut CommandInteraction) -> Option<Message> {
	let messages = std::mem::take(&mut interaction.data.resolved.messages);
	messages.into_values().next()
}

pub async fn fix_links(context: &Context, mut interaction: CommandInteraction) {
	let Some(message) = take_interacted_message(&mut interaction) else {
		eprintln!("Did not find a message for some reason.");
		let _ = interaction
			.ephemeral_reply(&context.http, "Did not receive the message.")
			.await;
		return;
	};

	let mut remaining_embed_urls = if can_suppress_embeds(&interaction.app_permissions) {
		message.embeds.iter().map(|embed| &embed.url).collect()
	} else {
		Vec::new()
	};
	let any_removeable_embeds = !remaining_embed_urls.is_empty();

	let content = &message.content;
	let output = find_and_fix(content)
		.map(|fix| {
			if fix.remove_embed {
				if let Some(pos) = remaining_embed_urls
					.iter()
					.position(|url| url.as_ref().is_some_and(|url| url.as_str() == fix.link))
				{
					remaining_embed_urls.remove(pos);
				}
			}
			fix.fixed
		})
		.join("\n");

	if output.is_empty() {
		let _ = interaction
			.ephemeral_reply(&context.http, ERROR_NONE_FOUND)
			.await;
		return;
	}

	let should_suppress_embeds = any_removeable_embeds && remaining_embed_urls.is_empty();
	if !should_suppress_embeds {
		println!(
			"Will not remove embeds. Embeds found: {}, embed urls: {:?}",
			any_removeable_embeds, remaining_embed_urls
		);
	}

	let result = interaction.public_reply(&context.http, output).await;
	if result.is_err() {
		println!("Did not remove embeds because message failed to send");
		return;
	}
	let react: OptionFuture<_> = can_react(&interaction.app_permissions)
		.then(|| message.react(context, ReactionType::Unicode("🔧".to_string())))
		.into();
	let suppress: OptionFuture<_> = should_suppress_embeds
		.then(|| {
			EditMessage::new()
				.suppress_embeds(true)
				.execute(context, (message.channel_id, message.id, None))
		})
		.into();
	let (_, suppress_result) = future::join(react, suppress).await;
	if let Some(Err(error)) = suppress_result {
		println!("Did not remove embeds because {:?}", error);
	}
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
