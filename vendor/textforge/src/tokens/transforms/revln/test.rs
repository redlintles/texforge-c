use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Revln::new().unwrap();
    for (input, expected) in [
        ("\na", "a\n\n"),
        ("", ""),
        ("é🍌é", "é🍌é"),
        ("a\r\nb\nc", "c\r\nb\na"),
        ("a\r\nb\r\n", "b\r\na\r\n"),
        ("\n\n", "\n\n"),
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
    let mut token = Revln::new().unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
