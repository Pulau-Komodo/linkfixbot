use serenity::all::{Context, Message, Permissions};

use crate::fix_existing_message::{
	can_react, can_suppress_embeds, fix_existing_message, try_react_and_suppress,
};

async fn get_permissions(context: &Context, message: &Message) -> Option<Permissions> {
	let member = context
		.http
		.get_current_user_guild_member(message.guild_id?)
		.await
		.ok()?;
	message
		.channel(&context)
		.await
		.ok()
		.and_then(|channel| channel.guild())
		.and_then(|channel| {
			message
				.guild(&context.cache)
				.map(|guild| guild.user_permissions_in(&channel, &member))
		})
}

pub async fn fix_links(context: &Context, message: &Message) {
	let permissions = get_permissions(context, message).await;

	let Some((output, intended_embeds, should_suppress_embeds)) =
		fix_existing_message(message, can_suppress_embeds(&permissions)).await
	else {
		return;
	};

	println!("{output}");

	let Ok(own_message) = message.reply(&context.http, output).await else {
		println!("Did not remove embeds because message failed to send");
		return;
	};

	try_react_and_suppress(
		context,
		message,
		Some(&own_message),
		intended_embeds,
		can_react(&permissions),
		should_suppress_embeds,
	)
	.await;
}
