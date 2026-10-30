<script lang="ts">
	// A text's words, each cut into its runs, a numbered letter wearing its
	// number over it, out of the flow, so the letters keep the text's own
	// spacing: its place in a count — or, read by a gematria method, the
	// letter the method reads it as and its share of the value
	// (`letterAs`). A run found by searches wears their colours (`style`).
	import { letterAs, type Method } from '$lib/gematria';
	import { numberClass, type Numbered } from '$lib/results';

	let {
		words,
		method = null,
		style = () => '',
		tiers = 2
	}: {
		words: Numbered[][];
		method?: Method | null;
		/** The colours of a run found, by its slots' bits. */
		style?: (slots: number) => string;
		/** Tiers the numbers take: more for longer ones (`tiersFor`). */
		tiers?: number;
	} = $props();
</script>

{#each words as word, k (k)}{#each word as run, r (r)}{@const ink = run.slots
			? style(run.slots)
			: ''}{#if run.n}{@const letter = letterAs(method, run)}<span class="relative"
				><span style={ink}>{letter.text}</span><span
					class={numberClass(run.n, tiers)}
					aria-hidden="true">{letter.over}</span
				></span
			>{:else if ink}<span style={ink}>{run.text}</span>{:else}{run.text}{/if}{/each}{k <
	words.length - 1
		? ' '
		: ''}{/each}
