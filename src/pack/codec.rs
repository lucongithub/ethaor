//! Context-mixing codec for the pointed Hebrew Bible symbol stream: the
//! verses' part of the text, the bulk of what the page downloads.
//!
//! Chosen on a measured bench against brotli, zstd, LZMA and gzip (see
//! CLAUDE.md, *Transport*): a fifth smaller than the best of them, for a
//! decode the page runs in a worker, off the frame loop.
//!
//! Input: one byte per character, symbols 0..=43 (0 space, 1..=15 marks,
//! 16..=42 consonants, 43 verse separator). Output: a few hundred bytes of
//! header followed by one arithmetic-coded stream.
//!
//! Model, per symbol:
//! - the symbol is coded as a path in a Huffman tree chosen by the previous
//!   symbol; a tree only holds the symbols ever seen after that symbol, and a
//!   context with a single successor costs nothing;
//! - each binary decision of the path is predicted by four context models
//!   (order 2 direct, order 4, current word prefix, order 6; one cache line of
//!   counters per context, hashed slots are two-way sets with a check tag)
//!   and a match model
//!   (longest earlier occurrence of the last 8 symbols);
//! - the predictions are mixed in the logistic domain by a gated linear mixer
//!   whose weight set is chosen by (match state, previous symbol, tree node).
//!
//! Everything is integer arithmetic, so the encoder (native, build time) and
//! the decoder (wasm32, run time) agree bit for bit. No dependency, no I/O,
//! no allocation inside the symbol loop.
#![allow(clippy::needless_range_loop)]

/// Alphabet size: symbols are 0..NSYM.
pub const NSYM: usize = 44;
/// Previous-symbol contexts: symbol + 1, and 0 at stream start.
const NCTX: usize = NSYM + 1;
/// The word separator, and the verse separator: the model's word and
/// position contexts read them.
pub const SPACE: u8 = 0;
pub const SEP: u8 = 43;

// ---------------------------------------------------------------- logistic tables

/// squash(d) = 4096 / (1 + e^(-d/256)), interpolated from 33 integer points.
fn squash(d: i32) -> i32 {
    const T: [i32; 33] = [
        1, 2, 3, 6, 10, 16, 27, 45, 73, 120, 194, 310, 488, 747, 1101, 1546, 2047, 2549, 2994,
        3348, 3607, 3785, 3901, 3975, 4024, 4050, 4068, 4079, 4085, 4089, 4092, 4093, 4094,
    ];
    if d > 2047 {
        return 4095;
    }
    if d < -2047 {
        return 1;
    }
    let w = d & 127;
    let i = ((d >> 7) + 16) as usize;
    (T[i] * (128 - w) + T[i + 1] * w + 64) >> 7
}

// ---------------------------------------------------------------- binary arithmetic coder

trait Coder {
    /// Codes one decision with P(1) = p / 4096. The encoder codes `bit` and
    /// returns it; the decoder ignores `bit` and returns the decoded one.
    fn bit(&mut self, p: u32, bit: u32) -> u32;
}

struct Encoder {
    x1: u32,
    x2: u32,
    out: Vec<u8>,
}

impl Coder for Encoder {
    #[inline(always)]
    fn bit(&mut self, p: u32, bit: u32) -> u32 {
        let r = self.x2 - self.x1;
        let xmid = self.x1 + (r >> 12) * p + (((r & 0xfff) * p) >> 12);
        if bit != 0 {
            self.x2 = xmid
        } else {
            self.x1 = xmid + 1
        }
        while (self.x1 ^ self.x2) & 0xff00_0000 == 0 {
            self.out.push((self.x2 >> 24) as u8);
            self.x1 <<= 8;
            self.x2 = (self.x2 << 8) | 255;
        }
        bit
    }
}

/// The arithmetic decoder. It owns its input, which may still be growing:
/// `Stream` appends to it as bytes arrive.
struct Decoder {
    x1: u32,
    x2: u32,
    x: u32,
    src: Vec<u8>,
    pos: usize,
}

impl Decoder {
    fn new(src: Vec<u8>, pos: usize) -> Self {
        let mut d = Decoder { x1: 0, x2: u32::MAX, x: 0, src, pos };
        for _ in 0..4 {
            d.x = (d.x << 8) | d.byte();
        }
        d
    }

    #[inline(always)]
    fn byte(&mut self) -> u32 {
        let b = if self.pos < self.src.len() { self.src[self.pos] } else { 0 };
        self.pos += 1;
        b as u32
    }
}

