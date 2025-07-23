use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::join;

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

async fn chat(req: Request) -> Response {
    let Request::Post(body) = req else {
        return Err(http::StatusCode::METHOD_NOT_ALLOWED);
    };

    let Ok(chat_req) = serde_json::from_str::<ChatRequest>(body.as_str()) else {
        return Err(http::StatusCode::INTERNAL_SERVER_ERROR);
    };

    let messages = Arc::new(chat_req.messages);

    let messages_clone = messages.clone();
    let (generated, idx) = join!(
        tokio::spawn(async move { chatbot::query_chat(&messages_clone).await }),
        chatbot::gen_random_number()
    );

    let generated = generated.unwrap();
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
