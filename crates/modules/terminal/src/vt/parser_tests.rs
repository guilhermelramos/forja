    use super::*;
    use forja_kernel::ui_contract::egui::Color32;

    fn tiny() -> Surface {
        let mut surface = Surface::new();
        surface.resize(8, 4);
        surface
    }

    fn row(surface: &Surface, index: usize) -> String {
        surface.lines[index].text().trim_end().to_owned()
    }

    #[test]
    fn titles_and_split_sequences() {
        let mut surface = Surface::new();
        surface.feed(b"\x1b]0;Shell\x07");
        assert_eq!(surface.title, "Shell");
        surface.feed(b"\x1b]2;Project; title\x1b");
        assert_eq!(surface.title, "Shell");
        surface.feed(b"\\");
        assert_eq!(surface.title, "Project; title");
        surface.feed(b"\x1b]2;\x07");
        assert_eq!(surface.title, "");
    }

    #[test]
    fn utf8_split_input_and_colors() {
        let mut surface = Surface::new();
        surface.feed(b"abc\rZ\x1b[2;3H\x1b[31mR");
        assert_eq!(surface.lines[0][0].ch, 'Z');
        assert_eq!(surface.lines[1][2].ch, 'R');
        assert_eq!(surface.lines[1][2].fg_source, 1);
        let bytes = "ação".as_bytes();
        surface.feed(&bytes[..2]);
        surface.feed(&bytes[2..]);
        assert!(surface.text().contains("Ração"));
        surface.feed(b"\x1b[6n");
        assert!(!surface.responses.is_empty());
    }

    #[test]
    fn alternate_buffer_restores_content() {
        let mut surface = Surface::new();
        surface.feed(b"original\x1b[?1049hother\x1b[?1049l");
        assert!(surface.text().contains("original"));
        assert!(!surface.text().contains("other"));
    }

    #[test]
    fn escape_intermediates_index_and_reset() {
        let mut surface = tiny();
        surface.feed(b"\x1b(B\x1b)0\x1b#8\x1b=\x1b>X");
        assert_eq!(row(&surface, 0), "X");
        surface.feed(b"\x1bDq\x1bEr");
        assert_eq!(surface.row, 2);
        assert_eq!(row(&surface, 2), "r");
        surface.feed(b"\x1b[1;1H\x1bM");
        assert_eq!(row(&surface, 0), "");
        assert_eq!(row(&surface, 1), "X");
        surface.feed(b"\x1bc");
        assert_eq!((surface.cols, surface.rows), (8, 4));
        assert!(surface.text().trim().is_empty());
    }

    #[test]
    fn scroll_regions_insert_delete_and_scroll() {
        let mut surface = tiny();
        surface.feed(b"1\x1b[2;1H2\x1b[3;1H3\x1b[4;1H4\x1b[2;3r\x1b[3;1H\n");
        assert_eq!(
            (row(&surface, 0), row(&surface, 1), row(&surface, 2), row(&surface, 3)),
            ("1".into(), "3".into(), "".into(), "4".into())
        );
        surface.feed(b"\x1b[2;1H\x1b[L");
        assert_eq!(row(&surface, 2), "3");
        surface.feed(b"\x1b[M");
        assert_eq!(row(&surface, 1), "3");
        surface.feed(b"\x1b[T");
        assert_eq!(row(&surface, 2), "3");
        surface.feed(b"\x1b[S");
        assert_eq!(row(&surface, 1), "3");
    }

    #[test]
    fn csi_prefix_and_device_attributes() {
        let mut surface = tiny();
        surface.feed(b"\x1b[>c");
        assert_eq!(surface.responses, b"\x1b[>0;1;0c");
        surface.responses.clear();
        surface.feed(b"\x1b[2 q\x1b[=c");
        assert!(surface.responses.is_empty());
        surface.feed(b"\x1b[c");
        assert_eq!(surface.responses, b"\x1b[?1;2c");
    }

    #[test]
    fn sgr_flags_and_extended_colors() {
        let mut surface = tiny();
        surface.feed(b"\x1b[1;2;3;4;7;8;41mA\x1b[22;23;24;27;28;49mB\x1b[38:2::10:20:30;48;5;123mC");
        assert_eq!(surface.lines[0][0].flags, 63);
        assert_eq!(surface.lines[0][0].bg_source, 1);
        assert_eq!(surface.lines[0][1].flags, 0);
        assert_eq!(surface.lines[0][2].fg, Color32::from_rgb(10, 20, 30));
        assert_eq!(surface.lines[0][2].bg_source, 123);
    }

    #[test]
    fn modes_autowrap_alternate_and_cursor_save() {
        let mut surface = tiny();
        surface.feed(b"\x1b[?1;2004h\x1b[?7l123456789");
        assert!(surface.application_cursor && surface.bracketed_paste);
        assert_eq!(row(&surface, 0), "12345679");
        surface.feed(b"\x1b[?1048h\x1b[3;2H\x1b[?1048l");
        assert_eq!(surface.row, 0);
        let before = surface.text();
        for mode in [47, 1047, 1049] {
            surface.feed(format!("\x1b[?{mode}hother\x1b[?{mode}l").as_bytes());
            assert_eq!(surface.text(), before);
        }
    }

    #[test]
    fn malformed_utf8_recovers() {
        let mut surface = tiny();
        surface.feed(&[b'a', 0xff, b'b', 0xe2]);
        assert_eq!(row(&surface, 0), "a\u{fffd}b");
        assert_eq!(surface.pending, vec![0xe2]);
        surface.feed(&[0x82, 0xac, b'c']);
        assert_eq!(row(&surface, 0), "a\u{fffd}b\u{20ac}c");
        assert!(surface.pending.is_empty());
    }

    #[test]
    fn history_is_bounded_by_the_catalog_limit() {
        let mut surface = tiny();
        surface.set_history_limit(1_000);
        for _ in 0..1500 {
            surface.feed(b"\r\nx");
        }
        assert_eq!(surface.history.len(), 1_000);
        surface.set_history_limit(0);
        assert_eq!(surface.history_limit, 1_000);
        surface.set_history_limit(usize::MAX);
        assert_eq!(surface.history_limit, 200_000);
    }

    #[test]
    fn wide_glyphs_survive_copy_and_partial_erase() {
        let mut surface = Surface::new();
        surface.resize(10, 3);
        surface.feed("a\u{4e2d}\u{1f600}z".as_bytes());
        assert_eq!(surface.col, 6);
        assert_eq!(surface.lines[0][1].width, 2);
        assert_eq!(surface.lines[0][2].width, 0);
        assert_eq!(surface.lines[0][5].ch, 'z');
        assert!(surface.text().starts_with("a\u{4e2d}\u{1f600}z"));
        surface.feed(b"\r\x1b[3Gx");
        assert_eq!(surface.lines[0][2].ch, 'x');
    }

    #[test]
    fn emoji_presentation_expands_and_survives_copy() {
        let mut surface = Surface::new();
        surface.feed("\u{2764}\u{fe0f}x".as_bytes());
        assert_eq!(surface.col, 3);
        assert_eq!(surface.lines[0][0].width, 2);
        assert!(surface.text().starts_with("\u{2764}\u{fe0f}x"));
    }

    #[test]
    fn protocol_events_are_reported() {
        let mut surface = Surface::new();
        surface.feed(b"\x07\x1b]52;c;aGVsbG8=\x1b\\\x1b]133;A\x07\x1b]133;D;7\x07");
        assert_eq!(
            surface.signals,
            vec![
                Signal::Bell,
                Signal::Clipboard("hello".into()),
                Signal::Prompt,
                Signal::CommandFinished(Some(7))
            ]
        );
    }

    #[test]
    fn shell_marks_setting_disables_command_events() {
        let mut surface = Surface::new();
        surface.set_shell_marks(false);
        surface.feed(b"\x1b]133;A\x07\x1b]133;D;0\x07");
        assert!(surface.signals.is_empty());
    }

    #[test]
    fn osc8_links_track_cells() {
        let mut surface = Surface::new();
        surface.feed(b"\x1b]8;id=x;https://example.test\x1b\\linked\x1b]8;;\x07plain");
        assert_eq!(
            surface.links[surface.lines[0][0].link as usize],
            "https://example.test"
        );
        assert_eq!(surface.lines[0][6].link, 0);
    }

    #[test]
    fn inline_image_is_gated_by_the_setting() {
        let mut surface = Surface::new();
        surface.resize(10, 5);
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        surface.feed(format!("\x1b]1337;File=inline=1;width=3;height=2:{png}\x07").as_bytes());
        assert_eq!(surface.images.len(), 1);
        assert_eq!((surface.images[0].cols, surface.images[0].rows), (3, 2));
        assert_eq!((surface.row, surface.col), (2, 0));
        surface.set_inline_images(false);
        surface.feed(format!("\x1b]1337;File=inline=1:{png}\x07").as_bytes());
        assert!(surface.images.is_empty());
    }
