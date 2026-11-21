#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub make: EdidProductText,
    pub model: EdidProductText,
    pub serial: EdidProductText,
    pub mode: OutputModeReport,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdidProductText {
    Unreported,
    Reported(String),
}

#[derive(Debug, Clone, Copy)]
pub enum OutputModeReport {
    Unreported,
    Current(OutputMode),
}

#[must_use]
pub fn product_field(edid_field: String) -> EdidProductText {
    let trimmed = edid_field.trim();
    let hexadecimal_code = trimmed.strip_prefix("0x").is_some_and(|rest| {
        !rest.is_empty() && rest.chars().all(|character| character.is_ascii_hexdigit())
    });
    let placeholder = trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown");
    if placeholder || hexadecimal_code {
        EdidProductText::Unreported
    } else {
        EdidProductText::Reported(edid_field)
    }
}

#[cfg(test)]
mod tests {
    use super::{EdidProductText, product_field};

    fn named_product(edid_field: &str) -> EdidProductText {
        product_field(edid_field.to_owned())
    }

    #[test]
    fn edid_product_codes_are_not_product_names() {
        assert_eq!(named_product("0x07BE"), EdidProductText::Unreported);
        assert_eq!(named_product("0x0000ffa1"), EdidProductText::Unreported);
        assert_eq!(named_product(""), EdidProductText::Unreported);
        assert_eq!(named_product("   "), EdidProductText::Unreported);
        assert_eq!(named_product("Unknown"), EdidProductText::Unreported);
    }

    #[test]
    fn real_edid_strings_are_product_names() {
        for product_name in ["LG Display", "U2720Q", "0xSomething"] {
            assert_eq!(
                named_product(product_name),
                EdidProductText::Reported(product_name.to_owned())
            );
        }
    }
}
