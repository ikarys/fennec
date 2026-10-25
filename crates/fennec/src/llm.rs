//! Model adapter: OpenAI-compatible endpoints (vLLM, llama.cpp, ...).

mod openai;
mod sse;

pub use openai::OpenAiClient;
