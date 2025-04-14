use std::collections::HashMap;

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
	fix_link::find_and_fix,
	util::{get_embed_urls, has_spoilers, x_to_twitter},
};

/// A message with embeds that may be suppressed in the future, if their replacements succeed in generating.
#[derive(Debug)]
pub struct FutureEmbedRemoval {
	/// The original message with the links.
	original_message: MessageId,
	/// The bot's message with the fixed links.
	bot_message: MessageId,
	/// The links of the embeds that should be getting replaced.
	fixable_embed_links: Vec<String>,
}

pub struct FutureEmbedRemovalOriginalGenerated {
	/// The original message with the links.
	original_message: MessageId,
	/// The bot's message with the fixed links.
	bot_message: MessageId,
	/// The number of embeds the bot's message should have before suppressing the embeds on the original.
	target_embed_count: usize,
}

pub struct FutureEmbedRemovalBotMessageGenerated {
	/// The original message with the links.
	original_message: MessageId,
	/// The bot's message with the fixed links.
	_bot_message: MessageId,
	/// The links of the embeds that should be getting replaced.
	fixable_embed_links: Vec<String>,
	/// The number of embeds the bot's message has.
	embed_count: usize,
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
	pub async fn add_neither_generated(
		&self,
		original_message: MessageId,
		bot_message: MessageId,
		fixable_embed_links: Vec<String>,
	) {
		println!(
			"===> Adding deferred suppression. Known: neither. Original message: {}, bot message: {} <===",
			original_message.get(),
			bot_message.get()
		);
		let mut inner = self.0.write().await;
		inner.neither.push(FutureEmbedRemoval {
			original_message,
			bot_message,
			fixable_embed_links,
		});
	}
	pub async fn add_original_generated(
		&self,
		original_message: MessageId,
		bot_message: MessageId,
		target_embed_count: usize,
	) {
		println!(
			"===> Adding deferred suppression. Known: original. Original message: {}, bot message: {} <===",
			original_message.get(),
			bot_message.get()
		);
		let mut inner = self.0.write().await;
		inner.original.push(FutureEmbedRemovalOriginalGenerated {
			original_message,
			bot_message,
			target_embed_count,
		});
	}
	pub async fn add_bot_generated(
		&self,
		original_message: MessageId,
		bot_message: MessageId,
		fixable_embed_links: Vec<String>,
		embed_count: usize,
	) {
		println!(
			"===> Adding deferred suppression. Known: bot. Original message: {}, bot message: {} <===",
			original_message.get(),
			bot_message.get()
		);
		let mut inner = self.0.write().await;
		inner.bot.push(FutureEmbedRemovalBotMessageGenerated {
			original_message,
			_bot_message: bot_message,
			fixable_embed_links,
			embed_count,
		});
	}
	/// Update the stored information with the fact a bot message just had its embeds generated. Returns the message to have its embeds suppressed if the embed count is right.
	pub async fn update_bot_generated(
		&self,
		bot_message: MessageId,
		embed_count: usize,
	) -> Option<MessageId> {
		let inner = self.0.read().await;
		let inner = if inner
			.neither
			.iter()
			.any(|removal| removal.bot_message == bot_message)
		{
			std::mem::drop(inner);
			let mut inner = self.0.write().await;
			if let Some(index) = inner
				.neither
				.iter()
				.position(|removal| removal.bot_message == bot_message)
			{
				let removal = inner.neither.swap_remove(index);
				// Upgrade the "neither" to include the relevant bot message embed info.
				inner.bot.push(FutureEmbedRemovalBotMessageGenerated {
					original_message: removal.original_message,
					_bot_message: removal.bot_message,
					fixable_embed_links: removal.fixable_embed_links,
					embed_count,
				});
				println!(
					"Upgraded neither to bot. Original message: {}, bot message: {}",
					removal.original_message.get(),
					removal.bot_message.get()
				);
				return None;
			}
			std::mem::drop(inner);
			self.0.read().await
		} else {
			inner
		};

		if inner
			.original
			.iter()
			.any(|removal| removal.bot_message == bot_message)
		{
			let removal = inner
				.original
				.iter()
				.find(|removal| removal.bot_message == bot_message)?;
			if removal.target_embed_count == embed_count {
				// Success! The bot message embed info matched.
				println!(
					"Success! Got both (bot last) and embed count matched at {}. Original message: {}, bot message: {}",
					embed_count,
					removal.original_message.get(),
					bot_message.get()
				);
				return Some(removal.original_message);
			}
			println!(
				"Got both and embed count didn't match {} (target) != {} (embed count)",
				removal.target_embed_count, embed_count
			);
		}
		println!(
			"Found no original message matching bot message {}",
			bot_message.get()
		);
		None
	}
	/// Update the stored information with the fact an original message just had its embed generated. Returns whether that message should have its embeds suppressed.
	pub async fn update_original_generated(
		&self,
		original_message: MessageId,
		embed_links: Vec<String>,
	) -> bool {
		println!("a");

		let inner = self.0.read().await;
		let inner = if inner
			.neither
			.iter()
			.any(|removal| removal.original_message == original_message)
		{
			println!("b");
			std::mem::drop(inner);
			let mut inner = self.0.write().await;
			if let Some(index) = inner
				.neither
				.iter()
				.position(|removal| removal.original_message == original_message)
			{
				println!("c");
				let removal = inner.neither.swap_remove(index);
				if let Some(target_embed_count) =
					determine_target_embed_count(embed_links, &removal.fixable_embed_links)
				{
					println!(
						"Upgraded neither to original. Original message: {}, bot message: {}",
						removal.original_message.get(),
						removal.bot_message.get()
					);
					// We now know enough to reduce our information to a target embed count.
					inner.original.push(FutureEmbedRemovalOriginalGenerated {
						original_message: removal.original_message,
						bot_message: removal.bot_message,
						target_embed_count,
					});
				} else {
					println!(
						"Removed neither because of result of determine_target_embed_count with {:?}. Original message: {}, bot message: {}",
						removal.fixable_embed_links,
						removal.original_message.get(),
						removal.bot_message.get()
					);
				}
				println!("d");
				return false;
			}
			std::mem::drop(inner);
			self.0.read().await
		} else {
			inner
		};

		println!("e");
		if inner
			.bot
			.iter()
			.any(|removal| removal.original_message == original_message)
		{
			println!("f");
			std::mem::drop(inner);
			let mut inner = self.0.write().await;
			println!("g");
			if let Some(index) = inner
				.bot
				.iter()
				.filter(|removal| removal.original_message == original_message)
				.position(|removal| {
					if let Some(target_embed_count) = determine_target_embed_count(
						embed_links.clone(),
						&removal.fixable_embed_links,
					) {
						target_embed_count == removal.embed_count
					} else {
						false
					}
				}) {
				let removal = inner.bot.swap_remove(index);
				println!(
					"Success! Got both (original last) and embed count matched at {}. Original message: {}, bot message: {}",
					removal.embed_count,
					removal.original_message.get(),
					removal._bot_message.get()
				);
				return true;
			} else {
				println!(
					"Found bot messages to match original {} but no embed count matched with {:?}.",
					original_message.get(),
					embed_links
				);
			}
		}
		println!(
			"Found no bot message matching original {}",
			original_message.get()
		);
		false
	}
}

