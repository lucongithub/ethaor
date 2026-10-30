//! The two transports answer alike: what the page reads from the module's
//! memory (`src/wasm/els.rs`) and what the API sends (`api/els.rs`) are the
//! same hits, in the same order, placed in the same verses.

use std::sync::Mutex;

use ethaor::shared::els::{MAX_LIMIT, STRIDE};
use ethaor::wasm::els::{els_hits_len, els_hits_ptr, els_query_ptr, els_search};

use super::els;

/// The module's exports share one global state — the query buffer among it:
/// every test driving them, here or in `words.rs`, takes this lock.
pub(super) static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The search exactly as `search-core.ts` drives it.
fn as_the_page(word: &str, skip_min: u32, skip_max: u32, exact: bool) -> (f64, Vec<u32>) {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { core::ptr::copy_nonoverlapping(word.as_ptr(), els_query_ptr(), word.len()) };
    let total = els_search(word.len() as u32, skip_min, skip_max, exact as u32);
    let n = els_hits_len() as usize * STRIDE;
    (total, unsafe { core::slice::from_raw_parts(els_hits_ptr(), n) }.to_vec())
}

#[tokio::test]
async fn the_page_and_the_api_find_the_same_hits() {
    // A word as written, a range, one letter, a long word, a flood past the
    // cap, skips running past the stream — and exact letters, a final and
    // its non-final form apart.
    let cases = [
        ("אָדָם", 0, 0, false),
        ("תורה", 40, 60, false),
        ("א", 3, 3, false),
        ("שלמה", 0, 5, false),
        ("ירושלים", 100, 400, false),
        ("את", 0, 200, false),
        ("אב", 2_000_000, u32::MAX, false),
        ("אָדָם", 0, 0, true),
        ("אדמ", 0, 0, true),
        ("מלך", 0, 30, true),
        ("ם", 0, 0, true),
    ];
    for (word, skip_min, skip_max, exact) in cases {
        let (total, records) = as_the_page(word, skip_min, skip_max, exact);
        let json = els(&[
            ("word", word),
            ("exact", if exact { "true" } else { "false" }),
            ("skip_min", &skip_min.to_string()),
            ("skip_max", &skip_max.to_string()),
            ("limit", &MAX_LIMIT.to_string()),
        ])
        .await
        .json();
        assert_eq!(json["total"].as_f64(), Some(total), "{word}");
        assert_eq!(json["listed"].as_u64(), Some((records.len() / STRIDE) as u64), "{word}");
        let hits = json["hits"].as_array().unwrap();
        assert_eq!(hits.len(), (records.len() / STRIDE).min(MAX_LIMIT), "{word}");
        for (hit, r) in hits.iter().zip(records.chunks(STRIDE)) {
            let first = &hit["letters"][0];
            assert_eq!(first["rank"].as_u64(), Some(r[0] as u64), "{word}");
            assert_eq!(hit["skip"].as_u64(), Some(r[1] as u64), "{word}");
            assert_eq!(hit["from"]["index"].as_u64(), Some(r[4] as u64), "{word}");
            assert_eq!(first["position"].as_u64(), Some(r[5] as u64), "{word}");
            assert_eq!(hit["to"]["index"].as_u64(), Some(r[6] as u64), "{word}");
        }
    }
}