impl Coder for Decoder {
    #[inline(always)]
    fn bit(&mut self, p: u32, _bit: u32) -> u32 {
        let r = self.x2 - self.x1;
        let xmid = self.x1 + (r >> 12) * p + (((r & 0xfff) * p) >> 12);
        let bit = (self.x <= xmid) as u32;
        // Branchless interval update: the decoded bit is unpredictable.
        let m = 0u32.wrapping_sub(bit);
        self.x2 = (xmid & m) | (self.x2 & !m);
        self.x1 = ((xmid + 1) & !m) | (self.x1 & m);
        while (self.x1 ^ self.x2) & 0xff00_0000 == 0 {
            self.x1 <<= 8;
            self.x2 = (self.x2 << 8) | 255;
            self.x = (self.x << 8) | self.byte();
        }
        bit
    }
}

// ---------------------------------------------------------------- Huffman trees

/// Huffman code lengths of one context (0 = absent, or the only symbol).
fn code_lengths(freq: &[u64; NSYM]) -> [u8; NSYM] {
    let mut nodes: Vec<(u64, Vec<usize>)> =
        (0..NSYM).filter(|&s| freq[s] > 0).map(|s| (freq[s], vec![s])).collect();
    let mut len = [0u8; NSYM];
    while nodes.len() > 1 {
        nodes.sort_by(|a, b| b.0.cmp(&a.0).then(b.1[0].cmp(&a.1[0])));
        let a = nodes.pop().unwrap();
        let b = nodes.pop().unwrap();
        for &s in a.1.iter().chain(b.1.iter()) {
            len[s] += 1;
        }
        let mut m = a.1;
        m.extend(b.1);
        m.sort();
        nodes.push((a.0 + b.0, m));
    }
    len
}

/// One canonical Huffman tree per previous-symbol context.
/// `child[t * NSYM + node]` holds both children; a value >= 64 is a leaf
/// (64 + symbol). Internal nodes are numbered breadth first, so the busy
/// shallow nodes sit together at the start of every counter slot.
struct Trees {
    child: Vec<[u8; 2]>,
    code: Vec<u32>,
    len: Vec<u8>,
    /// 255 = a real tree; otherwise the only symbol that context ever saw.
    single: [u8; NCTX],
}

impl Trees {
    fn build(lens: &[[u8; NSYM]; NCTX], present: &[[bool; NSYM]; NCTX]) -> Trees {
        let mut t = Trees {
            child: vec![[255u8; 2]; NCTX * NSYM],
            code: vec![0; NCTX * NSYM],
            len: vec![0; NCTX * NSYM],
            single: [255; NCTX],
        };
        for c in 0..NCTX {
            let len = &lens[c];
            let mut count = 0;
            for s in 0..NSYM {
                if present[c][s] {
                    count += 1;
                    t.single[c] = s as u8;
                }
            }
            if count != 1 {
                t.single[c] = 255;
            }
            if count < 2 {
                continue;
            }
            // Canonical codes: by length, then by symbol.
            let mut code = 0u32;
            for l in 1..15u8 {
                for s in 0..NSYM {
                    if present[c][s] && len[s] == l {
                        t.code[c * NSYM + s] = code;
                        t.len[c * NSYM + s] = l;
                        code += 1;
                    }
                }
                code <<= 1;
            }
            // Internal node = (depth, code prefix), numbered breadth first:
            // ids grow with depth, then with the prefix.
            let mut ids = [(0u8, 0u32); NSYM]; // (depth, prefix) per internal node
            let mut nids = 0usize;
            for d in 0..14u8 {
                let first = nids;
                for s in 0..NSYM {
                    let l = t.len[c * NSYM + s];
                    if present[c][s] && l > d {
                        let pfx = t.code[c * NSYM + s] >> (l - d);
                        if !ids[first..nids].iter().any(|&(_, q)| q == pfx) {
                            // insertion keeps this depth sorted by prefix
                            let mut k = nids;
                            while k > first && ids[k - 1].1 > pfx {
                                ids[k] = ids[k - 1];
                                k -= 1;
                            }
                            ids[k] = (d, pfx);
                            nids += 1;
                        }
                    }
                }
            }
            let id = |d: u8, pfx: u32| {
                ids[..nids].iter().position(|&e| e == (d, pfx)).unwrap() as u8
            };
            for s in 0..NSYM {
                if !present[c][s] {
                    continue;
                }
                let cs = t.code[c * NSYM + s];
                let l = t.len[c * NSYM + s];
                for d in 0..l {
                    let n = id(d, cs >> (l - d)) as usize;
                    let b = ((cs >> (l - 1 - d)) & 1) as usize;
                    t.child[c * NSYM + n][b] =
                        if d + 1 == l { 64 + s as u8 } else { id(d + 1, cs >> (l - d - 1)) };
                }
            }
        }
        t
    }
}

