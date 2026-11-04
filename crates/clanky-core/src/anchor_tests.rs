use super::*;

#[test]
fn masked_command_anchors() {
    let policy = masking::Policy::capture();
    let mask = |lines: &mut [String], wraps: &[bool], cursor| {
        if std::env::var_os("CLANKY_ANCHOR_FAULT").is_some() { return true; }
        policy.mask_lines(lines, wraps, cursor).is_ok()
    };
    // Generated fixture, never an environment credential. Exact expected output
    // is the assignment with its 40-byte value replaced by 40 stars.
    let secret = format!("ghp_{}", "A".repeat(36));
    policy.add_literal("GENERATED_FIXTURE", &secret).unwrap();
    let mut terminal = Terminal::new(3, 80);
    let input = format!("\x1b]133;A\x07$ \x1b]133;B\x07export KEY={secret};\x1b]133;C\x07\r\nok\x1b]133;D;0\x07");
    terminal.feed_masked("shell", input.as_bytes(), &mask);
    let blocks = terminal.blocks().0;
    assert!(blocks[0].command.as_deref() == Some(format!("export KEY={};", "*".repeat(40)).as_str()), "command must contain only masked text");
    assert_eq!((blocks[0].prompt_line, blocks[0].command_line, blocks[0].end_line), (Some(0), Some(0), Some(1)));

    let mut wrapped = Terminal::new(3, 16);
    wrapped.feed_masked("shell", b"\x1b]133;A\x07\x1b]133;B\x07echo abcdefghijklmnop\x1b]133;C\x07\r\nok\x1b]133;D;0\x07", &mask);
    let block = &wrapped.blocks().0[0];
    assert_eq!(block.command.as_deref(), Some("echo abcdefghijklmnop"));
    assert_eq!((block.prompt_line, block.command_line, block.end_line), (Some(0), Some(1), Some(2)));
    // Ten scrolls: history grows, absolute anchors do not move.
    wrapped.feed("shell", &b"\r\n".repeat(10));
    assert_eq!(wrapped.blocks().0[0].command_line, Some(1));
    // Beyond the 10,000-line history limit the original lines are gone.
    wrapped.feed("shell", &b"\r\n".repeat(10000));
    assert_eq!(wrapped.blocks().0[0].command_line, None);
    assert!(wrapped.card_metadata()["blocks"].as_array().unwrap().is_empty());

    terminal.resize(3, 40);
    assert_eq!(terminal.blocks().0[0].command_line, None);
    terminal.feed_masked("shell", b"\r\n\x1b]133;A\x07\x1b]133;B\x07echo ok\x1b]133;C\x07", &mask);
    assert!(terminal.blocks().0.last().unwrap().command_line.is_some());
    let mut plain = Terminal::new(3, 80);
    plain.feed_masked("shell", b"echo ok\r\nok", &mask);
    assert!(plain.blocks().0.is_empty());
    let mut unavailable = Terminal::new(3, 80);
    unavailable.feed("shell", input.as_bytes());
    assert!(unavailable.blocks().0[0].command.is_none());
}

#[test]
fn anchor_eviction_and_duplicate_end() {
    let mut terminal = Terminal::new(3, 80);
    terminal.feed("shell", b"\x1b]133;A\x07\x1b]133;B\x07echo ok\x1b]133;C\x07\r\nok\x1b]133;D;0\x07\r\n\x1b]133;D;0\x07");
    // The second D has no active block, so cannot move the first block's end.
    assert_eq!(terminal.blocks().0[0].end_line, Some(1));
    terminal.feed("shell", &b"\r\n".repeat(10010));
    // Cursor started on absolute line 2. After 10010 line feeds it is 10012;
    // with three visible rows the viewport starts at 10010.
    assert_eq!(terminal.viewport_line(), 10010);
    terminal.feed("shell", b"\x1b]133;B\x07echo next\x1b]133;C\x07");
    assert_eq!(terminal.blocks().0.last().unwrap().command_line, Some(10012));
}