pub struct FutureEmbedRemovalsInner {
	message_with_fixable_embeds: HashMap<MessageId, usize>,
	neither: Vec<FutureEmbedRemoval>,
	original: Vec<FutureEmbedRemovalOriginalGenerated>,
	bot: Vec<FutureEmbedRemovalBotMessageGenerated>,
}

impl FutureEmbedRemovalsInner {
	pub fn new() -> Self {
		FutureEmbedRemovalsInner {
			message_with_fixable_embeds: HashMap::new(),
			neither: Vec::new(),
			original: Vec::new(),
			bot: Vec::new(),
		}
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
pub async fn fix_existing_message(message: &Message) -> Option<(String, Vec<String>)> {
	if has_spoilers(&message.content) {
		return None;
	}

	let mut fixed_urls = Vec::new();
	let content = &message.content;
	let output = find_and_fix(content)
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
	match (
		original_message.embeds.is_empty(),
		bot_message.embeds.is_empty(),
	) {
		(true, true) => {
			removals
				.add_neither_generated(original_message.id, bot_message.id, fixable_embed_links)
				.await;
		}
		(true, false) => {
			let embed_urls = get_embed_urls(&original_message.embeds);
			if let Some(target_embed_count) =
				determine_target_embed_count(embed_urls, &fixable_embed_links)
			{
				removals
					.add_original_generated(original_message.id, bot_message.id, target_embed_count)
					.await;
			}
		}
		(false, true) => {
			removals
				.add_bot_generated(
					original_message.id,
					bot_message.id,
					fixable_embed_links,
					bot_message.embeds.len(),
				)
				.await;
		}
		(false, false) => unreachable!("Case covered separately above"),
	}
}

pub async fn handle_bot_message_embed_generation(context: &Context, event: &MessageUpdateEvent) {
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovalsTypeMap>() else {
		eprintln!("Future removals not present.");
		return;
	};

	if let Some(message) = removals
		.update_bot_generated(
			event.id,
			event.embeds.as_ref().map(|e| e.len()).unwrap_or(0),
		)
		.await
	{
		suppress_embeds(context, event.channel_id, message).await;
	}
}

pub async fn handle_user_message_embed_generation(context: &Context, event: &MessageUpdateEvent) {
	let data = context.data.read().await;
	let Some(removals) = data.get::<FutureEmbedRemovalsTypeMap>() else {
		eprintln!("Future removals not present.");
		return;
	};

	if let Some(embeds) = &event.embeds {
		let embed_links = embeds
			.iter()
			.filter_map(|embed| embed.url.clone())
			.collect();

		if removals
			.update_original_generated(event.id, embed_links)
			.await
		{
			suppress_embeds(context, event.channel_id, event.id).await;
		}
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
