use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Rln::new(2, "x").unwrap();
    for (input, expected) in [
        ("a\nb", "a\nx"),
        ("a\r\nb\r\n", "a\r\nx\r\n"),
        ("\n\n", "\nx\n"),
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
    let mut token = Rln::new(2, "x").unwrap();
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
    let mut token = Rln::new(2, "x").unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn indices_are_checked_without_overflow() {
    assert!(Rln::new(0, "x").is_err());
    assert!(
        Rln::new(usize::MAX, "x")
            .unwrap()
            .transform("a".into(), None)
            .is_err()
    );
}
#[test]
fn rejects_multiline_payload() {
    assert!(Rln::new(1, "x\ny").is_err());
}
#[test]
fn empty_document_has_no_addressable_lines() {
    assert!(
        Rln::new(2, "x")
            .unwrap()
            .transform("".into(), None)
            .is_err()
    );
}
#[test]
fn empty_payload_represents_a_real_line() {
    assert_eq!(
        Rln::new(2, "")
            .unwrap()
            .transform("a\nb".into(), None)
            .unwrap(),
        "a\n\n"
    );
}
