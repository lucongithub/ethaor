# Tests

Every test of the repository, along its file tree. **Generated** by
`web/scripts/tests-map.mjs` (`npm run tests:map`) — do not edit by hand:
`npm run verify` fails when this file is stale, or when a test the runners
know is missing here. The `tests` skill (`.claude/skills/tests`) reads it to
find what to strengthen next.

For each test file: what it covers, then each module and each test — its
name, what it holds (its doc comment), and its nature.

## Summary

| area | files | tests | ignored | without a doc |
|---|---|---|---|---|
| Rust core, module-private | 7 | 114 | 1 | 21 |
| Rust app (`src/tests/`) | 11 | 151 | 0 | 12 |
| Corpus build (`tests/`) | 1 | 31 | 0 | 15 |
| Server (`server/`) | 15 | 82 | 1 | 12 |
| Page (`web/`) | 8 | 62 | 0 | 0 |
| **total** | **42** | **440** | **2** | **60** |

## By file

- **server/**
  - **src/**
    - **tests/**
      - [`chapter.rs`](server/src/tests/chapter.rs) — 3 tests — covers [`server/src/api/chapter.rs`](server/src/api/chapter.rs)
        <br>`GET /api/chapter`: a chapter read as an agent reads it — every verse named every way, the other edition's forms set apart, the neighbours to read on, and each refusal its code.
        - module `tests::chapter` · ethaor-server
          - `a_chapter_is_read_verse_by_verse_and_named_every_way` — A chapter answers its verses in order, each our text and named every way — OSIS, English, Hebrew — the chapter too, its neighbours across books, and may be kept by anyone. <sub>service, called in memory · async</sub>
          - `a_verse_carries_the_other_reading_where_there_is_one` — Where the other edition reads otherwise, the verse carries it: the verse as shown, and the other forms alone, each found in it. <sub>service, called in memory · async</sub>
          - `a_chapter_not_named_is_refused_with_its_code` — Each mistake its code and a 400, numbered, never kept. <sub>service, called in memory · async</sub>
      - [`contents.rs`](server/src/tests/contents.rs) — 2 tests — covers [`server/src/api/contents.rs`](server/src/api/contents.rs)
        <br>`GET /api/contents`: the table of contents as an agent reads it — every portion, book and chapter named every way, and a parameter refused.
        - module `tests::contents` · ethaor-server
          - `the_contents_name_every_portion_book_and_chapter` — Three portions in order, the 39 books under them, the 929 chapters under their books, each named every way; a chapter's OSIS is one `/api/chapter` reads, and the answer may be kept by anyone. <sub>service, called in memory · async</sub>
          - `a_parameter_to_the_contents_is_refused` — The contents take no parameter: one given is refused with its code and a 400, numbered, never kept. <sub>service, called in memory · async</sub>
      - [`describe.rs`](server/src/tests/describe.rs) — 4 tests
        <br>`GET /api`: the description an agent — or an MCP server — reads, held to what the endpoints really accept.
        - module `tests::describe` · ethaor-server
          - `the_description_lists_each_endpoint_as_a_tool` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `the_els_schemas_are_what_the_endpoints_take` — The schemas' numbers are the code's, every property they declare is accepted — the scope's one way at a time — and one they do not is refused; every refusal the code can give is declared. <sub>service, called in memory · async</sub>
          - `the_chapter_schema_is_what_the_endpoint_takes` — The chapter's tool declares what the endpoint takes — `ref`, and nothing else — and every code it can refuse with. <sub>service, called in memory · async</sub>
          - `the_contents_schema_is_what_the_endpoint_takes` — The contents' tool declares no parameter and refuses one, and its errors are the endpoint's. <sub>service, called in memory · async</sub>
      - [`els.rs`](server/src/tests/els.rs) — 10 tests — covers [`server/src/api/els.rs`](server/src/api/els.rs)
        <br>`GET /api/els`: the answer's shape, its refusals, its pages.
        - module `tests::els` · ethaor-server
          - `a_hit_names_its_place_every_way` — `תורה` every fiftieth letter from the first tav of Genesis, named every way an agent may need. <sub>service, called in memory · async</sub>
          - `the_answer_is_the_shared_page` — The JSON is the shared layer's page, field for field: the endpoint adds a transport, never a computation. <sub>service, called in memory · async</sub>
          - `next_offset_walks_every_listed_hit` — Following `next_offset` walks every listed hit once, and the last page says null. <sub>service, called in memory · async</sub>
          - `every_refusal_says_why` — Each mistake gets its own code, among those the description announces, with a 400 and a message. <sub>service, called in memory · async</sub>
          - `an_unknown_endpoint_answers_json` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `a_flood_answers_its_total_and_its_last_page` — A flood over the HTTP side: the exact total beyond the listed hits, and the last listed page closing the walk. <sub>service, called in memory · async</sub>
          - `extreme_numbers_answer` — Numbers past any sensible range are taken, not crashed on; past their type, refused as a parameter. <sub>service, called in memory · async</sub>
          - `a_missing_text_is_unavailable_not_bad` — The text not there is the service's state, not the caller's mistake. <sub>service, called in memory</sub>
          - `a_scope_keeps_every_letter_inside` — Each way of scoping keeps every letter of every hit inside, and says the verses it kept to. <sub>service, called in memory · async</sub>
          - `a_scope_mistake_says_which` — Each scoping mistake gets its own code. <sub>service, called in memory · async</sub>
      - [`faults.rs`](server/src/tests/faults.rs) — 10 tests — covers [`server/src/fault.rs`](server/src/fault.rs)
        <br>Faults: each numbered once, its number the same in the answer and the log, its code naming one place, its escalation logged whole only when verbose — and the page's own faults landing in the same log.
        - module `tests::faults` · ethaor-server
          - `an_incident_number_reads_aloud_and_never_repeats` — *(no doc: say what it holds)* <sub>service, called in memory</sub>
          - `an_answer_and_its_log_share_the_incident` — A refusal: its number the same in the answer and in the log, the log saying the code, the place it was raised, and what the caller read. <sub>service, called in memory · async</sub>
          - `every_refusal_is_numbered` — Every refusal under `/api` carries its number. <sub>service, called in memory · async</sub>
          - `an_escalation_keeps_its_number_and_its_steps` — Escalated, a fault keeps its number and gains a step per layer, in order, each at its own line; verbose, the log has them all and the stack, else only where it began. <sub>service, called in memory</sub>
          - `a_panic_is_numbered_at_its_own_line` — A handler that panics: the caller gets `internal`, a number and words it can read; the log, under that number, the panic's own place. <sub>service, called in memory · async</sub>
          - `every_code_is_raised_at_one_place` — Each fault code is raised at one place, so it names that place; each raised is in the catalogue. <sub>service, called in memory</sub>
          - `a_page_report_is_logged_under_its_number` — A page's fault lands in the log under the number its user saw, with its code, place and the message the user read. <sub>service, called in memory · async</sub>
          - `a_report_is_bounded` — A report is bounded and checked: steps and texts cut, a number not of ours replaced (the one given kept beside it), the code tamed. <sub>service, called in memory</sub>
          - `a_bad_report_is_refused` — A malformed or oversized report is refused, numbered; a report is a POST. <sub>service, called in memory · async</sub>
          - `a_fault_is_logged_once` — One log line per fault, whatever layers it crossed: of all the lines a failing request wrote, besides the one saying it was answered, there is one — the fault's, under its number. <sub>service, called in memory · async</sub>
      - [`files.rs`](server/src/tests/files.rs) — 7 tests — covers [`server/src/files.rs`](server/src/files.rs)
        <br>The static build, served as the page needs it: ranges for tiles, no compression on top of gzip, a year of cache for hashed names, the app's own routing.
        - module `tests::files` · ethaor-server
          - `the_page_and_its_routes_get_index_html` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `a_page_of_its_own_is_served_at_its_directory` — A page prerendered as a directory's index (`/logs/`, the log page) is served there, never in the cache, and `/logs` is sent to it with its query whole — the token it carries once must not be dropped on the way. <sub>service, called in memory · async</sub>
          - `hashed_names_are_kept_a_year_and_the_module_streams` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `a_tile_gets_its_range_uncompressed` — A tile is a Range request: the bytes asked, exactly, never re-encoded. <sub>service, called in memory · async</sub>
          - `the_api_wins_over_the_files` — `/api` is never shadowed by the build, whatever it holds. <sub>service, called in memory · async</sub>
          - `no_path_climbs_out_of_the_build` — Nothing outside the build is ever served, however the path climbs. <sub>service, called in memory · async</sub>
          - `a_file_asked_again_is_not_resent` — A browser asking again with what it holds is told it holds it: no bytes resent. <sub>service, called in memory · async</sub>
      - [`guard.rs`](server/src/tests/guard.rs) — 4 tests — covers [`server/src/api/guard.rs`](server/src/api/guard.rs)
        <br>What a client may spend: a search too heavy is refused before it runs, a client past its budget waits, and another client is not held by it.
        - module `tests::guard` · ethaor-server
          - `a_search_too_heavy_is_refused_before_it_runs` — Past the allowed cost a search is refused before it runs, saying its estimate and how to narrow it; under it, it runs. <sub>service, called in memory · async</sub>
          - `a_client_past_its_budget_waits_alone` — Each request costs one: past the burst a client waits, told how long, while another client is served. <sub>service, called in memory · async · time budget</sub>
          - `a_search_costs_its_estimate_and_a_cached_one_almost_nothing` — A new search costs its estimate; the same search from the cache costs one — the budget is spent on engines, not on answers already given. <sub>service, called in memory · async</sub>
          - `a_client_is_named_by_its_proxy_only_when_trusted` — Who the client is: its socket, unless a trusted proxy names it — and only then, or anyone could pick a fresh budget at every request. <sub>service, called in memory</sub>
      - [`load.rs`](server/src/tests/load.rs) — 1 test
        <br>Load, measured rather than asserted: many callers at once, in memory — no socket, so what is timed is the service, not the network. Run in release: `cargo test -p ethaor-server --release load -- --ignored --nocapture`.
        - module `tests::load` · ethaor-server
          - `load` — *(no doc: say what it holds)* <sub>service, called in memory · ignored: a measurement, run by hand · multi-thread</sub>
      - [`logs.rs`](server/src/tests/logs.rs) — 13 tests — covers [`server/src/api/logs.rs`](server/src/api/logs.rs)
        <br>Logs: one JSON line per event, each request's lines carrying its id, written to files that roll within their budget, and followed live from them by the operator alone.
        - module `tests::logs` · ethaor-server
          - `the_logs_are_off_without_a_token_and_closed_without_it` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `a_request_is_found_in_the_logs_by_its_id` — A request's lines carry the id it was answered with, and the operator finds them in what the endpoint replays. <sub>service, called in memory · multi-thread</sub>
          - `the_logs_follow_live_and_filter_by_level` — Lines written after the follower arrived reach it live; `level` keeps only what is at least that grave. <sub>service, called in memory · multi-thread</sub>
          - `every_answer_has_an_id` — Every answer carries its request id — a refusal too. <sub>service, called in memory · async</sub>
          - `the_log_settings_are_read_and_fall_back` — Unset, the logs go to `logs` with the default budget; `off` writes no file; a budget that is not a positive number falls back and says so. <sub>service, called in memory</sub>
          - `the_files_roll_and_stay_within_their_budget` — Written past its share, a file rolls to the next; past `FILES` the oldest go. What is left is the newest lines, in order, none cut, and the disk never holds more than its budget. <sub>service, called in memory · time budget</sub>
          - `each_process_starts_a_new_file` — A process starts a file of its own after the last one: what an earlier run wrote is never written over. <sub>service, called in memory</sub>
          - `a_directory_that_cannot_be_written_is_refused_at_start` — A directory that cannot be made is an error at start — the service then logs to standard output alone — never a panic. <sub>service, called in memory</sub>
          - `the_history_is_read_back_across_files` — The history is the last lines at the level asked, oldest first, read back across the files. <sub>service, called in memory</sub>
          - `a_half_written_line_is_sent_once_whole` — A line half written is neither in the history nor sent live twice: the end stops before it, and the follower sends it once whole. <sub>service, called in memory · async</sub>
          - `a_follower_reads_across_files_and_is_told_what_it_missed` — A follower reads on as the files roll, every line once and in order; one so slow that files it had not read were deleted is told how many, then goes on from the oldest left. <sub>service, called in memory · async</sub>
          - `no_log_file_is_a_503` — With no log file, the endpoint says so — a 503, not an empty stream. <sub>service, called in memory · async</sub>
          - `a_flush_writes_the_lines_and_wakes_a_follower_at_once` — Lines wait in the buffer until the flush — no system call each — and the flush wakes a waiting follower at once, well before it would have looked on its own. <sub>service, called in memory · multi-thread</sub>
      - [`parity.rs`](server/src/tests/parity.rs) — 4 tests
        <br>The two transports answer alike: what the page reads from the module's memory (`src/wasm/els.rs`) and what the API sends (`api/els.rs`) are the same hits, in the same order, placed in the same verses.
        - module `tests::parity` · ethaor-server
          - `the_page_and_the_api_find_the_same_hits` — *(no doc: say what it holds)* <sub>service, called in memory · parity WebAssembly ↔ API · async</sub>
          - `the_page_and_the_api_read_the_same_chapter` — The book view and the API read a chapter alike: what the page reads from the module's record (`src/wasm/chapter.rs`) and what `/api/chapter` answers are the same verses, shown the same, the same forms the other edition's — on chapters holding another reading, and a stride through the rest. <sub>service, called in memory · parity WebAssembly ↔ API · async</sub>
          - `the_page_and_the_api_list_the_same_contents` — The book view and the API list the same contents: what the page reads from `contents_record` and what `/api/contents` answers are the same portions, books and chapters, named alike. <sub>service, called in memory · parity WebAssembly ↔ API · async</sub>
          - `the_page_and_the_api_keep_to_the_same_verses` — Scoped, too: the page's `els_search_in` over a stretch of verses and the API's scope over the same verses find the same hits. <sub>service, called in memory · parity WebAssembly ↔ API · async</sub>
      - [`robust.rs`](server/src/tests/robust.rs) — 4 tests
        <br>The service never goes down for a request: a panic is answered and the next request served, an engine that panicked is replaced, a bad setting falls back to its default.
        - module `tests::robust` · ethaor-server
          - `a_panic_is_answered_and_the_service_goes_on` — A handler that panics gets a JSON 500, logged — and the same service answers the next request. <sub>service, called in memory · async</sub>
          - `an_engine_that_panicked_is_replaced` — A search that panics: its caller gets a numbered fault whose origin is the panic's own line and message, the pool's step after it; its engine is dropped rather than lent again half-written, and the pool goes on. <sub>service, called in memory · async</sub>
          - `a_bad_setting_falls_back` — A setting that is not a positive number is ignored for its default; a burst below the heaviest search is raised to it; a short token is no token. <sub>service, called in memory</sub>
          - `the_browser_opens_only_when_asked` — The browser opens only when asked: off by default, on with `1`, `true` or `yes`, as every on/off setting. <sub>service, called in memory</sub>
      - [`routes.rs`](server/src/tests/routes.rs) — 4 tests
        <br>Under `/api`, every answer is JSON — a wrong path, a wrong method, a slash too many — and many callers at once get what each would alone.
        - module `tests::routes` · ethaor-server
          - `a_trailing_slash_still_describes` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `a_wrong_path_or_method_answers_json` — *(no doc: say what it holds)* <sub>service, called in memory · async</sub>
          - `head_answers_without_a_body` — HEAD says what GET would, without the body. <sub>service, called in memory · async</sub>
          - `many_callers_at_once_get_their_own_answers` — Callers at once share one engine: each gets the answer it would alone, whatever the order the lock serves them in. <sub>service, called in memory · multi-thread</sub>
      - [`scale.rs`](server/src/tests/scale.rs) — 7 tests
        <br>Many callers at once: engines lent one per search, answers shared by everyone, a full queue turned away — and nothing lost when a caller goes.
        - module `tests::scale` · ethaor-server
          - `searches_run_side_by_side_on_engines_of_their_own` — Searches run side by side, one engine each, never more engines than the pool allows — and every engine is back once they are done. <sub>service, called in memory · multi-thread</sub>
          - `an_answer_is_searched_once_for_everyone` — A query asked again — by anyone — is answered without an engine, and the same query asked by many at once is searched once. <sub>service, called in memory · multi-thread</sub>
          - `a_refusal_costs_no_engine` — A bad query takes no engine and leaves nothing in the cache. <sub>service, called in memory · async</sub>
          - `a_full_queue_turns_callers_away` — Every engine taken past `wait`: the caller is turned away at once with a 503 it may retry, never kept; once an engine frees, the same query answers. <sub>service, called in memory · async</sub>
          - `a_caller_gone_gives_its_engine_back` — A caller gone mid-search does not take its engine with it. <sub>service, called in memory · multi-thread</sub>
          - `answers_say_they_may_be_kept` — What anyone may keep says so, a CDN included; what is refused does not. <sub>service, called in memory · async</sub>
          - `the_defaults_scale_with_the_machine` — Unset, the endpoint takes a core per engine, a short queue and a cache. <sub>service, called in memory</sub>
      - [`settings.rs`](server/src/tests/settings.rs) — 5 tests
        <br>The settings file: read when there, the environment winning over it, ignored whole when broken — never a reason not to start.
        - module `tests::settings` · ethaor-server
          - `a_settings_file_is_read` — *(no doc: say what it holds)* <sub>service, called in memory</sub>
          - `the_environment_wins_over_the_file` — What the process' environment holds wins over the file: a host's secret overrides a local file. <sub>service, called in memory</sub>
          - `no_file_is_the_environment_alone` — *(no doc: say what it holds)* <sub>service, called in memory</sub>
          - `a_broken_file_is_ignored_whole` — A broken file is ignored whole — its good lines too — and says why. <sub>service, called in memory</sub>
          - `the_start_link_opens_in_a_browser` — The link printed at start opens in a browser: the address bound, and an unspecified one (every interface) named as this machine. <sub>service, called in memory</sub>
      - [`stream.rs`](server/src/tests/stream.rs) — 4 tests
        <br>`GET /api/els/stream`: every listed hit as a line, the same hits the pages give, and nothing left running when the caller goes.
        - module `tests::stream` · ethaor-server
          - `a_stream_is_the_pages_in_one_go` — A head line, one line per hit — the pages' hits, in their order — and a closing line. <sub>service, called in memory · async</sub>
          - `a_stream_goes_past_a_page` — Past a page's bound: a stream lists thousands, up to the cap. <sub>service, called in memory · async</sub>
          - `a_refused_stream_answers_json` — A refusal is a plain JSON refusal, not a stream. <sub>service, called in memory · async</sub>
          - `a_stream_left_stops` — The caller gone after the first lines: the placing stops, and says so. <sub>service, called in memory · multi-thread</sub>
- **src/**
  - **corpus/**
    - [`tests.rs`](src/corpus/tests.rs) — 18 tests — covers [`src/corpus.rs`](src/corpus.rs)
      <br>What the unpacked corpus promises: counts, a well-formed index, a hierarchy that partitions, and clean text.
      - module `corpus::tests` · ethaor
        - `counts_are_the_measured_ones` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `index_follows_its_layout` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `every_series_of_offsets_is_monotonic_and_bounded` — Every series of offsets is monotonic and stays inside the text: that is what makes `slice` safe without a single check at run time. <sub>unit, module-private</sub>
        - `the_hierarchy_partitions_the_corpus` — Books partition the chapters, chapters partition the verses: no hole, no overlap, canonical order kept. <sub>unit, module-private</sub>
        - `the_canonical_order_is_the_one_displayed` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_split_books_are_cut_at_the_traditional_rank` — The four books the source gives in one piece, cut at the traditional rank. The totals must match exactly. <sub>unit, module-private</sub>
        - `the_densest_book_is_the_hundred_and_fifty_psalms` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `chapter_labels_restart_at_one_and_avoid_the_divine_name` — Every book restarts its chapters at `א`, and `15` / `16` never form a divine name. <sub>unit, module-private</sub>
        - `verse_texts_are_clean` — Cleaning, as `build.rs` promises it: no marker, no punctuation, words separated by a single space, and the maqaf turned into a space rather than dropped. <sub>unit, module-private</sub>
        - `the_holam_haser_of_mechon_mamre_survived` — The 0xCA fix of `decode_cp1255`: without it, 414 signs are lost. <sub>unit, module-private</sub>
        - `a_fragment_is_found_back_from_its_offset` — `offset_of` is the only thing that crosses the boundary with the page: a fragment must be found back from its offset and its length alone. <sub>unit, module-private</sub>
        - `the_corpus_holds_a_space` — `App::space_ratio` measures the width of a space on a real space taken from the corpus: there must be one. <sub>unit, module-private</sub>
        - `section_names_are_the_three_portions` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `verse_labels_number_from_one` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_text_regions_follow_the_documented_order` — The text region order `build.rs` writes — section names, book names, chapter labels, verse labels, verse texts — is what lets the page tell a verse from a title by its offset alone. <sub>unit, module-private</sub>
        - `chapters_land_in_any_order_then_the_whole_text` — **Chapters land in any order, each straight into its place**: a landed chapter reads exactly as the whole corpus has it, one not landed reads as empty, the text stays valid UTF-8 throughout — and the whole text landing last, over chapters already in, gives the corpus byte for byte, readings included. <sub>unit, module-private</sub>
        - `a_tile_range_is_its_chapters_stretch` — A tile asks for exactly its chapters' stretch of the opened text part: consecutive runs are contiguous, and the whole run covers the stream. <sub>unit, module-private</sub>
        - `books_land_in_any_order_then_the_readings` — **The text lands a book at a time, in any order, then the readings**: the readings refuse to land while a chapter is missing, and once every book is in they complete the corpus byte for byte. <sub>unit, module-private</sub>
  - **els/**
    - [`tests.rs`](src/els/tests.rs) — 26 tests — covers [`src/els.rs`](src/els.rs)
      <br>The search: the welded stream is the text's letters and nothing else, the derived indices say what the stream says, every method finds exactly what trying every start and every skip finds, and every record places its hit where it really is.
      - module `els::tests` · ethaor
        - `the_stream_is_every_verse_read_letter_by_letter` — The stream is the verses' consonants, folded, in order, and nothing else: `build.rs` and `letter::code` agree on every verse. <sub>unit, module-private</sub>
        - `final_forms_fold_onto_their_letter` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `postings_list_every_position_of_every_letter` — Postings list every position of every letter, ascending, and the counts are theirs. <sub>unit, module-private</sub>
        - `residue_lists_split_a_letter_by_its_gap` — A residue list holds the positions of its letter with its residue, and only those, ascending. <sub>unit, module-private</sub>
        - `a_plane_bit_is_the_letter_at_that_step` — A plane's bit for an anchor and a step is the letter at that distance: forward for a gap after the anchor, backward for one before. And a window read anywhere gives the 64 bits that follow, wherever the word boundary. <sub>unit, module-private</sub>
        - `every_method_finds_exactly_what_the_naive_search_finds` — **Every method finds exactly what the naive search finds**, on random streams of every size from empty to thousands, over patterns of two to six letters, contiguous, one skip, narrow and wide ranges, and ranges past the end of the stream. <sub>unit, module-private</sub>
        - `the_methods_agree_on_the_corpus` — The methods agree on the corpus itself: all three over a narrow range, `Pairs` and `Planes` over ranges no naive search — nor `Direct` in a test build — could cover. Each is held to the naive search on random streams; here the real letter frequencies and the stream's ends are met. <sub>unit, module-private</sub>
        - `the_cap_keeps_the_first_hits_and_the_exact_total` — Past the cap, the total stays exact and the records are the first `cap` hits in (skip, start) order — for every cap, including one that falls in the middle of a skip. <sub>unit, module-private</sub>
        - `an_empty_query_finds_nothing` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `a_single_letter_ignores_the_skip` — A single letter has no skip: every occurrence once, under the first skip. <sub>unit, module-private</sub>
        - `pointing_and_final_forms_do_not_change_the_search` — Marks are ignored and final forms folded: pointed, bare and unfolded spellings are one search. <sub>unit, module-private</sub>
        - `torah_is_read_every_fiftieth_letter_from_the_first_tav` — The classic skip: "תורה" spelled from the ת of בראשית, every fiftieth letter — 49 letters jumped between two. A range that holds 49 finds it too. <sub>unit, module-private</sub>
        - `an_occurrence_may_span_two_verses` — An occurrence ignores verse boundaries: the last letter of a verse and the first two of the next are one hit, whose passage spans both. <sub>unit, module-private</sub>
        - `every_record_places_its_hit_where_it_is` — Every record places its hit where it is: the letter is the verse's start plus its rank, the verse is in the chapter, the chapter in the book, the last letter in the last verse — and the letters read at its skip spell the pattern. Records come sorted by skip, then start. <sub>unit, module-private</sub>
        - `a_flood_of_hits_is_capped_and_still_counted` — A pair of common letters over a wide range overflows the cap: the total is still exact — every method counts the same — and the page gets `CAP` records. <sub>unit, module-private</sub>
        - `a_range_search_runs_in_a_blink` — Ten thousand skips of a common word, in an unoptimised test build. The bound is loose — release answers in a few tens of milliseconds — but one scan per skip, the approach the range search replaced, takes tens of seconds here: that is the day it catches. <sub>unit, module-private · time budget</sub>
        - `the_cost_model_picks_the_winner` — The cost model picks, on the corpus, the method the benchmark measured fastest by a wide margin — in V8 and natively alike. Close calls are left out: there the choice hardly matters, and the measure is noise. <sub>unit, module-private</sub>
        - `bench_methods` — Times every method on every word and range, and prints what the cost model reads — anchors, steps tried, the second letter's rarity — beside each time. `COLD=1` empties the caches before each run, as a first query pays them. Not a test: run it in release with `cargo test --release bench_methods -- --ignored --nocapture`. <sub>unit, module-private · ignored: a measurement, run by hand</sub>
        - `the_radix_sort_sorts` — The radix sort sorts: random keys of every width, duplicates, empty. <sub>unit, module-private</sub>
        - `the_placement_tables_agree_with_the_corpus` — The placement tables give each letter its verse, each verse its chapter, each chapter its book — the very answers of the corpus index. <sub>unit, module-private</sub>
        - `a_repeated_query_is_answered_from_memory` — A query asked again is answered from memory, identically; the memory holds a few answers and forgets the oldest. <sub>unit, module-private</sub>
        - `two_letters_are_counted_exactly_without_listing` — Two letters are counted without listing, and the count is the naive one — forward and backward gaps, every range, up to past the stream. <sub>unit, module-private</sub>
        - `hits_are_the_records_unplaced` — `hits` is what `search` places: the same (start, skip) pairs, in the same order, with the same total — and one memory serves both, so a page asked through one is not searched again through the other. <sub>unit, module-private</sub>
        - `a_scoped_search_finds_what_lies_wholly_in_its_stretch` — A search kept to a stretch finds exactly the occurrences lying wholly in it — tried one by one — given back as ranks in the whole stream. <sub>unit, module-private</sub>
        - `the_whole_stretch_is_the_plain_search` — The whole stream as a stretch is the plain search, and a stretch asked again is remembered like any query. <sub>unit, module-private</sub>
        - `the_cost_is_estimated_without_searching` — The estimate is read without searching, grows with the range of skips, and tells a light query from a flood by orders of magnitude. <sub>unit, module-private</sub>
  - **letter/**
    - [`tests.rs`](src/letter/tests.rs) — 7 tests — covers [`src/letter.rs`](src/letter.rs)
      <br>Splitting into letters: a consonant carries the marks that follow it, and nothing is ever lost on the way.
      - module `letter::tests` · ethaor
        - `an_empty_word_has_no_letter` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `a_letter_is_a_consonant_and_its_marks` — A letter = one consonant and its marks. Every chunk must therefore start on a consonant, and hold a single one. <sub>unit, module-private</sub>
        - `the_marks_stay_with_their_consonant` — Marks must stay stuck to their consonant: detaching them would break the rendering. <sub>unit, module-private</sub>
        - `a_string_without_consonant_is_one_chunk` — A string with no consonant at all — marks alone — is one single chunk: the iterator never loops and never returns an empty slice. <sub>unit, module-private</sub>
        - `final_forms_count_as_consonants` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_split_is_lossless_on_the_corpus` — On the real corpus: the split is lossless and the count matches, word by word. That is the invariant `letter_cells` leans on to place a letter. <sub>unit, module-private</sub>
        - `the_iterator_can_be_walked_twice` — `Clone` is part of the contract: `letter_cells` walks the letters twice, once for the sum of the widths and once to place the cells. <sub>unit, module-private</sub>
  - **pack/**
    - [`tests.rs`](src/pack/tests.rs) — 6 tests — covers [`src/pack.rs`](src/pack.rs)
      <br>The packed corpus gives back the working form byte for byte, and the rules it rests on — numerals, label exceptions, varints — hold on their own.
      - module `pack::tests` · ethaor
        - `the_packed_corpus_unpacks_byte_for_byte` — **The packed corpus unpacks to the working form, byte for byte**: text, index and readings. Every other module reads the unpacked form, so this one equality is what lets them all go on unchanged. <sub>unit, module-private</sub>
        - `the_map_alone_holds_names_labels_and_counts` — The map alone already holds what the overview draws — every name and label in place, the index complete but for the verse offsets — and where every chapter will land. <sub>unit, module-private</sub>
        - `the_packed_corpus_is_a_fraction_of_the_working_form` — What packing is for: the map is a sliver, and the text part holds about one byte per character, where UTF-8 spends two on a Hebrew one. <sub>unit, module-private</sub>
        - `hebrew_numerals_follow_the_tradition` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `label_exceptions_carry_only_what_breaks_the_rule` — Labels that follow the rule cost nothing; one that breaks it is written whole, with the count it resumes from — and laying them gives the labels back. <sub>unit, module-private</sub>
        - `varints_round_trip` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
  - **shared/**
    - [`tests.rs`](src/shared/tests.rs) — 24 tests — covers [`src/shared.rs`](src/shared.rs)
      <br>The shared layer: what both transports answer, checked once.
      - module `shared::tests` · ethaor
        - `the_book_table_follows_the_corpus` — The English table walks the corpus' books in step: a book left out or swapped would name every later verse wrong. <sub>unit, module-private</sub>
        - `a_numeral_reads_as_its_value` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_numbers_are_the_labels` — Every label of the corpus is a numeral: chapters count 1, 2, 3… in each book, and verses go on from where the previous one — joined or not — stopped. <sub>unit, module-private</sub>
        - `a_verse_is_named_every_way` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `every_letter_is_placed_in_its_verse` — Every letter of the stream is found back in its verse, at its rank there. <sub>unit, module-private</sub>
        - `a_query_is_refused_with_its_reason` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `a_page_places_the_engines_hits` — A page says what the engine's records say: the same hits in the same order, each letter spelling the word in the verse the record names. <sub>unit, module-private</sub>
        - `torah_starts_in_the_first_verse` — `תורה` every fiftieth letter from the first tav of Genesis. <sub>unit, module-private</sub>
        - `pages_tile_the_listed_hits` — Pages cut the listed hits without gap or repeat, the last says there is no next, and a limit of 0 counts without listing. <sub>unit, module-private</sub>
        - `a_flood_is_counted_whole_and_listed_to_the_cap` — A flood: more hits than can be listed. The total stays exact, the listed stop at `CAP`, and the last listed page closes the walk. <sub>unit, module-private</sub>
        - `extreme_bounds_answer_without_panic` — Bounds a caller may send, however far: never a panic, never a hit past the stream. <sub>unit, module-private</sub>
        - `finals_and_marks_fold_into_one_search` — Final forms and marks change nothing; a word of marks alone, or of another script, has no letter to search. <sub>unit, module-private</sub>
        - `a_single_letter_is_each_of_its_occurrences` — One letter has no skip: each occurrence counts once, under `skip_min`. <sub>unit, module-private</sub>
        - `the_portions_are_the_maps` — The portions are the map's: same books, same order. <sub>unit, module-private</sub>
        - `a_book_is_named_by_its_osis_code_alone` — A book is named by its OSIS code, exactly, and by nothing else: no other case, no English, no Hebrew, no transliteration or abbreviation — those are what answers give back, never what is read. <sub>unit, module-private</sub>
        - `a_reference_names_its_verses` — An OSIS reference names a book, a chapter or a verse; anything else — another notation, a number past the text — names nothing. <sub>unit, module-private</sub>
        - `a_scope_is_named_one_way` — One way of scoping at a time, each naming the verses it says. <sub>unit, module-private</sub>
        - `a_scoped_search_stays_in_its_verses` — A scoped search keeps every letter of every hit in its verses, finds no more than the whole text, and is refused when it holds no verse. <sub>unit, module-private</sub>
        - `a_query_too_costly_is_refused_before_it_runs` — Past the allowed cost a query is refused before it runs, with its estimate; under it, it is admitted. <sub>unit, module-private</sub>
        - `every_chapter_reads_back_its_verses` — Every chapter reads back whole: its verses in order, our text as the corpus holds it, its book, portion and number the reference's, opening its book exactly when it is the first, and found again by its OSIS name. <sub>unit, module-private</sub>
        - `a_shown_verse_is_ours_with_the_other_forms_inserted` — A verse shown with the other reading is ours with the other forms inserted: taking every insertion out gives our verse back byte for byte, and each run said to be theirs is a non-empty stretch of the shown verse on a character boundary, carrying their index mark and never ours. Elsewhere the shown verse is ours, with no run. <sub>unit, module-private</sub>
        - `a_reference_names_its_chapter_or_is_refused` — A reference names a chapter, or the chapter of a verse; a book, a chapter past its book, or nothing at all names none and is refused with its code. <sub>unit, module-private</sub>
        - `neighbours_run_across_books_and_stop_at_the_ends` — Neighbours run across books and stop at the Tanakh's two ends; a chapter out of range reads nothing. <sub>unit, module-private</sub>
        - `the_contents_walk_every_book_and_chapter_in_order` — The contents walk the Tanakh whole and in order: the 39 books each once, each under the portion `PORTIONS` puts it in, named as the corpus names it, its chapters running on from the last book's with no gap or overlap to the 929th, each numbered as the corpus numbers it. <sub>unit, module-private</sub>
  - **tests/**
    - [`api.rs`](src/tests/api.rs) — 21 tests
      <br>The interface the page really calls.
      - module `tests::api` · ethaor
        - `the_page_can_draw_a_frame_from_a_cold_start` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_gestures_move_the_camera_and_come_back` — The gestures the page sends: wheel, drag, Escape. <sub>app: the module as the page drives it</sub>
        - `resizing_keeps_the_framing` — Resizing keeps the framing when it was framed, and leaves the view alone otherwise. <sub>app: the module as the page drives it</sub>
        - `a_click_is_dead_above_the_reading_zoom` — **A click is only live on a passage one can read.** At the framing, where a chapter is a title in a box, nothing is clickable and the cursor says so. <sub>app: the module as the page drives it</sub>
        - `clicking_gives_the_passage` — Once the lines are read, a click gives the passage — the slots of `selection_ptr`, the verse numbered. <sub>app: the module as the page drives it</sub>
        - `the_book_selects_a_verse_by_its_index` — The book view's click through the export, as the page calls it: a verse fills the slots `select` fills — the passage and its number — and a verse out of range clears them. <sub>app: the module as the page drives it</sub>
        - `the_breadcrumb_is_filled_from_the_centre_of_the_view` — `focus` feeds the breadcrumb from the centre of the view: section, book, chapter and verse, each as an (offset, length) into the corpus. <sub>app: the module as the page drives it</sub>
        - `the_breadcrumb_follows_the_pointer` — `focus_at` is the hover: it answers 1 when the pointer is on something, and 0 when it is on nothing — falling back on the centre of the view, so that the navbar never blinks out. <sub>app: the module as the page drives it</sub>
        - `a_mouse_gone_names_nothing_and_a_finger_lifted_hands_over_to_the_centre` — **A mouse gone from the canvas empties the breadcrumb**; a lifted finger hands it to the centre of the view. The mouse often leaves onto the breadcrumb row itself, and the centre then named a portion under the cursor, which read as a selection. <sub>app: the module as the page drives it</sub>
        - `a_font_change_forgets_every_width` — A font change forgets every measured width: a width taken on a fallback font would be wrong for good. <sub>app: the module as the page drives it</sub>
        - `the_probes_answer_on_every_state` — The probes the sondes read: they must answer on every state, and never lie about the depth. <sub>app: the module as the page drives it</sub>
        - `a_frame_can_be_painted_from_the_module_memory_alone` — Everything the page needs to paint is reachable from the module's memory: a frame is a flat list of words, and a fragment is an offset into the corpus. <sub>app: the module as the page drives it</sub>
        - `the_page_searches_through_the_query_buffer` — The search as the page calls it: write the query at `els_query_ptr`, call `els_search`, read `els_stride` words per hit at `els_hits_ptr`. It needs no window, and a query that is not UTF-8 finds nothing. <sub>app: the module as the page drives it</sub>
        - `the_landing_exports_take_a_tile_as_the_page_writes_it` — The landing exports, moved to `wasm/landing.rs`, as both modules call them: room for a tile, a tile landed, the readings landed once the whole text is in. Natively the text is in already: landing again changes nothing, and says the text is whole. <sub>app: the module as the page drives it</sub>
        - `the_page_searches_a_stretch_of_verses` — The scoped search as the page would call it: the records of `search_in` over the verses' letters, and nothing for an empty or outlying range. <sub>app: the module as the page drives it</sub>
        - `a_panic_leaves_its_place_and_message_for_the_page` — What a panic leaves for the page: its place, a new line, its message — read back through the two exports the page reads after a trap. <sub>app: the module as the page drives it</sub>
        - `the_book_view_reads_every_chapter_from_its_record` — A chapter's record, read word by word as `book-view.ts` reads it: the strings it points at are the chapter's number, its book's and portion's names, and per verse its number and its text as shown — the other edition's runs inside it. Every chapter; and nothing for one out of range. <sub>app: the module as the page drives it</sub>
        - `the_book_view_reads_the_contents_from_their_record` — The contents' record, read as `book-view.ts` reads it: three portions named, the 39 books each under its portion with its name and chapters, the 929 chapters' numbers — the strings the corpus' own, and the record the same at every call. <sub>app: the module as the page drives it</sub>
        - `the_page_reads_the_depth_and_frames_the_books` — The depth as the page reads it to hand a level over: the opening 0, `frame_books` 1 — after a chapter too — a book shown at its chapters 2, and a chapter shown 3. <sub>app: the module as the page drives it</sub>
        - `the_page_gives_the_portions_size` — The book view's size as the page gives it: kept when it is one, a size not finite or not positive ignored. <sub>app: the module as the page drives it</sub>
        - `the_page_hands_the_place_back_and_forth` — The hand-off as the page drives it: the map names where its centre is, and `show` takes it back to a chapter and verse — a verse of -1 is the chapter's top, a chapter out of range moves nothing. <sub>app: the module as the page drives it</sub>
    - [`breadcrumb.rs`](src/tests/breadcrumb.rs) — 14 tests
      <br>The breadcrumb: where one is, from the zoom and from the pointer.
      - module `tests::breadcrumb` · ethaor
        - `the_framing_names_a_portion_and_a_book` — At the framing, the map shows the portions and the books: the breadcrumb says exactly that, and not a chapter nobody can see. <sub>app: the module as the page drives it</sub>
        - `the_chapter_arrives_with_the_numbers` — The chapter arrives **with its number**, at the first notch. <sub>app: the module as the page drives it</sub>
        - `the_verse_arrives_with_the_text` — The verse arrives **with the text**, and it arrives: a dozen notches at most, on every window. <sub>app: the module as the page drives it</sub>
        - `the_depth_never_exceeds_what_is_drawn` — **The breadcrumb never says more than the map shows.** On the whole zoom range: a chapter only once the numbers are out, a verse only once the lines are read. <sub>app: the module as the page drives it</sub>
        - `the_path_holds_together` — Whatever the depth, the path holds together: the verse is of the chapter, the chapter of the book, the book of the portion. <sub>app: the module as the page drives it</sub>
        - `the_verse_level_is_a_number` — The verse level carries **a number, not the text of the verse**: it is a position, and it has a navbar to fit in. <sub>app: the module as the page drives it</sub>
        - `the_pointer_names_the_book_it_is_on` — **The pointer names what it is on.** Pointing at a book box gives that book, whatever the centre of the view says. <sub>app: the module as the page drives it</sub>
        - `the_pointer_on_a_portion_title_names_the_portion_alone` — **A portion is named by its own letters, and by nothing else.** <sub>app: the module as the page drives it</sub>
        - `the_pointer_in_the_air_names_nothing` — Pointing at the air between two portions names nothing — the same rule as a click — and the caller falls back on the centre of the view. <sub>app: the module as the page drives it</sub>
        - `the_pointer_names_the_cell_it_is_on` — Once the chapters are out, the pointer names the cell it is on — and the cell next to it names the next chapter. <sub>app: the module as the page drives it</sub>
        - `the_pointer_follows_the_verses_of_a_chapter` — Deep in a chapter, the pointer follows the **lines**: two lines of two different verses give two different breadcrumbs. <sub>app: the module as the page drives it</sub>
        - `reading_the_breadcrumb_does_not_touch_the_drawing` — Hovering still changes nothing in the drawing: the breadcrumb is read beside the frame, never inside it. <sub>app: the module as the page drives it</sub>
        - `the_void_of_a_cell_names_no_chapter` — **The pointer names what it is on, not what it is near.** <sub>app: the module as the page drives it</sub>
        - `the_void_of_a_book_box_names_no_book` — **The void of a book box names no book.** Once the numbers are out, the title falls back into its band and the block takes the top right: most of the box carries no ink, and pointing there used to name a book whose letters were an inch away. On nothing, the pointer names nothing, and the centre of the view takes over — the rule the air between portions already followed. <sub>app: the module as the page drives it</sub>
    - [`camera.rs`](src/tests/camera.rs) — 16 tests — covers [`src/map/camera.rs`](src/map/camera.rs)
      <br>Camera, framing, notch and thresholds.
      - module `tests::camera` · ethaor
        - `projecting_and_coming_back_is_the_identity` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `re_rooting_does_not_move_the_image` — `wrap_in` re-roots the camera in a cell holding a copy of the world: **the image does not move**, only the scale comes back down — which is what keeps the floats exact. <sub>app: the module as the page drives it</sub>
        - `a_fold_brings_the_scale_back_down` — Re-rooting is **a matter of precision**: after a fold the scale must come back near 1, whatever the depth reached. <sub>app: the module as the page drives it</sub>
        - `the_descent_is_one_way` — The descent is one-way: zooming back out never brings the previous Tanakh back. <sub>app: the module as the page drives it</sub>
        - `the_framing_has_a_floor` — The framing is the whole world in the window — **unless** that made the numbers illegible at the first notch. Below that floor we crop the map rather than push the levels back. <sub>app: the module as the page drives it</sub>
        - `the_framing_centres_the_world` — At launch the whole map is framed and centred. <sub>app: the module as the page drives it</sub>
        - `the_dive_always_takes_the_same_number_of_notches` — **The notch is computed, not fixed**: the dive from the framing down to a full-screen chapter always takes `DIVE_NOTCHES` notches, whatever the window. <sub>app: the module as the page drives it</sub>
        - `a_landscape_window_keeps_the_nominal_notch` — On a landscape window the framing is already proportional to the height, so the computation gives exactly the nominal notch. <sub>app: the module as the page drives it</sub>
        - `the_three_levels_are_one_notch_apart_on_every_window` — **The three levels are one notch apart, and the map opens on the first.** <sub>app: the module as the page drives it</sub>
        - `the_level_of_detail_does_not_oscillate` — The decision is always taken on the **chapter** layout, the one that will be in place after the swap: judging it on the current one made the level oscillate from one frame to the next. <sub>app: the module as the page drives it</sub>
        - `swapping_the_level_exchanges_the_two_layouts` — Swapping the level throws away what depends on the cells, and nothing else: the layouts are exchanged, never rebuilt. <sub>app: the module as the page drives it</sub>
        - `the_three_portions_are_one_notch_under_the_framing` — **One zoom-out notch, and the map is nothing but the three portions.** The threshold is just under the framing, so the framing itself stays on the books' side. <sub>app: the module as the page drives it</sub>
        - `zooming_out_centres_the_map_and_stops_at_the_technical_floor` — Zooming out keeps the map centred and never lets the scale fall to zero — the only bound left, and a technical one. <sub>app: the module as the page drives it</sub>
        - `panning_then_framing_comes_back_to_the_start` — Panning moves the world and nothing else; `fit` brings everything back. <sub>app: the module as the page drives it</sub>
        - `zooming_out_keeps_the_cursor_still` — A zoom gesture keeps the point under the cursor still — as long as no pull applies, i.e. when zooming out. <sub>app: the module as the page drives it</sub>
        - `absurd_windows_do_not_break_the_camera` — Resizing keeps the state consistent: a window of one pixel, or of none, must not break anything. <sub>app: the module as the page drives it</sub>
    - [`click.rs`](src/tests/click.rs) — 13 tests
      <br>What a click selects, and when it is live at all.
      - module `tests::click` · ethaor
        - `a_click_is_live_exactly_where_the_text_is_drawn` — **A click is live exactly where the text is drawn**, and nowhere else. <sub>app: the module as the page drives it</sub>
        - `a_click_is_dead_at_the_framing_and_at_the_chapter_level` — On the reference window, the measurement of `CLAUDE.md`: **nothing is clickable at the framing, nor at the chapter level.** One reads titles and numbers there, not passages. <sub>app: the module as the page drives it</sub>
        - `a_click_goes_live_with_the_text` — And it goes live **exactly when the lines are read** — the same threshold that puts the verse in the breadcrumb and the text in the frame. <sub>app: the module as the page drives it</sub>
        - `the_cursor_announces_exactly_what_a_click_will_do` — **The pointer announces what a click will do**: `hover` answers on the very condition of a click, everywhere and at every zoom. <sub>app: the module as the page drives it</sub>
        - `a_click_in_the_air_selects_nothing` — In the air between two portions, or in the black that borders the map, a click selects nothing — whatever the zoom. <sub>app: the module as the page drives it</sub>
        - `a_click_gives_the_passage_under_the_point` — A click gives a coherent path, and the verse it gives is the one under the point. <sub>app: the module as the page drives it</sub>
        - `a_click_numbers_the_verse_it_gives` — The passage comes with **its number**, the very label its verse is drawn with — so the page can name the place it shows without guessing it from the text. <sub>app: the module as the page drives it</sub>
        - `the_words_of_a_line_fill_it_right_to_left` — The word boxes of a line go **right to left**, hold their ink and nothing else, and never overlap: the gaps between them are the spaces. <sub>app: the module as the page drives it</sub>
        - `a_single_word_takes_the_whole_line` — A line of a single word gives one box, laid on the right edge. <sub>app: the module as the page drives it</sub>
        - `a_line_without_size_has_no_word_cell` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_letters_of_a_word_stay_inside_it` — Words and letters agree: a letter box is inside the box of the word that holds it — the two splits share one measured width. <sub>app: the module as the page drives it</sub>
        - `a_click_in_the_book_names_what_the_map_names` — The book's click, where the browser lays the verse out, **names what the map's click names**: for every line of a chapter at reading level, the map's pick at the line and the book's pick of its verse give the same slots. <sub>app: the module as the page drives it</sub>
        - `a_click_in_the_book_names_every_verse` — Every landed verse is selected by the book with its own text and number; a verse out of range names nothing. <sub>app: the module as the page drives it</sub>
    - [`frame.rs`](src/tests/frame.rs) — 32 tests — covers [`src/map/frame.rs`](src/map/frame.rs)
      <br>The command list: what a frame always holds, and what it never holds.
      - module `tests::frame` · ethaor
        - `the_command_list_has_a_whole_number_of_records` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `a_frame_holds_nothing_but_text_and_clips` — **No fill is ever painted.** Only text and clipping come out of here: the white square went away with the zoom-out floor and the early fold. <sub>app: the module as the page drives it</sub>
        - `clips_are_balanced` — Clips are opened and closed in pairs, and never left open at the end of a frame. <sub>app: the module as the page drives it</sub>
        - `no_frame_is_ever_without_text` — **No empty frame**, anywhere on the zoom range: a cell always shows something, its text as soon as that is legible, its number otherwise. <sub>app: the module as the page drives it</sub>
        - `every_text_command_is_drawable` — Every command carries a legible size, a usable alpha and a fragment that really is in the corpus. <sub>app: the module as the page drives it</sub>
        - `the_framing_shows_the_three_portions_and_the_thirty_nine_books` — At the framing the map shows **only the sections and the books**: three portion titles and 39 book titles, and not a single chapter number. <sub>app: the module as the page drives it</sub>
        - `an_overview_title_fills_its_box` — At the framing the title of a book **fills its whole box**: the chapter numbers being hidden, the box belongs to it. <sub>app: the module as the page drives it</sub>
        - `the_chapter_level_shows_three_sizes_and_three_only` — One notch in, the chapters are out: **three sizes, and three only** — the numbers, the books, the sections. <sub>app: the module as the page drives it</sub>
        - `the_rank_reads_in_the_intensity_of_the_white` — The rank reads in the intensity of the white: a section in pure white, a book one tone below, a number a touch lower still. <sub>app: the module as the page drives it</sub>
        - `one_notch_out_leaves_the_three_portions_alone` — **One zoom-out notch, and the map is nothing but the three portions, large, at equal distance** — one single right edge, and no fill behind them. <sub>app: the module as the page drives it</sub>
        - `the_opening_never_cuts_a_portion` — **The opening never cuts a portion's name**, however short the window: the floored framing crops the world there, and the three names shrink with the room instead — their ink inside the screen, still one size, one gap apart. <sub>app: the module as the page drives it</sub>
        - `the_opening_draws_the_portions_at_the_book_views_size` — **Given the book view's size, the opening draws the three names at it**, spaced as the book view spaces them, on every window — then, one notch out, smaller by that notch, as the map recedes. A size too large for the room is bounded by it: never a name cut. <sub>app: the module as the page drives it</sub>
        - `no_text_is_ever_laid_over_another` — **Nothing is ever laid over anything else.** Boxes of ink, on screen, do not meet — titles, numbers and verses alike. <sub>app: the module as the page drives it</sub>
        - `the_verses_arrive_within_a_dozen_notches` — The verses arrive, and they arrive **early**: between the seventh and the twelfth notch depending on the window, never later. <sub>app: the module as the page drives it</sub>
        - `a_cell_shows_its_text_or_its_number_but_not_both` — All the verses of a chapter share one character size, and a number is never drawn next to the text of its own cell. <sub>app: the module as the page drives it</sub>
        - `the_text_carries_its_chapter_in_the_corner_and_its_verses_at_their_right` — **Once the text is out, its numbers are beside it.** The chapter number is set in the top-right corner of its column, above the first verse, and each verse number at the right of its first line, on the same row — never inside the text, which the overlap test already holds. <sub>app: the module as the page drives it</sub>
        - `no_number_is_ever_drawn_twice` — **A number is drawn once or not at all.** The centred number of a cell and the one in the corner of its text are the same label: a frame holding both would number the chapter twice. Every chapter and verse label is a distinct slice of the corpus, so a repeated slice is a repeated number. <sub>app: the module as the page drives it</sub>
        - `every_book_title_on_screen_shares_one_size` — **All book titles share one size at a given zoom**, at every notch and not only at the first: `uniform_title` takes one band for all of them. <sub>app: the module as the page drives it</sub>
        - `no_title_grows_past_its_ceiling` — **Titles stop growing at their ceiling**: past it the band has left the screen, one is inside the book, and the title only recalls where one is. The portion title keeps its ratio over the book title, ceiling included. <sub>app: the module as the page drives it</sub>
        - `a_book_title_sits_at_the_bottom_of_its_band` — **Once the chapters are out, a book title sits at the bottom of its band**, just above the first chapter row: that keeps the gap between them stable at every zoom. <sub>app: the module as the page drives it</sub>
        - `the_measurements_converge_in_one_frame` — Rust draws from an estimate, lists what it does not know in `pending`, the page measures, and the next frame is right: **the convergence takes one frame.** <sub>app: the module as the page drives it</sub>
        - `a_measured_numeral_rebuilds_the_layout_once_and_keeps_the_framing` — A measurement that contradicts the estimate rebuilds the layout **once**, at the start — and the framing follows, so that a camera still sitting on it does not fall into the three-portions state. <sub>app: the module as the page drives it</sub>
        - `a_late_measurement_invalidates_the_caches` — A measurement arriving later throws away what depends on it, and nothing else. <sub>app: the module as the page drives it</sub>
        - `forgetting_the_measurements_leaves_a_drawable_state` — Forgetting every measured width — what a font change calls for — must leave a drawable state. <sub>app: the module as the page drives it</sub>
        - `the_list_of_measurements_is_bounded` — `pending` is bounded: a frame asking for the whole Tanakh must not grow without end. <sub>app: the module as the page drives it</sub>
        - `hovering_changes_nothing_in_the_drawing` — **Hovering changes nothing in the drawing**: the command list is identical to the bit, with and without. <sub>app: the module as the page drives it</sub>
        - `two_frames_at_the_same_camera_are_identical` — Two frames at the same camera give the same list: the rendering is a pure function of the state. <sub>app: the module as the page drives it</sub>
        - `a_book_title_keeps_the_right_edge_of_its_box` — Panning does not reshape a book title: it keeps the right edge of its box, never that of the screen. <sub>app: the module as the page drives it</sub>
        - `a_portion_title_is_pinned_to_the_screen` — A portion title, on the other hand, is **pinned to the screen**: it stays legible as long as one is inside its portion. <sub>app: the module as the page drives it</sub>
        - `the_command_list_stays_short` — **The frame is a short list, and it stays short.** <sub>app: the module as the page drives it</sub>
        - `a_frame_is_built_in_a_blink` — A frame must be built well inside a wheel notch. The figure quoted in `CLAUDE.md` — 0.03 ms for the overview — is a release one; this bound is loose enough for an unoptimised test build and still catches the day a frame starts doing work proportional to the whole corpus. <sub>app: the module as the page drives it · time budget</sub>
        - `a_record_keeps_the_layout_the_page_reads` — **The layout of a record is a contract with the page.** `ethaor.ts` reads the slots by their rank — kind, x, y, w, h, alpha, size, offset, length — so any change here must be made there too. This test is the alarm. <sub>app: the module as the page drives it</sub>
    - [`geometry.rs`](src/tests/geometry.rs) — 8 tests
      <br>The small pure functions the rest leans on: distances, text column, pull, keys, size of a number.
      - module `tests::geometry` · ethaor
        - `a_point_inside_a_box_is_at_no_distance` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `distance_grows_with_the_offset` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_text_column_is_a_share_of_its_cell` — **The margin of a cell follows the cell** — 6 % of its side — because the whole rendering is proportional. Fixed at 2 world units, it threw away 40 % of the ink of a small cell. <sub>app: the module as the page drives it</sub>
        - `the_pull_beats_the_drift_of_one_notch` — **The pull must beat the drift of the zoom.** One notch multiplies the distance from cursor to target by `factor`; a correction of `1 - 1/factor` compensates it exactly, and anything below lets the target escape. <sub>app: the module as the page drives it</sub>
        - `a_key_points_back_into_the_corpus` — The key is all that crosses the boundary: an offset and a length into the corpus. <sub>app: the module as the page drives it</sub>
        - `a_number_is_exactly_half_a_title` — **A number is worth half a book title.** `chapters_fit` deduces the height of the band from that ratio, so the ratio must hold in the finished layout — under the estimate as under a measurement. <sub>app: the module as the page drives it</sub>
        - `the_two_levels_share_one_world` — The two levels of detail are built on the same world, which is what keeps the camera valid when they are swapped. <sub>app: the module as the page drives it</sub>
        - `a_measured_numeral_never_overflows_its_cell` — A number of chapters never reads wider than its cell, whatever the digits the page comes back with. <sub>app: the module as the page drives it</sub>
    - [`handoff.rs`](src/tests/handoff.rs) — 7 tests
      <br>The hand-off between the map and the book view: the map names where the view is (`here`), never deeper than its image, and takes a reader back to a chapter at its reading level, the verse at the centre (`show`).
      - module `tests::handoff` · ethaor
        - `every_chapter_shown_is_read_and_named_back` — Every chapter, shown with its last verse: the map then reads it — its text, not its number — and names back that very chapter and verse. The last verse is the farthest from where a chapter starts, so the one a centring error would miss first. <sub>app: the module as the page drives it</sub>
        - `a_chapter_is_shown_on_every_window` — On every window, once the page has measured what the frame asks: a chapter shown again lands on its verse, and the frame draws verses. <sub>app: the module as the page drives it</sub>
        - `a_chapter_shown_without_a_verse_is_named_back` — Shown with no verse, a chapter is named back without one asked: the view lands on it, and on its first lines when it is taller than the screen. <sub>app: the module as the page drives it</sub>
        - `here_is_never_deeper_than_the_image` — `here` says no more than the image: the Tanakh's start at the opening, a book's first chapter while only books show, and no verse until the text is read. <sub>app: the module as the page drives it</sub>
        - `a_chapter_out_of_range_moves_nothing` — What `show` cannot place leaves the view as it was. <sub>app: the module as the page drives it</sub>
        - `the_depth_is_the_image_on_every_window` — The depth the book view's levels read is the image's, on every window: the opening is the three portions alone, the framing the books, a notch in the chapters' numbers, a chapter shown its text — and `here` never names deeper than that depth says. <sub>app: the module as the page drives it</sub>
        - `every_book_is_shown_at_its_chapters` — Every book shown at the chapters' level, on every window: its numbers out, its text not read, the centre naming one of its own chapters — the zoom the book view's chapters hand over to. <sub>app: the module as the page drives it</sub>
    - [`landing.rs`](src/tests/landing.rs) — 5 tests — covers [`src/map/landing.rs`](src/map/landing.rs)
      <br>The text lands after the map: until it does, the map is whole but the reading level stays closed — nothing drawn, clicked or named needs a verse that is not there — and it opens, at the same zoom, the moment it lands.
      - module `tests::landing` · ethaor
        - `the_map_stays_whole_until_the_text_lands` — **Before the text, the dive goes on with the numbers**: well past the reading level no frame is black, no verse is drawn, clicked or named — and the moment the text lands the verses appear. <sub>app: the module as the page drives it</sub>
        - `the_overview_needs_no_verse` — The map alone is the overview: at the framing, nothing drawn without the text differs from what is drawn with it. <sub>app: the module as the page drives it</sub>
        - `tiles_are_asked_for_ahead_of_the_reading_level` — **A chapter's tile is asked for before its lines are legible**: diving at the centre with no verse in, the chapter under it is asked two notches or more ahead of the reading level — so its verses travel while the zoom closes in. Nothing is asked twice, only chapters on screen are, and each run asks for its chapters' very bytes. <sub>app: the module as the page drives it</sub>
        - `books_are_fetched_nearest_to_the_view_first` — **The text follows the visitor**: the next book to fetch is the one under the centre of the view, then the nearest; every book is asked for exactly once; and when the view moves, the next one is the nearest to where it now is. <sub>app: the module as the page drives it</sub>
        - `the_verse_probe_tells_numbers_from_verses` — The probe the load benchmark times by — the first verse a visitor can read — answers no at the framing and before any chapter has landed, and yes once the dive has reached the reading level with the text in. <sub>app: the module as the page drives it</sub>
    - [`locate.rs`](src/tests/locate.rs) — 10 tests
      <br>Hit testing: what a click selects, and what it must **not** select.
      - module `tests::locate` · ethaor
        - `a_point_in_a_cell_names_its_chapter` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `every_point_of_a_book_box_names_one_of_its_chapters` — **195 points out of 195 of a book box name a chapter** — including in a book of three chapters, whose cells sit a hundred units apart. <sub>app: the module as the page drives it</sub>
        - `the_air_between_portions_names_nothing` — **The tolerance stops at the book.** Clicking in the air that separates two portions, or in the white at the bottom, selects nothing. <sub>app: the module as the page drives it</sub>
        - `the_pointer_announces_what_a_click_will_select` — The pointer announces what a click will do: `hover` follows the same rule as `locate`. <sub>app: the module as the page drives it</sub>
        - `a_located_point_gives_a_coherent_path` — A located point gives a coherent path: the chapter belongs to the book, the verse to the chapter, the letter to the line. <sub>app: the module as the page drives it</sub>
        - `the_nearest_chapter_is_found_from_anywhere` — The nearest chapter is found even from far away: that is what keeps the zoom productive when one aims between two boxes. <sub>app: the module as the page drives it</sub>
        - `a_book_is_found_by_its_box_only` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_nest_takes_seven_tenths_of_its_letter` — The nest is at the heart of a letter, and takes **seven tenths of its cell** — not a dot. <sub>app: the module as the page drives it</sub>
        - `the_zoom_target_tolerates_the_gaps` — The zoom target tolerates the gaps: aiming between two books must stay productive. <sub>app: the module as the page drives it</sub>
        - `the_breadcrumb_names_the_place_under_the_centre` — The breadcrumb always says where one is: section, book and chapter under the centre of the view, even when the title bands have left the screen. <sub>app: the module as the page drives it</sub>
    - [`readings.rs`](src/tests/readings.rs) — 12 tests
      <br>The other edition's reading, drawn inside our text.
      - module `tests::readings` · ethaor
        - `a_verse_with_another_reading_is_laid_out_from_the_display_string` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_runs_of_a_line_tile_it_from_the_right` — **The runs of a line follow each other from the right**, and together they are the line. The text is set flush right, so the first run of the string is the rightmost: each box ends where the next one begins. <sub>app: the module as the page drives it</sub>
        - `only_the_other_reading_carries_the_other_ink` — **The ink says theirs or ours, and nothing else does.** The two runs are drawn at the same strength — the colour alone sets them apart, and which colour is the page's business — so a run must never be found marked without being the other reading, nor the other way round. <sub>app: the module as the page drives it</sub>
        - `a_verse_without_a_variant_stays_one_command` — A verse without a variant is still one command: the cut costs nothing to the twenty-three thousand others. <sub>app: the module as the page drives it</sub>
        - `the_counter_follows_what_is_drawn` — The counter the page reads to show the index: zero when no other reading is on screen, more as soon as one is drawn. <sub>app: the module as the page drives it</sub>
        - `a_click_never_names_the_other_edition` — **A click keeps naming our text.** On a verse carrying another reading, the passage a click gives is ours — the other edition's forms and the marks that number them are not in it — so every word the passage offers to pick is ours. <sub>app: the module as the page drives it</sub>
        - `the_index_answers_on_the_two_forms_alone` — **The index answers on the two forms, and on nothing else** — the owner's rule. In a chapter holding another reading, read at its level: the pointer on our marked form or on any word of theirs lights it; on every other word of the chapter, the verses that carry them included, it stays shut. Which words are the forms is read off the marks the reader sees — `¹` on ours, theirs from `[` to `]²` — not off the table. <sub>app: the module as the page drives it</sub>
        - `the_answer_is_the_pointed_pair_s_reading` — **The answer is the pointed pair's reading, theirs alone** — what the page colours, and only while it is pointed at. On a word of theirs the range holds that word; on our marked form it starts right after it, nothing but the space between. Either way it reads `[…]²` and is exactly the stretch the frame cuts out as `INK_OTHER`, so the page can tell that reading's runs from every other on screen. <sub>app: the module as the page drives it</sub>
        - `at_the_framing_the_index_says_nothing` — **At the framing, nothing.** No word is drawn, so none can be pointed at — and a book holds a reading almost always. <sub>app: the module as the page drives it</sub>
        - `the_black_around_the_map_promises_nothing` — **Outside every box, nothing is promised.** The black around the map and the air between portions name no passage, so they can name no reading — the same rule the breadcrumb and the click follow. <sub>app: the module as the page drives it</sub>
        - `a_portion_band_says_nothing` — A portion band names its portion and stops there, so the index stays shut: each of the three holds readings, and saying so would be an index lit from the first frame to the last. <sub>app: the module as the page drives it</sub>
        - `a_chapter_number_lights_no_index` — **A chapter's number lights no index**, though its chapter holds a reading: a number is not one of the two forms. It used to answer for the whole chapter. <sub>app: the module as the page drives it</sub>
    - [`text.rs`](src/tests/text.rs) — 13 tests — covers [`src/map/text.rs`](src/map/text.rs)
      <br>Verses: measuring, wrapping, laying out, placing the letters.
      - module `tests::text` · ethaor
        - `the_width_of_a_fragment_is_deduced_from_its_words` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `a_space_is_measured_on_a_real_space_of_the_corpus` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `an_unknown_width_is_estimated_then_asked_for` — An unmeasured fragment is answered with an estimate **and** recorded: the next frame will be right. <sub>app: the module as the page drives it</sub>
        - `wrapping_keeps_contiguous_slices_of_the_corpus` — **Wrapping keeps slices of the corpus**: the lines of a verse, joined back with a space, give the verse again, to the byte. <sub>app: the module as the page drives it</sub>
        - `a_line_fits_in_its_column` — A line never overflows its column, except when a single word is wider than the column — there is nowhere left to cut. <sub>app: the module as the page drives it</sub>
        - `wrapping_survives_a_column_without_width` — Degenerate cases: a zero width or size gives the verse back whole rather than looping. <sub>app: the module as the page drives it</sub>
        - `a_chapter_lays_out_at_one_single_size` — **All the verses of a chapter share one size**, and the laid-out column fits in its cell. <sub>app: the module as the page drives it</sub>
        - `a_breathing_space_separates_two_verses` — A breathing space separates the verses, so one sees where one ends and the next begins. <sub>app: the module as the page drives it</sub>
        - `a_chapter_is_laid_out_once` — The layout of a chapter is computed **once**, then cached: it depends on the world, never on the zoom. <sub>app: the module as the page drives it</sub>
        - `the_letters_of_a_line_fill_it_right_to_left` — The cells of the letters of a line, **right to left**: their widths add up to the width of the line, and none leaves the line. <sub>app: the module as the page drives it</sub>
        - `an_empty_line_has_no_letter_cell` — *(no doc: say what it holds)* <sub>app: the module as the page drives it</sub>
        - `the_column_height_grows_with_the_size` — The height a chapter would take at a given size grows with the size: that is what lets `lay_out` converge in a few passes. <sub>app: the module as the page drives it</sub>
        - `bigger_cells_give_bigger_verses` — The layout follows the cell: at the chapter level, where cells are a third bigger, the verses read at a bigger size. <sub>app: the module as the page drives it</sub>
  - **treemap/**
    - [`tests.rs`](src/treemap/tests.rs) — 27 tests — covers [`src/treemap.rs`](src/treemap.rs)
      <br>The rules of the map, as `CLAUDE.md` states them.
      - module `treemap::tests` · ethaor
        - `contains_holds_the_right_edge_open` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `shrink_keeps_the_centre` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `inset_never_turns_a_box_inside_out` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `number_fill_is_bounded_by_height_then_by_width` — A number is bounded by the height of its cell **and** by its width: if the digits are wider than the estimate, width is what decides. <sub>unit, module-private</sub>
        - `chapters_fit_keeps_the_number_at_half_the_title` — Band, pitch and columns are **deduced together**: the title fills its band, the number fills its cell, and the number is worth half the title. <sub>unit, module-private</sub>
        - `the_band_is_only_trimmed_by_a_box_too_low_to_hold_it` — The band is **never** trimmed to a share of the box: a bound at `h / 2` gave the Torah, alone with a single row, boxes far taller than the others. <sub>unit, module-private</sub>
        - `an_empty_grid_has_no_cell` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `an_imposed_grid_keeps_its_cells_identical` — An imposed column count does **not** fall back on the number of elements: a row of five books in a grid of five keeps boxes the width of the others. <sub>unit, module-private</sub>
        - `a_grid_fills_right_to_left_then_downwards` — Reading order: right to left on a row, then the next row **below** — never above. <sub>unit, module-private</sub>
        - `a_square_grid_never_overflows_its_area` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_layout_covers_the_whole_corpus` — *(no doc: say what it holds)* <sub>unit, module-private</sub>
        - `the_books_sit_on_a_grid_of_five_columns` — The 39 books sit on a grid of five columns — the one the Torah imposes — which gives nine rows: one, five, three. <sub>unit, module-private</sub>
        - `every_book_box_is_the_same_size` — **All book boxes are identical**, from one section to the next: that is what the height of a section is computed for. <sub>unit, module-private</sub>
        - `every_chapter_cell_is_the_same_size` — **A single pitch for the 929 chapters.** A cell has exactly the same size everywhere, from one end of the corpus to the other — and so, therefore, does a chapter number. It is a rule, not a setting. <sub>unit, module-private</sub>
        - `the_hierarchy_nests_geometrically` — A chapter stays in its book, a book in its section, a section in the world. <sub>unit, module-private</sub>
        - `no_chapter_enters_its_book_band` — A chapter never bites into the title band of its book: that is what the band is reserved for. <sub>unit, module-private</sub>
        - `cells_do_not_overlap_and_keep_reading_order` — Cells never overlap, and they follow reading order: right to left, then the row below. <sub>unit, module-private</sub>
        - `every_block_hangs_from_the_top_right_of_its_box` — The block is **set to the top right**, where reading starts: every book puts its first row at the same distance from the top of its box, and the emptiness falls to the bottom. <sub>unit, module-private</sub>
        - `the_rows_follow_the_shape_of_the_box` — The number of rows is the one that gives the block **the shape of its body**, never the minimum — and never more than `CHAPTER_ROWS`. <sub>unit, module-private</sub>
        - `the_psalms_hold_seven_rows_of_twenty_two` — The densest book — the 150 Psalms — is what sets the grid: **seven rows of 22**. <sub>unit, module-private</sub>
        - `a_book_column_is_cut_on_its_chapter_block` — The width of a column comes from the chapter block, not from the world: the block takes some nine tenths of its box, and the map is centred in the world. <sub>unit, module-private</sub>
        - `the_three_sections_are_stacked_and_flush` — Sections are flush and stacked in order; what separates them is the air of their band. <sub>unit, module-private</sub>
        - `a_portion_is_separated_by_thirty_times_the_gutter` — Measured: 4.0 world units between two rows of a same block, 122.0 between two sections — 4 of gutter, 90 of air, 28 for the title line. Thirty times the gutter: a portion is confused with nothing. <sub>unit, module-private</sub>
        - `the_bottom_of_the_map_keeps_the_air_of_a_portion` — The bottom of the map takes back the air that precedes a portion title, so the last row of books breathes like the others. Measured: 92 world units, exactly `SECTION_LEAD + WORLD_MARGIN`. <sub>unit, module-private</sub>
        - `tight_air_gives_bigger_cells` — **The air of a portion is the only setting of the map, and it is paid for in cells.** Tight air gives boxes a third taller and cells a third bigger — the level of detail where the chapters are read. <sub>unit, module-private</sub>
        - `measured_digits_tighten_the_band_and_grow_the_cell` — Wider digits tighten the band and grow the cell: the two settings are the same one. <sub>unit, module-private</sub>
        - `the_layout_survives_absurd_inputs` — A degenerate call must not panic nor produce anything negative: the page could ask for a frame before the window has a size. <sub>unit, module-private</sub>
  - **variant/**
    - [`tests.rs`](src/variant/tests.rs) — 6 tests — covers [`src/variant.rs`](src/variant.rs)
      <br>What the welded table must hold for the drawing to be honest: the display string is our verse plus insertions, nothing else; every byte of it is either ours or named as the other edition's.
      - module `variant::tests` · ethaor
        - `the_table_holds_what_was_measured` — Verses carrying another reading, and differences in all. Measured against Miqra according to the Masorah, book by book: the numbering plays no part, so these two figures move only if the table or the source does. <sub>unit, module-private</sub>
        - `the_series_are_sorted_by_verse` — Both series are read by dichotomy, which only answers on a sorted table. <sub>unit, module-private</sub>
        - `only_the_verses_of_the_table_are_drawn_otherwise` — Every verse of the table is found again, and nothing else is. <sub>unit, module-private</sub>
        - `removing_the_insertions_gives_the_verse_back` — **The display string is our verse, with insertions.** Taking the insertions out gives the verse back byte for byte — that is what guarantees the map shows our text and adds to it, rather than replacing it. <sub>unit, module-private</sub>
        - `every_range_is_a_slice_of_the_display` — Each range stays inside the string and on character boundaries: the page slices the bytes, so a range cutting a letter in half would draw a hole. <sub>unit, module-private</sub>
        - `the_insertions_of_a_verse_follow_each_other` — The insertions of one verse do not overlap and come in reading order. <sub>unit, module-private</sub>
- **tests/**
  - [`builder.rs`](tests/builder.rs) — 31 tests
    <br>The compile-time builder, tested on its own source.
    - module `mamre::tests` · ethaor (tests/builder)
      - `the_unassigned_byte_of_mechon_mamre_is_decoded` — Mechon-Mamre uses byte 0xCA, unassigned in standard cp1255, for U+05BA. Without the fix, 414 signs are lost in the Tanakh. <sub>build: corpus reading</sub>
      - `cp1255_maps_the_three_hebrew_ranges` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `decoding_never_loses_a_byte` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `fifteen_and_sixteen_avoid_the_divine_name` — `15` and `16` are written `טו` and `טז`, never `יה` / `יו`, which would form a divine name. <sub>build: corpus reading</sub>
      - `the_numerals_follow_the_hebrew_notation` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `every_rank_of_the_tanakh_reads_in_three_letters` — The labels of the split books go up to 65 in the source; every rank a book can reach must read in at most three letters, since that is what the layout reserves for a number. <sub>build: corpus reading</sub>
      - `tags_are_stripped_whatever_their_nesting` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `cleaning_drops_the_markers_and_turns_the_maqaf_into_a_space` — Cleaning: markers `{פ} {ס} {ר} {ש}` and all punctuation go; the maqaf **becomes a space**, since it joins two distinct words. <sub>build: corpus reading</sub>
      - `cleaning_keeps_the_niqqud_and_the_cantillation` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `cleaning_normalises_the_spaces` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `an_editorial_mark_never_enters_a_rank` — Mechon-Mamre closes the bracketed section of Psalm 107 inside the label itself — `<B>קז,כג ]</B>` — and that bracket was read as part of the verse number, which the breadcrumb then announced. <sub>build: corpus reading</sub>
      - `a_joined_rank_keeps_its_hyphen` — The source gives a few ranks joined — Psalm 70 opens on `ע,א-ב`, one text under two numbers. The hyphen is part of the rank. <sub>build: corpus reading</sub>
      - `a_cover_page_opens_no_book` — A book file holds one or more `<H1>`; the cover page of `t01.htm` carries no verse at all and must be dropped. <sub>build: corpus reading</sub>
      - `chapters_keep_the_order_of_the_document` — Chapters keep **document order**: an alphabetical sort on Hebrew digits would give a wrong one. <sub>build: corpus reading</sub>
      - `two_titles_in_one_file_open_two_books` — Two `<H1>` in one file open two books: that is how Ezra and Nehemiah arrive. <sub>build: corpus reading</sub>
      - `a_split_book_is_cut_and_renumbered` — The source numbers Samuel from 1 to 55 with **no restart at `א`**: the cut happens at the traditional rank, and the halves are renumbered. <sub>build: corpus reading</sub>
      - `a_book_the_source_gives_whole_is_left_alone` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `the_four_cuts_are_the_documented_ones` — The four cuts and their ranks are the ones that make the totals match. <sub>build: corpus reading</sub>
      - `the_canonical_order_holds_the_thirty_nine_books` — The canonical order holds the 39 books: 5 of the Torah, 21 of the Prophets, 13 of the Writings. <sub>build: corpus reading</sub>
      - `reordering_follows_the_canonical_order` — `reorder` puts the source's books in the displayed order, whatever the order of the files. <sub>build: corpus reading</sub>
      - `a_missing_book_stops_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "book missing from the source"</sub>
    - module `variants::tests` · ethaor (tests/builder)
      - `comments_and_blank_lines_are_skipped` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `a_short_line_stops_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "columns"</sub>
      - `an_empty_table_stops_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "empty"</sub>
      - `a_span_is_found_by_rank` — A rank, never a search: a form may appear twice in the same verse, and only the index says which one is meant. <sub>build: corpus reading</sub>
      - `a_span_past_the_verse_stops_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "outside the verse"</sub>
      - `a_wrong_anchor_stops_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "anchor missing"</sub>
      - `the_display_string_is_the_verse_plus_the_other_reading` — **The display string is the verse, plus insertions.** Taking the insertions back out gives the verse again — that is what makes the map add to our text instead of replacing it. <sub>build: corpus reading</sub>
      - `several_readings_in_one_verse_stay_in_order` — *(no doc: say what it holds)* <sub>build: corpus reading</sub>
      - `entries_out_of_order_stop_the_build` — *(no doc: say what it holds)* <sub>build: corpus reading · must panic: "entries out of order"</sub>
      - `a_verse_without_an_entry_is_untouched` — A verse with no entry is left exactly as it is. <sub>build: corpus reading</sub>
- **web/**
  - **src/**
    - **lib/**
      - [`book-view.test.ts`](web/src/lib/book-view.test.ts) — 11 tests — covers [`web/src/lib/book-view.ts`](web/src/lib/book-view.ts)
        - module `a chapter read from the module` · vitest
          - `gives every chapter with its names and verses` — Every chapter reads whole: its number, its book's and portion's names, every verse numbered and holding text, Genesis opening the Torah. <sub>page logic, on the real artifacts</sub>
          - `cuts the other reading out of the verse as its own parts` — The other edition's forms are cut out of the verse as their own parts — each carrying its index, never ours — and the parts put back together are the verse as shown, with nothing lost at the cuts. <sub>page logic, on the real artifacts</sub>
          - `cuts our marked form out alone, before theirs` — Our form is cut out alone, just before theirs: one word carrying `¹`, no space in it — what the pointer must be on for the index to open — and every `¹` of a verse is in such a part, never left in our text. The two forms of a pair share their reading's number, and only they. <sub>page logic, on the real artifacts</sub>
        - module `top level` · vitest
          - `reads nothing of a chapter not landed, then reads it` — Before its verses land, a chapter has no record and the bridge reads nothing; once its book is in, it reads. <sub>page logic, on the real artifacts</sub>
        - module `estimate` · vitest
          - `follows the text, the size and the column` — The height a chapter not laid out is given grows with its text and its size, shrinks with a wider column, and never falls under one line a verse. <sub>page logic, on the real artifacts</sub>
        - module `zoomed` · vitest
          - `steps the size within its bounds` — One step is the browser's own, fractions come from a trackpad, and the size stays within its bounds whatever the gesture. <sub>page logic, on the real artifacts</sub>
        - module `missing` · vitest
          - `groups the chapters not in into runs` — Chapters not in come as runs of neighbours, one request each; those in break the runs. <sub>page logic, on the real artifacts</sub>
        - module `kept` · vitest
          - `reaches around the screen within the Tanakh` — The chapters laid out run a few each way past those on screen, never past the Tanakh's ends, in either order of the two. <sub>page logic, on the real artifacts</sub>
        - module `the contents` · vitest
          - `come whole from the module before any text` — Read from a module no text has landed in: the three portions, the 39 books each under its portion, their chapters tiling the 929 in order, every chapter numbered — all there before any verse. <sub>page logic, on the real artifacts</sub>
        - module `the breadcrumb` · vitest
          - `names the book and its portion below the books, and leads each step to its level` — The Tanakh leads to the portions alone, a portion to the books, a book to its chapters; the text and a book's chapters name the book and its portion, the two levels showing the whole Tanakh nothing under it. <sub>page logic, on the real artifacts</sub>
        - module `the hand-over between the views` · vitest
          - `lands each level on its own` — The map's depth opens the book on its level — its chapters' numbers on the books, as its framing does — and the book hands each level back to its depth: the text to its passage (3), a book's chapters to the map's chapters (2), the books to the framing (1), the portions alone to the overview (0). <sub>page logic, on the real artifacts</sub>
      - [`fault.test.ts`](web/src/lib/fault.test.ts) — 12 tests — covers [`web/src/lib/fault.ts`](web/src/lib/fault.ts)
        - module `a fault` · vitest
          - `is numbered so it reads aloud and never repeats` — is numbered so it reads aloud and never repeats. <sub>page logic, on the real artifacts</sub>
          - `is raised at its caller, with the words of its code and its number` — is raised at its caller, with the words of its code and its number. <sub>page logic, on the real artifacts</sub>
          - `keeps its number through every layer, each adding its step` — keeps its number through every layer, each adding its step. <sub>page logic, on the real artifacts</sub>
          - `is made of anything thrown, its own message and place as origin` — is made of anything thrown, its own message and place as origin. <sub>page logic, on the real artifacts</sub>
          - `crosses from a worker unchanged` — crosses from a worker unchanged. <sub>page logic, on the real artifacts</sub>
          - `is sent as the service reads it, bounded as it bounds it` — is sent as the service reads it, bounded as it bounds it. <sub>page logic, on the real artifacts</sub>
        - module `a WebAssembly trap` · vitest
          - `is traced to the Rust place its panic left` — is traced to the Rust place its panic left. <sub>page logic, on the real artifacts</sub>
        - module `the reporter` · vitest
          - `sends each incident once, a bounded number a minute, and never throws` — sends each incident once, a bounded number a minute, and never throws. <sub>page logic, on the real artifacts</sub>
        - module `what nothing caught` · vitest
          - `becomes an uncaught fault, reported and shown` — becomes an uncaught fault, reported and shown. <sub>page logic, on the real artifacts</sub>
          - `lets the ResizeObserver loop notice pass, and nothing else` — The browser's notice that a ResizeObserver's deliveries move to the next frame is no error — it once reached the reader as "Something unexpected went wrong" (ETH-1VZG5BNF) — while an error that merely says so is one. <sub>page logic, on the real artifacts</sub>
        - module `the catalogue` · vitest
          - `speaks English to the user, plainly` — speaks English to the user, plainly. <sub>page logic, on the real artifacts</sub>
          - `has each code raised at one place, which it therefore names` — has each code raised at one place, which it therefore names. <sub>page logic, on the real artifacts</sub>
      - [`keys.test.ts`](web/src/lib/keys.test.ts) — 9 tests — covers [`web/src/lib/keys.ts`](web/src/lib/keys.ts)
        - module `command` · vitest
          - `zooms one notch in on + or =, out on - or _` — zooms one notch in on + or =, out on - or _. <sub>page logic, on the real artifacts</sub>
          - `moves the view the way the arrow points, by a small share, a large one with Shift` — moves the view the way the arrow points, by a small share, a large one with Shift. <sub>page logic, on the real artifacts</sub>
          - `selects the passage at the centre on Enter` — selects the passage at the centre on Enter. <sub>page logic, on the real artifacts</sub>
          - `leaves every key held with Ctrl, Cmd or Alt to the browser, Ctrl + included` — leaves every key held with Ctrl, Cmd or Alt to the browser, Ctrl + included. <sub>page logic, on the real artifacts</sub>
          - `gives nothing for a key it does not know, Escape and Tab included` — gives nothing for a key it does not know, Escape and Tab included. <sub>page logic, on the real artifacts</sub>
        - module `escape` · vitest
          - `closes an open passage before anything else, the view kept` — closes an open passage before anything else, the view kept. <sub>page logic, on the real artifacts</sub>
          - `closes a notice next` — closes a notice next. <sub>page logic, on the real artifacts</sub>
          - `goes back to the overview only with nothing open` — goes back to the overview only with nothing open. <sub>page logic, on the real artifacts</sub>
          - `leaves the book view where it is with nothing open` — The book view has no overview: its place is the scroll, never thrown away. <sub>page logic, on the real artifacts</sub>
      - [`logs-view.test.ts`](web/src/lib/logs-view.test.ts) — 13 tests — covers [`web/src/lib/logs-view.ts`](web/src/lib/logs-view.ts)
        - module `takeToken` · vitest
          - `takes the token out of the address and keeps the rest` — The token leaves the address, the rest of it stays: the history and a bookmark keep `/logs/` alone. <sub>page logic, on the real artifacts</sub>
          - `replaces nothing without a token, and an empty one is none` — No token, nothing to replace; an empty one is taken out but is no token. <sub>page logic, on the real artifacts</sub>
        - module `parseEvents` · vitest
          - `gives whole events across any cut, and keeps the rest` — Events cut anywhere by the network come out whole, once, in order. <sub>page logic, on the real artifacts</sub>
          - `joins the data lines of one event` — Several data lines are one event, joined by a newline, as the format says. <sub>page logic, on the real artifacts</sub>
        - module `show` · vitest
          - `reads a log line for the eye` — The time, level and message apart, every other field after them. <sub>page logic, on the real artifacts</sub>
          - `shows a line that is not JSON as it is` — A line that is not JSON is shown as it is, never thrown on. <sub>page logic, on the real artifacts</sub>
        - module `rows` · vitest
          - `numbers the lines once, and the survivors keep their numbers` — Each line has its own number, and keeping the newest leaves every survivor under the number it had: the page moves nothing it keeps. <sub>page logic, on the real artifacts</sub>
        - module `keep` · vitest
          - `keeps the newest lines up to the bound, and a fresh opening replaces` — The page holds `MAX_LINES` at most, the newest; a fresh opening replaces. <sub>page logic, on the real artifacts</sub>
        - module `follow` · vitest
          - `sends the token in the header and hands the lines on` — The token goes in the header, never the URL; the history and level are asked; the lines arrive, the first batch replacing what was shown. <sub>page logic, on the real artifacts</sub>
          - `opens again after an end or a failure, the history afresh` — A stream that ends or a network failure is opened again after the pause, each opening replaying the history afresh. <sub>page logic, on the real artifacts</sub>
          - `clears what was shown even when the history is empty` — A level with no line yet clears the page all the same: the lines of the level before must not stay as if they were of this one. <sub>page logic, on the real artifacts</sub>
          - `stops on a refusal and says why` — A refusal says the service's code, message and incident, and is not asked again: a wrong token stays wrong. <sub>page logic, on the real artifacts</sub>
          - `says a page served without the service, and retries a bare 503` — Served without the service (`npm run dev`), the answer is not JSON: refused, its status said. A 503 without a code is a proxy: retried. <sub>page logic, on the real artifacts</sub>
      - [`passage.test.ts`](web/src/lib/passage.test.ts) — 3 tests — covers [`web/src/lib/passage.ts`](web/src/lib/passage.ts)
        - module `passage` · vitest
          - `cuts a verse at its spaces, every pointed letter whole` — cuts a verse at its spaces, every pointed letter whole. <sub>page logic, on the real artifacts</sub>
          - `picks each word in place: head, word and tail give the verse back` — picks each word in place: head, word and tail give the verse back. <sub>page logic, on the real artifacts</sub>
          - `picks nothing out of the verse, and reads no word picked as none` — picks nothing out of the verse, and reads no word picked as none. <sub>page logic, on the real artifacts</sub>
      - [`search-core.test.ts`](web/src/lib/search-core.test.ts) — 3 tests — covers [`web/src/lib/search-core.ts`](web/src/lib/search-core.ts)
        - module `the books file` · vitest
          - `holds the 39 books then the readings, in order` — holds the 39 books then the readings, in order. <sub>page logic, on the real artifacts</sub>
          - `opens, block after block, to the plain text the tiles read` — opens, block after block, to the plain text the tiles read. <sub>page logic, on the real artifacts</sub>
        - module `SearchCore` · vitest
          - `answers -1 until the whole text is in, then finds` — answers -1 until the whole text is in, then finds. <sub>page logic, on the real artifacts</sub>
      - [`search-worker.test.ts`](web/src/lib/search-worker.test.ts) — 2 tests — covers [`web/src/lib/search-worker.ts`](web/src/lib/search-worker.ts)
        - module `the search worker` · vitest
          - `holds a search until the text is in, then answers it by its id` — holds a search until the text is in, then answers it by its id. <sub>page logic, on the real artifacts</sub>
          - `answers a failed search by its id, and searches on with a new module` — answers a failed search by its id, and searches on with a new module. <sub>page logic, on the real artifacts</sub>
      - [`text-open.test.ts`](web/src/lib/text-open.test.ts) — 9 tests — covers [`web/src/lib/text-open.ts`](web/src/lib/text-open.ts)
        - module `inflate` · vitest
          - `opens what gzip closed, byte for byte` — opens what gzip closed, byte for byte. <sub>page logic, on the real artifacts</sub>
          - `reads a view into a larger buffer, not the whole buffer` — reads a view into a larger buffer, not the whole buffer. <sub>page logic, on the real artifacts</sub>
        - module `range` · vitest
          - `asks for the bytes from..to, end excluded, at the given priority` — asks for the bytes from..to, end excluded, at the given priority. <sub>page logic, on the real artifacts</sub>
          - `reads a whole-file answer only up to what it needs` — reads a whole-file answer only up to what it needs. <sub>page logic, on the real artifacts</sub>
          - `refuses a failed answer` — refuses a failed answer. <sub>page logic, on the real artifacts</sub>
        - module `lanes` · vitest
          - `takes each item once, never more at once than the lanes` — takes each item once, never more at once than the lanes. <sub>page logic, on the real artifacts</sub>
          - `chooses each item when a lane frees, so a later choice sees the view move` — chooses each item when a lane frees, so a later choice sees the view move. <sub>page logic, on the real artifacts</sub>
          - `ends at once when there is nothing to take` — ends at once when there is nothing to take. <sub>page logic, on the real artifacts</sub>
        - module `fragmentKey` · vitest
          - `keys every fragment apart, long verses included` — Two fragments never share a key: the cache once keyed on `offset * 1024 + length`, and a verse of 1100 bytes collided with the 76 bytes one further on, so the page could show another's text. <sub>page logic, on the real artifacts</sub>

## Functions no test names

Each function of the sources whose name appears in no test file: tested only
through others, if at all. Not all need a test of their own — a private
helper is often held by the test of its caller — but each is a question the
`tests` skill answers: held elsewhere, or a gap.

- [`build.rs`](build.rs): `weld_variants`, `write_output`
- [`build/mamre.rs`](build/mamre.rs): `label_only`, `renumber`
- [`server/src/api.rs`](server/src/api.rs): `not_allowed`, `not_allowed_post`
- [`server/src/api/els.rs`](server/src/api/els.rs): `osis_pattern`, `place_pattern`
- [`server/src/api/guard.rs`](server/src/api/guard.rs): `charge`
- [`server/src/api/report.rs`](server/src/api/report.rs): `code_of`, `receive`
- [`server/src/fault.rs`](server/src/fault.rs): `fmt`, `into_response`, `respond`, `retry_after`, `set_verbose`
- [`server/src/lib.rs`](server/src/lib.rs): `open_browser`, `trace`
- [`server/src/logs.rs`](server/src/logs.rs): `init_with`, `install`, `level_of`, `on_event`, `on_new_span`, `prune`, `read_range`, `record_bool`, `record_debug`, `record_f64`, `record_i64`, `record_str`, `record_u64`, `test_dir`
- [`src/els.rs`](src/els.rs): `forget_answers`, `index_of`
- [`src/els/index.rs`](src/els/index.rs): `make_room`
- [`src/map/camera.rs`](src/map/camera.rs): `at_opening`, `overview_scale`, `sync_detail`
- [`src/map/frame.rs`](src/map/frame.rs): `book_title_size`, `draw_book`, `emit_ink`, `emit_run`, `emit_text`, `overview_title`, `scale_of`
- [`src/map/hit.rs`](src/map/hit.rs): `books_drawn`, `ink_of`, `locate_in`, `names_chapter`
- [`src/map/landing.rs`](src/map/landing.rs): `line_height_once_in`, `push_tile`, `verses_in`
- [`src/map/text.rs`](src/map/text.rs): `line_at`
- [`src/pack.rs`](src/pack.rs): `get_str`, `get_string`, `lay_chapter`, `lay_readings`, `put_str`, `read_exceptions`, `unpack`, `write_exceptions`
- [`src/shared/els.rs`](src/shared/els.rs): `check_up_to`, `last_verse`
- [`src/shared/reference.rs`](src/shared/reference.rs): `book_english`, `book_hebrew`
- [`src/variant.rs`](src/variant.rs): `seek`
- [`src/wasm/chapter.rs`](src/wasm/chapter.rs): `write_contents`
- [`src/wasm/els.rs`](src/wasm/els.rs): `els_cap`, `els_prepare`, `probe_els_forget`, `probe_els_method`
- [`src/wasm/map.rs`](src/wasm/map.rs): `probe_verse_drawn`, `readings_end`, `tiles_ptr`, `tiles_wanted`, `variants_drawn`
- [`web/src/lib/ethaor.ts`](web/src/lib/ethaor.ts): `createTanakhMap`
- [`web/src/lib/fault.ts`](web/src/lib/fault.ts): `fromTrap`
