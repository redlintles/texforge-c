use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Apl::new("b").unwrap();
    for (input, expected) in [
        ("", "b"),
        ("a\n", "a\nb\n"),
        ("é🍌é\r\n", "é🍌é\r\nb\r\n"),
        ("\r\n", "\r\nb\r\n"),
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
    let mut token = Apl::new("b").unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Apl::new("b").unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn rejects_multiline_payload() {
    assert!(Apl::new("x\ny").is_err());
}
#[test]
fn empty_payload_represents_a_real_line() {
    assert_eq!(
        Apl::new("").unwrap().transform("".into(), None).unwrap(),
        "\n"
    );
}
