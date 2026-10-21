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
	import { pick, picked, words } from '$lib/passage';

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
		if (closes === 'selection') closePassage();
		else if (closes === 'notice') notice = null;
		else if (closes === 'overview') map?.reset();
	}

	// The other keys move the map, so they are read on it alone: an arrow in
	// the passage scrolls the passage.
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

	// The passage opens as a modal: the page beneath is inert, the focus held
	// inside until it closes, and then handed back to the map.
	let passage = $state<HTMLDialogElement | null>(null);
	$effect(() => {
		if (passage && !passage.open) passage.showModal();
	});

	// Closed before the focus moves: while the modal is open the view beneath
	// is inert and would refuse it. The focus goes back to the view read.
	function closePassage() {
		if (!selection) return;
		passage?.close();
		selection = null;
		if (view === 'book') book?.focus();
		else canvas.focus({ preventScroll: true });
	}

	// A word of the passage picked, or given back to its verse when it was
	// the one picked.
	function pickWord(k: number) {
		if (!selection) return;
		const word = k === picked(selection) ? null : pick(selection.verse, k);
		selection = { ...selection, ...(word ?? { word: '', head: '', tail: '' }) };
	}

	// A click on the backdrop — the dialog itself, never its content — is a
	// way out.
	function onPassageClick(event: MouseEvent) {
		if (event.target === passage) closePassage();
	}

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
							onselect={(hit) => (selection = hit)}
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
	<!-- Centred under the breadcrumb. -->
	<div
		class="fixed inset-x-3 top-[calc(var(--nav)+0.75rem+env(safe-area-inset-top))] mx-auto flex max-w-xl
		       items-start gap-3 rounded-lg border border-white/15 bg-black/90 px-4 py-2 text-sm text-white/80"
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
	{@const where = map?.book.contents}
	{@const at = picked(selection)}
	{@const all = words(selection.verse)}
	<!-- The passage, a modal centred over the map: read as a web page reads,
	     Tailwind's type scale stepped up with the screen, the map pushed back
	     beneath. A click on its backdrop closes it. -->
	<dialog
		bind:this={passage}
		dir="rtl"
		aria-labelledby="passage-place"
		onclick={onPassageClick}
		onclose={closePassage}
		class="m-auto max-h-[min(85dvh,52rem)] w-[calc(100%-2rem)] max-w-2xl flex-col overflow-hidden
		       rounded-xl border border-white/15 bg-black p-0 text-white transition-[opacity,scale]
		       duration-200 ease-out backdrop:bg-black/80 open:flex motion-reduce:transition-none
		       lg:max-w-3xl starting:scale-[0.98] starting:opacity-0"
	>
		<header
			class="flex shrink-0 items-center gap-3 border-b border-white/10 py-2 ps-5 pe-2 sm:ps-7 sm:pe-3"
		>
			<!-- Where the passage is, every level named in Hebrew as the
			     breadcrumb names it: book, then chapter and verse. -->
			<h2 id="passage-place" class="flex-1 truncate font-hebrew text-xl">
				<span class="text-white">{where?.books[selection.book]?.name}</span>
				<span class="text-white">{where?.labels[selection.chapter]},{selection.number}</span>
			</h2>
			<button
				type="button"
				lang="en"
				aria-label="Close"
				class="grid size-10 shrink-0 cursor-pointer place-items-center rounded text-white/58 transition-colors duration-150
				       hover:bg-white/[0.06] hover:text-white focus-visible:text-white
				       focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
				onclick={closePassage}
			>
				<svg viewBox="0 0 256 256" class="size-5 fill-current" aria-hidden="true">
					<path
						d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
					/>
				</svg>
			</button>
		</header>

		<!-- The passage, word by word: each word wears the plate of the book's
		     chapter buttons under the pointer, and a click picks it — shown big
		     above its verse, in place inside it, the rest dimmed. A click on the
		     word picked gives the verse back; another word, that one. -->
		<div
			aria-live="polite"
			class="[scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.25)_transparent] overflow-y-auto overscroll-contain px-5
			       py-6 sm:px-7 sm:py-8 lg:px-10 lg:py-10"
		>
			{#if at >= 0}
				<p
					class="mb-5 font-hebrew text-4xl leading-tight text-white sm:mb-6 sm:text-5xl lg:text-6xl"
				>
					{selection.word}
				</p>
			{/if}
			<p
				class={[
					'font-hebrew text-xl leading-loose text-pretty sm:text-2xl lg:text-3xl',
					at >= 0 ? 'text-white/45' : 'text-white'
				]}
			>
				{#each all as word, k (k)}<button
						type="button"
						aria-pressed={k === at}
						class={[
							'-mx-[0.12em] cursor-pointer rounded box-decoration-clone px-[0.12em] transition-colors duration-150',
							'focus-visible:outline-1 focus-visible:outline-white/60',
							k === at
								? 'bg-white/12 text-white'
								: 'hover:bg-white/6 hover:text-white focus-visible:text-white'
						]}
						onclick={() => pickWord(k)}>{word}</button
					>{k < all.length - 1 ? ' ' : ''}{/each}
			</p>
		</div>
	</dialog>
{/if}
