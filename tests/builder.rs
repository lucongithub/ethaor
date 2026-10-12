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
