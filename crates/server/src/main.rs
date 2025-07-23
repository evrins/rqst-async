use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use tokio::join;
use tokio::sync::{mpsc, oneshot};

async fn index(_req: Request) -> Response {
    let content = include_str!("../index.html").to_string();
    Ok(Content::Html(content))
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    messages: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    messages: Vec<String>,
}

async fn query_chat(messages: &Arc<Vec<String>>) -> Vec<String> {
    type Payload = (Arc<Vec<String>>, oneshot::Sender<Vec<String>>);

    static SENDER: LazyLock<mpsc::Sender<Payload>> = LazyLock::new(|| {
        let (tx, mut rx) = mpsc::channel::<Payload>(1024);
        tokio::spawn(async move {
            let mut chatbot = chatbot::Chatbot::new(vec![":-)".to_string(), "^^".to_string()]);
            while let Some((messages, responder)) = rx.recv().await {
                let response = chatbot.query_chat(&messages).await;
                responder.send(response).unwrap();
            }
        });
        tx
    });

    let (tx, rx) = oneshot::channel();
    SENDER.send((messages.clone(), tx)).await.unwrap();
    rx.await.unwrap()
}

async fn chat(req: Request) -> Response {
    let Request::Post(body) = req else {
        return Err(http::StatusCode::METHOD_NOT_ALLOWED);
    };

    let Ok(chat_req) = serde_json::from_str::<ChatRequest>(body.as_str()) else {
        return Err(http::StatusCode::INTERNAL_SERVER_ERROR);
    };

    let messages = Arc::new(chat_req.messages);

    let (generated, idx) = join!(query_chat(&messages), chatbot::gen_random_number());

    let new_message = generated[idx % generated.len()].clone();
    let mut messages = Arc::into_inner(messages).unwrap();
    messages.push(new_message);

    let chat_resp = ChatResponse { messages };
    let response = serde_json::to_string(&chat_resp).unwrap();
    Ok(Content::Json(response))
}

#[tokio::main]
async fn main() {
    miniserve::Server::new()
        .route("/", index)
        .route("/chat", chat)
        .run()
        .await
}
