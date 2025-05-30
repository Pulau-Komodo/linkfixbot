use std::sync::LazyLock;

use itertools::Itertools;
use regex::{Captures, Regex};

pub fn find_and_fix(text: &str) -> impl Iterator<Item = LinkFix<'_>> + '_ {
	let regex = &MEGAPATTERN.0;
	text.split_ascii_whitespace()
		.flat_map(|text| regex.captures_iter(text))
		.filter_map(LinkFix::new)
}

#[derive(Debug)]
pub struct LinkFix<'l> {
	pub link: &'l str,
	pub fixed: String,
	pub remove_embed: bool,
}

impl<'l> LinkFix<'l> {
	pub fn new(captures: Captures<'l>) -> Option<Self> {
		let replacements = &MEGAPATTERN.1;
		let index = captures
			.iter()
			.skip(1)
			.position(|group| group.is_some())
			.unwrap(); // If it matched the outer regex, it needs to match some group, because all subsections have groups.

		let mut offset = 0;
		let replacement = replacements
			.iter()
			.find(|replacement| {
				if (offset..offset + replacement.capture_group_count * 2).contains(&index) {
					true
				} else {
					offset += replacement.capture_group_count * 2;
					false
				}
			})
			.unwrap(); // One of the replacements must match the capture group found.

		// Whether it found the first version (with `<>`) or the second (without).
		let embed_suppressed = (offset..offset + replacement.capture_group_count).contains(&index);
		if embed_suppressed && matches!(replacement.embed_handling, EmbedHandling::Replace) {
			// Replacing the embed is presumed to be the point, but the original was embed suppressed.
			return None;
		}
		if !embed_suppressed {
			offset += replacement.capture_group_count;
		}
		let mut fixed = (replacement.closure)(&captures, offset);

		if embed_suppressed || matches!(replacement.embed_handling, EmbedHandling::DoNothing) {
			fixed = format!("<{fixed}>");
		}

		let fix = Self {
			link: captures.get(0).unwrap().as_str(),
			fixed,
			remove_embed: matches!(replacement.embed_handling, EmbedHandling::Replace)
				&& !embed_suppressed,
		};
		Some(fix)
	}
}

/// How to handle the existing embed and the new link.
#[derive(Debug, Clone, Copy)]
enum EmbedHandling {
	/// If the new link gets an embed, remove the old one.
	Replace,
	/// Leave the old embed, if it had one, and prevent a new one (using `<>`).
	DoNothing,
}

type ReplacementClosure = dyn Send + Sync + 'static + Fn(&Captures, usize) -> String;

pub struct Replacement {
	pattern: &'static str,
	capture_group_count: usize,
	closure: Box<ReplacementClosure>,
	/// Whether this replacement warrants removal of the original embed. `true` for embed-fixing links.
	embed_handling: EmbedHandling,
}

impl Replacement {
	fn new(
		pattern: &'static str,
		closure: impl Send + Sync + 'static + Fn(&Captures, usize) -> String,
		embed_handling: EmbedHandling,
	) -> Self {
		let regex = Regex::new(pattern).unwrap();
		let capture_group_count = regex.captures_len() - 1;
		assert!(capture_group_count > 0); // Every pattern needs a capture group.
		Self {
			pattern,
			capture_group_count,
			closure: Box::new(closure),
			embed_handling,
		}
	}
}

impl std::fmt::Debug for Replacement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Replacement")
			.field("pattern", &self.pattern)
			.field("capture_group_count", &self.capture_group_count)
			.field("closure", &"some closure I cannot print")
			.finish()
	}
}

