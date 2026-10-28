<script lang="ts" module>
	/** Where the reader is in the book: what the breadcrumb and the legend say. */
	export interface Reading {
		place: { chapter: number; verse: number };
		section: string;
		book: string;
		chapter: string;
		verse: string;
		/** The portion and the book the breadcrumb names, -1 for none: each leads to its level. */
		portion: number;
		bookIndex: number;
	}
</script>

<script lang="ts">
	// The Tanakh read as a book: one column, scrolled, the browser laying the
	// text out — shaping, wrapping, selecting, finding — as on any web page.
	// Zoom changes the text's size, not a camera: the column reflows, and the
	// verse under the eye stays where it was. The page is a traditional web
	// page: the text takes Tailwind's type scale, stepped up at each
	// breakpoint, times the reader's zoom (`--zoom`), kept for the next visit.
	//
	// The breadcrumb goes up (`climbTo`): the book to its chapters, the
	// portion to the portions with every book's name, the Tanakh to the three
	// portions alone — and each level goes back down where it is touched.
	//
	// The 929 chapters are all in the page, but only those near the screen are
	// laid out; the others keep their height — measured once laid, estimated
	// before — so the scroll never moves under the reader.
	import { onMount, tick, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Book, Place, Selection } from '$lib/ethaor';
	import {
		bookOf,
		BYTES_PER_VERSE,
		estimate,
		kept,
		SIZE,
		trail,
		zoomed,
		type Chapter,
		type Level
	} from '$lib/book-view';
	import { escalate, reporter, type Fault } from '$lib/fault';
	import { PORTION_TEXT } from '$lib/portions';
	import { barRow, COLORS, inkStyle, isShown, worn, type Marked } from '$lib/marks';

	let {
		book,
		start,
		startLevel = 'text',
		landed,
		searches = [],
		onreading,
		onform,
		onselect,
		onfault
	}: {
		book: Book;
		start: Place;
		/** The level it opens on: the map's depth, handed over. */
		startLevel?: Level;
		/** Bumped whenever verses land: the chapters near the screen are read again. */
		landed: number;
		/** The searches kept: their letters coloured in the text, their bars by the names above it. */
		searches?: Marked[];
		onreading: (reading: Reading | null) => void;
		/** The pointer is on one of the two forms, ours `¹` or theirs `[…]²`, or has left it. */
		onform: (on: boolean) => void;
		/** A click read the passage: what the map's click gives. */
		onselect: (selection: Selection | null) => void;
		onfault: (fault: Fault) => void;
	} = $props();

	/** Chapters laid out beyond the first and last on screen, each way. */
	const AROUND = 2;
	const STORED = 'ethaor.book.scale';

	let scroller: HTMLDivElement;
	let column: HTMLElement;
	let above = $state<HTMLDivElement | null>(null);
	/** The room's width, watched for a breakpoint crossed, and the reader's zoom. */
	let room = 0;
	let scale = $state(1);
	/** The verses' size in pixels, as the browser computes it: the step and the zoom. */
	let size = $state(SIZE);
	function measure() {
		size = parseFloat(getComputedStyle(column).fontSize) || size;
	}
	let width = $state(640);
	let level = $state<Level>('text');
	const contents = $derived(book.contents);
	let span = $state({ from: 0, to: 0 });
	const texts = new SvelteMap<number, Chapter>();
	const heights = new SvelteMap<number, number>();
	const onScreen = new Set<number>();
	const indices = $derived(Array.from({ length: book.count }, (_, c) => c));
	// The place to land on once its chapter is in: the verse may come later
	// than the scroll to its chapter.
	let target: Place | null = null;

	function read(chapter: number): Chapter | null {
		try {
			return book.read(chapter);
		} catch (cause) {
			const fault = escalate(cause, 'book-unread', `reading chapter ${chapter} for the book view`);
			reporter.report(fault);
			onfault(fault);
			return null;
		}
	}

	/** Reads what lies in the window and is not read yet — or read again. */
	function fill(again = false) {
		book.want(span.from, span.to);
		for (let c = span.from; c < span.to; c++) {
			if (again || !texts.has(c)) {
				const chapter = read(c);
				if (chapter) texts.set(c, chapter);
			}
		}
	}

	function moveWindow() {
		if (onScreen.size === 0) return;
		const seen = [...onScreen];
		const next = kept(Math.min(...seen), Math.max(...seen), AROUND, book.count);
		if (next.from === span.from && next.to === span.to) return;
		span = next;
		fill();
		// A chapter gone to lands once it is laid.
		if (target) void tick().then(land);
	}

	// Verses landed: what was missing comes, and the readings with the last.
	$effect(() => {
		if (landed < 0) return;
		fill(true);
		void tick().then(land);
	});

	// A search kept, shown, hidden or removed: the chapters read are read
	// again, their found letters cut anew. A colour changed needs no reading.
	const marking = $derived(searches.map((m) => `${m.id}:${m.slot}:${m.total}`).join('|'));
	let marked = '';
	$effect(() => {
		if (marking === marked) return;
		marked = marking;
		untrack(() => fill(true));
	});

	/** The style of letters the shown searches among `slots` found: their colour, or their bands. */
	function tint(slots: number): string {
		return inkStyle(worn(searches, slots));
	}

	/** The colours of the bars by a portion's, book's or chapter's name. */
	function bars(level: 0 | 1 | 2, index: number): string[] {
		if (!searches.some((m) => isShown(m) && m.total)) return [];
		return book
			.marks(level, index)
			.flatMap((slot) => worn(searches, 1 << slot).map((c) => COLORS[c]));
	}

	function placeholder(chapter: number): number {
		const known = heights.get(chapter);
		if (known !== undefined) return known;
		const bytes = book.size(chapter);
		const verses =
			texts.get(chapter)?.verses.length ?? Math.max(1, Math.round(bytes / BYTES_PER_VERSE));
		return estimate(bytes, verses, book.opens.has(chapter), size, width);
	}

	// ------------------------------------------------------------- observers

	let visible: IntersectionObserver;
	let measured: ResizeObserver;

	function chapterOf(element: Element): number {
		return Number((element as HTMLElement).dataset.chapter);
	}

	// ----------------------------------------------------------------- place

	/** Where the reader is: the verse at the middle of the column. */
	export function here(): Place {
		const at = centre() ?? last;
		return at ? { chapter: at.chapter, verse: at.verse } : start;
	}

	/** The last place read: what is kept while a level above covers the text. */
	let last: Place | null = null;

	function centre() {
		if (!scroller || level !== 'text') return null;
		const box = scroller.getBoundingClientRect();
		const x = column.getBoundingClientRect();
		const point = document.elementFromPoint(x.left + x.width / 2, box.top + box.height / 2);
		const section = point?.closest<HTMLElement>('[data-chapter]');
		if (!section) return null;
		const chapter = chapterOf(section);
		let verse = point?.closest<HTMLElement>('[data-verse]');
		// Between two verses, the nearest below.
		verse ??= section.querySelector<HTMLElement>('[data-verse]');
		return { chapter, verse: verse ? Number(verse.dataset.verse) : -1 };
	}

	let lastReading = '';
	function report() {
		const at = centre();
		const text = at && texts.get(at.chapter);
		if (!at || !text) {
			if (lastReading) onreading(null);
			lastReading = '';
			return;
		}
		const verse = text.verses.find((v) => v.index === at.verse);
		const steps = trail(contents, 'text', text.book);
		const reading: Reading = {
			place: at,
			section: text.sectionName,
			book: text.bookName,
			chapter: text.label,
			verse: verse?.label ?? '',
			portion: steps.portion,
			bookIndex: steps.book
		};
		last = at;
		const key = `${at.chapter}|${at.verse}`;
		if (key === lastReading) return;
		lastReading = key;
		onreading(reading);
	}

	let scheduled = 0;
	function onScroll() {
		scheduled ||= requestAnimationFrame(() => {
			scheduled = 0;
			report();
			// The text moved under a pointer that did not: what it is on now.
			if (pointer) form(document.elementFromPoint(pointer.x, pointer.y));
		});
	}

	// The index under the column opens on the two forms alone — our marked
	// word, theirs in brackets — never for the chapter around them; and theirs
	// wears its colour only then, that pair's alone.
	let lit = $state<string | null>(null);
	let pointer: { x: number; y: number } | null = null;
	function form(under: Element | null) {
		const at = under?.closest<HTMLElement>('[data-form]')?.dataset.form ?? null;
		if (at === lit) return;
		const was = lit !== null;
		lit = at;
		if (was !== (at !== null)) onform(at !== null);
	}
	function onOver(event: PointerEvent) {
		if (event.pointerType === 'touch') return;
		pointer = { x: event.clientX, y: event.clientY };
		form(event.target as Element);
	}
	function onLeave() {
		pointer = null;
		form(null);
	}

	/** Brings the target to the eye once its chapter, and its verse, are laid. */
	function land() {
		if (!target) return;
		const section = scroller.querySelector<HTMLElement>(`[data-chapter="${target.chapter}"]`);
		if (!section) return;
		// With no verse, the chapter's top — its book's and portion's names
		// with it, where it opens one — once it is laid.
		const verse =
			target.verse >= 0
				? section.querySelector<HTMLElement>(`[data-verse="${target.verse}"]`)
				: section.querySelector<HTMLElement>('header') && section;
		if (!verse) {
			section.scrollIntoView({ block: 'start' });
			return;
		}
		verse.scrollIntoView({ block: target.verse >= 0 ? 'center' : 'start' });
		target = null;
		report();
	}

	// ----------------------------------------------------------------- click

	// A click reads the passage, as on the map: the verse under it; its words
	// are picked in the passage, once open. A drag, which selects text, stays
	// the browser's.
	let press: { x: number; y: number } | null = null;

	function onClick(event: MouseEvent) {
		if (!press || Math.hypot(event.clientX - press.x, event.clientY - press.y) > 4) return;
		if (!(getSelection()?.isCollapsed ?? true)) return;
		const verse = (event.target as Element).closest<HTMLElement>('[data-verse]');
		if (!verse) return;
		onselect(book.select(Number(verse.dataset.verse)));
	}

	/** Back to the text, as the passage closes: the keys scroll it again. */
	export function focus() {
		scroller.focus({ preventScroll: true });
	}

	// ------------------------------------------------------------------ zoom

	/**
	 * The text's size changes and the column reflows; the verse at `y` stays
	 * at `y`. Heights measured at the old size are forgotten.
	 */
	async function resize(steps: number, y: number) {
		const next = zoomed(size, steps);
		if (next === size) return;
		const ratio = next / size;
		await refit(() => (scale *= ratio), y);
		try {
			localStorage.setItem(STORED, String(scale));
		} catch {
			// No storage: the zoom lasts the visit.
		}
	}

	/** The room changed width — a breakpoint may be crossed: the verse at the middle kept there. */
	function refitRoom() {
		const now = scroller.clientWidth;
		if (now === room) return;
		room = now;
		const box = scroller.getBoundingClientRect();
		void refit(() => {}, box.top + box.height / 2);
	}

	/** Applies a change of size, keeping the verse at `y` where it is. */
	async function refit(change: () => void, y: number) {
		const box = scroller.getBoundingClientRect();
		const x = column.getBoundingClientRect();
		const anchor =
			document.elementFromPoint(x.left + x.width / 2, y)?.closest<HTMLElement>('[data-verse]') ??
			document
				.elementFromPoint(x.left + x.width / 2, box.top + box.height / 2)
				?.closest<HTMLElement>('[data-verse]');
		const before = anchor?.getBoundingClientRect().top ?? 0;
		const was = size;
		change();
		await tick();
		measure();
		if (size === was) return;
		heights.clear();
		await tick();
		width = column.clientWidth;
		if (anchor?.isConnected) scroller.scrollTop += anchor.getBoundingClientRect().top - before;
		report();
	}

	function onWheel(event: WheelEvent) {
		// Ctrl with the wheel — and a trackpad's pinch, which arrives so — sizes
		// the text here, not the page around it.
		if (!event.ctrlKey) return;
		event.preventDefault();
		const steps = event.deltaMode === 1 ? -event.deltaY : -event.deltaY / 100;
		void resize(steps, event.clientY);
	}

	// ----------------------------------------------------------------- levels

	/** The chapter read when the text was left: what the levels above mark. */
	let current = $state(-1);
	/** The book whose chapters are shown, and which the levels above mark. */
	let opened = $state(0);
	const openedPortion = $derived(contents.books[opened]?.section ?? 0);

	/** Where the reader is, and at which level: what the map is handed. */
	export function where(): { place: Place; level: Level } {
		if (level !== 'chapters') return { place: here(), level };
		// The book whose chapters are shown, at the chapter read if it is that one's.
		const b = contents.books[opened];
		const chapter = bookOf(contents, current) === opened ? current : b.first;
		return { place: { chapter, verse: -1 }, level };
	}

	// Above the text, the breadcrumb says the level: the book whose chapters
	// are shown and its portion, or the Tanakh alone over the books and the
	// portions.
	$effect(() => {
		if (level === 'text') return;
		const steps = trail(contents, level, opened);
		onreading({
			place: { chapter: contents.books[opened]?.first ?? 0, verse: -1 },
			section: contents.portions[steps.portion] ?? '',
			book: contents.books[steps.book]?.name ?? '',
			chapter: '',
			verse: '',
			portion: steps.portion,
			bookIndex: steps.book
		});
	});

	/** Shows a level above the text, the reader's place in it marked and focused. */
	async function enter(next: Level, portion = -1) {
		level = next;
		// Back at the text, the place read is said again, even unchanged.
		lastReading = '';
		await tick();
		const focus = (...selectors: string[]) => {
			const found = selectors
				.filter(Boolean)
				.map((s) => above?.querySelector<HTMLElement>(s))
				.find(Boolean);
			found?.focus({ preventScroll: true });
		};
		if (next === 'chapters') {
			reveal(`[data-chapters-of="${opened}"]`);
			focus(
				bookOf(contents, current) === opened ? `[data-goes="${current}"]` : '',
				`[data-chapters-of="${opened}"] [data-goes]`
			);
		} else if (next === 'books') {
			const at = portion >= 0 ? `[data-portion="${portion}"]` : `[data-opens="${opened}"]`;
			reveal(at);
			focus(portion >= 0 ? `${at} [data-opens]` : at);
		} else if (next === 'portions') {
			focus(`[data-portion-button="${openedPortion}"]`);
		}
	}

	/**
	 * The chapters scroll through every book, one a screen: the one at the
	 * middle of the screen is the one open — what the levels above mark, what the map
	 * is handed.
	 */
	let following = 0;
	function onChaptersScroll() {
		following ||= requestAnimationFrame(() => {
			following = 0;
			if (!above) return;
			const box = above.getBoundingClientRect();
			const at = document
				.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
				?.closest<HTMLElement>('[data-chapters-of]');
			if (at) opened = Number(at.dataset.chaptersOf);
		});
	}

	/**
	 * A wheel turns one book a gesture. Snapping alone took a notch — a
	 * hundred pixels of a screen-high book — back to the book it left, so
	 * the wheel seemed stuck.
	 *
	 * A book taller than the screen (the Psalms on most screens) is scrolled
	 * by the wheel itself, never past its own edges: left to the browser, the
	 * notch that crossed its end landed on the next book, and its last
	 * chapters could not be reached. Only a new gesture made at an edge turns
	 * the book; the rest of the one that reached it — a long scroll, a
	 * trackpad's momentum — is absorbed until it pauses. Coming back up, such
	 * a book is entered by its end.
	 */
	let turning = 0;
	let turned = false;
	let aim = 0;
	const GESTURE_MS = 250;
	function onChaptersWheel(event: WheelEvent) {
		if (event.ctrlKey || !above) return;
		const dy = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
		if (Math.abs(dy) < 1) return;
		event.preventDefault();
		const now = performance.now();
		const within = now < turning;
		turning = now + GESTURE_MS;
		// The rest of a gesture that turned a book is absorbed whole.
		if (within && turned) return;
		const box = above.getBoundingClientRect();
		// The book on screen: the one at its middle.
		const section = document
			.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
			?.closest<HTMLElement>('[data-chapters-of]');
		if (!section) return;
		const here = section.getBoundingClientRect();
		// How far it still goes past the screen, the way the wheel turns.
		const left = dy > 0 ? here.bottom - box.bottom : here.top - box.top;
		if (dy > 0 ? left > 1 : left < -1) {
			if (!within) aim = above.scrollTop;
			turned = false;
			aim += dy > 0 ? Math.min(dy, left) : Math.max(dy, left);
			above.scrollTo({ top: aim, behavior: 'smooth' });
			return;
		}
		if (within) return;
		const next = above.querySelector<HTMLElement>(
			`[data-chapters-of="${Number(section.dataset.chaptersOf) + Math.sign(dy)}"]`
		);
		if (!next) return;
		opened = Number(next.dataset.chaptersOf);
		turned = true;
		// Entered by its end when coming back up to a book taller than the screen.
		const end = dy < 0 && next.offsetHeight > above.clientHeight + 1;
		aim = end ? next.offsetTop + next.offsetHeight - above.clientHeight : next.offsetTop;
		above.scrollTo({ top: aim, behavior: 'smooth' });
	}

	/**
	 * Up to a level from the breadcrumb: a book's chapters (`index` the
	 * book), the books (`index` the portion revealed), the portions alone.
	 * Left from the text, the chapter read is what the levels mark.
	 */
	export function climbTo(next: Exclude<Level, 'text'>, index = -1) {
		if (level === 'text') {
			current = here().chapter;
			opened = Math.max(0, bookOf(contents, current));
		}
		if (next === 'chapters' && index >= 0) opened = index;
		void enter(next, next === 'books' ? index : -1);
	}

	/** The books of a portion, from the portions alone. */
	function openPortion(portion: number) {
		void enter('books', portion);
	}

	/** A book's chapters, from the books. */
	function openBook(index: number) {
		opened = index;
		void enter('chapters');
	}

	function reveal(selector: string) {
		above?.querySelector<HTMLElement>(selector)?.scrollIntoView({ block: 'start' });
	}

	/** Down to the text, at a chapter's top. */
	async function go(chapter: number) {
		level = 'text';
		target = { chapter, verse: -1 };
		span = kept(chapter, chapter, AROUND, book.count);
		fill();
		await tick();
		land();
		scroller.focus({ preventScroll: true });
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		// Enter reads the passage at the middle of the column, as on the map.
		if (event.key === 'Enter') {
			const at = centre();
			if (!at || at.verse < 0) return;
			event.preventDefault();
			onselect(book.select(at.verse));
			return;
		}
		const steps = event.key === '+' || event.key === '=' ? 1 : event.key === '-' ? -1 : 0;
		if (steps === 0) return;
		event.preventDefault();
		const box = scroller.getBoundingClientRect();
		void resize(steps, box.top + box.height / 2);
	}

	// Two fingers size the text; one scrolls, natively (`touch-action: pan-y`).
	const fingers = new Map<number, { x: number; y: number }>();
	let spread = 0;
	const apart = () => {
		const [a, b] = [...fingers.values()];
		return Math.hypot(a.x - b.x, a.y - b.y);
	};
	function onPointerDown(event: PointerEvent) {
		press = { x: event.clientX, y: event.clientY };
		if (event.pointerType !== 'touch') return;
		fingers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		if (fingers.size === 2) spread = apart();
	}
	function onPointerMove(event: PointerEvent) {
		if (!fingers.has(event.pointerId)) return;
		fingers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		if (fingers.size !== 2 || spread === 0) return;
		const now = apart();
		const steps = Math.log(now / spread) / Math.log(1.1);
		if (Math.abs(steps) < 0.25) return;
		spread = now;
		const [a, b] = [...fingers.values()];
		void resize(steps, (a.y + b.y) / 2);
	}
	function onPointerUp(event: PointerEvent) {
		fingers.delete(event.pointerId);
		spread = fingers.size === 2 ? apart() : 0;
	}

	// ----------------------------------------------------------------- start

	onMount(() => {
		try {
			const stored = Number(localStorage.getItem(STORED));
			if (stored > 0 && Number.isFinite(stored)) scale = stored;
		} catch {
			// No storage: no zoom.
		}
		room = scroller.clientWidth;
		width = column.clientWidth;
		visible = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					const chapter = chapterOf(entry.target);
					if (entry.isIntersecting) onScreen.add(chapter);
					else onScreen.delete(chapter);
				}
				moveWindow();
			},
			{ root: scroller }
		);
		// What it measures is applied at the next frame, never inside the
		// callback: a new width resizes every chapter not laid out, which the
		// observer sees again in the same frame — the browser then reports a
		// loop. Measured, the reader saw it as a fault while scrolling.
		let sizes = new Map<number, number>();
		let columnMoved = false;
		let roomMoved = false;
		let applying = 0;
		measured = new ResizeObserver((entries) => {
			for (const entry of entries) {
				if (entry.target === scroller) {
					roomMoved = true;
					continue;
				}
				if (entry.target === column) {
					columnMoved = true;
					continue;
				}
				const chapter = chapterOf(entry.target);
				// Only a laid-out chapter's height is its own.
				if (texts.has(chapter) && chapter >= span.from && chapter < span.to) {
					sizes.set(
						chapter,
						Math.round(entry.borderBoxSize[0]?.blockSize ?? entry.contentRect.height)
					);
				}
			}
			applying ||= requestAnimationFrame(() => {
				applying = 0;
				for (const [chapter, height] of sizes) heights.set(chapter, height);
				sizes = new Map();
				if (columnMoved) width = column.clientWidth;
				if (roomMoved) refitRoom();
				columnMoved = roomMoved = false;
			});
		});
		measured.observe(scroller);
		measured.observe(column);
		for (const section of scroller.querySelectorAll<HTMLElement>('[data-chapter]')) {
			visible.observe(section);
			measured.observe(section);
		}

		target = start;
		current = start.chapter;
		opened = Math.max(0, bookOf(contents, start.chapter));
		span = kept(start.chapter, start.chapter, AROUND, book.count);
		fill();
		void tick().then(() => {
			measure();
			land();
			if (startLevel === 'text') scroller.focus({ preventScroll: true });
			else void enter(startLevel);
		});

		return () => {
			visible.disconnect();
			measured.disconnect();
			cancelAnimationFrame(scheduled);
		};
	});

	/**
	 * The verses' size: Tailwind's `text-lg`, `text-xl` from `sm`, `text-2xl`
	 * from `lg`, times the reader's zoom. The levels above set it too, though
	 * their text has its own: their column, 34 of it, is the verses' column.
	 */
	const VERSES =
		'text-[calc(1.125rem*var(--zoom))] sm:text-[calc(1.25rem*var(--zoom))] lg:text-[calc(1.5rem*var(--zoom))]';

	const laid = (chapter: number) => chapter >= span.from && chapter < span.to && texts.has(chapter);
	const opensPortion = (text: Chapter) => text.opens && [0, 5, 26].includes(text.book);
