<script lang="ts">
	import { onMount } from 'svelte';
	import { createTanakhMap, type Focus, type Selection, type TanakhMap } from '$lib/ethaor';
	import { escalate, reporter, watch, type Fault } from '$lib/fault';

	let canvas: HTMLCanvasElement;
	let map = $state<TanakhMap | null>(null);
	let selection = $state<Selection | null>(null);
	let focus = $state<Focus | null>(null);
	let readings = $state(0);
	// What the user is told: the map stopped (`fatal`, the whole screen,
	// and a way back), or something failed that the map lives through (a
	// line under it, closed at will). Each with the number to quote; every
	// fault reaches the log whether shown or not.
	let fatal = $state<Fault | null>(null);
	let notice = $state<Fault | null>(null);

	function shown(fault: Fault) {
		if (fault.fatal) fatal = fault;
		else notice = fault;
	}

	onMount(() => {
		let handle: TanakhMap | null = null;
		const unwatch = watch(window, shown);

		createTanakhMap(canvas, {
			onSelect: (hit) => (selection = hit),
			onFocus: (where) => (focus = where),
			onReadings: (count) => (readings = count),
			onFault: shown
		})
			.then((created) => {
				handle = created;
				map = created;
			})
			.catch((cause) => {
				const fault = escalate(cause, 'map-unloaded', 'loading the map');
				reporter.report(fault);
				shown(fault);
			});

		return () => {
			unwatch();
			handle?.destroy();
		};
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape') return;
		selection = null;
		map?.reset();
	}
</script>

<svelte:window on:keydown={onKeydown} />

<div class="flex h-full flex-col bg-black">
	<!-- Breadcrumb. The title bands stay inside the map and therefore never
	     cover anything; when they leave the screen, it is this that says where
	     one is. It takes its own row: not one pixel of map lost.

	     Its depth is the module's: a portion and a book at the framing, the
	     chapter once the numbers are out, the verse once the lines are read.
	     Its place is the pointer as soon as there is one — hovering a title, a
	     chapter or a line says where **that** is. -->
	<header
		class="flex h-[calc(var(--nav)+env(safe-area-inset-top))] shrink-0 items-center gap-2
		       border-b border-white/10 px-3 pt-[env(safe-area-inset-top)]
		       text-sm sm:px-4 sm:text-base 2xl:text-lg"
	>
		{#if focus}
			<span class="font-hebrew text-white">{focus.section}</span>
			{#if focus.book}
				<span class="text-white/25">›</span>
				<span class="font-hebrew text-white/90">{focus.book}</span>
			{/if}
			{#if focus.chapter}
				<span class="text-white/25">›</span>
				<span class="font-hebrew text-white">{focus.chapter}</span>
			{/if}
			{#if focus.verse}
				<span class="text-white/25">›</span>
				<span class="font-hebrew text-white">{focus.verse}</span>
			{/if}
		{/if}
	</header>

	<!-- `touch-action: none` is indispensable: without it the mobile browser
	     intercepts the pinch and the drag before we do. -->
	<canvas bind:this={canvas} class="min-h-0 w-full flex-1 touch-none bg-black"></canvas>

	<!-- Index of the readings. Where another publication of the same edition
	     reads otherwise, the map shows our form then theirs, and this row says
	     which is which. It appears only while there is one on screen, and it
	     takes its own row: nothing is ever covered. -->
	{#if readings > 0}
		<footer
			class="flex shrink-0 flex-wrap items-center justify-center gap-x-5 gap-y-1 border-t border-white/10
			       px-3 py-2 pb-[calc(0.5rem+env(safe-area-inset-bottom))] text-xs text-white
			       sm:px-4 sm:text-sm"
			dir="rtl"
		>
			<span>
				¹
				<span class="font-hebrew" dir="rtl">נוסח ברויאר</span>
				<span class="font-hebrew" dir="rtl">(מכון ממרא)</span>
			</span>
			<!-- The second entry wears the colour of the reading it names, the
			     one the map paints in the text. -->
			<span class="text-yellow-200">
				²
				<span class="font-hebrew" dir="rtl">מקרא על פי המסורה</span>
				<span dir="ltr">(sefaria.org)</span>
			</span>
		</footer>
	{/if}
</div>

{#if !map && !fatal}
	<div
		class="pointer-events-none fixed inset-0 grid place-content-center text-sm tracking-widest text-white/55"
		dir="ltr"
		lang="en"
	>
		loading the corpus…
	</div>
{/if}

{#if fatal}
	<!-- The map cannot go on: said plainly, with the number to quote and the
	     way back. The page itself stays up. -->
	<div
		class="fixed inset-0 grid place-content-center gap-3 bg-black p-6 text-center text-sm text-white/70"
		dir="ltr"
		lang="en"
		role="alert"
	>
		<p class="text-white">{fatal.message}</p>
		<p class="font-mono text-xs text-white/50">incident {fatal.incident}</p>
		<button
			type="button"
			class="mx-auto cursor-pointer rounded border border-white/25 px-3 py-1 text-white hover:border-white/60"
			onclick={() => location.reload()}
		>
			Reload
		</button>
		{#if import.meta.env.DEV}
			<p class="text-xs text-white/40">
				In development: <code class="text-white/60">npm run wasm</code> if the module is missing.
			</p>
		{/if}
	</div>
{/if}

{#if notice && !fatal}
	<div
		class="fixed inset-x-3 top-[calc(var(--nav)+0.75rem+env(safe-area-inset-top))] mx-auto flex max-w-xl
		       items-start gap-3 rounded-lg border border-white/15 bg-black/90 px-4 py-2 text-sm text-white/80"
		dir="ltr"
		lang="en"
		role="status"
	>
		<p class="flex-1">
			{notice.message}
			<span class="font-mono text-xs text-white/45">incident {notice.incident}</span>
		</p>
		<button
			type="button"
			aria-label="Close"
			class="cursor-pointer text-lg leading-none text-white/55 hover:text-white"
			onclick={() => (notice = null)}
		>
			×
		</button>
	</div>
{/if}

{#if selection}
	<aside
		class="fixed inset-x-3 bottom-[calc(1rem+env(safe-area-inset-bottom))] max-h-[45vh] overflow-auto
		       rounded-xl border border-white/15 bg-black/90 p-4 backdrop-blur
		       sm:inset-x-auto sm:top-[calc(var(--nav)+0.75rem+env(safe-area-inset-top))] sm:right-[calc(0.75rem+env(safe-area-inset-right))]
		       sm:bottom-auto sm:max-h-[60vh] sm:w-[26rem]"
	>
		<button
			type="button"
			dir="ltr"
			lang="en"
			aria-label="Close"
			class="absolute top-1 left-2 cursor-pointer px-2 text-xl leading-none text-white/55 hover:text-white"
			onclick={() => (selection = null)}
		>
			×
		</button>

		<!-- A click gives the passage; a double click gives the word, shown in
		     place inside its passage rather than torn out of it. -->
		{#if selection.word}
			<p class="mb-3 font-hebrew text-[30px] leading-tight text-white">{selection.word}</p>
			<p class="font-hebrew text-[22px] leading-loose text-white/45">
				{selection.head}<span class="text-white">{selection.word}</span>{selection.tail}
			</p>
		{:else}
			<p class="font-hebrew text-[22px] leading-loose">{selection.verse}</p>
		{/if}
	</aside>
{/if}
