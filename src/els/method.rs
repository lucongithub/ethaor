//! Three ways to find a pattern over a range of steps.
//!
//! All three walk the **anchors** — the occurrences of the pattern's rarest
//! letter — and differ in how they find the steps that fit around each:
//!
//! - `Direct` tries every step and reads the other letters, rarest first.
//!   Unbeatable on a handful of steps.
//! - `Pairs` lists the occurrences of the second rarest letter that fall on a
//!   step — the ones with the anchor's residue, within reach — and checks the
//!   rest on those. Work follows that letter's rarity, not the range.
//! - `Planes` reads, for the other letters, bit planes decimated by their gap
//!   in the pattern: from an anchor, a letter's bits for consecutive steps are
//!   consecutive there, so one AND of words tests 64 steps at once.
//!
//! A step here is the distance between two letters of the pattern: `skip + 1`.

use super::index::{Index, Plane};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Direct,
    Pairs,
    Planes,
}

pub const METHODS: [Method; 3] = [Method::Direct, Method::Pairs, Method::Planes];

/// Letters `Planes` ANDs together at most; a longer pattern checks the others
/// in the text on the few steps that pass. Three leave so few that a fourth
/// plane costs more than it saves; one lets too many through.
const LANES: usize = 3;

/// A pattern of at least two letters and its steps, `s0..=s1`, already cut
/// to the stream: `1 <= s0 <= s1` and `(len - 1) * s1 < n`.
pub struct Query<'p> {
    pub pattern: &'p [u8],
    pub s0: usize,
    pub s1: usize,
}

/// The anchor — rank of the rarest letter in the pattern — and the others as
/// (signed gap from the anchor, code), rarest first.
struct Shape {
    anchor: usize,
    others: Vec<(isize, u8)>,
}

fn shape(index: &Index, pattern: &[u8]) -> Shape {
    let anchor = (0..pattern.len()).min_by_key(|&k| index.freq[pattern[k] as usize]).unwrap();
    let mut others: Vec<(isize, u8)> = (0..pattern.len())
        .filter(|&k| k != anchor)
        .map(|k| (k as isize - anchor as isize, pattern[k]))
        .collect();
    others.sort_by_key(|&(_, c)| index.freq[c as usize]);
    Shape { anchor, others }
}

/// The last step that keeps the whole pattern inside the stream, per anchor.
///
/// Away from both ends of the stream every step of the range fits, which two
/// comparisons establish; only the anchors near an end pay the divisions.
struct Reach {
    anchor: usize,
    after: usize,
    n: usize,
    s0: usize,
    s1: usize,
    /// Anchors in `inner.0..=inner.1` reach `s1`.
    inner: (usize, usize),
}

impl Reach {
    fn new(q: &Query, anchor: usize, n: usize) -> Reach {
        let after = q.pattern.len() - 1 - anchor;
        // `(len - 1) * s1 < n`, so neither product overflows nor exceeds n.
        let inner = (anchor * q.s1, n - 1 - after * q.s1);
        Reach { anchor, after, n, s0: q.s0, s1: q.s1, inner }
    }

    /// `None` if not even `s0` fits.
    #[inline(always)]
    fn at(&self, i: usize) -> Option<usize> {
        if i >= self.inner.0 && i <= self.inner.1 {
            return Some(self.s1);
        }
        let mut hi = self.s1;
        if let Some(before) = i.checked_div(self.anchor) {
            hi = hi.min(before);
        }
        if let Some(after) = (self.n - 1 - i).checked_div(self.after) {
            hi = hi.min(after);
        }
        (hi >= self.s0).then_some(hi)
    }
}

/// Where a method sends what it finds.
pub trait Sink {
    /// One occurrence: rank of its first letter, and its step.
    fn hit(&mut self, start: usize, step: usize);
    /// The largest step still worth listing. Past it a method may only count:
    /// the occurrences the sink keeps are already all below.
    fn limit(&self) -> usize;
}

/// Sends every occurrence to `sink`, in no particular order — or only counts
/// those past its limit — and returns how many there are in all.
pub fn run(index: &mut Index, q: &Query, method: Method, sink: &mut impl Sink) -> u64 {
    let shape = shape(index, q.pattern);
    match method {
        Method::Direct => direct(index, q, &shape, sink),
        Method::Pairs => pairs(index, q, &shape, sink),
        Method::Planes => match shape.others.len().min(LANES) {
            1 => planes::<1>(index, q, &shape, sink),
            2 => planes::<2>(index, q, &shape, sink),
            _ => planes::<3>(index, q, &shape, sink),
        },
    }
}

