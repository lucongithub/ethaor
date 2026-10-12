//! The compile-time builder, tested on its own source.
//!
//! `build.rs` is what turns 12 MB of Mechon-Mamre HTML into the two blocks the
//! module welds in. Its traps — the unassigned byte 0xCA, the cover page, the
//! maqaf, the books the source gives in one piece — are checked here, on the
//! very functions the build runs — `build/mamre.rs`, included rather than
//! copied, so that the two can never drift apart.

#[allow(dead_code, unused_imports)]
mod mamre {
    use std::collections::BTreeMap;

    include!("../build/mamre.rs");

    #[cfg(test)]
    mod tests {
        use super::*;

        // -------------------------------------------------------- decoding

        /// Mechon-Mamre uses byte 0xCA, unassigned in standard cp1255, for
        /// U+05BA. Without the fix, 414 signs are lost in the Tanakh.
        #[test]
        fn the_unassigned_byte_of_mechon_mamre_is_decoded() {
            assert_eq!(decode_cp1255(&[0xCA]), "\u{05BA}");
        }

        #[test]
        fn cp1255_maps_the_three_hebrew_ranges() {
            // Niqqud and cantillation, U+05B0 onwards.
            assert_eq!(decode_cp1255(&[0xC0]), "\u{05B0}");
            assert_eq!(decode_cp1255(&[0xCF]), "\u{05BF}");
            // Punctuation, U+05C0 onwards.
            assert_eq!(decode_cp1255(&[0xD0]), "\u{05C0}");
            // Consonants, U+05D0 onwards: alef to tav.
            assert_eq!(decode_cp1255(&[0xE0]), "א");
            assert_eq!(decode_cp1255(&[0xFA]), "ת");
            // ASCII passes through, NBSP becomes a space, the rest separates.
            assert_eq!(decode_cp1255(b"ab,"), "ab,");
            assert_eq!(decode_cp1255(&[0xA0]), " ");
            assert_eq!(decode_cp1255(&[0x80]), " ");
        }

        #[test]
        fn decoding_never_loses_a_byte() {
            let bytes: Vec<u8> = (0u8..=255).collect();
            assert_eq!(decode_cp1255(&bytes).chars().count(), bytes.len());
        }

        // -------------------------------------------------------- numerals

        /// `15` and `16` are written `טו` and `טז`, never `יה` / `יו`, which
        /// would form a divine name.
        #[test]
        fn fifteen_and_sixteen_avoid_the_divine_name() {
            assert_eq!(hebrew_numeral(15), "טו");
            assert_eq!(hebrew_numeral(16), "טז");
            assert_eq!(hebrew_numeral(115), "קטו");
            assert_eq!(hebrew_numeral(116), "קטז");
        }

        #[test]
        fn the_numerals_follow_the_hebrew_notation() {
            assert_eq!(hebrew_numeral(1), "א");
            assert_eq!(hebrew_numeral(9), "ט");
            assert_eq!(hebrew_numeral(10), "י");
            assert_eq!(hebrew_numeral(11), "יא");
            assert_eq!(hebrew_numeral(14), "יד");
            assert_eq!(hebrew_numeral(17), "יז");
            assert_eq!(hebrew_numeral(20), "כ");
            assert_eq!(hebrew_numeral(21), "כא");
            assert_eq!(hebrew_numeral(99), "צט");
            assert_eq!(hebrew_numeral(100), "ק");
            assert_eq!(hebrew_numeral(150), "קנ");
        }

        /// The labels of the split books go up to 65 in the source; every rank
        /// a book can reach must read in at most three letters, since that is
        /// what the layout reserves for a number.
        #[test]
        fn every_rank_of_the_tanakh_reads_in_three_letters() {
            for n in 1..=150 {
                let label = hebrew_numeral(n);
                assert!(!label.is_empty(), "le rang {n} n'a pas de numero");
                assert!(label.chars().count() <= 3, "le rang {n} s'ecrit {label}");
                assert_ne!(label, "יה", "rang {n}");
                assert_ne!(label, "יו", "rang {n}");
            }
            // And no two ranks share a numeral.
            let mut all: Vec<String> = (1..=150).map(hebrew_numeral).collect();
            all.sort();
            let count = all.len();
            all.dedup();
            assert_eq!(all.len(), count, "deux rangs portent le meme numero");
        }

        // ---------------------------------------------------------- parsing

        #[test]
        fn tags_are_stripped_whatever_their_nesting() {
            assert_eq!(strip_tags("<B>1,1</B>"), "1,1");
            assert_eq!(strip_tags("a<FONT SIZE=2>b</FONT>c"), "abc");
            assert_eq!(strip_tags("<A HREF=\"x\"><B>y</B></A>"), "y");
            assert_eq!(strip_tags("sans balise"), "sans balise");
        }

