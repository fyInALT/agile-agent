const CHUNK_SIZE: usize = 10;

pub fn build_reply_chunks(input: &str) -> Vec<String> {
    let reply = format!("Mock reply: {input}");
    let mut chunks = Vec::new();
    let mut current = String::new();

    for ch in reply.chars() {
        current.push(ch);
        if current.chars().count() >= CHUNK_SIZE {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::build_reply_chunks;

    #[test]
    fn reply_chunks_join_back_to_the_full_reply() {
        let chunks = build_reply_chunks("hello");

        assert_eq!(chunks.concat(), "Mock reply: hello");
        assert!(!chunks.is_empty());
    }
}