</script>

<!-- A search's bars, as the map draws them: beside a name, where Hebrew
     ends, in the search's colour; under a chapter's number, its cell too
     narrow beside it. Sized in the name's own em, so they follow its step;
     each search adds one, and past the row's widest they thin (`barRow`). -->
{#snippet beside(colors: string[])}
	{@const row = barRow(colors.length)}
	{#if colors.length}<span
			class="ms-[0.35em] inline-flex h-[0.42em] align-middle"
			style:gap="{row.space}em"
			aria-hidden="true"
			>{#each colors as color, k (k)}<span
					class="h-full"
					style:width="{row.bar}em"
					style:background-color={color}
				></span>{/each}</span
		>{/if}
{/snippet}

<!-- The same, out of the flow: the three portions alone keep the place the
     map's opening gives them, to the pixel, bars or none — inline, the bars
     widened the block and moved it off the map's. Sized and spaced as the
     map draws them (`BAR_WIDTH`, `BAR_SPACE`, `BAR_HEIGHT`, `BAR_GAP`,
     `BAR_RISE`). -->
{#snippet outside(colors: string[])}
	<!-- Hung on the name's own baseline, as the map hangs them: each bar an
	     empty inline block, which sits on the baseline by its bottom margin —
	     its centre 0.30 em above it, where the canvas sets it (its baseline
	     0.36 em under the line's middle, the bar `BAR_RISE` under it),
	     whatever the font's metrics. Centring on the line box put them a
	     little low. -->
	{@const row = barRow(colors.length)}
	{#if colors.length}<span
			class="absolute top-0 right-full mr-[0.35em] whitespace-nowrap"
			aria-hidden="true"
			>{#each colors as color, k (k)}<span
					class="mb-[0.09em] inline-block h-[0.42em] align-baseline"
					style:width="{row.bar}em"
					style:margin-inline-start={k ? `${row.space}em` : null}
					style:background-color={color}
				></span>{/each}</span
		>{/if}
{/snippet}

{#snippet under(colors: string[])}
	{@const row = barRow(colors.length)}
	{#if colors.length}<span
			class="absolute inset-x-0 bottom-[0.2em] flex h-[0.24em] justify-center"
			style:gap="{row.space}em"
			aria-hidden="true"
			>{#each colors as color, k (k)}<span
					class="h-full"
					style:width="{row.bar}em"
					style:background-color={color}
				></span>{/each}</span
		>{/if}
{/snippet}

<!-- `touch-action: pan-y`: one finger scrolls natively, two size the text.
     Focusable, as a scrolled region must be for the keys to scroll it, and
     named with its keys like the map's canvas. -->
<!-- The text's size is set here, for the column every level shares. -->
<div class="relative h-full" style:--zoom={scale}>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		bind:this={scroller}
		inert={level !== 'text'}
		class="h-full touch-pan-y [scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto overscroll-contain
	       bg-black outline-none
	       focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/25"
		tabindex="0"
		role="region"
		aria-label="The Tanakh as a book. Plus and minus change the size of the text, Enter reads the passage at the middle."
		onscroll={onScroll}
		onclick={onClick}
		onwheel={onWheel}
		onkeydown={onKeydown}
		onpointerdown={onPointerDown}
		onpointermove={(event) => {
			onPointerMove(event);
			if (event.pointerType !== 'touch') pointer = { x: event.clientX, y: event.clientY };
		}}
		onpointerover={onOver}
		onpointerleave={onLeave}
		onpointerup={onPointerUp}
		onpointercancel={onPointerUp}
	>
		<article
			bind:this={column}
			lang="he"
			dir="rtl"
			class="{VERSES} mx-auto w-full max-w-[34em] px-4 pt-[1.5em] pb-[40vh] font-hebrew leading-[1.9]
		       text-white sm:px-8 lg:px-10"
		>
			{#each indices as c (c)}
				{@const text = laid(c) ? texts.get(c) : undefined}
				<section data-chapter={c} style:height={text ? null : `${placeholder(c)}px`}>
					{#if text}
						{#if opensPortion(text)}
							<h2 class="pt-[2.5em] pb-[0.6em] text-[2.1em] leading-tight text-white">
								{text.sectionName}
							</h2>
						{/if}
						{#if text.opens}
							<h3 class="pt-[1.4em] pb-[0.4em] text-[1.7em] leading-tight text-white/78">
								{text.bookName}
							</h3>
						{/if}
						<header
							class="pt-[1.6em] pb-[0.5em] text-[0.82em] text-white/58"
							aria-label={`${text.bookName} ${text.label}`}
						>
							{text.label}
						</header>
						{#each text.verses as verse (verse.index)}
							<p
								data-verse={verse.index}
								class="-mx-[0.4em] mb-[0.35em] grid cursor-pointer grid-cols-[1.6em_1fr] items-baseline rounded px-[0.4em]
								       transition-colors duration-150 hover:bg-white/6 sm:grid-cols-[2.2em_1fr]"
							>
								<span class="text-[0.6em] text-white/58 select-none" aria-hidden="true"
									>{verse.label}</span
								>
								<span>
									{#each verse.parts as part, k (k)}
										{#if part.theirs}<span
												data-form={`${verse.index}.${part.reading}`}
												class={lit === `${verse.index}.${part.reading}` ? 'text-yellow-200' : ''}
												>{part.text}</span
											>{:else if part.marked}<span
												data-form={`${verse.index}.${part.reading}`}
												style={tint(part.slots)}>{part.text}</span
											>{:else if tint(part.slots)}<span style={tint(part.slots)}>{part.text}</span
											>{:else}{part.text}{/if}
									{/each}
								</span>
							</p>
						{/each}
					{/if}
				</section>
			{/each}
		</article>
	</div>

	{#if level === 'chapters'}
		<!-- Every book's chapters, one book a screen, alone and centred: each
		     scroll snaps to the book before or after (a book taller than the
		     screen — the Psalms — scrolls through first); it opens on the book
		     asked for. A portion's
		     name heads its first book, as in the text. Tailwind's type scale,
		     stepped up at each breakpoint, below the books' level as the map's
		     chapter numbers are below its book titles. A chapter is one tap from
		     the text; the one read when the text was left is marked. -->
		<div
			bind:this={above}
			class="[container-type:size] absolute inset-0 touch-pan-y snap-y snap-mandatory [scrollbar-width:thin]
		       [scrollbar-color:rgb(255_255_255/0.25)_transparent] [scrollbar-gutter:stable] overflow-y-auto overscroll-contain bg-black"
			role="navigation"
			aria-label="Chapters"
			onscroll={onChaptersScroll}
			onwheel={onChaptersWheel}
		>
			<div
				lang="he"
				dir="rtl"
				class="{VERSES} mx-auto w-full max-w-[34em] px-4 font-hebrew text-white sm:px-8 lg:px-10"
			>
				{#each contents.books as b (b.index)}
					<section
						data-chapters-of={b.index}
						class="relative flex min-h-[100cqh] snap-start snap-always flex-col justify-center py-8"
					>
						<!-- Its end is a resting place too: a book taller than the
						     screen can stop on its last chapters, by touch or key. -->
						<div class="absolute bottom-0 h-0 w-full snap-end" aria-hidden="true"></div>
						{#if contents.books[b.index - 1]?.section !== b.section}
							<h2
								class="pt-6 pb-4 text-xl leading-tight text-white xl:text-2xl 2xl:text-3xl min-[120rem]:text-4xl"
							>
								{contents.portions[b.section]}{@render beside(bars(0, b.section))}
							</h2>
						{/if}
						<h3
							class="pb-3 text-lg leading-tight text-white/78 xl:text-xl 2xl:text-2xl min-[120rem]:text-3xl"
						>
							{b.name}{@render beside(bars(1, b.index))}
						</h3>
						<ol class="grid grid-cols-6 gap-1 sm:grid-cols-10 xl:grid-cols-12">
							{#each { length: b.end - b.first }, k (k)}
								{@const c = b.first + k}
								<li>
									<button
										type="button"
										data-goes={c}
										aria-current={c === current ? 'location' : undefined}
										aria-label={`${b.name} ${contents.labels[c]}`}
										class={[
											'relative grid h-9 w-full cursor-pointer place-items-center rounded text-base transition-colors duration-150',
											'xl:h-10 xl:text-lg 2xl:h-11 2xl:text-xl min-[120rem]:h-12 min-[120rem]:text-2xl',
											'focus-visible:outline-1 focus-visible:outline-white/60',
											c === current
												? 'bg-white/12 text-white'
												: 'text-white/58 hover:bg-white/6 hover:text-white focus-visible:text-white'
										]}
										onclick={() => go(c)}
									>
										{contents.labels[c]}{@render under(bars(2, c))}
									</button>
								</li>
							{/each}
						</ol>
					</section>
				{/each}
			</div>
		</div>
	{:else if level === 'books'}
		<!-- The portions and every book's name, as the map's framing shows
		     them: Tailwind's type scale, stepped up at each breakpoint, below the
		     portions alone and a portion's name a step above its books', as the
		     map's are. Two columns on a phone, three past it, read from the
		     right, on the verses' column. A long name wraps, never cut; a screen
		     too short scrolls, its scrollbar hidden but its gutter kept, the
		     verses' own, so the column stands where theirs does. A book is one
		     tap from its chapters; the one read when the text was left is
		     marked. -->
		<div
			bind:this={above}
			class="absolute inset-0 [scrollbar-width:thin] [scrollbar-color:transparent_transparent] [scrollbar-gutter:stable]
		       overflow-y-auto overscroll-contain bg-black"
			role="navigation"
			aria-label="Books"
		>
			<div
				lang="he"
				dir="rtl"
				class="{VERSES} mx-auto flex min-h-full w-full max-w-[34em] flex-col justify-center-safe px-4 py-8
				       font-hebrew text-white sm:px-8 lg:px-10"
			>
				{#each contents.portions as portion, p (p)}
					<section data-portion={p} class="pb-6 last:pb-0 xl:pb-8">
						<h2 class="pb-2 text-xl leading-tight xl:text-2xl 2xl:text-3xl min-[120rem]:text-4xl">
							{portion}{@render beside(bars(0, p))}
						</h2>
						<ul class="-mx-2 grid grid-cols-2 gap-x-4 sm:grid-cols-3">
							{#each contents.books.filter((b) => b.section === p) as b (b.index)}
								<li>
									<button
										type="button"
										data-opens={b.index}
										aria-current={b.index === opened ? 'location' : undefined}
										class={[
											'w-full cursor-pointer rounded px-2 py-0.5 text-start text-base leading-snug text-balance break-words',
											'xl:text-lg 2xl:text-xl min-[120rem]:text-2xl',
											'transition-colors duration-150 focus-visible:outline-1 focus-visible:outline-white/60',
											b.index === opened
												? 'bg-white/12 text-white'
												: 'text-white/78 hover:bg-white/6 hover:text-white focus-visible:text-white'
										]}
										onclick={() => openBook(b.index)}
									>
										{b.name}{@render beside(bars(1, b.index))}
									</button>
								</li>
							{/each}
						</ul>
					</section>
				{/each}
			</div>
		</div>
	{:else if level === 'portions'}
		<!-- The three portions alone, rendered as the map's opening draws them:
		     one size, one right edge, the three centred together, the space
		     between two names a little more than their size, nudged down by the
		     difference between the canvas' baseline and a CSS line box. The size is the
		     map's, read off it on common screens and rounded to Tailwind's
		     nearest step at each breakpoint: 48 px below 1280, 60 to 1536, 72
		     to 1920, 96 to 2560, 128 past it. A screen too short scrolls rather
		     than cuts (auto margins). -->
		<div
			bind:this={above}
			class="absolute inset-0 flex [scrollbar-width:none] flex-col items-center overflow-y-auto overscroll-contain bg-black"
			role="navigation"
			aria-label="The three portions"
		>
			<div
				lang="he"
				dir="rtl"
				class="{PORTION_TEXT} my-auto flex translate-y-[0.06em] flex-col items-start gap-[1.14em] py-8 font-hebrew leading-none"
			>
				{#each contents.portions as portion, p (p)}
					<button
						type="button"
						data-portion-button={p}
						aria-current={p === openedPortion ? 'location' : undefined}
						class="relative cursor-pointer rounded text-white focus-visible:outline-1 focus-visible:outline-offset-[0.15em]
						       focus-visible:outline-white/60"
						onclick={() => openPortion(p)}
					>
						{portion}{@render outside(bars(0, p))}
					</button>
				{/each}
			</div>
		</div>
	{/if}
</div>