static MEGAPATTERN: LazyLock<(Regex, [Replacement; 9])> = LazyLock::new(|| {
	let twitter = Replacement::new(
		r"https://(?:x|twitter)\.com/([0-9a-z_]+/status/[0-9]+)\S*",
		|find, offset| format!("https://fixupx.com/{}", &find[offset + 1]),
		EmbedHandling::Replace,
	);
	let instagram = Replacement::new(
		r"https://www\.instagram\.com/(p|reels?)/([-0-9a-z_]+)(?:/\S*)?",
		|find, offset| {
			format!(
				"https://www.instagramez.com/{}/{}/",
				&find[offset + 1],
				&find[offset + 2],
			)
		},
		EmbedHandling::Replace,
	);
	let tiktok = Replacement::new(
		r"https://www\.tiktok\.com/@([a-z0-9_\.]+)/video/([0-9]+)\S*",
		|find, offset| {
			format!(
				"https://www.vxtiktok.com/@{}/video/{}",
				&find[offset + 1],
				&find[offset + 2],
			)
		},
		EmbedHandling::Replace,
	);
	let reddit = Replacement::new(
		r"https://(www|old)\.reddit\.com/r/([0-9a-z_]+)/(comments)/([0-9a-z]+)/[0-9a-z_]+/?(?:\?\S*)?",
		|find, offset| {
			format!(
				"https://{}.rxddit.com/r/{}/{}/{}/_/",
				&find[offset + 1],
				&find[offset + 2],
				&find[offset + 3],
				&find[offset + 4],
			)
		},
		EmbedHandling::Replace,
	);
	let reddit_share = Replacement::new(
		r"https://(www|old)\.reddit\.com/r/([0-9a-z_]+)/s/([0-9a-z]+)/?\S*",
		|find, offset| {
			format!(
				"https://{}.rxddit.com/r/{}/s/{} (⚠️ this is a share link ⚠️)",
				&find[offset + 1],
				&find[offset + 2],
				&find[offset + 3],
			)
		},
		EmbedHandling::Replace,
	);
	let reddit_short = Replacement::new(
		r"https://redd\.it/([0-9a-z]+)/?\S*",
		|find, offset| format!("https://rxddit.com/{}", &find[offset + 1],),
		EmbedHandling::Replace,
	);
	let youtube = Replacement::new(
		r"https://(?:www\.)?youtube\.com/shorts/([-0-9a-z_]+)\S*",
		|find, offset| format!("https://www.youtube.com/watch?v={}", &find[offset + 1]),
		EmbedHandling::DoNothing,
	);
	let amazon = Replacement::new(
		r"https://www\.amazon\.(com|ca|co\.(?:uk|jp)|de|fr|it|es|in|nl|sg)/[^\s/]+/dp/([A-Z0-9]+)\S*",
		|find, offset| {
			format!(
				"https://www.amazon.{}/dp/{}",
				&find[offset + 1],
				&find[offset + 2]
			)
		},
		EmbedHandling::DoNothing,
	);
	let amazon2 = Replacement::new(
		r"https://www\.amazon\.(com|ca|co\.(?:uk|jp)|de|fr|it|es|in|nl|sg)/gp/product/([A-Z0-9]+)\S*",
		|find, offset| {
			format!(
				"https://www.amazon.{}/dp/{}",
				&find[offset + 1],
				&find[offset + 2]
			)
		},
		EmbedHandling::DoNothing,
	);
	let replacements = [
		twitter,
		instagram,
		tiktok,
		reddit,
		reddit_share,
		reddit_short,
		youtube,
		amazon,
		amazon2,
	];
	//dbg!(&replacements);
	let inner = replacements
		.iter()
		.flat_map(|replacement| {
			[
				format!("<{}>", replacement.pattern),
				String::from(replacement.pattern),
			]
		})
		.join("|");
	(
		Regex::new(&format!("(?i)^(?:{inner})$")).unwrap(),
		replacements,
	)
});

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn find_instagram() {
		let string = "blahblah https://www.instagram.com/reel/abc blahblah";
		let find = find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("https://www.instagramez.com/reel/abc/"))
		);
	}
	#[test]
	fn find_reddit() {
		let string = "blahblah https://www.reddit.com/r/fictitious/comments/abc/def blahblah";
		let find = find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from(
				"https://www.rxddit.com/r/fictitious/comments/abc/_/"
			))
		);
	}
	#[test]
	fn find_twitter() {
		let string = "blahblah https://x.com/fictitious/status/0123 blahblah";
		let find = find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("https://fixupx.com/fictitious/status/0123"))
		);
	}
	#[test]
	fn find_youtube() {
		let string = "blahblah https://www.youtube.com/shorts/GX5wEDmbpQA blahblah";
		let find = find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from(
				"<https://www.youtube.com/watch?v=GX5wEDmbpQA>"
			))
		);
	}
	#[test]
	fn find_amazon() {
		let string = "https://www.amazon.ca/Some-Item-With-Code-ABC012/dp/ABC012?all_sorts_of=tracking.data&other_random=bs&believability_of_the_volume=false";
		let find = find_and_fix(&string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("<https://www.amazon.ca/dp/ABC012>"))
		);
	}
	#[test]
	fn find_each() {
		let string = r"hey <https://www.amazon.ca/Some-Item-With-Code-ABC012/dp/ABC012?all_sorts_of=tracking.data&other_random=bs&believability_of_the_volume=false> and https://www.instagram.com/reel/abc blahblah https://www.reddit.com/r/fictitious/comments/abc/def https://x.com/fictitious/status/0123 and https://www.youtube.com/shorts/GX5wEDmbpQA";
		let mut links = find_and_fix(&string);
		assert_eq!(
			links.next().map(|fix| fix.fixed),
			Some(String::from("<https://www.amazon.ca/dp/ABC012>"))
		);
		assert_eq!(
			links.next().map(|fix| fix.fixed),
			Some(String::from("https://www.instagramez.com/reel/abc/"))
		);
		assert_eq!(
			links.next().map(|fix| fix.fixed),
			Some(String::from(
				"https://www.rxddit.com/r/fictitious/comments/abc/_/"
			))
		);
		assert_eq!(
			links.next().map(|fix| fix.fixed),
			Some(String::from("https://fixupx.com/fictitious/status/0123"))
		);
		assert_eq!(
			links.next().map(|fix| fix.fixed),
			Some(String::from(
				"<https://www.youtube.com/watch?v=GX5wEDmbpQA>"
			))
		);
	}
}
