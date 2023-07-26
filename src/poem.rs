use futures_util::{stream::StreamExt, SinkExt};
use poem::{
    get, handler,
    listener::TcpListener,
    web::websocket::{Message, WebSocket, WebSocketStream},
    IntoResponse, Route,
};
use serde::{Deserialize, Serialize};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    time::Duration,
};
use tokio::{
    spawn,
    sync::{
        broadcast,
        mpsc::{self, UnboundedSender},
    },
    time::sleep,
};

use crate::{stream::Stream, SessionDescriptor};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientMessage {
    Play(Session),
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Session {
    Sdp(String),
    Custom(SessionDescriptor),
}

#[handler]
async fn ws(ws: WebSocket) -> impl IntoResponse {
    ws.protocols(vec!["aes67-to-ws"])
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve(socket).await {
                log::error!("Error in WS connection: {e}");
            }
        })
}

pub async fn start() -> anyhow::Result<()> {
    let app = Route::new().nest(format!("/ws"), get(ws));
    poem::Server::new(TcpListener::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        9999,
    )))
    .run(app)
    .await?;
    Ok(())
}

async fn serve(websocket: WebSocketStream) -> anyhow::Result<()> {
    let (payload_tx, mut payload_rx) = mpsc::unbounded_channel();
    let (stop_tx, _stop_rx) = broadcast::channel(100);
    let (mut ws_tx, mut ws_rx) = websocket.split();

    spawn(async move {
        while let Some(rtp_payload) = payload_rx.recv().await {
            let msg = Message::Binary(rtp_payload);
            if let Err(e) = ws_tx.send(msg).await {
                log::error!("Error forwarding rtp payload: {e}");
                break;
            }
        }
    });

    loop {
        if let Some(Ok(incoming_msg)) = ws_rx.next().await {
            if let Message::Text(json) = incoming_msg {
                if let Ok(client_message) = serde_json::from_str(&json) {
                    match client_message {
                        ClientMessage::Play(session) => {
                            if let Some(sd) = match session {
                                Session::Sdp(sdp) => sdp.parse().ok(),
                                Session::Custom(sd) => Some(sd),
                            } {
                                play(sd, payload_tx.clone(), stop_tx.clone()).await?;
                            }
                        }
                        ClientMessage::Stop => {
                            stop_tx.send(()).ok();
                        }
                    }
                }
            }
        } else {
            stop_tx.send(()).ok();
            log::info!("Client disconnected.");
            break;
        }
    }

    Ok(())
}

async fn play(
    sd: SessionDescriptor,
    payload_tx: UnboundedSender<Vec<u8>>,
    stop_tx: broadcast::Sender<()>,
) -> anyhow::Result<()> {
    stop_tx.send(()).ok();
    sleep(Duration::from_millis(100)).await;
    log::info!("Playing {sd:?}");
    let mut stream = Stream::new(sd, Ipv4Addr::UNSPECIFIED).await?;
    stream.play(payload_tx, stop_tx).await?;
    log::info!("Stream started.");
    Ok(())
}
