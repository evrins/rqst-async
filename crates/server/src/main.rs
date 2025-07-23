use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};

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
    if let Request::Post(body) = req {
        match serde_json::from_str::<ChatRequest>(body.as_str()) {
            Ok(chat_req) => {
                let mut messages = chat_req.messages;
                messages.push("And how does that make you feel?".to_string());
                let chat_resp = ChatResponse { messages };
                let response = serde_json::to_string(&chat_resp).unwrap();
                Ok(Content::Json(response))
            }
            Err(_) => Err(http::StatusCode::BAD_REQUEST),
        }
    } else {
        Err(http::StatusCode::BAD_REQUEST)
    }
}

#[tokio::main]
async fn main() {
    miniserve::Server::new()
        .route("/", index)
        .route("/chat", chat)
        .run()
        .await
}
