use std::sync::LazyLock;

use itertools::Itertools;
use regex::Regex;
use serenity::all::{
	CommandInteraction, CommandType, Context, CreateCommand, CreateInteractionResponse,
	CreateInteractionResponseMessage,
};

pub async fn fix_link(context: &Context, interaction: CommandInteraction) {
	let Some(message) = interaction.data.resolved.messages.values().next() else {
		eprintln!("Did not find a message for some reason.");
		let _ = interaction
			.create_response(
				context,
				CreateInteractionResponse::Message(
					CreateInteractionResponseMessage::new()
						.content("Did not receive the message.")
						.ephemeral(true),
				),
			)
			.await;
		return;
	};
	//println!("Parsing content: \"{}\"", message.content);
	let content = &message.content;
	let output = embed_twitter(content)
		.chain(embed_instagram(content))
		.chain(embed_reddit(content))
		.chain(unshort_youtube(content))
		.join("\n");

	if output.is_empty() {
		let _ = interaction
			.create_response(
				context,
				CreateInteractionResponse::Message(
					CreateInteractionResponseMessage::new()
						.content("Found no links to fix. I only fix embed links for x.com, instagram.com and reddit.com, and unshort Youtube shorts links.")
						.ephemeral(true),
				),
			)
			.await;
		return;
	}

	let _ = interaction
		.create_response(
			context,
			CreateInteractionResponse::Message(
				CreateInteractionResponseMessage::new().content(output),
			),
		)
		.await;
}

static TWITTER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?i)(?:\s|^)(<)?https://x.com/([0-9a-z_]+/status/[0-9]+)(>)?(?:\s|$)").unwrap()
});

fn embed_twitter(content: &str) -> impl Iterator<Item = String> + '_ {
	TWITTER_REGEX.captures_iter(content).filter_map(|find| {
		(find.get(1).is_some() == find.get(3).is_some())
			.then(|| format!("https://fixupx.com/{}", &find[2]))
	})
}

static INSTAGRAM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)(?:\s|^)(<)?https://www.instagram.com/(p|reel)/([-0-9a-z]+)(?:/\S*)(>)?(?:\s|$)",
	)
	.unwrap()
});

fn embed_instagram(content: &str) -> impl Iterator<Item = String> + '_ {
	INSTAGRAM_REGEX.captures_iter(content).filter_map(|find| {
		(find.get(1).is_some() == find.get(4).is_some())
			.then(|| format!("https://ddinstagram.com/{}/{}/", &find[2], &find[3]))
	})
}

static REDDIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)(?:\s|^)(<)?https://www.reddit.com/r/([0-9a-z_]+)/(comments)/([0-9a-z]+)/[0-9a-z_]+/?(>)?(?:\s|$)",
	)
	.unwrap()
});

fn embed_reddit(content: &str) -> impl Iterator<Item = String> + '_ {
	REDDIT_REGEX.captures_iter(content).filter_map(|find| {
		(find.get(1).is_some() == find.get(5).is_some()).then(|| {
			format!(
				"https://www.rxddit.com/r/{}/{}/{}/_/",
				&find[2], &find[3], &find[4]
			)
		})
	})
}

static YOUTUBE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?i)(?:\s|^)(<)?https://www.youtube.com/shorts/([-0-9a-z_]+)(>)?(?:\s|$)").unwrap()
});

fn unshort_youtube(content: &str) -> impl Iterator<Item = String> + '_ {
	YOUTUBE_REGEX.captures_iter(content).filter_map(|find| {
		(find.get(1).is_some() == find.get(3).is_some())
			.then(|| format!("<https://www.youtube.com/watch?v={}>", &find[2]))
	})
}

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix link")
		.description("")
		.kind(CommandType::Message)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_twitter() {
		assert_eq!(
			TWITTER_REGEX
				.captures_iter("https://x.com/ShouldHaveCat/status/1825533507487060046")
				.count(),
			1
		);
	}
}
