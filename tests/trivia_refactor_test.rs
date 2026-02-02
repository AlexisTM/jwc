use jwc::Trivia;

#[test]
fn test_trivia_constructors() {
    let line = Trivia::line(" foo");
    assert!(line.is_line());
    assert_eq!(line.text(), " foo");

    let block = Trivia::block(" bar ");
    assert!(block.is_block());
    assert_eq!(block.text(), " bar ");
}

#[test]
fn test_trivia_from_str_is_line() {
    let t: Trivia = " note".into();
    assert!(t.is_line());
    assert_eq!(t.text(), " note");
}
