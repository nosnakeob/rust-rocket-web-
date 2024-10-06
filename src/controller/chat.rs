use std::future;
use rocket::futures::{SinkExt, StreamExt, TryStreamExt};
use rocket::State;
use rocket::tokio::try_join;
use rocket_ws::{
    Channel, Message, WebSocket,
    frame::{CloseCode, CloseFrame},
};
use openchat_bot::ChatBot;
use redis::{AsyncCommands, Client, Commands};
use tokio_stream::wrappers::BroadcastStream;
use web_common::core::resp::R;
use crate::domain::chat::ChatMessage;
use crate::domain::user::User;

rocket_base_path!("/chat");


/// 建立WebSocket连接, 全局聊天室
/// api文档不能WebSocket连接时发token, 所以这里用id来代替token
#[get("/connect")]
pub async fn connect(ws: WebSocket, user: User, redis_client: &State<Client>) -> Channel<'_> {
    ws.channel(move |stream| Box::pin(async move {
        let (write, read) = stream.split();

        let (mut rsink, mut rstream) = redis_client.get_async_pubsub().await.unwrap().split();
        rsink.subscribe("global room").await.unwrap();

        let mut conn = redis_client.get_connection().unwrap();
        let _: () = conn.publish("global room", ChatMessage::new_user_online(&user.username)).unwrap();

        // 接收用户消息, 广播给其他用户
        let broadcast_task = read.try_for_each(|msg| {
            match msg {
                Message::Text(msg) => {
                    // println!("recv msg: {}", msg);
                    let mut chat_msg = ChatMessage::from_json_str(&msg).unwrap();

                    if let ChatMessage::UserMessage { ref mut username, content } = &mut chat_msg {
                        username.get_or_insert(user.username.clone());
                    }

                    let _: () = conn.publish("global room", chat_msg).unwrap();
                }
                Message::Close(_close_msg) => {
                    let _: () = conn.publish("global room", ChatMessage::new_user_offline(&user.username)).unwrap();
                }
                _ => {}
            }

            future::ready(Ok(()))
        });

        // 订阅通道转发给websocket流
        let forward_task = rstream
            .filter_map(|msg| future::ready(msg.get_payload().ok()))
            .map(Message::Text)
            .map(Ok)
            .forward(write);

        // todo check alive

        if let Err(err) = try_join!(broadcast_task, forward_task) {
            // eprintln!("{}", err);

            info!("{} disconnect", user.username);
        }

        Ok(())
    }))
}

// #[utoipa::path(context_path = BASE)]
// #[delete("/<id>")]
// pub async fn kick(id: u32, clients: &State<ClientMap>) -> R {
//     clients.read().unwrap()[&id].unbounded_send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "管理员踢出".into() }))).unwrap();
//     R::no_val_success()
// }

// #[utoipa::path(context_path = BASE)]
// #[get("/status")]
// pub async fn status(clients: &State<ClientMap>) -> R {
//     R::success(clients.read().unwrap().keys().collect::<Vec<_>>())
// }

// 聊天机器人
#[utoipa::path(context_path = BASE)]
#[get("/connect_bot")]
pub async fn connect_bot(ws: WebSocket, _user: User) -> Channel<'static> {
    ws.channel(move |mut stream| Box::pin(async move {
        let mut bot = ChatBot::from_default_args().await.unwrap();

        while let Some(Ok(Message::Text(msg))) = stream.next().await {
            let mut rx = bot.chat(msg);

            let cap = 32;
            let mut buf = Vec::with_capacity(cap);
            while rx.recv_many(&mut buf, cap).await > 0 {
                let chat_msg = ChatMessage::new_bot_msg(&buf.join(""));
                stream.send(Message::Text(chat_msg.to_json_str().unwrap())).await?;
                buf.clear();
            }
        }

        Ok(())
    }))
}
