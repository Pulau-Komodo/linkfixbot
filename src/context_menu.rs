use serenity::all::*;

use crate::{
	fix_existing_message::{
		can_react, can_suppress_embeds, fix_existing_message, try_react_and_suppress,
	},
	reply_shortcuts::ReplyShortcuts,
	strings::ERROR_NONE_FOUND,
};

fn take_interacted_message(interaction: &mut CommandInteraction) -> Option<Message> {
	let messages = std::mem::take(&mut interaction.data.resolved.messages);
	messages.into_values().next()
}

pub async fn fix_links(context: &Context, mut interaction: CommandInteraction) {
	let Some(message) = take_interacted_message(&mut interaction) else {
		eprintln!("Did not find a message for some reason.");
		let _ = interaction
			.ephemeral_reply(&context.http, "Did not receive the message.")
			.await;
		return;
	};

	let Some((output, intended_embeds, should_suppress_embeds)) =
		fix_existing_message(&message, can_suppress_embeds(&interaction.app_permissions)).await
	else {
		let _ = interaction
			.ephemeral_reply(&context.http, ERROR_NONE_FOUND)
			.await;
		return;
	};

	let result = interaction.public_reply(&context.http, output).await;
	if result.is_err() {
		println!("Did not remove embeds because message failed to send");
		return;
	};

	try_react_and_suppress(
		context,
		&message,
		interaction.get_response(&context.http).await.ok().as_ref(),
		intended_embeds,
		can_react(&interaction.app_permissions),
		should_suppress_embeds,
	)
	.await;
}

pub fn create_command() -> CreateCommand {
	CreateCommand::new("fix links")
		.description("")
		.kind(CommandType::Message)
		.contexts(vec![
			InteractionContext::Guild,
			InteractionContext::BotDm,
			InteractionContext::PrivateChannel,
		])
}
