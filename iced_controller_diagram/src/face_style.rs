#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FaceStyle {
    PlayStation,
    #[default]
    XboxLetters,
    NintendoLetters,
    SwappedLetters,
    Unlabeled,
}