        /// Cleaning: markers `{פ} {ס} {ר} {ש}` and all punctuation go; the maqaf
        /// **becomes a space**, since it joins two distinct words.
        #[test]
        fn cleaning_drops_the_markers_and_turns_the_maqaf_into_a_space() {
            assert_eq!(clean("בראשית {פ}"), "בראשית");
            assert_eq!(clean("{ס} שלום {ר}"), "שלום");
            assert_eq!(clean("אל-תעש"), "אל תעש");
            assert_eq!(clean("א, ב. ג; ד:"), "א ב ג ד");
            assert_eq!(clean("  espaces   multiples  "), "");
            assert_eq!(clean("<B>1,1</B>בראשית"), "בראשית");
            assert_eq!(clean(""), "");
        }

        #[test]
        fn cleaning_keeps_the_niqqud_and_the_cantillation() {
            let pointed = "בְּרֵאשִׁ֖ית";
            assert_eq!(clean(pointed), pointed);
            assert_eq!(clean("עֲוֺן"), "עֲוֺן", "le U+05BA doit survivre au nettoyage");
        }

        #[test]
        fn cleaning_normalises_the_spaces() {
            assert_eq!(clean("א   ב"), "א ב");
            assert_eq!(clean("\nא\tב\n"), "א ב");
            assert!(!clean("א - ב").contains("  "));
        }

        /// Mechon-Mamre closes the bracketed section of Psalm 107 inside the
        /// label itself — `<B>קז,כג ]</B>` — and that bracket was read as part
        /// of the verse number, which the breadcrumb then announced.
        #[test]
        fn an_editorial_mark_never_enters_a_rank() {
            let html = "<BODY><H1>תהילים</H1><B>קז,כג ]</B> יורדי הים";
            let mut corpus = Corpus::default();
            parse_book_file(html, &mut corpus);

            let chapter = &corpus.books[0].chapters[0];
            assert_eq!(chapter.label, "קז");
            assert_eq!(chapter.verses[0].label, "כג");
        }

        /// The source gives a few ranks joined — Psalm 70 opens on `ע,א-ב`,
        /// one text under two numbers. The hyphen is part of the rank.
        #[test]
        fn a_joined_rank_keeps_its_hyphen() {
            let html = "<BODY><H1>תהילים</H1><B>ע,א-ב</B> למנצח לדויד";
            let mut corpus = Corpus::default();
            parse_book_file(html, &mut corpus);

            assert_eq!(corpus.books[0].chapters[0].verses[0].label, "א-ב");
        }

        /// A book file holds one or more `<H1>`; the cover page of `t01.htm`
        /// carries no verse at all and must be dropped.
        #[test]
        fn a_cover_page_opens_no_book() {
            let html = "<BODY><H1>garde</H1>du texte sans verset\
                        <H1>בראשית</H1><B>א,א</B> בראשית ברא";
            let mut corpus = Corpus::default();
            parse_book_file(html, &mut corpus);
            assert_eq!(corpus.books.len(), 1, "la page de garde a ouvert un livre");
            assert_eq!(corpus.books[0].name, "בראשית");
            assert_eq!(corpus.books[0].chapters.len(), 1);
            assert_eq!(corpus.books[0].chapters[0].verses.len(), 1);
            assert_eq!(corpus.books[0].chapters[0].verses[0].text, "בראשית ברא");
            assert_eq!(corpus.books[0].chapters[0].verses[0].label, "א");
        }

        /// Chapters keep **document order**: an alphabetical sort on Hebrew
        /// digits would give a wrong one.
        #[test]
        fn chapters_keep_the_order_of_the_document() {
            let html = "<BODY><H1>ספר</H1>\
                        <B>א,א</B> אחד\
                        <B>א,ב</B> שניים\
                        <B>ב,א</B> שלושה\
                        <B>י,א</B> עשר";
            let mut corpus = Corpus::default();
            parse_book_file(html, &mut corpus);
            let labels: Vec<&str> =
                corpus.books[0].chapters.iter().map(|c| c.label.as_str()).collect();
            assert_eq!(labels, vec!["א", "ב", "י"]);
            assert_eq!(corpus.books[0].chapters[0].verses.len(), 2);
        }

