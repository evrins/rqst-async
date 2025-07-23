use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};
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

    let mut messages = chat_req.messages;
    let (generated, idx) = join!(chatbot::query_chat(&messages), chatbot::gen_random_number());
    messages.push(generated[idx % generated.len()].clone());
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
