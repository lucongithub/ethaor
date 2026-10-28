<script lang="ts">
	// A search's hits, listed as passages: opened by its count in the panel,
	// a modal centred over the view as the passage is — or the searches shown
	// taken together, the passages holding any of them (∪) or every one (∩),
	// opened by the panel's counts of them. Each passage says where
	// it lies — portion, book, chapter — then reads every verse its hits
	// reach, the letters found in their colours: the search listed always in
	// its own, shown on the map or not; the others shown in theirs. Nothing
	// is dimmed — the colours are what the list is for.
	//
	// The passages come a page at a time, as the reader scrolls down to them:
	// a common word has thousands, and pointed Hebrew is heavy to lay out.
	import { onMount } from 'svelte';
	import type { Contents } from '$lib/book-view';
	import { COLORS, grouped, inkStyle, isShown, skipPrefix, type Marked } from '$lib/marks';
	import { inked } from '$lib/passage';
	import { colorsOf, PAGE, stretches, type Passage } from '$lib/results';

	let {
		search,
		join = 'any',
		list,
		passages,
		listed,
		contents,
		read,
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
		contents: Contents;
		/** Passages `from..from + count`. */
		read: (from: number, count: number) => Passage[];
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

	function more() {
		if (shown.length >= passages) return;
		shown = [...shown, ...read(shown.length, PAGE)];
	}

	onMount(() => {
		dialog?.showModal();
		more();
		// The next page once the end of the list nears the bottom of the scroll.
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((e) => e.isIntersecting)) more();
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

	// The letters found, as the passage paints them: one colour, or bands.
	const styleOf = (bits: number) => inkStyle(colorsOf(list, search, bits));

	// A click on the backdrop — the dialog itself, never its content — closes it.
	function onclick(event: MouseEvent) {
		if (event.target === dialog) dialog?.close();
	}
</script>

<dialog
	bind:this={dialog}
	dir="rtl"
	aria-labelledby="results-word"
	aria-describedby="results-count"
	{onclick}
	{onclose}
	class="m-auto max-h-[min(88dvh,56rem)] w-[calc(100%-2rem)] max-w-2xl flex-col overflow-hidden
	       rounded-xl border border-white/15 bg-black p-0 text-white transition-[opacity,scale]
	       duration-200 ease-out backdrop:bg-black/80 open:flex motion-reduce:transition-none
	       lg:max-w-3xl starting:scale-[0.98] starting:opacity-0"
>
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
			class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 font-hebrew text-lg leading-tight sm:flex-none"
		>
			{#each words as m, k (m.id)}
				{#if k > 0}<span
						class="font-sans text-base text-white/45"
						aria-label={join === 'all' ? 'and' : 'or'}>{sign}</span
					>{/if}
				<!-- An equidistant search reads `2_` then its word, left to right:
				     the skip before the Hebrew, the underscore between them; a row
				     of an "every nth letter" run its letter the same way. -->
				{#if skipPrefix(m)}<span dir="ltr"
						><span lang="en" class="font-sans text-sm text-white/58 tabular-nums"
							>{skipPrefix(m)}</span
						><span style:color={COLORS[m.color]}>{m.word}</span></span
					>{:else}<span style:color={COLORS[m.color]}>{m.word}</span>{/if}
			{/each}
		</h2>
		<p
			id="results-count"
			dir="ltr"
			lang="en"
			class="order-last basis-full pb-1 text-sm text-white/58 tabular-nums sm:order-none sm:me-auto sm:min-w-0 sm:basis-auto sm:truncate sm:pb-0"
		>
			<!-- One search: its hits and passages, named. The searches together:
			     the count bare, the owner's rule — the screen reader still hears
			     the word. -->
			{#if search}
				{plural(search.total ?? 0, 'hit')} | {plural(
					passages,
					'passage'
				)}{#if search.total !== null && listed !== undefined && listed < search.total}
					| first {grouped(listed)} listed{/if}
			{:else}
				{grouped(passages)}<span class="sr-only"> {passages === 1 ? 'passage' : 'passages'}</span>
			{/if}
		</p>
		<button
			type="button"
			lang="en"
			aria-label="Close"
			class="grid size-10 shrink-0 cursor-pointer place-items-center rounded text-white/58 transition-colors duration-150
			       hover:bg-white/[0.06] hover:text-white focus-visible:text-white
			       focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
			onclick={() => dialog?.close()}
		>
			<svg viewBox="0 0 256 256" class="size-5 fill-current" aria-hidden="true">
				<path
					d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
				/>
			</svg>
		</button>
	</header>

	<div
		class="[scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto
		       overscroll-contain selection:bg-white/20"
	>
		<ol class="divide-y divide-white/10">
			{#each shown as passage, p (p)}
				<li class="px-5 py-5 sm:px-7 sm:py-6">
					{#each stretches(passage, contents) as stretch, g (g)}
						<!-- Where it lies, as the breadcrumb names it — said again
						     wherever a word read far apart reaches another chapter,
						     book or portion. -->
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
							{@const runs = inked(verse.text, verse.found)}
							<!-- The book's verse plate under the pointer: what a click reads. -->
							<div
								role="button"
								tabindex="0"
								class="-mx-[0.4em] cursor-pointer rounded px-[0.4em] font-hebrew text-lg leading-loose text-pretty text-white
							       transition-colors duration-150 outline-none hover:bg-white/6 focus-visible:bg-white/6
							       focus-visible:outline-1 focus-visible:outline-white/60 sm:text-xl"
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
								{#each runs as word, k (k)}{#each word as run, r (r)}{@const style = run.slots
											? styleOf(run.slots)
											: ''}{#if style}<span {style}>{run.text}</span
											>{:else}{run.text}{/if}{/each}{k < runs.length - 1 ? ' ' : ''}{/each}
							</div>
						{/each}
					{/each}
				</li>
			{/each}
		</ol>
		<div bind:this={end} aria-hidden="true">
			{#if shown.length < passages}
				<p dir="ltr" lang="en" class="px-5 py-6 text-center text-xs text-white/45 sm:px-7">
					{grouped(passages - shown.length)} more
				</p>
			{/if}
		</div>
	</div>
</dialog>
