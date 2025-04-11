use std::collections::{HashMap, hash_map::Entry};

use itertools::Itertools;
use serenity::{
	all::{
		Builder as _, ChannelId, Context, EditMessage, Message, MessageId, MessageUpdateEvent,
		Permissions, ReactionType,
	},
	futures::{
		future::{self, OptionFuture},
		lock::Mutex,
	},
	prelude::TypeMapKey,
};

use crate::{fix_link::find_and_fix, util::has_spoilers};

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

pub fn can_react(permissions: &Option<Permissions>) -> bool {
	permissions
		.map(|perm| perm.add_reactions())
		.unwrap_or(false)
}

pub fn can_suppress_embeds(permissions: &Option<Permissions>) -> bool {
	permissions
		.map(|perm| perm.manage_messages())
		.unwrap_or(false)
}

/// Take an existing message and fix any links it has. Returns `None` if there were none. Otherwise, returns the message with the fixed links, the target number of embeds, and whether the embeds should be attempted to be suppressed.
pub async fn fix_existing_message(
	message: &Message,
	can_suppress: bool,
) -> Option<(String, usize, bool)> {
	if has_spoilers(&message.content) {
		return None;
	}
	let mut remaining_embed_urls = if can_suppress {
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
		return None;
	}

	let should_suppress_embeds = any_removeable_embeds && remaining_embed_urls.is_empty();
	if !should_suppress_embeds {
		println!(
			"Will not remove embeds. Embeds found: {}, embed urls: {:?}",
			any_removeable_embeds, remaining_embed_urls
		);
	}

	Some((output, intended_embeds, should_suppress_embeds))
}

pub async fn try_react_and_suppress(
	context: &Context,
	message: &Message,
	own_message: Option<&Message>,
	intended_embeds: usize,
	can_react: bool,
	should_suppress: bool,
) {
	let react: OptionFuture<_> = can_react
		.then(|| message.react(context, ReactionType::Unicode("🔧".to_string())))
		.into();

	let suppress: OptionFuture<_> = should_suppress
		.then(|| {
			own_message.map(|own_message| {
				handle_embed_suppression(
					context,
					message.channel_id,
					message.id,
					own_message,
					intended_embeds,
				)
			})
		})
		.flatten()
		.into();

	let _ = future::join(react, suppress).await;
}

async fn handle_embed_suppression(
	context: &Context,
	channel: ChannelId,
	message: MessageId,
	own_message: &Message,
	embed_count: usize,
) {
	if own_message.embeds.is_empty() {
		// No immediate embeds, so we may get them later.
		let data = context.data.read().await;
		if let Some(removals) = data.get::<FutureEmbedRemovals>() {
			removals.lock().await.insert(
				own_message.id,
				FutureEmbedRemoval {
					message,
					embed_count,
				},
			);
		}
	} else if embed_count == own_message.embeds.len() {
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
