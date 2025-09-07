use std::collections::{HashMap, hash_map};

use itertools::Itertools;
use serenity::{
	all::{
		Builder as _, ChannelId, Context, EditMessage, Message, MessageId, MessageUpdateEvent,
		Permissions, ReactionType,
	},
	futures::future::{self, OptionFuture},
	prelude::TypeMapKey,
};
use tokio::sync::RwLock;

use crate::{
	fix_link::LinkFixer,
	util::{get_embed_urls, has_spoilers, x_to_twitter},
};

/// A message with embeds that may be suppressed in the future, if their replacements succeed in generating.
#[derive(Debug)]
struct BotMessage {
	/// The original message with the links.
	original_message: MessageId,
	/// The number of embeds the bot message should have for it to be OK to suppress embeds on the original message. `None` if the bot message embeds have not yet been generated.
	embed_count: Option<usize>,
}

pub struct FutureEmbedRemovalsInner {
	/// Key: original message, value: target embed count
	messages_with_fixable_embeds: HashMap<MessageId, usize>,
	/// Key: bot message
	bot_messages: HashMap<MessageId, BotMessage>,
}

impl FutureEmbedRemovalsInner {
	pub fn new() -> Self {
		FutureEmbedRemovalsInner {
			messages_with_fixable_embeds: HashMap::new(),
			bot_messages: HashMap::new(),
		}
	}
}

#[derive(Debug)]
pub struct FutureEmbedRemovalsTypeMap;

impl TypeMapKey for FutureEmbedRemovalsTypeMap {
	type Value = FutureEmbedRemovals;
}

pub struct FutureEmbedRemovals(RwLock<FutureEmbedRemovalsInner>);

