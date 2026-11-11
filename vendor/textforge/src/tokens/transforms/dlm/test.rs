use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Dlm::new("^b$").unwrap();
    for (input, expected) in [
        ("", ""),
        ("b\r\na\nb", "a\n"),
        ("a\r\nb\r\n", "a\r\n"),
        ("é🍌é", "é🍌é"),
    ] {
        assert_eq!(
            token.transform(input.into(), None).unwrap(),
            expected,
            "input={input:?}"
        );
    }
}
#[test]
fn invalid_binding_is_atomic() {
    let mut token = Dlm::new("^b$").unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Dlm::new("^b$").unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn regex_errors_and_content_anchors() {
    assert!(Dlm::new("[").is_err());
    let token = Dlm::new("^é🍌e\u{301}$").unwrap();
    assert_eq!(
        token.transform("é🍌e\u{301}\r\nx".into(), None).unwrap(),
        "x"
    );
}

#[test]
fn valid_posix_character_classes_are_not_unresolved_parameters() {
    let token = Dlm::new("^[[:alpha:]]+$").unwrap();
    assert_eq!(
        token.transform("abc\r\n123\nXYZ".into(), None).unwrap(),
        "123\n"
    );
}
