//! The two transports answer alike: what the page reads from the module's
//! memory (`src/wasm/els.rs`) and what the API sends (`api/els.rs`) are the
//! same hits, in the same order, placed in the same verses.

use std::sync::Mutex;

use ethaor::shared::els::{MAX_LIMIT, STRIDE};
use ethaor::wasm::els::{els_hits_len, els_hits_ptr, els_query_ptr, els_search};

use super::els;

/// The module's exports share one global state.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The search exactly as `search-core.ts` drives it.
fn as_the_page(word: &str, skip_min: u32, skip_max: u32) -> (f64, Vec<u32>) {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { core::ptr::copy_nonoverlapping(word.as_ptr(), els_query_ptr(), word.len()) };
    let total = els_search(word.len() as u32, skip_min, skip_max);
    let n = els_hits_len() as usize * STRIDE;
    (total, unsafe { core::slice::from_raw_parts(els_hits_ptr(), n) }.to_vec())
}

#[tokio::test]
async fn the_page_and_the_api_find_the_same_hits() {
    // A word as written, a range, one letter, a long word, a flood past the
    // cap, and skips running past the stream.
    let cases = [
        ("אָדָם", 0, 0),
        ("תורה", 40, 60),
        ("א", 3, 3),
        ("שלמה", 0, 5),
        ("ירושלים", 100, 400),
        ("את", 0, 200),
        ("אב", 2_000_000, u32::MAX),
    ];
    for (word, skip_min, skip_max) in cases {
        let (total, records) = as_the_page(word, skip_min, skip_max);
        let json = els(&[
            ("word", word),
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
            let total = els_search_in(word.len() as u32, skip_min, skip_max, first, end);
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
