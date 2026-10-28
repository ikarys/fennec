//! Model adapter: OpenAI-compatible endpoints (vLLM, llama.cpp, ...).

mod openai;

pub use openai::OpenAiClient;
