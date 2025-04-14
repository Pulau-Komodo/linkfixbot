use serenity::all::{Context, Message, Permissions};

use crate::fix_existing_message::{
	can_react, can_suppress_embeds, fix_existing_message, try_react_and_suppress,
};

async fn get_permissions(context: &Context, message: &Message) -> Option<Permissions> {
	let guild = message.guild_id?.to_guild_cached(&context.cache)?;
	let member = guild.members.get(&context.cache.current_user().id)?;
	let channel = guild.channels.get(&message.channel_id)?;
	Some(guild.user_permissions_in(channel, member))
}

pub async fn fix_links(context: &Context, message: &Message) {
	let permissions = get_permissions(context, message).await;

	let Some((output, embeds_to_suppress)) = fix_existing_message(message).await else {
		return;
	};

	let Ok(own_message) = message.reply(&context.http, output).await else {
		println!("Did not remove embeds because message failed to send");
		return;
	};

	try_react_and_suppress(
		context,
		message,
		Some(&own_message),
		embeds_to_suppress,
		can_react(&permissions),
		can_suppress_embeds(&permissions),
	)
	.await;
}
