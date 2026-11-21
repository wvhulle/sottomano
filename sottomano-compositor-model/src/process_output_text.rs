#[must_use]
pub fn shown_process_text(process_bytes: &[u8]) -> String {
    process_bytes
        .utf8_chunks()
        .fold(String::new(), |mut shown_text, utf8_chunk| {
            shown_text.push_str(utf8_chunk.valid());
            shown_text.extend(utf8_chunk.invalid().escape_ascii().map(char::from));
            shown_text
        })
}
