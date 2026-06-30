use crate::error::{CodeGrabberError, Result};

pub fn estimate_tokens(tokenizer: &str, content: &str) -> Result<usize> {
    let bpe =
        match tokenizer {
            "o200k_base" => tiktoken_rs::o200k_base()
                .map_err(|err| CodeGrabberError::Tokenizer(err.to_string()))?,
            "cl100k_base" => tiktoken_rs::cl100k_base()
                .map_err(|err| CodeGrabberError::Tokenizer(err.to_string()))?,
            "p50k_base" => tiktoken_rs::p50k_base()
                .map_err(|err| CodeGrabberError::Tokenizer(err.to_string()))?,
            _ => tiktoken_rs::o200k_base()
                .map_err(|err| CodeGrabberError::Tokenizer(err.to_string()))?,
        };
    Ok(bpe.encode_with_special_tokens(content).len())
}
