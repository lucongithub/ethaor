<script lang="ts">
	// A search, opened by its word in the panel: the word on top, then its
	// colour beside its name as the row's tip gives it, then its count — the
	// owner's rule. A modal centred over the view, as a search's passages are;
	// its backdrop, its close button or Escape put it away.
	import { onMount } from 'svelte';
	import { COLORS, COUNT, grouped, searchName, shownWord, unitOf, type Marked } from '$lib/marks';

	let { search, onclose }: { search: Marked; onclose: () => void } = $props();

	const color = $derived(COLORS[search.color]);
	// A count's selection may run to whole chapters: read as a passage is,
	// flush right, scrolling in its own box under a fixed name and count.
	const counting = $derived(search.kind === COUNT);
	const plural = (n: number, one: string) => `${grouped(n)} ${one}${n === 1 ? '' : 's'}`;

	let dialog = $state<HTMLDialogElement>();

	onMount(() => dialog?.showModal());

	// A click on the backdrop — the dialog itself, never its content — closes it.
	function onclick(event: MouseEvent) {
		if (event.target === dialog) dialog?.close();
	}
</script>

<dialog
	bind:this={dialog}
	dir="ltr"
	lang="en"
	aria-labelledby="info-name"
	aria-describedby="info-count"
	{onclick}
	{onclose}
	class="relative m-auto w-fit max-w-[min(calc(100%-2rem),48rem)] min-w-[min(calc(100%-2rem),20rem)] overflow-hidden rounded-xl border border-white/15 bg-black
	       p-0 text-white transition-[opacity,scale] duration-200 ease-out backdrop:bg-black/80
	       motion-reduce:transition-none starting:scale-[0.98] starting:opacity-0"
>
	<button
		type="button"
		aria-label="Close"
		class={[
			'absolute top-2 right-2 grid size-9 cursor-pointer place-items-center rounded text-white/45 transition-colors duration-150',
			'hover:bg-white/[0.06] hover:text-white focus-visible:text-white',
			'focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60'
		]}
		onclick={() => dialog?.close()}
	>
		<svg viewBox="0 0 256 256" class="size-4 fill-current" aria-hidden="true">
			<path
				d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
			/>
		</svg>
	</button>

	<div class="flex flex-col items-center gap-4 px-8 pt-12 pb-10 text-center">
		<!-- The word, whole size: the dialog widens to hold it, and past the
		     screen's width it wraps onto more lines, never shrunk or cut. A
		     count's text, each name and verse on its line, scrolls in its box;
		     focusable, so the keys scroll it too. -->
		{#if counting}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex — a scrolled region the keys must reach -->
			<div
				dir="rtl"
				lang="he"
				tabindex="0"
				role="region"
				aria-label="The text counted"
				class="max-h-[min(50dvh,28rem)] w-[min(40rem,calc(100vw-6rem))] max-w-full [scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent]
				       overflow-y-auto overscroll-contain
				       rounded text-start font-hebrew text-xl leading-loose text-white
				       selection:bg-white/25 focus-visible:outline-1 focus-visible:outline-offset-4
				       focus-visible:outline-white/60 sm:text-2xl"
			>
				{#each search.word.split('\n') as line, k (k)}<p>{line}</p>{/each}
			</div>
		{:else}
			<p
				dir="rtl"
				lang="he"
				class="max-w-full font-hebrew text-3xl leading-tight [overflow-wrap:anywhere] text-white"
			>
				{shownWord(search)}
			</p>
		{/if}
		<!-- What follows takes the word's width and adds none: `w-0 min-w-full`
		     keeps a long name from widening the dialog — it wraps instead. -->
		<div class="flex w-0 min-w-full flex-col items-center gap-4">
			<p class="flex max-w-full items-center justify-center gap-3">
				<span class="size-6 shrink-0 rounded" style:background-color={color} aria-hidden="true"
				></span>
				<span id="info-name" class="min-w-0 text-2xl leading-snug text-balance text-white">
					{searchName(search)}
				</span>
			</p>
			<p id="info-count" class="text-lg text-white/58 tabular-nums">
				{#if search.passages === null}
					searching…
				{:else}
					{plural(search.total ?? 0, unitOf(search))} · {plural(search.passages, 'passage')}
				{/if}
			</p>
		</div>
	</div>
</dialog>