impl FutureEmbedRemovals {
	pub fn new() -> Self {
		Self(RwLock::new(FutureEmbedRemovalsInner::new()))
	}
	pub async fn add_bot_message(
		&self,
		original_message: MessageId,
		bot_message: MessageId,
		embed_count: Option<usize>,
	) -> bool {
		let mut inner = self.0.write().await;
		if let Some(embed_count) = embed_count
			&& let hash_map::Entry::Occupied(occupied_entry) =
				inner.messages_with_fixable_embeds.entry(original_message)
			&& *occupied_entry.get() == embed_count
		{
			occupied_entry.remove();
			println!(
				"Success! add_bot_message Removed embeds on {} due to {}",
				original_message.get(),
				bot_message.get()
			);
			return true;
		}
		inner.bot_messages.insert(
			bot_message,
			BotMessage {
				original_message,
				embed_count,
			},
		);
		println!(
			"Added bot message {} with embed count {:?}",
			bot_message.get(),
			embed_count
		);
		false
	}
	pub async fn update_bot_message(
		&self,
		bot_message_id: MessageId,
		embed_count: usize,
	) -> Option<MessageId> {
		let mut inner = self.0.write().await;
		let Some(bot_message) = inner.bot_messages.get(&bot_message_id) else {
			println!(
				"Tried to update a bot message that was not in the list, but should have been."
			);
			return None;
		};
		if let Some(&target_embed_count) = inner
			.messages_with_fixable_embeds
			.get(&bot_message.original_message)
		{
			let original_message = bot_message.original_message;
			// Both are found so bot message is no longer waiting, no matter which outcome.
			inner.bot_messages.remove(&bot_message_id);
			if target_embed_count == embed_count {
				// Success! Remove original message too since it is no longer waiting on anything.
				inner.messages_with_fixable_embeds.remove(&original_message);
				println!(
					"Success! update_bot_message Remove membeds on {} due to {}",
					original_message.get(),
					bot_message_id.get()
				);
				return Some(original_message);
			}
		}
		// Insert the embed count and keep waiting for the original message.
		inner
			.bot_messages
			.entry(bot_message_id)
			.and_modify(|bot_message| bot_message.embed_count = Some(embed_count));
		println!(
			"Inserted embed count {} for {}",
			embed_count,
			bot_message_id.get()
		);
		None
	}
	pub async fn add_original_message(
		&self,
		original_message: MessageId,
		target_embed_count: usize,
	) -> bool {
		let mut inner = self.0.write().await;
		if let Some((&bot_message_id, bot_message)) = inner
			.bot_messages
			.iter()
			.find(|(_, bot_message)| bot_message.original_message == original_message)
			&& let Some(embed_count) = bot_message.embed_count
		{
			// Both known, so bot message is no longer waiting.
			inner.bot_messages.remove(&bot_message_id);
			if embed_count == target_embed_count {
				// Success.
				println!(
					"Success! add_original_message Removing embeds for {} due to {}",
					original_message.get(),
					bot_message_id.get()
				);
				return true;
			}
		}
		// No match, so wait for the right bot message to come along.
		inner
			.messages_with_fixable_embeds
			.insert(original_message, target_embed_count);
		println!(
			"Insert the target embed count {} for {}",
			target_embed_count,
			original_message.get()
		);
		false
	}
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

/// Take an existing message and fix any links it has. Returns `None` if there were none. Otherwise, returns the message with the fixed links and the list of links that were fixed that should end up with their embeds replaced.
pub async fn fix_existing_message(
	content: &str,
	link_fixer: &LinkFixer,
) -> Option<(String, Vec<String>)> {
	if has_spoilers(content) {
		return None;
	}

	let mut fixed_urls = Vec::new();
	let output = link_fixer
		.find_and_fix(content)
		.map(|fix| {
			if fix.remove_embed {
				let url = x_to_twitter(fix.link).unwrap_or_else(|| fix.link.to_string());
				fixed_urls.push(url);
			}
			fix.fixed
		})
		.join("\n");

	if output.is_empty() {
		return None;
	}

	Some((output, fixed_urls))
}

pub fn determine_target_embed_count(
	mut embed_urls: Vec<String>,
	fixable_embed_links: &[String],
) -> Option<usize> {
	let mut target_embed_count = 0;
	for fixable_url in fixable_embed_links {
		if let Some(pos) = embed_urls.iter().position(|url| url == fixable_url) {
			target_embed_count += 1;
			embed_urls.swap_remove(pos);
		}
	}
	embed_urls.is_empty().then_some(target_embed_count)
}

pub async fn try_react_and_suppress(
	context: &Context,
	original_message: &Message,
	bot_message: Option<&Message>,
	fixable_embed_links: Vec<String>,
	can_react: bool,
	can_suppress: bool,
) {
	let react: OptionFuture<_> = can_react
		.then(|| original_message.react(context, ReactionType::Unicode("🔧".to_string())))
		.into();

	let suppress: OptionFuture<_> = can_suppress
		.then(|| {
			bot_message.map(|own_message| {
				handle_embed_suppression(
					context,
					original_message,
					own_message,
					fixable_embed_links,
				)
			})
		})
		.flatten()
		.into();

	let _ = future::join(react, suppress).await;
}

async fn handle_embed_suppression(
	context: &Context,
	original_message: &Message,
	bot_message: &Message,
	fixable_embed_links: Vec<String>,
) {
	if !original_message.embeds.is_empty() && !bot_message.embeds.is_empty() {
		println!("Attempting to remove immediately as neither message's embed list is empty.");
		// Both immediately have embeds, so try removing them now.
		if determine_target_embed_count(
			get_embed_urls(&original_message.embeds),
			&fixable_embed_links,
		)
		.map(|target_embed_count| target_embed_count == bot_message.embeds.len())
		.unwrap_or(false)
		{
			println!("Success!");
			suppress_embeds(context, original_message.channel_id, original_message.id).await;
		} else {
			println!("Failure.");
		}
		return;
	}
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovalsTypeMap>() else {
		eprintln!("Couldn't get FutureEmbedRemovals.");
		return;
	};
	if !original_message.embeds.is_empty()
		&& let Some(target_embed_count) = determine_target_embed_count(
			get_embed_urls(&original_message.embeds),
			&fixable_embed_links,
		) {
		removals
			.add_original_message(original_message.id, target_embed_count)
			.await;
	}
	let embed_count = (!bot_message.embeds.is_empty()).then_some(bot_message.embeds.len());
	if removals
		.add_bot_message(original_message.id, bot_message.id, embed_count)
		.await
	{
		println!("Success upon adding bot message immediately.");
		suppress_embeds(context, original_message.channel_id, original_message.id).await;
	}
}

pub async fn handle_bot_message_embed_generation(context: &Context, event: &MessageUpdateEvent) {
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovalsTypeMap>() else {
		eprintln!("Future removals not present.");
		return;
	};

	if let Some(embed_count) = event
		.embeds
		.as_ref()
		.and_then(|embeds| (!embeds.is_empty()).then_some(embeds.len()))
		&& let Some(message) = removals.update_bot_message(event.id, embed_count).await
	{
		suppress_embeds(context, event.channel_id, message).await;
	}
}

pub async fn handle_user_message_embed_generation(
	context: &Context,
	event: &MessageUpdateEvent,
	link_fixer: &LinkFixer,
) {
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovalsTypeMap>() else {
		eprintln!("Future removals not present.");
		return;
	};

	let Some(embeds) = event.embeds.as_ref() else {
		return;
	};
	if embeds.is_empty() {
		return;
	}
	let Some(content) = event.content.as_ref() else {
		return;
	};
	let Some((_output, embeds_to_suppress)) = fix_existing_message(content, link_fixer).await
	else {
		return;
	};
	let Some(target_embed_count) =
		determine_target_embed_count(get_embed_urls(embeds), &embeds_to_suppress)
	else {
		return;
	};

	if removals
		.add_original_message(event.id, target_embed_count)
		.await
	{
		suppress_embeds(context, event.channel_id, event.id).await;
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