// ---------------------------------------------------------------- counters

/// Adaptive probability: 12-bit P(1) xor 2048 in the high bits (so a zeroed
/// table means 1/2) and a 4-bit hit count in the low bits; the adaptation
/// rate is 1/(n + 1.5) down to 1/(LIMIT + 1.5).
const LIMIT: u32 = 10;
const RATE: [i32; 16] = {
    let mut t = [0i32; 16];
    let mut i = 0;
    while i < 16 {
        let n = if i < LIMIT as usize { i } else { LIMIT as usize };
        t[i] = 65536 * 2 / (2 * n as i32 + 3);
        i += 1;
    }
    t
};

#[inline(always)]
fn ctr_p(v: u16) -> usize {
    ((v >> 4) ^ 2048) as usize
}

#[inline(always)]
fn ctr_upd(v: u16, bit: u32) -> u16 {
    let n = (v & 15) as u32;
    let p = ((v >> 4) ^ 2048) as i32;
    let p2 = p + ((((bit as i32) * 4095 - p) * RATE[n as usize]) >> 16);
    (((p2 as u16) ^ 2048) << 4) | (n + (n < LIMIT) as u32) as u16
}

// ---------------------------------------------------------------- model

/// Counters per context slot, one per tree node: a slot is one cache line.
/// Nodes past the slot share its last usable counter (0.04 % of decisions).
const SLOT: usize = 32;
#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct Line([u16; SLOT]);

/// Counter models, in order: order 2 (direct), order 4, word prefix, order 6.
const NM: usize = 4;
/// Mixer inputs: the counter models, the match model, a bias.
const NIN: usize = NM + 2;
const O4_BITS: u32 = 16;
const W_BITS: u32 = 16;
const O6_BITS: u32 = 18;
const MM_BITS: u32 = 20;
const MM_MIN: usize = 8;
const MM_MAX: usize = 63;
// Mixer learning rate: retuned on the final verse stream, where 6 and 7 tie
// for the smallest output.
const LR: i32 = 6;

const OFF_O2: usize = 0;
const OFF_O4: usize = (NCTX * NCTX + 1) & !1; // even, so slot pairs never straddle tables
const OFF_W: usize = OFF_O4 + (1 << O4_BITS);
const OFF_O6: usize = OFF_W + (1 << W_BITS);
const TOTAL: usize = OFF_O6 + (1 << O6_BITS);

/// Rolling hash base of the match model, and its MM_MIN-th power.
const MM_B: u32 = 0x2F0B_3A49;
const MM_BK: u32 = {
    let mut r = 1u32;
    let mut i = 0;
    while i < MM_MIN {
        r = r.wrapping_mul(MM_B);
        i += 1;
    }
    r
};

#[inline(always)]
fn hash(a: u32, b: u32) -> u32 {
    let h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77).rotate_left(13);
    h ^ (h >> 15)
}

/// A zero-filled vector; all-zero bits are a valid value of every `T` used here.
fn zeroed<T: Copy>(n: usize) -> Vec<T> {
    let mut v: Vec<T> = Vec::with_capacity(n);
    // SAFETY: capacity is n; integers and `Line` are valid when all-zero.
    unsafe {
        std::ptr::write_bytes(v.as_mut_ptr(), 0, n);
        v.set_len(n);
    }
    v
}

struct Model<'t> {
    trees: &'t Trees,
    stretch: [i16; 4096],
    squash: [i16; 4096],
    ctr: Vec<Line>,
    w: Vec<i32>,
    mm_tab: Vec<u32>,
    mm_sm: [u16; 128],
    /// Decoded (or encoded) symbols so far: the output of the decoder.
    hist: Vec<u8>,
    mm_ptr: usize,
    mm_len: usize,
    word: u32,
    /// Last six symbols + 1, six bits each, most recent in the low bits.
    h6: u64,
    /// Rolling hash of the last MM_MIN symbols.
    mh: u32,
}

