use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Dle::new().unwrap();
    for (input, expected) in [
        ("", ""),
        ("\n\r\n", ""),
        (" \n\t\r\n", " \n\t\r\n"),
        ("é🍌é\r\n\r\nx", "é🍌é\r\nx"),
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
    let mut token = Dle::new().unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
