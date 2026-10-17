<script lang="ts" module>
	/** Where the reader is in the book: what the breadcrumb and the legend say. */
	export interface Reading {
		place: { chapter: number; verse: number };
		section: string;
		book: string;
		chapter: string;
		verse: string;
	}
</script>

<script lang="ts">
	// The Tanakh read as a book: one column, scrolled, the browser laying the
	// text out — shaping, wrapping, selecting, finding — as on any web page.
	// Zoom changes the text's size, not a camera: the column reflows, and the
	// verse under the eye stays where it was.
	//
	// The 929 chapters are all in the page, but only those near the screen are
	// laid out; the others keep their height — measured once laid, estimated
	// before — so the scroll never moves under the reader.
	import { onMount, tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Book, Place } from '$lib/ethaor';
	import { BYTES_PER_VERSE, estimate, kept, SIZE, zoomed, type Chapter } from '$lib/book-view';
	import { escalate, reporter, type Fault } from '$lib/fault';

	let {
		book,
		start,
		landed,
		onreading,
		onform,
		onfault
	}: {
		book: Book;
		start: Place;
		/** Bumped whenever verses land: the chapters near the screen are read again. */
		landed: number;
		onreading: (reading: Reading | null) => void;
		/** The pointer is on one of the two forms, ours `¹` or theirs `[…]²`, or has left it. */
		onform: (on: boolean) => void;
		onfault: (fault: Fault) => void;
	} = $props();

	/** Chapters laid out beyond the first and last on screen, each way. */
	const AROUND = 2;
	const STORED = 'ethaor.book.size';

	let scroller: HTMLDivElement;
	let column: HTMLElement;
	let size = $state(SIZE);
	let width = $state(640);
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
	}

	// Verses landed: what was missing comes, and the readings with the last.
	$effect(() => {
		if (landed < 0) return;
		fill(true);
		void tick().then(land);
	});

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
		const at = centre();
		return at ? { chapter: at.chapter, verse: at.verse } : start;
	}

	function centre() {
		if (!scroller) return null;
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
		const reading: Reading = {
			place: at,
			section: text.sectionName,
			book: text.bookName,
			chapter: text.label,
			verse: verse?.label ?? ''
		};
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
	// word, theirs in brackets — never for the chapter around them.
	let onForm = false;
	let pointer: { x: number; y: number } | null = null;
	function form(under: Element | null) {
		const on = under?.closest('[data-form]') != null;
		if (on === onForm) return;
		onForm = on;
		onform(on);
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

	// ------------------------------------------------------------------ zoom

	/**
	 * The text's size changes and the column reflows; the verse at `y` stays
	 * at `y`. Heights measured at the old size are forgotten.
	 */
	async function resize(steps: number, y: number) {
		const next = zoomed(size, steps);
		if (next === size) return;
		const box = scroller.getBoundingClientRect();
		const x = column.getBoundingClientRect();
		const anchor =
			document.elementFromPoint(x.left + x.width / 2, y)?.closest<HTMLElement>('[data-verse]') ??
			document
				.elementFromPoint(x.left + x.width / 2, box.top + box.height / 2)
				?.closest<HTMLElement>('[data-verse]');
		const before = anchor?.getBoundingClientRect().top ?? 0;
		size = next;
		heights.clear();
		try {
			localStorage.setItem(STORED, String(next));
		} catch {
			// No storage: the size lasts the visit.
		}
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

	function onKeydown(event: KeyboardEvent) {
		if (event.ctrlKey || event.metaKey || event.altKey) return;
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
			if (stored) size = zoomed(stored, 0);
		} catch {
			// No storage: the default size.
		}
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
		let applying = 0;
		measured = new ResizeObserver((entries) => {
			for (const entry of entries) {
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
				columnMoved = false;
			});
		});
		measured.observe(column);
		for (const section of scroller.querySelectorAll<HTMLElement>('[data-chapter]')) {
			visible.observe(section);
			measured.observe(section);
		}

		target = start;
		span = kept(start.chapter, start.chapter, AROUND, book.count);
		fill();
		void tick().then(() => {
			land();
			scroller.focus({ preventScroll: true });
		});

		return () => {
			visible.disconnect();
			measured.disconnect();
			cancelAnimationFrame(scheduled);
		};
	});

	const laid = (chapter: number) => chapter >= span.from && chapter < span.to && texts.has(chapter);
	const opensPortion = (text: Chapter) => text.opens && [0, 5, 26].includes(text.book);
</script>

<!-- `touch-action: pan-y`: one finger scrolls natively, two size the text.
     Focusable, as a scrolled region must be for the keys to scroll it, and
     named with its keys like the map's canvas. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
	bind:this={scroller}
	class="h-full touch-pan-y [scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto overscroll-contain
	       bg-black outline-none
	       focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/25"
	style:--book-size="{size}px"
	tabindex="0"
	role="region"
	aria-label="The Tanakh as a book. Plus and minus change the size of the text."
	onscroll={onScroll}
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
		class="mx-auto w-full max-w-[34em] px-4 pt-[1.5em] pb-[40vh] font-hebrew text-[length:var(--book-size)] leading-[1.9]
		       text-white sm:px-8"
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
							class="mb-[0.35em] grid grid-cols-[1.6em_1fr] items-baseline sm:grid-cols-[2.2em_1fr]"
						>
							<span class="text-[0.6em] text-white/58 select-none" aria-hidden="true"
								>{verse.label}</span
							>
							<span>
								{#each verse.parts as part, k (k)}
									{#if part.theirs}<span data-form class="text-yellow-200">{part.text}</span
										>{:else if part.marked}<span data-form>{part.text}</span>{:else}{part.text}{/if}
								{/each}
							</span>
						</p>
					{/each}
				{/if}
			</section>
		{/each}
	</article>
</div>