/// Counts the occurrences of a two-letter pattern without listing them: for
/// each anchor, the other letter's occurrences within reach — a span of the
/// stream, counted by two ranks on its plain plane. Constant work per anchor,
/// whatever the range.
pub fn count_pairs(index: &mut Index, q: &Query) -> u64 {
    debug_assert_eq!(q.pattern.len(), 2);
    let shape = shape(index, q.pattern);
    let (g, c) = shape.others[0];
    index.make_room(1);
    let k = index.plane(c, 1, false);
    let index = &*index;
    let plane = index.plane_at(k);
    let reach = Reach::new(q, shape.anchor, index.text.len());
    let mut total = 0u64;
    for &i in index.positions(q.pattern[shape.anchor]) {
        let i = i as usize;
        let Some(last) = reach.at(i) else { continue };
        // The other letter sits at `i + g·s`, `g = ±1`, for s in s0..=last.
        let (from, to) = if g > 0 { (i + q.s0, i + last) } else { (i - last, i - q.s0) };
        total += (plane.rank(to + 1) - plane.rank(from)) as u64;
    }
    total
}

/// Whether every (gap, code) of `letters` reads its code at step `s` from the
/// anchor `i`.
///
/// # Safety
/// `i + gap * s` lies inside `text` for every gap — what `Reach` ensures.
#[inline(always)]
unsafe fn matches(text: &[u8], i: usize, s: usize, letters: &[(isize, u8)]) -> bool {
    letters.iter().all(|&(g, c)| {
        let p = (i as isize + g * s as isize) as usize;
        debug_assert!(p < text.len(), "lecture hors du flux");
        unsafe { *text.get_unchecked(p) == c }
    })
}

fn direct(index: &Index, q: &Query, shape: &Shape, sink: &mut impl Sink) -> u64 {
    let (text, a) = (index.text, shape.anchor);
    let reach = Reach::new(q, a, text.len());
    // The rarest other letter is read inline, walking by its gap: most steps
    // stop there. The rest only on the steps it lets through.
    let (g1, c1) = shape.others[0];
    let rest = &shape.others[1..];
    let mut total = 0;
    for &i in index.positions(q.pattern[a]) {
        let i = i as usize;
        let Some(hi) = reach.at(i) else { continue };
        let mut p = (i as isize + g1 * q.s0 as isize) as usize;
        for s in q.s0..=hi {
            // SAFETY: `Reach` keeps every letter of the pattern inside the
            // stream for every step up to `hi`, this one and the rest alike.
            debug_assert!(p < text.len(), "lecture hors du flux");
            if unsafe { *text.get_unchecked(p) } == c1 && unsafe { matches(text, i, s, rest) } {
                total += 1;
                sink.hit(i - a * s, s);
            }
            p = p.wrapping_add_signed(g1);
        }
    }
    total
}

fn pairs(index: &mut Index, q: &Query, shape: &Shape, sink: &mut impl Sink) -> u64 {
    let a = shape.anchor;
    let (gb, cb) = shape.others[0];
    let rest = &shape.others[1..];
    let g = gb.unsigned_abs();
    index.make_room(0);
    let lists = (g > 1).then(|| index.residues(cb, g));
    let index = &*index;
    let text = index.text;
    let reach = Reach::new(q, a, text.len());
    // Where the last anchor of each residue started reading its list. The
    // window never moves back within a residue: forward, it starts at
    // `i + g·s0`; backward, at `i - g·hi`, and between two anchors of one
    // residue `i` grows by a multiple of `g` while `g·hi` grows by no more —
    // `hi` is capped by `i / a`, and `g <= a` — or shrinks near the end. So
    // the cursor only walks on: one pass over each list.
    let mut cursor = vec![0usize; g];
    let mut total = 0;

    for &i in index.positions(q.pattern[a]) {
        let i = i as usize;
        let Some(hi) = reach.at(i) else { continue };
        let r = i % g;
        let list = match lists {
            Some(k) => index.residue_list(k, r),
            None => index.positions(cb),
        };
        let (lo_j, hi_j) = if gb > 0 { (i + g * q.s0, i + g * hi) } else { (i - g * hi, i - g * q.s0) };
        let c = &mut cursor[r];
        debug_assert!(*c == 0 || (list[*c - 1] as usize) < lo_j, "la fenetre de Pairs a recule");
        while *c < list.len() && (list[*c] as usize) < lo_j {
            *c += 1;
        }
        for &j in &list[*c..] {
            let j = j as usize;
            if j > hi_j {
                break;
            }
            let s = j.abs_diff(i) / g;
            // SAFETY: `s <= hi`, which `Reach` keeps inside the stream.
            if unsafe { matches(text, i, s, rest) } {
                total += 1;
                sink.hit(i - a * s, s);
            }
        }
    }
    total
}

