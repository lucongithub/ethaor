//! Indices derived from a letter stream, for the search.
//!
//! Built in memory, never welded: each is a few milliseconds from the stream,
//! where welding them would add megabytes to the download. The postings are
//! built once with the index; residue lists and planes are built on demand,
//! per letter and gap, and kept for the next query.

use crate::letter::ALPHABET;

/// Planes or residue lists kept before the cache is emptied: a plane is one
/// bit per letter of the stream, so this bounds the cache to a few megabytes.
const KEEP: usize = 48;

pub struct Index<'t> {
    pub text: &'t [u8],
    pub freq: [u32; ALPHABET],
    /// Every position of every letter, grouped by letter, ascending in a group.
    post: Vec<u32>,
    post_at: [u32; ALPHABET + 1],
    residues: Vec<Residues>,
    planes: Vec<Plane>,
}

/// The positions of one letter, split by residue modulo a gap: the letters a
/// pattern can meet at that gap from a given anchor all share its residue.
pub struct Residues {
    code: u8,
    gap: usize,
    at: Vec<u32>,
    list: Vec<u32>,
}

/// One bit per position of the stream, set where a letter stands, **decimated
/// by a gap**: the positions of residue 0 first (0, g, 2g…), then residue 1,
/// and so on. Read from the end when `reverse`. A pattern letter `g` places
/// after its anchor sits at `i + g·s`: for consecutive steps `s` its bits are
/// consecutive here, so one word answers 64 steps at once.
pub struct Plane {
    code: u8,
    gap: usize,
    reverse: bool,
    /// First bit of each residue.
    pub base: Vec<u32>,
    /// Two words of padding at the end, so a window read never leaves it.
    pub bits: Vec<u64>,
    /// Set bits before each word — kept for the plain plane only (gap 1,
    /// forward), where it counts a letter's occurrences in any span.
    pub before: Vec<u32>,
}

impl<'t> Index<'t> {
    pub fn new(text: &'t [u8]) -> Index<'t> {
        assert!(text.len() < 1 << 31, "the stream goes past the index addressing");
        let mut freq = [0u32; ALPHABET];
        for &c in text {
            freq[c as usize] += 1;
        }
        let mut post_at = [0u32; ALPHABET + 1];
        for c in 0..ALPHABET {
            post_at[c + 1] = post_at[c] + freq[c];
        }
        let mut fill = post_at;
        let mut post = vec![0u32; text.len()];
        for (p, &c) in text.iter().enumerate() {
            post[fill[c as usize] as usize] = p as u32;
            fill[c as usize] += 1;
        }
        Index { text, freq, post, post_at, residues: Vec::new(), planes: Vec::new() }
    }

    /// Drops every plane and residue list: what a cold query pays again.
    pub fn forget(&mut self) {
        self.planes.clear();
        self.residues.clear();
    }

    /// Positions of a letter, ascending.
    pub fn positions(&self, code: u8) -> &[u32] {
        &self.post[self.post_at[code as usize] as usize..self.post_at[code as usize + 1] as usize]
    }

    /// Empties the caches if `planes` more planes might not fit: called once
    /// per query, before asking for its planes and lists, so the ranks it
    /// receives stay valid for the whole query.
    pub fn make_room(&mut self, planes: usize) {
        if self.planes.len() + planes > KEEP {
            self.planes.clear();
        }
        if self.residues.len() + 1 > KEEP {
            self.residues.clear();
        }
    }

    /// Builds, if missing, the residue lists of a letter for a gap above 1,
    /// and returns their rank in the cache.
    pub fn residues(&mut self, code: u8, gap: usize) -> usize {
        if let Some(k) = self.residues.iter().position(|r| r.code == code && r.gap == gap) {
            return k;
        }
        let positions = self.positions(code);
        let mut at = vec![0u32; gap + 1];
        for &p in positions {
            at[p as usize % gap + 1] += 1;
        }
        for r in 0..gap {
            at[r + 1] += at[r];
        }
        let mut fill = at.clone();
        let mut list = vec![0u32; positions.len()];
        for &p in positions {
            let r = p as usize % gap;
            list[fill[r] as usize] = p;
            fill[r] += 1;
        }
        self.residues.push(Residues { code, gap, at, list });
        self.residues.len() - 1
    }

    pub fn residue_list(&self, k: usize, residue: usize) -> &[u32] {
        let r = &self.residues[k];
        &r.list[r.at[residue] as usize..r.at[residue + 1] as usize]
    }

    /// Builds, if missing, the plane of a letter at a gap, read forward or
    /// from the end, and returns its rank in the cache.
    pub fn plane(&mut self, code: u8, gap: usize, reverse: bool) -> usize {
        if let Some(k) = self.planes.iter().position(|p| p.code == code && p.gap == gap && p.reverse == reverse) {
            return k;
        }
        let n = self.text.len();
        let mut base = vec![0u32; gap];
        for r in 1..gap {
            let len = n.saturating_sub(r - 1).div_ceil(gap);
            base[r] = base[r - 1] + len as u32;
        }
        let mut bits = vec![0u64; n / 64 + 3];
        for &p in self.positions(code) {
            let q = if reverse { n - 1 - p as usize } else { p as usize };
            let bit = base[q % gap] as usize + q / gap;
            bits[bit >> 6] |= 1 << (bit & 63);
        }
        let before = if gap == 1 && !reverse {
            let mut sum = 0u32;
            bits.iter()
                .map(|w| {
                    let at = sum;
                    sum += w.count_ones();
                    at
                })
                .collect()
        } else {
            Vec::new()
        };
        self.planes.push(Plane { code, gap, reverse, base, bits, before });
        self.planes.len() - 1
    }

    pub fn plane_at(&self, k: usize) -> &Plane {
        &self.planes[k]
    }
}

impl Plane {
    /// Occurrences of the letter before position `p`, `p <= n`, on the plain
    /// plane: the words before, then the bits of its own word.
    #[inline]
    pub fn rank(&self, p: usize) -> u32 {
        let k = p >> 6;
        self.before[k] + (self.bits[k] & ((1u64 << (p & 63)) - 1)).count_ones()
    }

    /// Bit offset of step 0 for an anchor at `i`, in a stream of `n` letters.
    #[inline]
    pub fn origin(&self, i: usize, n: usize) -> usize {
        let q = if self.reverse { n - 1 - i } else { i };
        if self.gap == 1 {
            return q;
        }
        // In u32: the stream fits, and a 32-bit division is the cheap one.
        let (q, g) = (q as u32, self.gap as u32);
        self.base[(q % g) as usize] as usize + (q / g) as usize
    }
}