impl<'t> Model<'t> {
    fn new(trees: &'t Trees, n: usize) -> Self {
        let mut m = Model {
            trees,
            stretch: [0; 4096],
            squash: [0; 4096],
            ctr: zeroed(TOTAL),
            w: vec![1 << 12; 3 * NCTX * SLOT * NIN],
            mm_tab: zeroed(1 << MM_BITS),
            mm_sm: [0; 128],
            hist: Vec::with_capacity(n),
            mm_ptr: 0,
            mm_len: 0,
            word: 0,
            h6: 0,
            mh: 0,
        };
        for i in 0..4096 {
            m.squash[i] = squash(i as i32 - 2048) as i16;
        }
        // stretch = inverse of squash
        let mut pi = 0usize;
        for x in -2047..=2047i32 {
            let v = squash(x) as usize;
            for j in pi..=v {
                m.stretch[j] = x as i16;
            }
            pi = v + 1;
        }
        for j in pi..4096 {
            m.stretch[j] = 2047;
        }
        m
    }

    /// Codes one symbol (encoder: `sym`; decoder: ignored) and returns it.
    /// Not inlined on purpose: V8 tiers a wasm function up between calls, so
    /// a per-symbol function reaches optimized code early in a single decode.
    #[inline(never)]
    fn symbol<C: Coder, const ENC: bool>(&mut self, c: &mut C, sym: u8) -> u8 {
        let h6 = self.h6;
        let c1 = (h6 & 63) as usize;
        let trees = self.trees;
        let single = trees.single[c1];
        let s: u8 = if single != 255 {
            single
        } else {
            let c2 = ((h6 >> 6) & 63) as usize;
            let o4 = (h6 & 0xFF_FFFF) as u32;
            let o6 = (h6 >> 24) as u32;
            let hs = [
                hash(o4, 0x51ED_27A1),
                hash(self.word, c1 as u32 + 0x9E37),
                hash(o4, o6 ^ 0x2F6B_59C3),
            ];
            let mut base = [
                OFF_O2 + c2 * NCTX + c1,
                OFF_O4 + (hs[0] as usize & ((1 << O4_BITS) - 1)),
                OFF_W + (hs[1] as usize & ((1 << W_BITS) - 1)),
                OFF_O6 + (hs[2] as usize & ((1 << O6_BITS) - 1)),
            ];
            // The last counter of a hashed slot holds a check tag. Two-way
            // set: the slot and its neighbour; on a miss the one whose root
            // counter has seen fewer symbols is reset and taken.
            for k in 1..NM {
                let tag = (hs[k - 1] >> 16) as u16 | 1;
                let b0 = base[k];
                let b1 = b0 ^ 1;
                if self.ctr[b0].0[SLOT - 1] == tag {
                    continue;
                }
                if self.ctr[b1].0[SLOT - 1] == tag {
                    base[k] = b1;
                    continue;
                }
                let v = if (self.ctr[b1].0[0] & 15) < (self.ctr[b0].0[0] & 15) { b1 } else { b0 };
                let line = &mut self.ctr[v];
                line.0 = [0; SLOT];
                line.0[SLOT - 1] = tag;
                base[k] = v;
            }
            let tbase = c1 * NSYM;
            // Match model: the predicted symbol's code in this tree.
            let (mut mm_code, mut mm_left) = (0u32, 0u32);
            if self.mm_len > 0 {
                let e = self.hist[self.mm_ptr] as usize;
                mm_code = trees.code[tbase + e];
                mm_left = trees.len[tbase + e] as u32;
            }
            let msel = if self.mm_len == 0 { 0 } else if self.mm_len < 24 { 1 } else { 2 };
            let mm_lb = self.mm_len.min(63) << 1;
            let (sym_code, sym_len) = if ENC {
                (trees.code[tbase + sym as usize], trees.len[tbase + sym as usize] as u32)
            } else {
                (0, 0)
            };
            let st = &self.stretch;
            let mut x = [0i32; NIN];
            x[NIN - 1] = 256;
            let mut depth = 0u32;
            let mut node = 0usize;
            loop {
                let nd = node.min(SLOT - 2);
                for k in 0..NM {
                    // SAFETY: base[k] < TOTAL by construction (masked hashes).
                    let v = unsafe { self.ctr.get_unchecked(base[k]).0[nd] };
                    x[k] = st[ctr_p(v)] as i32;
                }
                let mut expect = 2u32;
                let mut mm_idx = 0usize;
                let ms;
                if mm_left > 0 {
                    expect = (mm_code >> (mm_left - 1)) & 1;
                    mm_idx = mm_lb | expect as usize;
                    x[NM] = st[ctr_p(self.mm_sm[mm_idx])] as i32;
                    ms = msel;
                } else {
                    x[NM] = 0;
                    ms = 0;
                }
                let ws = ((ms * NCTX + c1) * SLOT + nd) * NIN;
                let wv = &mut self.w[ws..ws + NIN];
                let mut dot: i32 = 0;
                for k in 0..NIN {
                    dot = dot.wrapping_add(x[k].wrapping_mul(wv[k]));
                }
                let d = (dot >> 14).clamp(-2047, 2047);
                let p = self.squash[(d + 2048) as usize] as i32;
                let want = if ENC { (sym_code >> (sym_len - 1 - depth)) & 1 } else { 0 };
                let bit = c.bit(p.clamp(1, 4095) as u32, want);
                for k in 0..NM {
                    // SAFETY: as above.
                    let e = unsafe { &mut self.ctr.get_unchecked_mut(base[k]).0[nd] };
                    *e = ctr_upd(*e, bit);
                }
                if mm_left > 0 {
                    self.mm_sm[mm_idx] = ctr_upd(self.mm_sm[mm_idx], bit);
                    mm_left = if bit != expect { 0 } else { mm_left - 1 };
                }
                let err = (((bit as i32) << 12) - p) * LR;
                for k in 0..NIN {
                    wv[k] = wv[k].wrapping_add((x[k] * err) >> 16);
                }
                let ch = trees.child[tbase + node][bit as usize] as usize;
                depth += 1;
                if ch >= 64 {
                    break (ch - 64) as u8;
                }
                node = ch;
            }
        };
        self.after(s);
        s
    }

