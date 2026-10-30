<script lang="ts">
	// A search's hits, listed as passages: opened by its row in the panel, a
	// modal centred over the view as the passage is, its kind small on top —
	// or the searches shown
	// taken together, the passages holding any of them (∪) or two at least
	// (∩), opened by the panel's counts of them — ∩ cut by crossing, the most
	// searches first, each under a title naming them; ∪ one list in the
	// order of the text. Each passage says where
	// it lies — portion, book, chapter — then reads every verse its hits
	// reach, the letters found in their colours: the search listed always in
	// its own, shown on the map or not; the others shown in theirs. Nothing
	// is dimmed — the colours are what the list is for.
	//
	// The passages come a page at a time, as the reader scrolls down to them:
	// a common word has thousands, and pointed Hebrew is heavy to lay out.
	//
	// A gematria's list reads its passage, then the passage again under each
	// method, a few at a time: its letters as the method reads them, each
	// share of the value over its letter, the value in the method's title.
	import { onMount, tick } from 'svelte';
	import type { Contents } from '$lib/book-view';
	import { formula, tiersFor, type Gematria } from '$lib/gematria';
	import {
		COLORS,
		COUNT,
		grouped,
		inkStyle,
		isSelection,
		isShown,
		searchName,
		shownWord,
		skipPrefix,
		totalOf,
		type Marked
	} from '$lib/marks';
	import { inked } from '$lib/passage';
	import Letters from './Letters.svelte';
	import {
		colorsOf,
		countedWords,
		crossed,
		crossingsTitled,
		leadingFor,
		numbered,
		OWN,
		PAGE,
		stretches,
		type Crossing,
		type Numbered,
		type Passage
	} from '$lib/results';

	let {
		search,
		join = 'any',
		list,
		passages,
		listed,
		crossings,
		contents,
		read,
		gematria = null,
		onselect,
		onclose
	}: {
		/** The search listed; `null` for the searches shown, together. */
		search: Marked | null;
		/** The searches shown together: passages holding any of them, or all. */
		join?: 'any' | 'all';
		/** Every search kept: the colours of the others shown. */
		list: Marked[];
		/** How many passages the hits make. */
		passages: number;
		/** Hits the list holds: fewer than the total past the module's cap. */
		listed?: number;
		/** The searches together: the list's crossings, the widest first. */
		crossings?: Crossing[];
		contents: Contents;
		/** Passages `from..from + count`. */
		read: (from: number, count: number) => Passage[];
		/** A gematria's reading by every method, for its list. */
		gematria?: Gematria | null;
		/** A verse clicked, by its global index: its passage opens, as on the map. */
		onselect: (verse: number) => void;
		onclose: () => void;
	} = $props();

	// A click reads the verse's passage, as on the map and in the book; a
	// drag, which selects text, stays the browser's.
	function pick(verse: number) {
		if (!(getSelection()?.isCollapsed ?? true)) return;
		onselect(verse);
	}

	let dialog = $state<HTMLDialogElement>();
	let end = $state<HTMLElement>();
	let shown = $state<Passage[]>([]);
	// A gematria's methods laid so far, after its passages.
	let methods = $state(0);
	const METHODS_AT_ONCE = 4;

	// The next page — of passages, then of a gematria's methods; whether
	// anything was added.
	function more(): boolean {
		if (shown.length < passages) {
			shown = [...shown, ...read(shown.length, PAGE)];
			return true;
		}
		if (gematria && methods < gematria.methods.length) {
			methods += METHODS_AT_ONCE;
			return true;
		}
		return false;
	}

	onMount(() => {
		dialog?.showModal();
		more();
		// The next page once the end of the list nears the bottom of the scroll
		// — and again while it stays near once laid: the observer only says
		// when it comes near, and a short page leaves it there.
		const observer = new IntersectionObserver(
			(entries) => {
				if (!entries.some((e) => e.isIntersecting) || !more() || !end) return;
				const at = end;
				tick().then(() => {
					observer.unobserve(at);
					observer.observe(at);
				});
			},
			{ root: end?.parentElement, rootMargin: '0px 0px 600px 0px' }
		);
		if (end) observer.observe(end);
		return () => observer.disconnect();
	});

	// What the list is of: one search, or the shown ones joined by their sign.
	const words = $derived(search ? [search] : list.filter(isShown));
	const sign = $derived(join === 'all' ? '∩' : '∪');
	const plural = (n: number, one: string) => `${grouped(n)} ${one}${n === 1 ? '' : 's'}`;

	// The list cut by crossing, each under its title; one search's list is
	// one section, untitled.
	// A crossing's own title only among several: one alone would repeat the
	// list's title above it (`crossingsTitled`).
	const titled = $derived(crossingsTitled(crossings ?? []));
	const sections = $derived<Crossing[]>(
		crossings?.length ? crossings : [{ slots: 0, passages, from: 0 }]
	);

	// A count's list numbers each of its letters, over it, in reading order
	// across the passage; any other's verses are cut into their runs alone.
	const counting = $derived(search?.kind === COUNT);
	// A selection kept — a count, a gematria — colours its own letters alone.
	const selection = $derived(search !== null && isSelection(search));

	// The letters found, as the passage paints them: one colour, or bands.
	// A selection's list colours its own letters alone — the others' would
	// only stand between the reader and the numbers.
	const styleOf = (bits: number) =>
		selection
			? bits & OWN && search
				? inkStyle([search.color])
				: ''
			: inkStyle(colorsOf(list, search, bits));
	const cut = (passage: Passage): Numbered[][][] =>
		counting
			? numbered(passage)
			: passage.map((verse) =>
					inked(verse.text, verse.found).map((word) => word.map((run) => ({ ...run, n: 0 })))
				);

	// A selection may run long: in a title it holds one line, cut with an
	// ellipsis, so the header keeps its count and its close button.
	const clamp = (m: Marked) => (isSelection(m) ? 'block min-w-0 truncate' : '');

	// A click on the backdrop — the dialog itself, never its content — closes it.
	function onclick(event: MouseEvent) {
		if (event.target === dialog) dialog?.close();
	}
