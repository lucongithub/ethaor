<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createTanakhMap,
		type Focus,
		type Place,
		type Selection,
		type TanakhMap
	} from '$lib/ethaor';
	import { escalate, reporter, watch, type Fault } from '$lib/fault';
	import { command, escape } from '$lib/keys';
	import Rail, { type View } from './Rail.svelte';
	import Logo from './Logo.svelte';
	import BookView, { type Reading } from './BookView.svelte';
	import { CRUMB_GOES, fromMap, TANAKH, toMap, type Level } from '$lib/book-view';

	let canvas: HTMLCanvasElement;
	let reload = $state<HTMLButtonElement | null>(null);
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

	// Two ways to read the same Tanakh: the map, zoomed, and the book,
	// scrolled. The map stays alive under the book — its text keeps landing —
	// and each hands the other the place it is at.
	let view = $state<View>('map');
	let opened = $state<Place>({ chapter: 0, verse: -1 });
	let openedLevel = $state<Level>('text');
	let book = $state<BookView | null>(null);
	let reading = $state<Reading | null>(null);
	let landed = $state(0);

	function switchTo(next: View) {
		if (!map || next === view) return;
		if (next === 'book') {
			selection = null;
			opened = map.here();
			openedLevel = fromMap(map.depth());
			reading = null;
			onForm = false;
			view = 'book';
		} else {
			// Each level lands on the map's own: the text on its passage, a
			// book's chapters on that book's chapters, the books on the
			// framing, the portions alone on the overview.
			const { place, level } = book?.where() ?? { place: opened, level: openedLevel };
			view = 'map';
			const depth = toMap(level);
			if (depth === 3) map.show(place);
			else if (depth === 2) map.showBook(place.chapter);
			else if (depth === 1) map.frameBooks();
			else map.reset();
			canvas.focus({ preventScroll: true });
		}
	}

	// The mark in the sidebar leads home: back to the map's opening, as Escape
	// gives it with nothing open — the passage closed, the book left.
	function home() {
		if (!map) return;
		selection = null;
		if (view === 'book') {
			view = 'map';
			reading = null;
			onForm = false;
		}
		map.reset();
		canvas.focus({ preventScroll: true });
	}

	// A step of the book's breadcrumb: a link up, brightened on hover as the
	// rail's icons are.
	const step = [
		'cursor-pointer rounded font-hebrew transition-colors duration-150',
		'decoration-white/40 underline-offset-[0.3em] hover:underline',
		'focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60'
	];

	// What the breadcrumb row says: the map's focus, or the book's place.
	const crumbs = $derived(view === 'book' ? reading && { ...reading, hovered: false } : focus);
	// The book's pointer on one of the two forms: the index opens there alone.
	let onForm = $state(false);
	const legend = $derived(view === 'book' ? onForm : readings > 0);

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
			onFault: shown,
			onLanded: () => landed++
		})
			.then((created) => {
				handle = created;
				map = created;
				// The keys work from the first image, with nothing to click first.
				canvas.focus({ preventScroll: true });
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

	// Escape anywhere on the page: it closes what is open, nearest first, and
	// only then goes back to the overview (`escape`).
	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape') return;
		const closes = escape({
			selection: selection !== null,
			notice: notice !== null,
			book: view === 'book'
		});
		if (closes === 'selection') selection = null;
		else if (closes === 'notice') notice = null;
		else if (closes === 'overview') map?.reset();
	}

	// The other keys move the map, so they are read on it alone: an arrow in
	// the passage panel scrolls the panel.
	function onCanvasKeydown(event: KeyboardEvent) {
		const asked = command(event);
		if (!asked || !map) return;
		event.preventDefault();
		map.key(asked);
	}

	// The screen reader hears where the view is, not where a mouse passes:
	// only the centre's place is spoken (the keys, a finger), and only once it
	// settles, so a run of arrows says one place, not ten.
	let spoken = $state('');
	$effect(() => {
		const said =
			crumbs && !crumbs.hovered
				? [crumbs.section, crumbs.book, crumbs.chapter, crumbs.verse].filter(Boolean).join(', ')
				: '';
		if (!said) return;
		const timer = setTimeout(() => (spoken = said), 600);
		return () => clearTimeout(timer);
	});

	// The map has stopped: the way back takes the focus, one key from use.
	$effect(() => {
		if (fatal) reload?.focus();
	});
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- The top bar runs the whole width, its rule unbroken; the sidebar and the
     reading column share the row under it. The bar and that row are laid left
     to right, whatever the page's direction, so the mark and the sidebar keep
     the left and the breadcrumb takes Hebrew's right. -->
<div dir="ltr" class="relative flex h-full flex-col bg-black">
	<header
		class="flex h-[calc(var(--nav)+env(safe-area-inset-top))] shrink-0 items-center
	       border-b border-white/10 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)]
	       pl-[env(safe-area-inset-left)]"
	>
		<!-- The mark and its name lead home: the map's opening, as Escape gives
		     it. The mark stands over the sidebar's icons, on their column; both
		     brighten on hover like the items. -->
		<div class="shrink-0 px-1 sm:px-2">
			<button
				type="button"
				dir="ltr"
				lang="en"
				class="group flex h-10 cursor-pointer items-center gap-2.5 rounded pr-3 pl-[0.6875rem]
				       text-white/58 transition-colors duration-150 hover:text-white
				       focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
				aria-label="Ethaor — back to the overview"
				onclick={home}
			>
				<Logo class="h-5 w-5 text-[1.25rem]" />
				<span
					class="pt-0.5 text-[0.9375rem] leading-none tracking-wide sm:text-base"
					aria-hidden="true">Ethaor</span
				>
			</button>
		</div>

		<!-- Breadcrumb. The title bands stay inside the map and therefore never
	     cover anything; when they leave the screen, it is this that says where
	     one is. It takes its own row: not one pixel of map lost.

	     Its depth is the module's: a portion and a book at the framing, the
	     chapter once the numbers are out, the verse once the lines are read.
	     Its place is the pointer as soon as there is one — hovering a title, a
	     chapter or a line says where **that** is. -->
		<div
			dir="rtl"
			class="flex min-w-0 flex-1 items-center gap-2 overflow-hidden px-3 text-sm whitespace-nowrap
		       sm:px-4 sm:text-base 2xl:text-lg"
		>
			{#if view === 'book'}
				<!-- In the book the breadcrumb is the way up: the Tanakh to the
				     portions alone, a portion to its books, a book to its chapters. -->
				<button
					type="button"
					class={[step, 'text-white']}
					onclick={() => book?.climbTo(CRUMB_GOES.tanakh)}
				>
					{TANAKH}
				</button>
				{#if reading?.section}
					<span class="text-white/25" aria-hidden="true">›</span>
					<button
						type="button"
						class={[step, 'text-white']}
						onclick={() => book?.climbTo(CRUMB_GOES.portion, reading?.portion)}
					>
						{reading.section}
					</button>
				{/if}
				{#if reading?.book}
					<span class="text-white/25" aria-hidden="true">›</span>
					<button
						type="button"
						class={[step, 'text-white/90']}
						onclick={() => book?.climbTo(CRUMB_GOES.book, reading?.bookIndex)}
					>
						{reading.book}
					</button>
				{/if}
				{#if reading?.chapter}
					<span class="text-white/25" aria-hidden="true">›</span>
					<span class="font-hebrew text-white">{reading.chapter}</span>
				{/if}
				{#if reading?.verse}
					<span class="text-white/25" aria-hidden="true">›</span>
					<span class="font-hebrew text-white">{reading.verse}</span>
				{/if}
			{:else if crumbs}
				<span class="font-hebrew text-white">{crumbs.section}</span>
				{#if crumbs.book}
					<span class="text-white/25" aria-hidden="true">›</span>
					<span class="font-hebrew text-white/90">{crumbs.book}</span>
				{/if}
				{#if crumbs.chapter}
					<span class="text-white/25" aria-hidden="true">›</span>
					<span class="font-hebrew text-white">{crumbs.chapter}</span>
				{/if}
				{#if crumbs.verse}
					<span class="text-white/25" aria-hidden="true">›</span>
					<span class="font-hebrew text-white">{crumbs.verse}</span>
				{/if}
			{/if}
		</div>
	</header>

	<div class="flex min-h-0 flex-1">
		<Rail {view} onview={switchTo} />

		<div dir="rtl" class="flex min-w-0 flex-1 flex-col">
			<p class="sr-only" aria-live="polite">{spoken}</p>

			<!-- `touch-action: none` is indispensable: without it the mobile browser
	     intercepts the pinch and the drag before we do. Focusable, and named
	     with its keys: without that the whole map is silent to a keyboard and
	     a screen reader. -->
			<!-- The map and the book share the room under the breadcrumb: the book
	     lies over the map, which waits beneath it, inert. -->
			<div class="relative min-h-0 flex-1">
				<canvas
					bind:this={canvas}
					inert={view === 'book'}
					tabindex="0"
					aria-label="Map of the Tanakh. Plus and minus zoom, the arrows move, Enter reads the passage at the centre, Escape closes it or returns to the overview."
					lang="en"
					onkeydown={onCanvasKeydown}
					class="absolute inset-0 size-full touch-none bg-black outline-none
		       focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/25"
				></canvas>
				{#if view === 'book' && map}
					<div class="absolute inset-0">
						<BookView
							bind:this={book}
							book={map.book}
							start={opened}
							startLevel={openedLevel}
							{landed}
							onreading={(next) => (reading = next)}
							onform={(on) => (onForm = on)}
							onfault={shown}
						/>
					</div>
				{/if}
			</div>

			<!-- Index of the readings. Where another publication of the same edition
	     reads otherwise, the map shows our form then theirs, and this row says
	     which is which. It appears only while there is one on screen, and it
	     takes its own row: nothing is ever covered. -->
			{#if legend}
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
	</div>
</div>

{#if !map && !fatal}
	<div
		class="pointer-events-none fixed inset-0 grid place-content-center text-sm tracking-widest text-white/55"
		dir="ltr"
		lang="en"
	>
		loading Torah…
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
		<p class="font-mono text-xs text-white/55">incident {fatal.incident}</p>
		<button
			bind:this={reload}
			type="button"
			class="mx-auto cursor-pointer rounded border border-white/25 px-3 py-1 text-white hover:border-white/60
			       focus-visible:border-white/60 focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
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
	<!-- Centred, unless the passage panel holds the right of the same band:
	     then it keeps to the left and to what the panel leaves, so the two
	     never overlap. -->
	<div
		class={[
			'fixed inset-x-3 top-[calc(var(--nav)+0.75rem+env(safe-area-inset-top))] mx-auto flex max-w-xl',
			'items-start gap-3 rounded-lg border border-white/15 bg-black/90 px-4 py-2 text-sm text-white/80',
			selection && 'sm:right-auto sm:mx-0 sm:max-w-[min(36rem,calc(100vw-28.5rem))]'
		]}
		dir="ltr"
		lang="en"
		role="status"
	>
		<p class="flex-1">
			{notice.message}
			<span class="font-mono text-xs text-white/55">incident {notice.incident}</span>
		</p>
		<button
			type="button"
			aria-label="Close"
			class="-my-1 -mr-2 grid size-8 shrink-0 cursor-pointer place-items-center rounded text-lg leading-none text-white/55
			       hover:text-white focus-visible:text-white focus-visible:outline-1 focus-visible:outline-white/60"
			onclick={() => (notice = null)}
		>
			×
		</button>
	</div>
{/if}

{#if selection}
	<!-- On a phone a sheet above the readings' row, never over it; the left
	     padding is the close button's, so a long word never runs under it. -->
	<aside
		aria-label="Passage"
		aria-live="polite"
		class={[
			'fixed inset-x-3 max-h-[45vh] overflow-auto rounded-xl border border-white/15 bg-black/90 p-4 pl-11 backdrop-blur',
			'[scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent]',
			readings > 0
				? 'bottom-[calc(3.25rem+env(safe-area-inset-bottom))]'
				: 'bottom-[calc(1rem+env(safe-area-inset-bottom))]',
			'sm:inset-x-auto sm:top-[calc(var(--nav)+0.75rem+env(safe-area-inset-top))] sm:right-[calc(0.75rem+env(safe-area-inset-right))]',
			'sm:bottom-auto sm:max-h-[60vh] sm:w-[26rem]'
		]}
	>
		<button
			type="button"
			dir="ltr"
			lang="en"
			aria-label="Close"
			class="absolute top-1 left-1 grid size-9 cursor-pointer place-items-center rounded text-xl leading-none text-white/55
			       hover:text-white focus-visible:text-white focus-visible:outline-1 focus-visible:outline-white/60"
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
			<p class="font-hebrew text-[22px] leading-loose text-white">{selection.verse}</p>
		{/if}
	</aside>
{/if}