    /// Per-symbol bookkeeping: history, word hash, match model.
    #[inline(always)]
    fn after(&mut self, s: u8) {
        if self.mm_len > 0 {
            if self.hist[self.mm_ptr] == s {
                self.mm_len = (self.mm_len + 1).min(MM_MAX);
                self.mm_ptr += 1;
            } else {
                self.mm_len = 0;
            }
        }
        self.hist.push(s);
        self.h6 = ((self.h6 << 6) | (s as u64 + 1)) & ((1u64 << 36) - 1);
        self.word = if s == SPACE || s == SEP {
            0
        } else {
            hash(self.word.wrapping_add(s as u32), 0x1234_5679)
        };
        let len = self.hist.len();
        // Rolling hash: sum of (s + 1) * B^age over the last MM_MIN symbols.
        self.mh = self.mh.wrapping_mul(MM_B).wrapping_add(s as u32 + 1);
        if len > MM_MIN {
            let old = self.hist[len - 1 - MM_MIN] as u32 + 1;
            self.mh = self.mh.wrapping_sub(old.wrapping_mul(MM_BK));
        }
        if len >= MM_MIN {
            let hist = &self.hist;
            let hi = (self.mh.wrapping_mul(0x9E37_79B1) >> (32 - MM_BITS)) as usize;
            if self.mm_len == 0 {
                let p = self.mm_tab[hi] as usize;
                if p > 0 {
                    let mut l = 0;
                    while l < MM_MAX && l < p && hist[p - 1 - l] == hist[len - 1 - l] {
                        l += 1;
                    }
                    if l >= MM_MIN {
                        self.mm_len = l;
                        self.mm_ptr = p;
                    }
                }
            }
            self.mm_tab[hi] = len as u32;
        }
    }
}

// ---------------------------------------------------------------- container
//
// u32 LE symbol count, then for each of the NCTX previous-symbol contexts the
// 44 code lengths as nibbles (0 = never follows; 15 = the only successor),
// then the arithmetic-coded stream.

const HEADER: usize = 4 + NCTX * NSYM / 2;

