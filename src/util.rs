use std::sync::LazyLock;

use regex::Regex;

static SPOILERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|\|").unwrap());

/// Overly aggressive function detecting any possible presence of spoiler tags.
pub fn has_spoilers(str: &str) -> bool {
	SPOILERS.captures_iter(str).nth(1).is_some()
}
