//! The other reading, where another publication of the edition differs.
//!
//! `build.rs` measured the table against Miqra according to the Masorah and
//! welded, for each verse concerned, a **display string**: our verse with the
//! other reading inserted right after ours, each form carrying its index. The
//! module draws that string instead of the verse, and paints the inserted part
//! in its own ink.
//!
//! Like `corpus`, everything here is an `O(1)` read — or a dichotomy over
//! eighty-eight verses — over the table `corpus` unpacks with the text.
//! Nothing is parsed, nothing is allocated.

use crate::corpus;

/// One insertion inside a display string: where it starts, the length of the
/// mark that numbers our form, the length of the other reading.
///
/// The two lengths are kept apart because they are not painted alike: the mark
/// belongs to our form, what follows is the other edition's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Insert {
    pub at: usize,
    pub mark: usize,
    pub other: usize,
}

impl Insert {
    /// Range of the other reading, mark excluded: what is painted apart.
    pub fn other_range(&self) -> (usize, usize) {
        (self.at + self.mark, self.other)
    }

    /// Range of the whole insertion: what is not in the verse.
    pub fn range(&self) -> (usize, usize) {
        (self.at, self.mark + self.other)
    }
}

fn words() -> &'static [u32] {
    corpus::variant_table()
}

/// Verses carrying a display string.
pub fn verse_count() -> usize {
    words()[0] as usize
}

/// Differences, all verses together.
pub fn count() -> usize {
    words()[1] as usize
}

/// `verse, offset, length` per verse, sorted by verse.
fn displays() -> &'static [u32] {
    &words()[2..2 + verse_count() * 3]
}

/// `verse, at, mark, other` per difference, sorted by verse then by position.
fn inserts() -> &'static [u32] {
    let from = 2 + verse_count() * 3;
    &words()[from..from + count() * 4]
}

/// Rank of a verse in a series of records of `stride` u32 keyed by verse.
fn seek(table: &'static [u32], stride: usize, verse: usize) -> Option<usize> {
    let (mut lo, mut hi) = (0usize, table.len() / stride);
    while lo < hi {
        let mid = (lo + hi) / 2;
        match (table[mid * stride] as usize).cmp(&verse) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => return Some(mid),
        }
    }
    None
}

/// The verse as it is drawn, when it carries another reading.
///
/// `None` for the twenty-three thousand others, which are drawn from the
/// corpus as before — the difference costs them one dichotomy.
pub fn display(verse: usize) -> Option<&'static str> {
    let table = displays();
    let at = seek(table, 3, verse)?;
    let (offset, len) = (table[at * 3 + 1] as usize, table[at * 3 + 2] as usize);
    Some(&corpus::text()[offset..offset + len])
}

/// Insertions of a verse, in the order of the text.
pub fn inserts_of(verse: usize) -> impl Iterator<Item = Insert> {
    let table = inserts();
    let mut from = seek(table, 4, verse).unwrap_or(table.len() / 4);
    // The dichotomy lands anywhere among the records of the verse; walk back to
    // the first one, there are never more than a handful.
    while from > 0 && table[(from - 1) * 4] as usize == verse {
        from -= 1;
    }
    (from..table.len() / 4)
        .take_while(move |&i| table[i * 4] as usize == verse)
        .map(move |i| Insert {
            at: table[i * 4 + 1] as usize,
            mark: table[i * 4 + 2] as usize,
            other: table[i * 4 + 3] as usize,
        })
}

#[cfg(test)]
mod tests;
