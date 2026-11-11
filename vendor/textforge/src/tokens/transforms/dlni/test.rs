use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Dlni::new(2, 2).unwrap();
    for (input, expected) in [
        ("a\nb\nc", "a\nc"),
        ("a\r\né🍌é\r\nc", "a\r\nc"),
        ("\n\n", "\n"),
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
    let mut token = Dlni::new(2, 2).unwrap();
    let before = token.to_textforge_line();
    assert!(
        token
            .set_params(&[String::new().into(), String::new().into()])
            .is_err()
    );
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Dlni::new(2, 2).unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn indices_are_checked_without_overflow() {
    assert!(Dlni::new(0, 0).is_err());
    assert!(
        Dlni::new(usize::MAX, usize::MAX)
            .unwrap()
            .transform("a".into(), None)
            .is_err()
    );
}
#[test]
fn empty_document_has_no_addressable_lines() {
    assert!(Dlni::new(2, 2).unwrap().transform("".into(), None).is_err());
}
#[test]
fn inverted_range_is_rejected() {
    assert!(
        Dlni::new(2, 1)
            .unwrap()
            .transform("a\nb".into(), None)
            .is_err()
    );
}
