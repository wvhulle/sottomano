use crate::widgets::simple_icons::SimpleIcon;

#[must_use]
pub fn program_brand_icon(program_name: &str) -> Option<SimpleIcon> {
    match program_name {
        "nu" | "nushell" => Some(SimpleIcon::Nushell),
        "bash" => Some(SimpleIcon::GNUBash),
        "zsh" => Some(SimpleIcon::Zsh),
        "nvim" | "neovim" => Some(SimpleIcon::Neovim),
        "vim" => Some(SimpleIcon::Vim),
        "hx" | "helix" => Some(SimpleIcon::Helix),
        "claude" => Some(SimpleIcon::ClaudeCode),
        "cargo" | "rustc" | "rustup" | "bacon" => Some(SimpleIcon::Rust),
        "nix" | "nh" | "nom" | "nixos-rebuild" | "home-manager" => Some(SimpleIcon::NixOS),
        "git" | "jj" | "lazygit" | "gitui" => Some(SimpleIcon::Git),
        "uv" | "ipython" | "pip" => Some(SimpleIcon::Python),
        python if python.starts_with("python") => Some(SimpleIcon::Python),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::program_brand_icon;
    use crate::widgets::simple_icons::SimpleIcon;

    #[test]
    fn a_nushell_prompt_wears_the_nushell_logo() {
        assert_eq!(program_brand_icon("nu"), Some(SimpleIcon::Nushell));
    }

    #[test]
    fn versioned_python_interpreters_wear_the_python_logo() {
        assert_eq!(program_brand_icon("python3.12"), Some(SimpleIcon::Python));
    }

    #[test]
    fn an_unknown_program_has_no_brand_logo() {
        assert_eq!(program_brand_icon("tokei"), None);
    }
}