</script>

{#snippet reading(passage: Passage, pieces: Numbered[][][])}
	{#each stretches(passage, contents) as stretch, g (g)}
		<!-- Where it lies, as the breadcrumb names it — said again wherever a
		     word read far apart reaches another chapter, book or portion. -->
		<p
			class={[
				'mb-2 flex flex-wrap items-baseline gap-x-2 font-hebrew text-sm sm:text-base',
				g > 0 && 'mt-4'
			]}
		>
			<span class="text-white/58">{stretch.place.portion}</span>
			<span class="text-white/25" aria-hidden="true">›</span>
			<span class="text-white/58">{stretch.place.book}</span>
			<span class="text-white/25" aria-hidden="true">›</span>
			<span class="text-white/58">{stretch.place.chapter}</span>
		</p>
		{#each stretch.verses as verse (verse.verse)}
			<!-- The book's verse plate under the pointer: what a click reads. A
			     numbered letter wears its number over it, out of the flow — its
			     place in the count, or its share of a method's value — the
			     letters spaced as the text is. -->
			<div
				role="button"
				tabindex="0"
				class={[
					'-mx-[0.4em] cursor-pointer rounded px-[0.4em] font-hebrew text-pretty text-white',
					'transition-colors duration-150 outline-none hover:bg-white/6 focus-visible:bg-white/6',
					'focus-visible:outline-1 focus-visible:outline-white/60',
					counting ? ['text-xl sm:text-2xl', leadingFor(2)] : 'text-lg leading-loose sm:text-xl'
				]}
				onclick={() => pick(verse.verse)}
				onkeydown={(event) => {
					if (event.key !== 'Enter' && event.key !== ' ') return;
					event.preventDefault();
					onselect(verse.verse);
				}}
			>
				<span class="me-1 align-[0.15em] text-[0.7em] text-white/45 select-none"
					>{verse.number}</span
				>
				<Letters words={pieces[passage.indexOf(verse)] ?? []} style={styleOf} />
			</div>
		{/each}
	{/each}
{/snippet}

<dialog
	bind:this={dialog}
	dir="rtl"
	aria-labelledby="results-word"
	aria-describedby={search ? 'results-kind results-count' : 'results-count'}
	{onclick}
	{onclose}
	class="m-auto max-h-[min(88dvh,56rem)] w-[calc(100%-2rem)] max-w-2xl flex-col overflow-hidden
	       rounded-xl border border-white/15 bg-black p-0 text-white transition-[opacity,scale]
	       duration-200 ease-out backdrop:bg-black/80 open:flex motion-reduce:transition-none
	       lg:max-w-3xl starting:scale-[0.98] starting:opacity-0"
>
	<!-- What the search is, small over its word: the row's tip, kept in sight.
	     The searches together are no one kind. -->
	{#if search}
		<p
			id="results-kind"
			dir="ltr"
			lang="en"
			class="shrink-0 border-b border-white/10 bg-white/[0.06] px-5 py-2 text-center text-xs text-balance text-white"
		>
			{searchName(search)}
		</p>
	{/if}
	<header
		class="flex shrink-0 flex-wrap items-center gap-x-3 border-b border-white/10 py-2 ps-5 pe-2 sm:flex-nowrap sm:ps-7 sm:pe-3"
	>
		{#if search}
			<span
				class="size-3 shrink-0 rounded-[2px]"
				style:background-color={COLORS[search.color]}
				aria-hidden="true"
			></span>
		{/if}
		<!-- The words, each in its colour; together, joined by the sign the
		     panel's count wears. -->
		<h2
			id="results-word"
			lang="he"
			class={[
				'flex min-w-0 flex-1 items-baseline gap-x-2 font-hebrew text-lg leading-tight',
				selection ? 'basis-0' : 'flex-wrap sm:flex-none'
			]}
		>
			{#each words as m, k (m.id)}
				{#if k > 0}<span
						class="font-sans text-base text-white/45"
						aria-label={join === 'all' ? 'and' : 'or'}>{sign}</span
					>{/if}
				<!-- A row of an "every nth letter" run reads `2_` then its word,
				     left to right: the letter before the Hebrew, the underscore
				     between them. An equidistant search reads its word alone, no
				     skip, and so do the searches together (∪, ∩) — the owner's
				     rule. -->
				{#if search && skipPrefix(m)}<span dir="ltr"
						><span lang="en" class="font-sans text-sm text-white/58 tabular-nums"
							>{skipPrefix(m)}</span
						><span style:color={COLORS[m.color]}>{shownWord(m)}</span></span
					>{:else}<span class={clamp(m)} style:color={COLORS[m.color]}>{shownWord(m)}</span>{/if}
			{/each}
		</h2>
		<p
			id="results-count"
			dir="ltr"
			lang="en"
			class={selection
				? 'order-last basis-full pb-1 pl-3 text-center text-sm text-white/58 tabular-nums sm:order-none sm:shrink-0 sm:basis-auto sm:pb-0 sm:pl-0 sm:text-start'
				: search
					? 'order-last basis-full pb-1 pl-3 text-center text-sm text-white/58 tabular-nums sm:order-none sm:me-auto sm:min-w-0 sm:basis-auto sm:truncate sm:pb-0 sm:pl-0 sm:text-start'
					: 'order-first shrink-0 text-sm text-white/58 tabular-nums'}
		>
			<!-- On its own line, on a small screen, it is centred on the frame:
			     `pl-3` evens out the header's two paddings. One search: its hits
			     and passages, named, after its word. The
			     searches together: the count bare, the owner's rule — the screen
			     reader still hears the word — and before the words, the owner's
			     rule, as each crossing's count below. -->
			{#if search}
				{totalOf(search)} | {plural(
					passages,
					'passage'
				)}{#if !selection && search.total !== null && listed !== undefined && listed < search.total}
					| first {grouped(listed)} listed{/if}
			{:else}
				{grouped(passages)}<span class="sr-only"> {passages === 1 ? 'passage' : 'passages'}</span>
			{/if}
		</p>
		<button
			type="button"
			lang="en"
			aria-label="Close"
			class={[
				'grid size-10 shrink-0 cursor-pointer place-items-center rounded bg-white/[0.06] text-white/58 transition-colors duration-150',
				'hover:bg-white/[0.1] hover:text-white focus-visible:text-white',
				'focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60',
				// The words no longer pushed by a count after them: the button
				// keeps the far end itself.
				!search && 'ms-auto'
			]}
			onclick={() => dialog?.close()}
		>
			<svg viewBox="0 0 256 256" class="size-5 fill-current" aria-hidden="true">
				<path
					d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
				/>
			</svg>
		</button>
	</header>

	<!-- Left to right, so its scrollbar takes the right edge: on the left it
	     pushed every crossing's count off the close button's edge. -->
	<div
		dir="ltr"
		class="[scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto
		       overscroll-contain selection:bg-white/20"
	>
		{#each sections as section (section.from)}
			{#if section.from < shown.length}
				<section
					dir="rtl"
					aria-labelledby={section.slots && titled ? `crossing-${section.slots}` : undefined}
				>
					<!-- A crossing's title, over its passages, held at the top while
					     they scroll: the searches its passages all hold, each in its
					     colour, joined by ∩ — the widest first — after how many, the
					     owner's rule: the count first, on the right where it reads. -->
					{#if section.slots && titled}
						<h3
							id={`crossing-${section.slots}`}
							class="sticky top-0 z-10 flex items-baseline gap-x-3 border-y border-white/10 bg-black py-2 ps-5 pe-2 sm:ps-7 sm:pe-3"
						>
							<span dir="ltr" lang="en" class="shrink-0 text-xs text-white/58 tabular-nums"
								>{grouped(section.passages)}<span class="sr-only">
									{section.passages === 1 ? 'passage' : 'passages'}</span
								></span
							>
							<span
								lang="he"
								class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 font-hebrew text-base leading-tight"
							>
								{#each crossed(list, section.slots) as m, k (m.id)}
									{#if k > 0}<span class="font-sans text-sm text-white/45" aria-label="and">∩</span
										>{/if}<span class={clamp(m)} style:color={COLORS[m.color]}>{shownWord(m)}</span>
								{/each}
							</span>
						</h3>
					{/if}
					<ol class="divide-y divide-white/10">
						{#each shown.slice(section.from, section.from + section.passages) as passage, p (section.from + p)}
							<li class="px-5 py-5 sm:px-7 sm:py-6">
								{@render reading(passage, cut(passage))}
							</li>
						{/each}
					</ol>
				</section>
			{/if}
		{/each}
		<!-- A gematria, method after method: its title held at the top while
		     it scrolls — the value first, the method's names at the line's far
		     end, the owner's rule — then the letters counted alone, word by
		     word, never the whole passage: each as the method reads it, its
		     share of the value over it; a method no sum of its letters says how
		     it reaches its value under them. -->
		{#if gematria && shown.length >= passages}
			{#each gematria.methods.slice(0, methods) as method (method.key)}
				{@const tiers = tiersFor(method.shares)}
				<section dir="rtl" aria-labelledby={`method-${method.key}`}>
					<h3
						id={`method-${method.key}`}
						class="sticky top-0 z-10 flex items-baseline gap-x-3 border-y border-white/10 bg-black py-2 ps-5 pe-2 sm:ps-7 sm:pe-3"
					>
						<span dir="ltr" lang="en" class="shrink-0 text-base text-white tabular-nums"
							>{grouped(method.value)}</span
						>
						<span class="ms-auto flex min-w-0 items-baseline gap-x-3">
							<span lang="he" class="shrink-0 font-hebrew text-base leading-tight text-white"
								>{method.hebrew}</span
							>
							<span dir="ltr" lang="en" class="min-w-0 truncate text-xs text-white/58"
								>{method.name}</span
							>
						</span>
					</h3>
					<ol class="divide-y divide-white/10">
						{#each shown as passage, p (p)}
							<li class="px-5 py-5 sm:px-7 sm:py-6">
								<p
									lang="he"
									class={[
										'text-center font-hebrew text-xl [overflow-wrap:anywhere] text-white sm:text-2xl',
										leadingFor(tiers)
									]}
								>
									<Letters
										words={countedWords(numbered(passage))}
										{method}
										style={styleOf}
										{tiers}
									/>
								</p>
								{#if !method.summed}
									<p
										dir="ltr"
										lang="en"
										class="mt-3 text-center text-sm text-white/58 tabular-nums"
									>
										{formula(gematria, method)}
									</p>
								{/if}
							</li>
						{/each}
					</ol>
				</section>
			{/each}
		{/if}
		<div bind:this={end} aria-hidden="true">
			{#if shown.length < passages}
				<p dir="ltr" lang="en" class="px-5 py-6 text-center text-xs text-white/45 sm:px-7">
					{grouped(passages - shown.length)} more
				</p>
			{:else if gematria && methods < gematria.methods.length}
				<p dir="ltr" lang="en" class="px-5 py-6 text-center text-xs text-white/45 sm:px-7">
					{plural(gematria.methods.length - methods, 'more method')}
				</p>
			{/if}
		</div>
	</div>
</dialog>
