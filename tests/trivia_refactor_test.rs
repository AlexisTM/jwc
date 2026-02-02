use jwc::Trivia;

#[test]
fn test_trivia_conversion() {
    let mut line = Trivia::LineComment(" foo".to_string());
    line.make_block_comment();
    if let Trivia::BlockComment(c) = line {
        assert_eq!(c, " foo");
    } else {
        panic!("Expected BlockComment");
    }

    let mut block = Trivia::BlockComment(" bar ".to_string());
    block.make_line_comment();
    if let Trivia::LineComment(c) = block {
        assert_eq!(c, " bar ");
    } else {
        panic!("Expected LineComment");
    }
}
