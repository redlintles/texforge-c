use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Joinln::new(" ").unwrap();
    for (input, expected) in [
        ("", ""),
        ("é🍌é", "é🍌é"),
        ("a\r\nb\rc\n", "a b c"),
        ("\n\n", " "),
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
    let mut token = Joinln::new(" ").unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Joinln::new(" ").unwrap();
    assert!(token.set_params(&[]).is_err());
}