        /// Two `<H1>` in one file open two books: that is how Ezra and Nehemiah
        /// arrive.
        #[test]
        fn two_titles_in_one_file_open_two_books() {
            let html = "<BODY><H1>עזרא / נחמיה</H1><B>א,א</B> טקסט\
                        <H1>אחר</H1><B>א,א</B> טקסט";
            let mut corpus = Corpus::default();
            parse_book_file(html, &mut corpus);
            assert_eq!(corpus.books.len(), 2);
        }

        // ----------------------------------------------------------- splits

        /// The source numbers Samuel from 1 to 55 with **no restart at `א`**:
        /// the cut happens at the traditional rank, and the halves are
        /// renumbered.
        #[test]
        fn a_split_book_is_cut_and_renumbered() {
            let chapters = |count: usize| {
                (1..=count)
                    .map(|i| Chapter {
                        label: hebrew_numeral(i),
                        verses: vec![Verse { label: "א".into(), text: "טקסט".into() }],
                    })
                    .collect::<Vec<_>>()
            };
            let mut corpus = Corpus {
                books: vec![Book { name: "שמואל".into(), chapters: chapters(55) }],
            };
            split_books(&mut corpus);

            assert_eq!(corpus.books.len(), 2);
            assert_eq!(corpus.books[0].name, "שמואל א");
            assert_eq!(corpus.books[1].name, "שמואל ב");
            assert_eq!(corpus.books[0].chapters.len(), 31);
            assert_eq!(corpus.books[1].chapters.len(), 24);
            // Both halves start again at א.
            assert_eq!(corpus.books[0].chapters[0].label, "א");
            assert_eq!(corpus.books[1].chapters[0].label, "א");
            assert_eq!(corpus.books[1].chapters[23].label, hebrew_numeral(24));
            // Nothing is lost on the way.
            let total: usize = corpus.books.iter().map(|b| b.chapters.len()).sum();
            assert_eq!(total, 55);
        }

        #[test]
        fn a_book_the_source_gives_whole_is_left_alone() {
            let mut corpus = Corpus {
                books: vec![Book {
                    name: "בראשית".into(),
                    chapters: vec![Chapter { label: "א".into(), verses: Vec::new() }],
                }],
            };
            split_books(&mut corpus);
            assert_eq!(corpus.books.len(), 1);
            assert_eq!(corpus.books[0].name, "בראשית");
        }

        /// The four cuts and their ranks are the ones that make the totals
        /// match.
        #[test]
        fn the_four_cuts_are_the_documented_ones() {
            let ranks: Vec<(&str, usize)> = SPLITS.iter().map(|&(n, at, ..)| (n, at)).collect();
            assert_eq!(
                ranks,
                vec![
                    ("שמואל", 31),
                    ("מלכים", 22),
                    ("דברי הימים", 29),
                    ("עזרא / נחמיה", 10),
                ]
            );
            // Every half named by a split must exist in the canonical order.
            for &(_, _, first, second) in &SPLITS {
                assert!(ORDER.contains(&first), "{first} absent de ORDER");
                assert!(ORDER.contains(&second), "{second} absent de ORDER");
            }
        }

        // ------------------------------------------------------------ order

        /// The canonical order holds the 39 books: 5 of the Torah, 21 of the
        /// Prophets, 13 of the Writings.
        #[test]
        fn the_canonical_order_holds_the_thirty_nine_books() {
            assert_eq!(ORDER.len(), 39);
            assert_eq!(ORDER[0], "בראשית");
            assert_eq!(ORDER[4], "דברים");
            assert_eq!(ORDER[5], "יהושוע");
            assert_eq!(ORDER[25], "מלאכי");
            assert_eq!(ORDER[26], "תהילים");
            assert_eq!(ORDER[38], "דברי הימים ב");

            let mut sorted = ORDER.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 39, "deux entrees de ORDER sont identiques");
        }

        /// `reorder` puts the source's books in the displayed order, whatever
        /// the order of the files.
        #[test]
        fn reordering_follows_the_canonical_order() {
            let book = |name: &str| Book { name: name.into(), chapters: Vec::new() };
            let mut corpus = Corpus { books: ORDER.iter().rev().map(|n| book(n)).collect() };
            reorder(&mut corpus);
            let names: Vec<&str> = corpus.books.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(names, ORDER.to_vec());
        }

        #[test]
        #[should_panic(expected = "livre absent de la source")]
        fn a_missing_book_stops_the_build() {
            let mut corpus = Corpus {
                books: vec![Book { name: "בראשית".into(), chapters: Vec::new() }],
            };
            reorder(&mut corpus);
        }
    }
}

/// The table of the other edition's readings, tested on the very code the
/// build runs — `build/variants.rs`, included rather than copied.
#[allow(dead_code, unused_imports)]
mod variants {
    include!("../build/variants.rs");