/// `L` letters are ANDed, one plane each; any further letters are read in the
/// text on the steps that survive. A word of steps is counted with one
/// popcount and listed only while the sink still wants it — which makes a
/// flood of hits countable.
fn planes<const L: usize>(index: &mut Index, q: &Query, shape: &Shape, sink: &mut impl Sink) -> u64 {
    let a = shape.anchor;
    let (lanes, rest) = shape.others.split_at(L);
    index.make_room(L);
    let ks: [usize; L] = core::array::from_fn(|k| {
        let (g, c) = lanes[k];
        index.plane(c, g.unsigned_abs(), g < 0)
    });
    let index = &*index;
    let n = index.text.len();
    let text = index.text;
    let planes: [&Plane; L] = core::array::from_fn(|k| index.plane_at(ks[k]));
    let bits: [&[u64]; L] = core::array::from_fn(|k| &planes[k].bits[..]);
    let reach = Reach::new(q, a, n);
    let mut total = 0u64;

    for &i in index.positions(q.pattern[a]) {
        let i = i as usize;
        let Some(hi) = reach.at(i) else { continue };
        let mut at: [usize; L] = core::array::from_fn(|k| planes[k].origin(i, n) + q.s0);
        let count = hi - q.s0 + 1;
        let mut base = q.s0;
        let mut left = count;
        // Listing: word by word while the sink still wants these steps.
        while left > 0 && (base <= sink.limit() || !rest.is_empty()) {
            let mut acc = if left >= 64 { !0 } else { (1u64 << left) - 1 };
            for k in 0..L {
                // SAFETY: every offset read is the bit of a step in
                // `s0..=hi`, which `Reach` keeps inside the stream, so
                // `at < n` and `at + 63 < n + 64`: the two words read are
                // within the plane's `n / 64 + 3`.
                acc &= unsafe { window(bits[k], at[k]) };
            }
            if acc != 0 {
                if rest.is_empty() {
                    total += acc.count_ones() as u64;
                    while acc != 0 {
                        let s = base + acc.trailing_zeros() as usize;
                        sink.hit(i - a * s, s);
                        acc &= acc - 1;
                    }
                } else {
                    while acc != 0 {
                        let s = base + acc.trailing_zeros() as usize;
                        // SAFETY: `s <= hi`, which `Reach` keeps inside.
                        if unsafe { matches(text, i, s, rest) } {
                            total += 1;
                            sink.hit(i - a * s, s);
                        }
                        acc &= acc - 1;
                    }
                }
            }
            for x in at.iter_mut() {
                *x += 64;
            }
            base += 64;
            left -= left.min(64);
        }
        // Counting: the sink wants no more of this anchor's steps, so the
        // rest is ANDs and popcounts with no branch — a loop the compiler
        // can unroll and vectorise.
        if left > 0 {
            let full = left / 64;
            let mut sum = 0u64;
            for w in 0..full {
                let mut acc = !0u64;
                for k in 0..L {
                    // SAFETY: as above — still steps of `s0..=hi`.
                    acc &= unsafe { window(bits[k], at[k] + 64 * w) };
                }
                sum += acc.count_ones() as u64;
            }
            let tail = left % 64;
            if tail > 0 {
                let mut acc = (1u64 << tail) - 1;
                for k in 0..L {
                    // SAFETY: as above.
                    acc &= unsafe { window(bits[k], at[k] + 64 * full) };
                }
                sum += acc.count_ones() as u64;
            }
            total += sum;
        }
    }
    total
}

/// 64 bits of a plane starting at any bit offset.
///
/// # Safety
/// `(at >> 6) + 1 < bits.len()`.
#[inline(always)]
pub unsafe fn window(bits: &[u64], at: usize) -> u64 {
    let k = at >> 6;
    let sh = (at & 63) as u32;
    debug_assert!(k + 1 < bits.len(), "lecture hors du plan");
    // `<< 1 << (63 - sh)` rather than `<< (64 - sh)`: a shift by 64 is not
    // zero, it is an overflow — this way sh = 0 needs no branch.
    unsafe { (bits.get_unchecked(k) >> sh) | ((bits.get_unchecked(k + 1) << 1) << (63 - sh)) }
}
