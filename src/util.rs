use std::sync::LazyLock;

use regex::Regex;
use serenity::all::{Embed, Message, MessageFlags};

static SPOILERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|\|").unwrap());

/// Overly aggressive function detecting any possible presence of spoiler tags.
pub fn has_spoilers(str: &str) -> bool {
	SPOILERS.captures_iter(str).nth(1).is_some()
}

static X: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)^https://x\.com/([0-9a-z_]+/status/[0-9]+\S*)$").unwrap());

/// Ugly fix to deal with X embeds actually linking to Twitter.
pub fn x_to_twitter(link: &str) -> Option<String> {
	let mut captures = X.captures_iter(link);
	let find = captures.next()?;
	Some(format!("https://twitter.com/{}", find.get(1)?.as_str()))
}

pub fn has_suppressed_embeds(message: &Message) -> bool {
	message
		.flags
		.map(|flags| flags.contains(MessageFlags::SUPPRESS_EMBEDS))
		.unwrap_or(false)
}

pub fn get_embed_urls<'l>(embeds: impl IntoIterator<Item = &'l Embed>) -> Vec<String> {
	embeds
		.into_iter()
		.filter_map(|embed| embed.url.clone())
		.collect()
}
