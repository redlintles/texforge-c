use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Inl::new(2, "x").unwrap();
    for (input, expected) in [
        ("a\nb", "a\nx\nb"),
        ("é🍌é", "é🍌é\nx"),
        ("a\r\nb\r\n", "a\r\nx\r\nb\r\n"),
        ("\n", "\nx\n"),
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
    let mut token = Inl::new(2, "x").unwrap();
    let before = token.to_textforge_line();
    assert!(
        token
            .set_params(&[String::new().into(), 1usize.into()])
            .is_err()
    );
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Inl::new(2, "x").unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn indices_are_checked_without_overflow() {
    assert!(Inl::new(0, "x").is_err());
    assert!(
        Inl::new(usize::MAX, "x")
            .unwrap()
            .transform("a".into(), None)
            .is_err()
    );
}
#[test]
fn rejects_multiline_payload() {
    assert!(Inl::new(1, "x\ny").is_err());
}
#[test]
fn empty_payload_represents_a_real_line() {
    assert_eq!(
        Inl::new(1, "").unwrap().transform("".into(), None).unwrap(),
        "\n"
    );
}
