pub fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}
