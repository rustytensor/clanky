//! The snapshot carries each SGR text attribute a renderer needs, on exactly the cells the program set it on.
use clanky_core::{
    STYLE_BOLD, STYLE_DOUBLE_UNDERLINE, STYLE_FAINT, STYLE_STRIKEOUT, STYLE_TERMINAL_ITALIC,
    STYLE_UNDERCURL, STYLE_UNDERLINE, snapshot_from_chunks,
};

#[test]
fn sgr_attributes_reach_the_snapshot() {
    // One letter per attribute, each reset after itself, then a plain letter. Expected bits follow ECMA-48 for
    // SGR 1, 2, 3, 4 and 9, and the kitty underline extension for 4:3 (curly) and 4:2 (double). SGR 21 is left
    // out: clanky-vte reads it as "bold off" (crates/clanky-vte/src/ansi.rs, as Alacritty and the Linux
    // console do), not as ECMA-48's double underline.
    let line = b"\x1b[1mB\x1b[0m\x1b[2mF\x1b[0m\x1b[3mI\x1b[0m\x1b[4mU\x1b[0m\x1b[4:3mC\x1b[0m\
\x1b[9mS\x1b[0m\x1b[4:2mE\x1b[0m\x1b[3;9mX\x1b[0mP";
    let snapshot = snapshot_from_chunks(2, 20, &[line]).expect("a 2 x 20 terminal is a valid size");
    let expected = [
        ('B', STYLE_BOLD),
        ('F', STYLE_FAINT),
        ('I', STYLE_TERMINAL_ITALIC),
        ('U', STYLE_UNDERLINE),
        ('C', STYLE_UNDERCURL),
        ('S', STYLE_STRIKEOUT),
        ('E', STYLE_DOUBLE_UNDERLINE),
        ('X', STYLE_TERMINAL_ITALIC | STYLE_STRIKEOUT),
        ('P', 0),
    ];
    let got: Vec<(char, u8)> = snapshot.cells[..expected.len()]
        .iter()
        .map(|cell| (cell.ch, cell.style))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn cursor_shape_follows_decscusr() {
    // DECSCUSR (xterm ctlseqs): 2 steady block, 4 steady underline, 6 steady bar; 0 resets to the default block.
    let shape = |sequence: &[u8]| {
        snapshot_from_chunks(2, 10, &[sequence])
            .expect("a 2 x 10 terminal is a valid size")
            .cursor_shape
    };
    assert_eq!(shape(b"\x1b[2 q"), clanky_core::CURSOR_BLOCK);
    assert_eq!(shape(b"\x1b[4 q"), clanky_core::CURSOR_UNDERLINE);
    assert_eq!(shape(b"\x1b[6 q"), clanky_core::CURSOR_BAR);
    assert_eq!(shape(b"\x1b[6 q\x1b[0 q"), clanky_core::CURSOR_BLOCK);
}
