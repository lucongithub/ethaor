// The word a letter search asks for, typed or tapped: Hebrew consonants
// only. The keyboard lays the alphabet in its order, each final form right
// after its letter — 27 keys, three rows of nine. Marks and anything else
// typed are dropped: the search reads letters, and folds finals itself.

export const KEYS = [
	'א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז', 'ח', 'ט',
	'י', 'כ', 'ך', 'ל', 'מ', 'ם', 'נ', 'ן', 'ס',
	'ע', 'פ', 'ף', 'צ', 'ץ', 'ק', 'ר', 'ש', 'ת'
]; // prettier-ignore

// U+05D0 alef to U+05EA tav: the consonants, finals among them.
const NOT_A_LETTER = /[^א-ת]/g;

/** The Hebrew letters of `text`, in order, nothing else. */
export function letters(text: string): string {
	return text.replace(NOT_A_LETTER, '');
}

/**
 * `letter` put in place of the selection `from..to` of `word` (a caret when
 * they are equal), and where the caret stands after it. Offsets are UTF-16
 * units, as an input gives them; a consonant is one.
 */
export function insert(
	word: string,
	from: number,
	to: number,
	letter: string
): { word: string; caret: number } {
	const a = Math.max(0, Math.min(from, to, word.length));
	const b = Math.min(word.length, Math.max(from, to, a));
	const typed = letters(letter);
	return { word: word.slice(0, a) + typed + word.slice(b), caret: a + typed.length };
}

/** The selection `from..to` of `word` erased, or the letter before the caret. */
export function erase(word: string, from: number, to: number): { word: string; caret: number } {
	const a = Math.max(0, Math.min(from, to, word.length));
	const b = Math.min(word.length, Math.max(from, to, a));
	const start = a === b ? Math.max(0, a - 1) : a;
	return { word: word.slice(0, start) + word.slice(b), caret: start };
}
