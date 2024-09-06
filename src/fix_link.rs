use std::sync::LazyLock;

use itertools::Itertools;
use regex::{Captures, Regex};

pub fn find_and_fix(text: &str) -> impl Iterator<Item = String> + '_ {
	let (regex, replacements) = &*MEGAPATTERN;
	let closing_bracket = regex.captures_len() - 1;
	text.split_ascii_whitespace()
		.flat_map(|text| regex.captures_iter(text))
		.filter(move |find| find.get(1).is_some() == find.get(closing_bracket).is_some())
		.map(|find| {
			let index = find
				.iter()
				.skip(2)
				.position(|group| group.is_some())
				.unwrap(); // If it matched the outer regex, it needs to match some group, because all subsections have groups.

			let mut offset = 0;
			let replacement = replacements
				.iter()
				.find(|replacement| {
					if (offset..offset + replacement.capture_group_count).contains(&index) {
						true
					} else {
						offset += replacement.capture_group_count;
						false
					}
				})
				.unwrap(); // One of the replacements must match the capture group found.
			(replacement.closure)(&find, offset + 1)
		})
}

type ReplacementClosure = dyn Send + Sync + 'static + Fn(&Captures, usize) -> String;

pub struct Replacement {
	pattern: &'static str,
	capture_group_count: usize,
	closure: Box<ReplacementClosure>,
}

impl Replacement {
	fn new(
		pattern: &'static str,
		closure: impl Send + Sync + 'static + Fn(&Captures, usize) -> String,
	) -> Self {
		let regex = Regex::new(pattern).unwrap();
		let capture_group_count = regex.captures_len() - 1;
		assert!(capture_group_count > 0); // Every pattern needs a capture group.
		Self {
			pattern,
			capture_group_count,
			closure: Box::new(closure),
		}
	}
}

static MEGAPATTERN: LazyLock<(Regex, [Replacement; 4])> = LazyLock::new(|| {
	let twitter = Replacement::new(
		r"https://(?:x|twitter)\.com/([0-9a-z_]+/status/[0-9]+)\S*",
		|find, offset| format!("https://fixupx.com/{}", &find[offset + 1]),
	);
	let instagram = Replacement::new(
		r"https://www\.instagram\.com/(p|reel)/([-0-9a-z_]+)(?:/\S*)?",
		|find, offset| {
			format!(
				"https://www.ddinstagram.com/{}/{}/",
				&find[offset + 1],
				&find[offset + 2],
			)
		},
	);
	let reddit = Replacement::new(
		r"https://(www|old)\.reddit\.com/r/([0-9a-z_]+)/(comments)/([0-9a-z]+)/[0-9a-z_]+/?\S*",
		|find, offset| {
			format!(
				"https://{}.rxddit.com/r/{}/{}/{}/_/",
				&find[offset + 1],
				&find[offset + 2],
				&find[offset + 3],
				&find[offset + 4],
			)
		},
	);
	let youtube = Replacement::new(
		r"https://www\.youtube\.com/shorts/([-0-9a-z_]+)\S*",
		|find, offset| format!("<https://www.youtube.com/watch?v={}>", &find[offset + 1]),
	);
	let replacements = [twitter, instagram, reddit, youtube];
	let inner = replacements
		.iter()
		.map(|replacement| replacement.pattern)
		.join("|");
	let start = r"(?i)^(<)?(?:";
	let end = r")(>)?$";
	(
		Regex::new(&format!("{start}{inner}{end}")).unwrap(),
		replacements,
	)
});

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn find_each() {
		let string = r"https://www.instagram.com/reel/abc blahblah <https://www.reddit.com/r/fictitious/comments/abc/def> https://x.com/fictitious/status/0123 and https://www.youtube.com/shorts/GX5wEDmbpQA";
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
		assert_eq!(
			links.next(),
			Some(String::from("https://fixupx.com/fictitious/status/0123"))
		);
		assert_eq!(
			links.next(),
			Some(String::from(
				"<https://www.youtube.com/watch?v=GX5wEDmbpQA>"
			))
		);
	}
}
