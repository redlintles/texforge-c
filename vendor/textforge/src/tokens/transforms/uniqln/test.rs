use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Uniqln::new().unwrap();
    for (input, expected) in [
        ("", ""),
        ("a\r\na\nb", "a\r\nb"),
        ("é\né\né", "é\né\n"),
        ("\n\n", "\n"),
        ("🍌\n🍌", "🍌\n"),
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
    let mut token = Uniqln::new().unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
