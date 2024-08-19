use std::sync::LazyLock;

use itertools::Itertools;
use regex::{Captures, Regex};
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
	let output = find_and_fix(content).join("\n");

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

fn find_and_fix(text: &str) -> impl Iterator<Item = String> + '_ {
	let closing_bracket = MEGAPATTERN.0.captures_len() - 1;
	MEGAPATTERN
		.0
		.captures_iter(&text)
		.filter(move |find| find.get(1).is_some() == find.get(closing_bracket).is_some())
		.filter_map(|find| {
			find.iter()
				.skip(2)
				.position(|group| group.is_some())
				.map(|index| {
					let mut offset = 0;
					for replacement in &MEGAPATTERN.1 {
						if (offset..offset + replacement.capture_group_count).contains(&index) {
							return (replacement.closure)(&find, offset + 1);
						}
						offset += replacement.capture_group_count;
					}
					return String::from("");
				})
		})
}

pub struct Replacement {
	pattern: &'static str,
	capture_group_count: usize,
	closure: Box<dyn Send + Sync + 'static + Fn(&Captures, usize) -> String>,
}

impl Replacement {
	fn new(
		pattern: &'static str,
		capture_group_count: usize,
		closure: impl Send + Sync + 'static + Fn(&Captures, usize) -> String,
	) -> Self {
		Self {
			pattern,
			capture_group_count,
			closure: Box::new(closure),
		}
	}
}

static MEGAPATTERN: LazyLock<(Regex, [Replacement; 4])> = LazyLock::new(|| {
	let twitter = Replacement::new(
		r"https://(?:x|twitter).com/([0-9a-z_]+/status/[0-9]+)",
		1,
		|find, offset| format!("https://fixupx.com/{}", &find[offset + 1]),
	);
	let instagram = Replacement::new(
		r"https://www.instagram.com/(p|reel)/([-0-9a-z]+)(?:/\S*)?",
		2,
		|find, offset| {
			format!(
				"https://www.ddinstagram.com/{}/{}/",
				&find[offset + 1],
				&find[offset + 2]
			)
		},
	);
	let reddit = Replacement::new(
		r"https://www.reddit.com/r/([0-9a-z_]+)/(comments)/([0-9a-z]+)/[0-9a-z_]+/?",
		3,
		|find, offset| {
			format!(
				"https://www.rxddit.com/r/{}/{}/{}/_/",
				&find[offset + 1],
				&find[offset + 2],
				&find[offset + 3]
			)
		},
	);
	let youtube = Replacement::new(
		r"https://www.youtube.com/shorts/([-0-9a-z_]+)",
		1,
		|find, offset| format!("<https://www.youtube.com/watch?v={}>", &find[offset + 1]),
	);
	let replacements = [twitter, instagram, reddit, youtube];
	let inner = replacements
		.iter()
		.map(|replacement| replacement.pattern)
		.join("|");
	let start = r"(?i)(?:\s|^)(<)?(?:";
	let end = r")(>)?(?:\s|$)";
	(
		Regex::new(&format!("{start}{inner}{end}")).unwrap(),
		replacements,
	)
});

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix link")
		.description("")
		.kind(CommandType::Message)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_name() {
		let string = r"https://www.instagram.com/reel/abc blahblah <https://www.reddit.com/r/fictitious/comments/abc/def>";
		let mut links = find_and_fix(&string);
		assert_eq!(
			links.next(),
			Some(String::from("https://www.ddinstagram.com/reel/abc/"))
		);
		assert_eq!(
			links.next(),
			Some(String::from(
				"https://www.rxddit.com/r/fictitious/comments/abc/_/"
			))
		);
	}
}