/// The book view and the API read a chapter alike: what the page reads from
/// the module's record (`src/wasm/chapter.rs`) and what `/api/chapter`
/// answers are the same verses, shown the same, the same forms the other
/// edition's — on chapters holding another reading, and a stride through
/// the rest.
#[tokio::test]
async fn the_page_and_the_api_read_the_same_chapter() {
    use ethaor::wasm::chapter::{HEAD, chapter_count, chapter_record, chapter_record_ptr};
    use ethaor::wasm::map::text_ptr;
    let text = |offset: u32, len: u32| -> &'static str {
        let bytes = unsafe { core::slice::from_raw_parts(text_ptr().add(offset as usize), len as usize) };
        std::str::from_utf8(bytes).unwrap()
    };
    let mut carrying = 0;
    for c in (0..chapter_count()).filter(|c| c % 37 == 0 || *c == 13) {
        let record: Vec<u32> = {
            let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let n = chapter_record(c) as usize;
            unsafe { core::slice::from_raw_parts(chapter_record_ptr(), n) }.to_vec()
        };
        let osis = ethaor::shared::passage::chapter(c as usize).unwrap().osis();
        let json = super::get(super::api_only(), &format!("/api/chapter?ref={osis}"), &[]).await.json();
        assert_eq!(json["chapter"]["index"], c, "chapter {c}");
        let verses = json["verses"].as_array().unwrap();
        assert_eq!(verses.len(), record[9] as usize, "chapter {c}");
        let mut i = HEAD;
        for v in verses {
            assert_eq!(v["index"], record[i], "chapter {c}");
            let shown = text(record[i + 3], record[i + 4]);
            let runs = record[i + 5] as usize;
            let forms: Vec<&str> = (0..runs).map(|k| text(record[i + 6 + 2 * k], record[i + 7 + 2 * k])).collect();
            if runs == 0 {
                assert_eq!(v["text"], shown, "chapter {c}");
                assert!(v["other_reading"].is_null(), "chapter {c}");
            } else {
                carrying += 1;
                assert_eq!(v["other_reading"]["shown"], shown, "chapter {c}");
                assert_eq!(v["other_reading"]["forms"], serde_json::json!(forms), "chapter {c}");
            }
            i += 6 + 2 * runs;
        }
    }
    assert!(carrying > 0, "no chapter compared carries another reading");
}

/// The book view and the API list the same contents: what the page reads
/// from `contents_record` and what `/api/contents` answers are the same
/// portions, books and chapters, named alike.
#[tokio::test]
async fn the_page_and_the_api_list_the_same_contents() {
    use ethaor::wasm::chapter::{CONTENTS_HEAD, contents_ptr, contents_record};
    use ethaor::wasm::map::text_ptr;
    let text = |offset: u32, len: u32| -> &'static str {
        let bytes = unsafe { core::slice::from_raw_parts(text_ptr().add(offset as usize), len as usize) };
        std::str::from_utf8(bytes).unwrap()
    };
    let record: Vec<u32> = {
        let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let n = contents_record() as usize;
        unsafe { core::slice::from_raw_parts(contents_ptr(), n) }.to_vec()
    };
    let json = super::get(super::api_only(), "/api/contents", &[]).await.json();
    let portions = json["portions"].as_array().unwrap();
    let (p, b) = (record[0] as usize, record[1] as usize);
    assert_eq!(portions.len(), p);
    let books_at = CONTENTS_HEAD + 2 * p;
    let labels_at = books_at + 5 * b;
    let mut book = 0;
    for (s, portion) in portions.iter().enumerate() {
        assert_eq!(portion["hebrew"], text(record[CONTENTS_HEAD + 2 * s], record[CONTENTS_HEAD + 2 * s + 1]));
        for listed in portion["books"].as_array().unwrap() {
            let r = &record[books_at + 5 * book..books_at + 5 * book + 5];
            assert_eq!(r[0] as usize, s, "book {book}");
            assert_eq!(listed["hebrew"], text(r[1], r[2]), "book {book}");
            let chapters = listed["chapters"].as_array().unwrap();
            assert_eq!(chapters.len(), (r[4] - r[3]) as usize, "book {book}");
            for (k, c) in chapters.iter().enumerate() {
                let index = r[3] as usize + k;
                assert_eq!(c["index"], index, "book {book}");
                let label = text(record[labels_at + 2 * index], record[labels_at + 2 * index + 1]);
                assert_eq!(c["hebrew"], format!("{} {label}", text(r[1], r[2])), "chapter {index}");
            }
            book += 1;
        }
    }
    assert_eq!(book, b);
}

