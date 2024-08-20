use std::sync::Arc;

use serenity::{
	all::CommandInteraction,
	async_trait,
	builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
	http::Http,
	Result as SerenityResult,
};

#[async_trait]
pub trait ReplyShortcuts {
	async fn reply<S>(&self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Into<String> + Send;
	async fn ephemeral_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + std::marker::Send;
	async fn public_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + std::marker::Send;
}

#[async_trait]
impl ReplyShortcuts for CommandInteraction {
	async fn reply<S>(&self, http: &Arc<Http>, content: S, ephemeral: bool) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.create_response(
			http,
			CreateInteractionResponse::Message(
				CreateInteractionResponseMessage::new()
					.content(content)
					.ephemeral(ephemeral),
			),
		)
		.await
	}
	async fn ephemeral_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.reply(http, content, true).await
	}
	async fn public_reply<S>(&self, http: &Arc<Http>, content: S) -> SerenityResult<()>
	where
		S: Into<String> + Send,
	{
		self.reply(http, content, false).await
	}
}
