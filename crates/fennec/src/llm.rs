//! Model adapter: OpenAI-compatible endpoints (vLLM, llama.cpp, ...).

mod models;
mod openai;
mod sse;

pub use models::{ListedModel, list_models};
pub use openai::OpenAiClient;