    #[cfg(test)]
    mod tests {
        use super::*;

        fn entry(word: usize, count: usize, ours: &str, other: &str) -> Variant {
            Variant {
                book: 0,
                chapter: 1,
                verse: "א".into(),
                word,
                count,
                ours: ours.into(),
                other: other.into(),
                kind: "letters".into(),
            }
        }

        // ------------------------------------------------------------ reading

        #[test]
        fn comments_and_blank_lines_are_skipped() {
            let tsv = "# un commentaire\n\n0\t1\tא\t2\t1\tאב\tאג\tletters\n";
            let out = parse_variants(tsv);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0], entry(2, 1, "אב", "אג"));
        }

        #[test]
        #[should_panic(expected = "colonnes")]
        fn a_short_line_stops_the_build() {
            parse_variants("0\t1\tא\t2\n");
        }

        #[test]
        #[should_panic(expected = "vide")]
        fn an_empty_table_stops_the_build() {
            parse_variants("# rien que des commentaires\n");
        }

        // ------------------------------------------------------------ anchors

        /// A rank, never a search: a form may appear twice in the same verse,
        /// and only the index says which one is meant.
        #[test]
        fn a_span_is_found_by_rank() {
            let text = "אא בב גג דד";
            assert_eq!(word_span(text, 0, 1), (0, 4));
            assert_eq!(word_span(text, 1, 1), (5, 4));
            assert_eq!(word_span(text, 3, 1), (15, 4));
            // Several words: the span runs from the first to the last, spaces
            // included.
            assert_eq!(word_span(text, 1, 2), (5, 9));
            assert_eq!(word_span(text, 0, 4), (0, 19));
            let (at, len) = word_span(text, 1, 2);
            assert_eq!(&text[at..at + len], "בב גג");
        }

        #[test]
        #[should_panic(expected = "hors du verset")]
        fn a_span_past_the_verse_stops_the_build() {
            word_span("אא בב", 1, 5);
        }

        #[test]
        #[should_panic(expected = "ancre absente")]
        fn a_wrong_anchor_stops_the_build() {
            display_of("אא בב גג", &[entry(1, 1, "דד", "הה")]);
        }

        // ------------------------------------------------------------ display

        /// **The display string is the verse, plus insertions.** Taking the
        /// insertions back out gives the verse again — that is what makes the
        /// map add to our text instead of replacing it.
        #[test]
        fn the_display_string_is_the_verse_plus_the_other_reading() {
            let text = "אא בב גג";
            let (shown, inserts) = display_of(text, &[entry(1, 1, "בב", "בג")]);
            assert_eq!(inserts.len(), 1);

            let Insert { at, mark, other } = inserts[0];
            assert_eq!(&shown[..at], "אא בב", "ce qui precede est intact");
            assert_eq!(&shown[at..at + mark], MARK_OURS, "la marque suit notre forme");
            assert_eq!(&shown[at + mark..at + mark + other], format!(" {OPEN}בג{CLOSE}{MARK_OTHER}"));
            assert_eq!(&shown[at + mark + other..], " גג", "ce qui suit est intact");

            let mut rebuilt = String::from(&shown[..at]);
            rebuilt.push_str(&shown[at + mark + other..]);
            assert_eq!(rebuilt, text, "le verset n'est pas retrouve");
        }

        #[test]
        fn several_readings_in_one_verse_stay_in_order() {
            let text = "אא בב גג דד";
            let (shown, inserts) =
                display_of(text, &[entry(0, 1, "אא", "אב"), entry(2, 1, "גג", "גד")]);
            assert_eq!(inserts.len(), 2);
            assert!(inserts[0].at < inserts[1].at, "insertions desordonnees");

            let mut rebuilt = String::new();
            let mut cursor = 0;
            for Insert { at, mark, other } in inserts {
                rebuilt.push_str(&shown[cursor..at]);
                cursor = at + mark + other;
            }
            rebuilt.push_str(&shown[cursor..]);
            assert_eq!(rebuilt, text);
        }

        #[test]
        #[should_panic(expected = "entrees non ordonnees")]
        fn entries_out_of_order_stop_the_build() {
            display_of("אא בב גג", &[entry(2, 1, "גג", "גד"), entry(0, 1, "אא", "אב")]);
        }

        /// A verse with no entry is left exactly as it is.
        #[test]
        fn a_verse_without_an_entry_is_untouched() {
            let (shown, inserts) = display_of("אא בב", &[]);
            assert_eq!(shown, "אא בב");
            assert!(inserts.is_empty());
        }
    }
}
