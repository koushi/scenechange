use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::{
    api::APIBuilder,
    peer_connection::{configuration::RTCConfiguration, RTCPeerConnection},
};

struct AppState {
    peer_connection: Mutex<Option<Arc<RTCPeerConnection>>>,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        peer_connection: Mutex::new(None),
    });

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .route("/api/upload", post(upload_image_handler))
        .with_state(state);

    println!("Server running at http://127.0.0.1:3000");
    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

async fn handle_websocket(mut socket: WebSocket, state: Arc<AppState>) {
    let api = APIBuilder::new().build();
    let config = RTCConfiguration::default();

    let peer_connection = Arc::new(api.new_peer_connection(config).await.unwrap());

    peer_connection.on_data_channel(Box::new(|data_channel| {
        data_channel.on_message(Box::new(|msg| {
            println!("Received on DataChannel: {:?}", msg);
            Box::pin(async {})
        }));
        Box::pin(async {})
    }));

    {
        let mut pc_lock = state.peer_connection.lock().await;
        *pc_lock = Some(peer_connection);
    }

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            println!("Received message: {}", text);
        }
    }
}

async fn upload_image_handler(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let new_image_url = "/static/images/new_image.jpg";
    notify_image_update(state, new_image_url).await;
    axum::response::Json(json!({ "status": "success", "new_image_url": new_image_url }))
}

async fn notify_image_update(state: Arc<AppState>, new_image_url: &str) {
    if let Some(peer_connection) = state.peer_connection.lock().await.as_ref() {
        let new_image_url = new_image_url.to_string(); // Convert to owned String

        // Register on_data_channel callback
        peer_connection.on_data_channel(Box::new(move |data_channel| {
            // Clone data_channel for inner use
            let cloned_data_channel = data_channel.clone();
            let new_image_url_for_open = new_image_url.clone(); // Clone for the on_open callback

            // Register on_open callback
            data_channel.on_open(Box::new(move || {
                let data_channel = cloned_data_channel.clone(); // Clone for async move block
                let new_image_url = new_image_url_for_open.clone(); // Clone for async block
                Box::pin(async move {
                    let update_message = json!({ "type": "imageUpdate", "imageUrl": new_image_url }).to_string();
                    data_channel.send_text(&update_message).await.unwrap();
                })
            }));

            // Ensure the callback returns a future
            Box::pin(async {})
        }));
    }
}


