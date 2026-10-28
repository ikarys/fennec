//! Model adapter: OpenAI-compatible endpoints (vLLM, llama.cpp, ...).

mod concurrency;
mod openai;

pub use concurrency::ModelGate;
pub use openai::OpenAiClient;
