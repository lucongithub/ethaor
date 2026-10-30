<script lang="ts">
	// A gematria with nothing to keep: a selection of names and numbers alone
	// holds no verse's letter, so nothing is inked, squared or listed — the
	// owner's rule, as for a count: no row in the panel, this modal once. It
	// reads as a gematria's list does: method after method, its title held
	// at the top while the text scrolls, the text's letters as the method
	// reads them, each share of the value over its letter. Its backdrop, its
	// close button or Escape put it away, and it is gone for good.
	import { onMount } from 'svelte';
	import { formula, tiersFor, type Gematria } from '$lib/gematria';
	import { grouped } from '$lib/marks';
	import { leadingFor, numberedLetters, type Numbered } from '$lib/results';
	import Letters from './Letters.svelte';

	let { text, gematria, onclose }: { text: string; gematria: Gematria; onclose: () => void } =
		$props();

	// Each name and number its line, its letters numbered on from the line
	// before: the n-th letter of the text reads the n-th share.
	const lines = $derived(
		text.split('\n').reduce<Numbered[][]>((all, line) => {
			const before = all.flat().filter((p) => p.n > 0).length;
			return [...all, numberedLetters(line, before)];
		}, [])
	);
	const [standard] = $derived(gematria.methods);

	let dialog = $state<HTMLDialogElement>();

	onMount(() => dialog?.showModal());

	// A click on the backdrop — the dialog itself, never its content — closes it.
	function onclick(event: MouseEvent) {
		if (event.target === dialog) dialog?.close();
	}
</script>

<dialog
	bind:this={dialog}
	dir="rtl"
	aria-labelledby="reading-text"
	aria-describedby="reading-value"
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
		<!-- The selection, on one line, cut with an ellipsis: the header keeps
		     its value and its close button. -->
		<h2
			id="reading-text"
			lang="he"
			class="min-w-0 flex-1 basis-0 truncate font-hebrew text-lg leading-tight text-white"
		>
			{text.split('\n').join(' · ')}
		</h2>
		<p
			id="reading-value"
			dir="ltr"
			lang="en"
			class="order-last basis-full pb-1 text-sm text-white/58 tabular-nums sm:order-none sm:shrink-0 sm:basis-auto sm:pb-0"
		>
			Mispar haPanim {grouped(standard.value)}
		</p>
		<button
			type="button"
			lang="en"
			aria-label="Close"
			class={[
				'grid size-10 shrink-0 cursor-pointer place-items-center rounded bg-white/[0.06] text-white/58 transition-colors duration-150',
				'hover:bg-white/[0.1] hover:text-white focus-visible:text-white',
				'focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60'
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

	<!-- Left to right, so its scrollbar takes the right edge, as the lists'. -->
	<div
		dir="ltr"
		class="[scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto
		       overscroll-contain selection:bg-white/20"
	>
		{#each gematria.methods as method (method.key)}
			{@const tiers = tiersFor(method.shares)}
			<section dir="rtl" aria-labelledby={`reading-${method.key}`}>
				<h3
					id={`reading-${method.key}`}
					class="sticky top-0 z-10 flex items-baseline gap-x-3 border-b border-white/10 bg-black py-2 ps-5 pe-2 sm:ps-7 sm:pe-3"
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
				<div class="px-5 py-5 sm:px-7 sm:py-6">
					<p
						lang="he"
						class={[
							'text-center font-hebrew text-xl [overflow-wrap:anywhere] text-white sm:text-2xl',
							leadingFor(tiers)
						]}
					>
						{#each lines as line, k (k)}<span class="block"
								><Letters words={[line]} {method} {tiers} /></span
							>{/each}
					</p>
					{#if !method.summed}
						<p dir="ltr" lang="en" class="mt-3 text-center text-sm text-white/58 tabular-nums">
							{formula(gematria, method)}
						</p>
					{/if}
				</div>
			</section>
		{/each}
	</div>
</dialog>