/// Scoped, too: the page's `els_search_in` over a stretch of verses and the
/// API's scope over the same verses find the same hits.
#[tokio::test]
async fn the_page_and_the_api_keep_to_the_same_verses() {
    use ethaor::wasm::els::els_search_in;
    let cases: [(&str, u32, u32, super::Pairs); 3] = [
        ("אל", 0, 20, &[("book", "Gen")]),
        ("משה", 0, 300, &[("portion", "torah")]),
        ("יהוה", 0, 50, &[("from", "Isa.1"), ("to", "Isa.12")]),
    ];
    for (word, skip_min, skip_max, scope) in cases {
        let (lo, hi) = (skip_min.to_string(), skip_max.to_string());
        let mut pairs = vec![("word", word), ("skip_min", lo.as_str()), ("skip_max", hi.as_str()), ("limit", "1000")];
        pairs.extend_from_slice(scope);
        let json = els(&pairs).await.json();
        let first = json["query"]["scope"]["from"]["index"].as_u64().unwrap() as u32;
        let end = json["query"]["scope"]["to"]["index"].as_u64().unwrap() as u32 + 1;
        let (total, records) = {
            let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            unsafe { core::ptr::copy_nonoverlapping(word.as_ptr(), els_query_ptr(), word.len()) };
            let total = els_search_in(word.len() as u32, skip_min, skip_max, first, end, 0);
            let n = els_hits_len() as usize * STRIDE;
            (total, unsafe { core::slice::from_raw_parts(els_hits_ptr(), n) }.to_vec())
        };
        assert_eq!(json["total"].as_f64(), Some(total), "{word} {scope:?}");
        for (hit, r) in json["hits"].as_array().unwrap().iter().zip(records.chunks(STRIDE)) {
            assert_eq!(hit["letters"][0]["rank"].as_u64(), Some(r[0] as u64), "{word}");
            assert_eq!(hit["skip"].as_u64(), Some(r[1] as u64), "{word}");
            assert_eq!(hit["to"]["index"].as_u64(), Some(r[6] as u64), "{word}");
        }
    }
}

/// A selection's gematria as the page reads it (`src/wasm/gematria.rs`) and
/// as `/api/gematria` answers it: every method's value, and each letter a
/// method replacing letters reads, the same — on a word, a pointed verse and
/// a text mixing finals, marks and Latin.
#[tokio::test]
async fn the_page_and_the_api_read_the_same_gematria() {
    use ethaor::shared::gematria::Method;
    use ethaor::wasm::gematria::*;
    let texts = ["תורה", "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ", "מֶלֶךְ, abc צדק ץ"];
    for text in texts {
        let (n, values, read_as) = {
            let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let room = gematria_buffer(text.len() as u32);
            unsafe { core::ptr::copy_nonoverlapping(text.as_ptr(), room, text.len()) };
            let n = gematria_read(text.len() as u32) as usize;
            let methods = Method::ALL.len();
            let values = unsafe { core::slice::from_raw_parts(gematria_values_ptr(), methods) }.to_vec();
            let read_as = unsafe { core::slice::from_raw_parts(gematria_read_as_ptr(), n * methods) }.to_vec();
            (n, values, read_as)
        };
        let json = super::get(super::api_only(), &format!("/api/gematria?{}", super::query(&[("text", text)])), &[])
            .await
            .json();
        let methods = json["methods"].as_array().unwrap();
        assert_eq!(methods.len(), values.len(), "{text}");
        for (m, method) in methods.iter().enumerate() {
            assert_eq!(method["value"].as_f64(), Some(values[m]), "{text}: {}", method["method"]);
            if let Some(letters) = method["letters"].as_str() {
                let page: String = read_as[m * n..(m + 1) * n].iter().map(|&f| char::from_u32(0x05D0 + u32::from(f)).unwrap()).collect();
                assert_eq!(page, letters, "{text}: {}", method["method"]);
            }
        }
    }
}
