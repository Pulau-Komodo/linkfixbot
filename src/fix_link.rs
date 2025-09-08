use itertools::Itertools;
use regex::{Captures, Regex};

pub struct LinkFixer {
	replacements: Vec<Replacement>,
	megapattern: Regex,
}

impl LinkFixer {
	pub fn from_config() -> Self {
		let replacements = load_replacements();
		let megapattern = make_megapattern(&replacements);

		let group_sum = replacements
			.iter()
			.map(|r| r.capture_group_count)
			.sum::<usize>()
			* 2;
		let megapattern_group_count = megapattern.captures_len() - 1;
		assert_eq!(
			group_sum, megapattern_group_count,
			"The megapattern has more groups than the replacements combined."
		);

		Self {
			replacements,
			megapattern,
		}
	}
	pub fn find_and_fix<'s>(&'s self, text: &'s str) -> impl Iterator<Item = LinkFix<'s>> + 's {
		text.split_ascii_whitespace()
			.flat_map(|text| self.megapattern.captures_iter(text))
			.filter_map(|captures| LinkFix::new(captures, &self.replacements))
	}
}

#[derive(Debug)]
pub struct LinkFix<'l> {
	pub link: &'l str,
	pub fixed: String,
	pub remove_embed: bool,
}

impl<'l> LinkFix<'l> {
	pub fn new(captures: Captures<'l>, replacements: &[Replacement]) -> Option<Self> {
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
		let mut fixed = replacement.apply(&captures, offset);

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

impl EmbedHandling {
	/// # Panics
	///
	/// Panics if the config file did not have a valid embed handling mode.
	fn from_string(string: &str) -> Self {
		match string {
			"replace" => EmbedHandling::Replace,
			"do nothing" => EmbedHandling::DoNothing,
			_ => panic!("The only options for embed handling are \"replace\" and \"do nothing\"."),
		}
	}
}

#[derive(Debug)]
pub struct Replacement {
	pattern: String,
	capture_group_count: usize,
	replacement: Vec<String>,
	insertion_points: Vec<usize>,
	embed_handling: EmbedHandling,
}

impl Replacement {
	fn from_config(
		pattern: &str,
		replacement: &str,
		embed_handling: &str,
		insertion_point_regex: &Regex,
	) -> Self {
		let regex = Regex::new(pattern).unwrap();
		let capture_group_count = regex.captures_len() - 1;
		assert!(
			capture_group_count > 0,
			"Every pattern needs a capture group."
		);
		let embed_handling = EmbedHandling::from_string(embed_handling);
		let insertion_point_iter = insertion_point_regex.find_iter(replacement);
		let replacement = insertion_point_regex
			.split(replacement)
			.map(String::from)
			.collect::<Vec<_>>();
		assert!(
			capture_group_count == replacement.len() - 1,
			"Number of capture groups ({}) does not match number of insertion points in the replacement string ({}) on pattern \"{}\".",
			capture_group_count,
			replacement.len() - 1,
			pattern
		);
		let insertion_points: Vec<_> = insertion_point_iter
			.map(|point| {
				let str = point.as_str();
				str[1..str.len() - 1].parse::<usize>().unwrap()
			})
			.collect();

		if !is_contiguous_starting_at_zero(&insertion_points) {
			panic!(
				"Insertion points need to start at 0 and not skip any numbers. Insertion points were: {:?}",
				insertion_points
			);
		}

		Self {
			pattern: pattern.to_string(),
			capture_group_count,
			replacement,
			insertion_points,
			embed_handling,
		}
	}
	fn apply(&self, captures: &Captures<'_>, offset: usize) -> String {
		let mut output = String::new();
		let mut insertion_iter = self.insertion_points.iter();
		for part in self
			.replacement
			.iter()
			.map(String::as_str)
			.intersperse_with(|| &captures[1 + offset + insertion_iter.next().unwrap()])
		{
			output.push_str(part);
		}
		output
	}
}

fn load_replacements() -> Vec<Replacement> {
	let insertion_point_regex = Regex::new(r"\{\d+}").unwrap();

	let file = std::fs::read_to_string("./replacements.txt").unwrap();
	let mut lines = file.lines();
	let mut replacements = Vec::new();

	while let Some(pattern) = lines.next() {
		let replacement = lines.next().unwrap();
		let embed_handling = lines.next().unwrap();
		replacements.push(Replacement::from_config(
			pattern,
			replacement,
			embed_handling,
			&insertion_point_regex,
		));
		if let Some(line) = lines.next()
			&& !line.is_empty()
		{
			panic!("Expected line to be empty, but found \"{}\".", line);
		}
	}

	replacements
}

fn make_megapattern(replacements: &[Replacement]) -> Regex {
	let inner = replacements
		.iter()
		.flat_map(|replacement| {
			[
				format!("<{}>", replacement.pattern),
				replacement.pattern.clone(),
			]
		})
		.join("|");
	Regex::new(&format!("(?i)^(?:{inner})$")).unwrap()
}

fn is_contiguous_starting_at_zero(list: &[usize]) -> bool {
	let mut found_values = vec![false; list.len()];
	for number in list {
		let Some(was_found) = found_values.get_mut(*number) else {
			return false;
		};
		*was_found = true;
	}
	found_values.into_iter().all(std::convert::identity)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn find_instagram() {
		let link_fixer = LinkFixer::from_config();
		let string = "blahblah https://www.instagram.com/reel/abc blahblah";
		let find = link_fixer.find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("https://www.instagramez.com/reel/abc/"))
		);
	}
	#[test]
	fn find_reddit() {
		let link_fixer = LinkFixer::from_config();
		let string = "blahblah https://www.reddit.com/r/fictitious/comments/abc/dëf blahblah";
		let find = link_fixer.find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from(
				"https://www.rxddit.com/r/fictitious/comments/abc/_/"
			))
		);
	}
	#[test]
	fn find_twitter() {
		let link_fixer = LinkFixer::from_config();
		let string = "blahblah https://x.com/fictitious/status/0123 blahblah";
		let find = link_fixer.find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("https://fixupx.com/fictitious/status/0123"))
		);
	}
	#[test]
	fn find_youtube() {
		let link_fixer = LinkFixer::from_config();
		let string = "blahblah https://www.youtube.com/shorts/GX5wEDmbpQA blahblah";
		let find = link_fixer.find_and_fix(string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from(
				"<https://www.youtube.com/watch?v=GX5wEDmbpQA>"
			))
		);
	}
	#[test]
	fn find_amazon() {
		let link_fixer = LinkFixer::from_config();
		let string = "https://www.amazon.ca/Some-Item-With-Code-ABC012/dp/ABC012?all_sorts_of=tracking.data&other_random=bs&believability_of_the_volume=false";
		let find = link_fixer.find_and_fix(&string).next();
		assert_eq!(
			find.map(|fix| fix.fixed),
			Some(String::from("<https://www.amazon.ca/dp/ABC012>"))
		);
	}
	#[test]
	fn find_each() {
		let link_fixer = LinkFixer::from_config();
		let string = r"hey <https://www.amazon.ca/Some-Item-With-Code-ABC012/dp/ABC012?all_sorts_of=tracking.data&other_random=bs&believability_of_the_volume=false> and https://www.instagram.com/reel/abc blahblah https://www.reddit.com/r/fictitious/comments/abc/def https://x.com/fictitious/status/0123 and https://www.youtube.com/shorts/GX5wEDmbpQA";
		let mut links = link_fixer.find_and_fix(&string);
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
