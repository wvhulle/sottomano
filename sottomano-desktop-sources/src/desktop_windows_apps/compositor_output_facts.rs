#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub mode: Option<OutputMode>,
    pub scale: f64,
    pub logical_width: u32,
    pub logical_height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: f64,
}

#[must_use]
pub fn product_field(edid_field: String) -> Option<String> {
    let trimmed = edid_field.trim();
    let hexadecimal_code = trimmed.strip_prefix("0x").is_some_and(|rest| {
        !rest.is_empty() && rest.chars().all(|character| character.is_ascii_hexdigit())
    });
    let placeholder = trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown");
    (!placeholder && !hexadecimal_code).then_some(edid_field)
}

#[cfg(test)]
mod tests {
    use super::product_field;

    fn named_product(edid_field: &str) -> Option<String> {
        product_field(edid_field.to_owned())
    }

    #[test]
    fn edid_product_codes_are_not_product_names() {
        assert_eq!(named_product("0x07BE"), None);
        assert_eq!(named_product("0x0000ffa1"), None);
        assert_eq!(named_product(""), None);
        assert_eq!(named_product("   "), None);
        assert_eq!(named_product("Unknown"), None);
    }

    #[test]
    fn real_edid_strings_are_product_names() {
        assert!(named_product("LG Display").is_some());
        assert!(named_product("U2720Q").is_some());
        assert!(named_product("0xSomething").is_some());
    }
}
