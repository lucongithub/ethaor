<script lang="ts">
	import { base } from '$app/paths';
	import { onMount } from 'svelte';
	import { createTanakhMap, type Focus, type Selection, type TanakhMap } from '$lib/ethaor';

	let canvas: HTMLCanvasElement;
	let map = $state<TanakhMap | null>(null);
	let selection = $state<Selection | null>(null);
	let focus = $state<Focus | null>(null);
	let error = $state<string | null>(null);

	onMount(() => {
		let handle: TanakhMap | null = null;

		createTanakhMap(canvas, {
			wasmUrl: `${base}/ethaor.wasm`,
			onSelect: (hit) => (selection = hit),
			onFocus: (where) => (focus = where)
		})
			.then((created) => {
				handle = created;
				map = created;
			})
			.catch((cause) => {
				error = cause instanceof Error ? cause.message : String(cause);
			});

		return () => handle?.destroy();
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape') return;
		selection = null;
		map?.reset();
	}
</script>

<svelte:window on:keydown={onKeydown} />

<div class="flex h-full flex-col bg-black">
	<!-- Fil d'Ariane. Les bandes de titre restent dans la carte et ne recouvrent
	     donc jamais rien ; quand elles sortent de l'écran, c'est lui qui dit où
	     l'on se trouve. Il occupe sa propre rangée : aucun pixel de carte perdu. -->
	<header
		class="flex h-9 shrink-0 items-center gap-2 border-b border-white/10 px-3
		       pt-[env(safe-area-inset-top)] text-sm"
	>
		{#if focus}
			<span class="font-hebrew text-white/50">{focus.section}</span>
			{#if focus.book}
				<span class="text-white/25">›</span>
				<span class="font-hebrew text-white/90">{focus.book}</span>
			{/if}
			{#if focus.chapter}
				<span class="text-white/25">›</span>
				<span class="font-hebrew text-white/60">{focus.chapter}</span>
			{/if}
		{/if}
	</header>

	<!-- `touch-action: none` est indispensable : sans lui le navigateur mobile
	     intercepte le pincement et le glisser avant nous. -->
	<canvas bind:this={canvas} class="min-h-0 w-full flex-1 touch-none bg-black"></canvas>
</div>

{#if !map && !error}
	<div
		class="pointer-events-none fixed inset-0 grid place-content-center text-sm tracking-widest text-white/55"
		dir="ltr"
	>
		chargement du corpus…
	</div>
{/if}

{#if error}
	<div
		class="fixed inset-0 grid place-content-center gap-2 p-6 text-center text-sm text-white/70"
		dir="ltr"
	>
		<p class="text-white">Le module n’a pas pu être chargé.</p>
		<p class="font-mono text-xs text-white/50">{error}</p>
		<p class="text-xs text-white/50">
			Compiler puis copier&nbsp;: <code class="text-white/70">npm run wasm</code>
		</p>
	</div>
{/if}

{#if selection}
	<aside
		class="fixed inset-x-3 bottom-[calc(1rem+env(safe-area-inset-bottom))] max-h-[45vh] overflow-auto
		       rounded-xl border border-white/15 bg-black/90 p-4 backdrop-blur
		       sm:inset-x-auto sm:top-[calc(3rem+env(safe-area-inset-top))] sm:bottom-auto
		       sm:right-[calc(0.75rem+env(safe-area-inset-right))] sm:max-h-[60vh] sm:w-[26rem]"
	>
		<button
			type="button"
			dir="ltr"
			aria-label="Fermer"
			class="absolute top-1 left-2 cursor-pointer px-2 text-xl leading-none text-white/55 hover:text-white"
			onclick={() => (selection = null)}
		>
			×
		</button>

		<p dir="ltr" class="mb-2 text-xs tracking-wider text-white/55">
			livre {selection.book + 1} · chapitre {selection.chapter + 1}
		</p>
		<p class="font-hebrew text-[22px] leading-loose">{selection.verse}</p>
	</aside>
{/if}
