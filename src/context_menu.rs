use std::collections::{HashMap, hash_map::Entry};

use itertools::Itertools;
use serenity::{
	all::*,
	futures::{
		future::{self, OptionFuture},
		lock::Mutex,
	},
	prelude::TypeMapKey,
};

use crate::{fix_link::find_and_fix, reply_shortcuts::ReplyShortcuts, strings::ERROR_NONE_FOUND};

/// A message with embeds that may be suppressed in the future, if their replacements succeed in generating.
#[derive(Debug)]
pub struct FutureEmbedRemoval {
	/// The message with the links.
	message: MessageId,
	/// The number of embeds it should have.
	embed_count: usize,
}

#[derive(Debug)]
pub struct FutureEmbedRemovals;

impl TypeMapKey for FutureEmbedRemovals {
	type Value = Mutex<HashMap<MessageId, FutureEmbedRemoval>>;
}

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

	let mut intended_embeds = 0;

	let content = &message.content;
	let output = find_and_fix(content)
		.map(|fix| {
			if fix.remove_embed {
				intended_embeds += 1;
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
	};

	let react: OptionFuture<_> = can_react(&interaction.app_permissions)
		.then(|| message.react(context, ReactionType::Unicode("🔧".to_string())))
		.into();
	let suppress: OptionFuture<_> = should_suppress_embeds
		.then(|| {
			handle_embed_suppression(
				context,
				&interaction,
				message.channel_id,
				message.id,
				intended_embeds,
			)
		})
		.into();

	let _ = future::join(react, suppress).await;
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

async fn handle_embed_suppression(
	context: &Context,
	interaction: &CommandInteraction,
	channel: ChannelId,
	message: MessageId,
	embed_count: usize,
) {
	let Ok(response) = interaction.get_response(&context.http).await else {
		return;
	};
	if response.embeds.is_empty() {
		// No immediate embeds, so we may get them later.
		let data = context.data.read().await;
		if let Some(removals) = data.get::<FutureEmbedRemovals>() {
			removals.lock().await.insert(
				response.id,
				FutureEmbedRemoval {
					message,
					embed_count,
				},
			);
		}
	} else if embed_count == response.embeds.len() {
		// Immediate embeds, so try removing them now.
		suppress_embeds(context, channel, message).await;
	}
}

pub async fn handle_delayed_embed_suppression(context: &Context, event: &MessageUpdateEvent) {
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovals>() else {
		eprintln!("Future removals not present.");
		return;
	};

	if let Entry::Occupied(entry) = removals.lock().await.entry(event.id) {
		let removal = entry.get();
		if event.embeds.as_ref().map(|v| v.len()).unwrap_or(0) == removal.embed_count {
			suppress_embeds(context, event.channel_id, removal.message).await;
		}
		entry.remove();
	}
}

async fn suppress_embeds(context: &Context, channel: ChannelId, message: MessageId) {
	if let Err(error) = EditMessage::new()
		.suppress_embeds(true)
		.execute(context, (channel, message, None))
		.await
	{
		println!("Did not remove embeds because {:?}", error);
	}
}
