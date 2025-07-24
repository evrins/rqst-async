mod stateful;

use crate::stateful::{StatefulFunction, StatefulThread};
use miniserve::{http, Content, Request, Response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tokio::task::JoinSet;
use tokio::{fs, join};

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
    while let Some(result) = doc_futs.join_next().await {
        docs.push(result.unwrap().unwrap());
    }
    docs
}

struct LogFunction {
    logger: chatbot::Logger,
}

impl StatefulFunction for LogFunction {
    type Input = Arc<Vec<String>>;
    type Output = ();

    async fn call(&mut self, messages: Self::Input) -> Self::Output {
        self.logger.append(messages.last().unwrap());
        self.logger.save().await.unwrap();
    }
}

static LOG_THREAD: LazyLock<StatefulThread<LogFunction>> = LazyLock::new(|| {
    StatefulThread::new(LogFunction {
        logger: chatbot::Logger::default(),
    })
});

struct ChatbotFunction {
    chatbot: chatbot::Chatbot,
}

impl StatefulFunction for ChatbotFunction {
    type Input = Arc<Vec<String>>;
    type Output = Vec<String>;

    async fn call(&mut self, messages: Self::Input) -> Self::Output {
        let doc_paths = self.chatbot.retrieval_documents(&messages);
        let docs = load_docs(doc_paths).await;
        self.chatbot.query_chat(&messages, &docs).await
    }
}

static CHATBOT_THREAD: LazyLock<StatefulThread<ChatbotFunction>> = LazyLock::new(|| {
    StatefulThread::new(ChatbotFunction {
        chatbot: chatbot::Chatbot::new(vec![":-)".into(), "^^".into()]),
    })
});

async fn chat(req: Request) -> Response {
    let Request::Post(body) = req else {
        return Err(http::StatusCode::METHOD_NOT_ALLOWED);
    };

    let Ok(chat_req) = serde_json::from_str::<ChatRequest>(body.as_str()) else {
        return Err(http::StatusCode::INTERNAL_SERVER_ERROR);
    };

    let messages = Arc::new(chat_req.messages);

    let (generated, idx, _) = join!(
        CHATBOT_THREAD.call(messages.clone()),
        chatbot::gen_random_number(),
        LOG_THREAD.call(messages.clone()),
    );

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
    CHATBOT_THREAD.cancel().await;

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