/// Encodes a stream of symbols 0..NSYM. Panics on a symbol >= NSYM.
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut freq = [[0u64; NSYM]; NCTX];
    let mut prev = 0usize;
    for &b in data {
        assert!((b as usize) < NSYM, "symbol out of range");
        freq[prev][b as usize] += 1;
        prev = b as usize + 1;
    }
    let mut lens = [[0u8; NSYM]; NCTX];
    let mut present = [[false; NSYM]; NCTX];
    let mut out = Vec::with_capacity(HEADER + data.len() / 4);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    for c in 0..NCTX {
        // Length-limit the code to 14: raise rare counts until it fits a nibble.
        let mut f = freq[c];
        let mut floor = 1u64;
        loop {
            lens[c] = code_lengths(&f);
            if lens[c].iter().all(|&l| l < 15) {
                break;
            }
            floor *= 2;
            for s in 0..NSYM {
                if f[s] > 0 {
                    f[s] = f[s].max(floor);
                }
            }
        }
        let mut nib = [0u8; NSYM];
        for s in 0..NSYM {
            present[c][s] = freq[c][s] > 0;
            nib[s] = if !present[c][s] { 0 } else if lens[c][s] == 0 { 15 } else { lens[c][s] };
        }
        for s in (0..NSYM).step_by(2) {
            out.push(nib[s] | nib[s + 1] << 4);
        }
    }
    let trees = Trees::build(&lens, &present);
    let mut e = Encoder { x1: 0, x2: u32::MAX, out };
    let mut m = Model::new(&trees, 0);
    for &s in data {
        m.symbol::<Encoder, true>(&mut e, s);
    }
    let x = e.x1;
    e.out.extend_from_slice(&x.to_be_bytes());
    e.out
}

/// Number of symbols `decode` will produce, read from the header.
pub fn decoded_len(src: &[u8]) -> usize {
    u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize
}

/// Decodes a stream produced by `encode`. A corrupt stream yields garbage or
/// a panic, never undefined behaviour.
pub fn decode(src: &[u8]) -> Vec<u8> {
    let mut stream = Stream::default();
    stream.push(src);
    stream.finish()
}

/// Input kept ahead of the decoder while the stream is still arriving. A
/// symbol is at most 14 decisions and a decision shifts in at most three
/// bytes: with this margin a symbol never reads past what has arrived.
const AHEAD: usize = 64;

/// A decode that takes its input as it arrives — chunk after chunk from the
/// network — and decodes as far as the bytes received allow. Over a link
/// slower than the decoder, the decode is then done when the download is.
#[derive(Default)]
pub struct Stream {
    /// Input received before the header is whole.
    head: Vec<u8>,
    running: Option<Running>,
}

struct Running {
    model: Model<'static>,
    decoder: Decoder,
    n: usize,
}

impl Stream {
    /// Appends bytes of the stream, in order.
    pub fn push(&mut self, chunk: &[u8]) {
        match &mut self.running {
            Some(r) => r.decoder.src.extend_from_slice(chunk),
            None => self.head.extend_from_slice(chunk),
        }
    }

    /// Decodes every symbol the bytes received so far allow; with `complete`,
    /// the rest of the stream — no more input is coming.
    pub fn run(&mut self, complete: bool) {
        if self.running.is_none() {
            if self.head.len() < HEADER + if complete { 0 } else { AHEAD } {
                if !complete {
                    return;
                }
                // A truncated stream: decode what there is, as `decode` did.
                self.head.resize(HEADER, 0);
            }
            let src = core::mem::take(&mut self.head);
            let n = decoded_len(&src);
            let mut lens = [[0u8; NSYM]; NCTX];
            let mut present = [[false; NSYM]; NCTX];
            let mut pos = 4;
            for c in 0..NCTX {
                for s in (0..NSYM).step_by(2) {
                    let b = src[pos];
                    pos += 1;
                    for (k, v) in [(s, b & 15), (s + 1, b >> 4)] {
                        present[c][k] = v != 0;
                        lens[c][k] = if v == 15 { 0 } else { v };
                    }
                }
            }
            // The model borrows its trees for its whole life: one small leak
            // per decode, and a page decodes once.
            let trees: &'static Trees = Box::leak(Box::new(Trees::build(&lens, &present)));
            let model = Model::new(trees, n);
            self.running = Some(Running { model, decoder: Decoder::new(src, pos), n });
        }
        let r = self.running.as_mut().unwrap();
        while r.model.hist.len() < r.n && (complete || r.decoder.pos + AHEAD <= r.decoder.src.len()) {
            r.model.symbol::<Decoder, false>(&mut r.decoder, 0);
        }
    }

    /// Symbols decoded so far.
    pub fn decoded(&self) -> usize {
        self.running.as_ref().map_or(0, |r| r.model.hist.len())
    }

    /// Decodes the rest — no more input is coming — and gives the symbols.
    pub fn finish(mut self) -> Vec<u8> {
        self.run(true);
        self.running.map_or_else(Vec::new, |r| r.model.hist)
    }
}
