use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::{fs, join, select};

async fn index(_req: Request) -> Response {
    let content = include_str!("../index.html").to_string();
    Ok(Content::Html(content))
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    messages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "messages")]
enum ChatResponse {
    Cancelled,
    Success(Vec<String>),
}

async fn load_docs(paths: Vec<PathBuf>) -> Vec<String> {
    let mut doc_futs = paths
        .into_iter()
        .map(fs::read_to_string)
        .collect::<JoinSet<_>>();

    let mut docs = Vec::new();
    while let Some(doc) = doc_futs.join_next().await {
        docs.push(doc.unwrap().unwrap());
    }

    docs
}

type Payload = (Arc<Vec<String>>, oneshot::Sender<Option<Vec<String>>>);

fn chatbot_thread() -> (mpsc::Sender<Payload>, mpsc::Sender<()>) {
    let (request_tx, mut request_rx) = mpsc::channel::<Payload>(1024);
    let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        let mut cb = chatbot::Chatbot::new(vec![":-)".to_string(), "^^".to_string()]);
        while let Some((messages, responder)) = request_rx.recv().await {
            let paths = cb.retrieval_documents(&messages);
            let contents = load_docs(paths).await;
            let chat_fut = cb.query_chat(&messages, &contents);
            let cancel_fut = cancel_rx.recv();
            select! {
                response = chat_fut => {
                    responder.send(Some(response)).unwrap();
                }
                _ = cancel_fut => {
                    responder.send(None).unwrap();
                }
            }
        }
    });
    (request_tx, cancel_tx)
}

static CHATBOT_THREAD: LazyLock<(mpsc::Sender<Payload>, mpsc::Sender<()>)> =
    LazyLock::new(chatbot_thread);

async fn query_chat(messages: &Arc<Vec<String>>) -> Option<Vec<String>> {
    let (tx, rx) = oneshot::channel();

    CHATBOT_THREAD.0.send((messages.clone(), tx)).await.unwrap();

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

    let chat_resp = match generated {
        Some(generated_messages) => {
            let msg = generated_messages[idx % generated_messages.len()].clone();
            let mut messages = Arc::into_inner(messages).unwrap();
            messages.push(msg);
            ChatResponse::Success(messages)
        }
        None => ChatResponse::Cancelled,
    };

    let response = serde_json::to_string(&chat_resp).unwrap();
    Ok(Content::Json(response))
}

async fn cancel(_req: Request) -> Response {
    CHATBOT_THREAD.1.send(()).await.unwrap();

    Ok(Content::Html("success".into()))
}

#[tokio::main]
async fn main() {
    miniserve::Server::new()
        .route("/", index)
        .route("/chat", chat)
        .route("/cancel", cancel)
        .run()
        .await
}
