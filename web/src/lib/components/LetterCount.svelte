<script lang="ts">
	// A count of letters with nothing to keep: a selection of names and
	// numbers alone holds no verse's letter, so nothing is inked, squared or
	// listed — the owner's rule: no row in the panel, this modal once, the
	// selection on top and its count under it. Its backdrop, its close button
	// or Escape put it away, and it is gone for good.
	import { onMount } from 'svelte';
	import { grouped } from '$lib/marks';
	import { numberClass, numberedLetters, type Numbered } from '$lib/results';

	let { text, count, onclose }: { text: string; count: number; onclose: () => void } = $props();

	// Each line its letters, numbered on from the line before: every letter
	// counted wears its place in the count over it.
	const lines = $derived(
		text.split('\n').reduce<Numbered[][]>((all, line) => {
			const before = all.flat().filter((p) => p.n > 0).length;
			return [...all, numberedLetters(line, before)];
		}, [])
	);

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
	aria-labelledby="count-text"
	aria-describedby="count-total"
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
		<!-- The selection, each name and number on its line, as it was read,
		     each letter numbered over it, out of the flow: its place in the count. -->
		<p
			id="count-text"
			dir="rtl"
			lang="he"
			class="max-h-[min(50dvh,28rem)] max-w-full [scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto overscroll-contain font-hebrew
			       text-3xl leading-[2.4] [overflow-wrap:anywhere] text-white"
		>
			{#each lines as line, k (k)}<span class="block"
					>{#each line as piece, p (p)}{#if piece.n}<span class="relative"
								>{piece.text}<span class={numberClass(piece.n)} aria-hidden="true"
									>{grouped(piece.n)}</span
								></span
							>{:else}{piece.text}{/if}{/each}</span
				>{/each}
		</p>
		<p id="count-total" class="text-lg text-white/58 tabular-nums">
			{grouped(count)}
			{count === 1 ? 'letter' : 'letters'}
		</p>
	</div>
</dialog>
