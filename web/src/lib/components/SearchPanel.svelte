<script lang="ts">
	// The searches kept, in a panel on the right — as many as the reader runs,
	// newest last. Each row: its colour (the palette), its word, its count of
	// hits, the
	// eye (shows or hides its marks on the map), the cross (takes it away).
	// Ten are shown at most, each its own colour; a hidden one waits for a
	// slot. The word opens what the search did (`SearchInfo`): its kind, how
	// the text was read, what was asked. The count is a control of its own, the owner's rule:
	// it opens the list of the search's passages (`ResultsModal`). Above the
	// rows, once two are shown, the passages they make together: holding any
	// (∪) or every one (∩), each count opening its list. Each sign picks what
	// the map draws, the owner's rule: ∪ by default, every search whole; ∩,
	// each only in the passages of all. A count opening its list picks its
	// sign too.
	//
	// It slides over the map from the right and takes no room from it — the
	// owner's rule. On a wide screen the map stays live beside it; on a narrow
	// one a veil lies beneath it, and the veil, the close button or Escape put
	// it away. Every target is a finger's size there. Its words are Hebrew,
	// the rest English.
	import {
		canShow,
		COLORS,
		grouped,
		isShown,
		MAX_SHOWN,
		searchName,
		rowLabel,
		shownWord,
		totalOf,
		type Color,
		type Marked
	} from '$lib/marks';

	let {
		list,
		veil,
		onclose,
		ontoggle,
		onremove,
		onclear,
		onrecolor,
		onresults,
		oninfo,
		together = { any: 0, all: 0 },
		ontogether,
		join = 'any',
		onjoin
	}: {
		list: Marked[];
		/** A veil beneath, the map inert: a narrow screen. */
		veil: boolean;
		onclose: () => void;
		ontoggle: (id: number) => void;
		onremove: (id: number) => void;
		/** Every search taken away at once: the reset at the panel's foot. */
		onclear: () => void;
		onrecolor: (id: number, color: Color) => void;
		/** A search's passages asked for, by its count. */
		onresults?: (id: number) => void;
		/** A search explained, asked for by its word. */
		oninfo?: (id: number) => void;
		/** Passages the searches shown make together: holding any, or all. */
		together?: { any: number; all: number };
		/** Those passages asked for: of all (`true`), of any (`false`). */
		ontogether?: (all: boolean) => void;
		/** How the map joins the searches shown: any (∪), each drawn whole, or all (∩). */
		join?: 'any' | 'all';
		/** A join picked by its sign. */
		onjoin?: (join: 'any' | 'all') => void;
	} = $props();

	const names = Object.keys(COLORS) as Color[];
	const shown = $derived(list.filter(isShown).length);
	const room = $derived(canShow(list));
	// A search shown still searching: the counts together wait for it.
	const waiting = $derived(list.some((m) => isShown(m) && m.passages === null));

	let panel = $state<HTMLElement>();

	/** Gives the panel the focus: its first control. */
	export function focus() {
		panel?.querySelector<HTMLElement>('button')?.focus();
	}

	// The search whose palette is open, if any.
	let picking = $state<number | null>(null);
	let palette = $state<HTMLDivElement>();

	function open(id: number) {
		picking = picking === id ? null : id;
		if (picking !== null) {
			queueMicrotask(() =>
				palette?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus()
			);
		}
	}

	function close(refocus: boolean) {
		const id = picking;
		picking = null;
		if (refocus && id !== null) document.getElementById(`swatch-${id}`)?.focus();
	}

	// A press anywhere but in the palette or on its swatch closes it.
	function onWindowPointerdown(event: PointerEvent) {
		if (picking === null) return;
		const target = event.target as Element;
		if (!palette?.contains(target) && !target.closest?.(`#swatch-${picking}`)) close(false);
	}

	// The palette's keys: arrows walk the colours, Escape closes it — and goes
	// no further, or the window would close the panel too.
	function onPaletteKeydown(event: KeyboardEvent) {
		const all = [...(palette?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]') ?? [])];
		const at = all.indexOf(document.activeElement as HTMLButtonElement);
		const go = (k: number) => all[(k + all.length) % all.length]?.focus();
		if (event.key === 'ArrowRight' || event.key === 'ArrowDown') go(at + 1);
		else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') go(at - 1);
		else if (event.key === 'Escape') close(true);
		else if (event.key === 'Tab') return close(false);
		else return;
		event.preventDefault();
		event.stopPropagation();
	}

	const count = grouped;
	const passagesOf = (n: number) => `${grouped(n)} ${n === 1 ? 'passage' : 'passages'}`;

	// The two ways the searches shown join: their sign, the tip naming their
	// passages, and the tip naming what the map then draws.
	const joins = [
		{
			all: false,
			sign: '∪',
			name: 'Passages holding any search shown',
			map: 'On the map: any search shown'
		},
		{
			all: true,
			sign: '∩',
			name: 'Passages holding every search shown',
			map: 'On the map: every search shown'
		}
	] as const;

	// The name a row shows, on hover of its word or eye, or keyboard focus of
	// its eye — the rail's tip. It lies in the top layer, so neither the list's
	// scroll nor the page clips it, and outside the panel, left of its row,
	// so it covers no colour; under its row where the screen leaves no room.
	let tip = $state<HTMLDivElement>();

	function named(search: Marked, blocked: boolean): string {
		if (blocked) return `${MAX_SHOWN} shown at most`;
		return searchName(search);
	}

	function showTip(text: string, row: Element) {
		if (!tip || !panel) return;
		// Written at once, not through state: it is measured next.
		tip.textContent = text;
		if (!tip.matches(':popover-open')) tip.showPopover();
		const gap = 8;
		const edge = panel.getBoundingClientRect().left;
		const at = row.getBoundingClientRect();
		const beside = edge - gap >= tip.offsetWidth + gap;
		tip.style.right = `${innerWidth - (beside ? edge : at.right) + gap}px`;
		tip.style.top = beside
			? `${at.top + at.height / 2 - tip.offsetHeight / 2}px`
			: `${at.bottom + 4}px`;
	}

	function hideTip() {
		if (tip?.matches(':popover-open')) tip.hidePopover();
	}

	// A pointer that hovers — a finger has no hover, and its tap would leave
	// the tip open.
	const hovers = (event: PointerEvent) => event.pointerType !== 'touch';

	// A square control: a finger's size on a narrow screen, the rail's on a wide one.
	const square = [
		'grid size-11 shrink-0 cursor-pointer place-items-center rounded md:size-9',
		'transition-colors duration-150 hover:bg-white/[0.06]',
		'focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60'
	];
</script>

<svelte:window onpointerdown={onWindowPointerdown} />

{#if veil}
	<!-- The veil: the map dimmed under the panel, a tap on it puts the panel away. -->
	<button
		type="button"
		tabindex="-1"
		aria-label="Close searches"
		class="fixed inset-0 z-30 cursor-default bg-black/60 transition-opacity duration-200 ease-out
		       motion-reduce:transition-none starting:opacity-0"
		onclick={onclose}
	></button>
{/if}

<aside
	bind:this={panel}
	id="searches"
	dir="ltr"
	lang="en"
	aria-label="Searches"
	class={[
		'fixed top-[calc(var(--nav)+env(safe-area-inset-top))] right-0 bottom-0 z-40 flex w-[min(20rem,88vw)]',
		'flex-col border-l border-white/10 bg-black shadow-[-12px_0_32px_rgb(0_0_0/0.6)]',
		'transition-[translate] duration-250 ease-out motion-reduce:transition-none starting:translate-x-full',
		'2xl:w-88'
	]}
	onscrollcapture={hideTip}
>
	<header
		class="flex h-12 shrink-0 items-center gap-2 pr-[calc(0.25rem+env(safe-area-inset-right))] pl-4"
	>
		<!-- How many the map shows: the panel's only title, the number bare —
		     the owner's rule, no "Searches" over it, no "of 10 on the map"
		     after it. The screen reader still hears the words. -->
		<p class="flex-1 text-sm text-white tabular-nums" aria-live="polite">
			{shown}<span class="sr-only"> of {MAX_SHOWN} on the map</span>
		</p>
		<button
			type="button"
			class={[square, 'text-white/58 hover:text-white focus-visible:text-white']}
			aria-label="Close searches"
			aria-expanded="true"
			aria-controls="searches"
			onclick={onclose}
		>
			<!-- The panel's own icon, the one that opened it: it closes it. -->
			<svg
				viewBox="0 0 256 256"
				class="size-5"
				fill="none"
				stroke="currentColor"
				stroke-width="12"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<rect x="32" y="48" width="192" height="160" rx="8" />
				<path d="M160 48v160" />
			</svg>
		</button>
	</header>

	{#if shown >= 2}
		<!-- The searches shown, together: each sign picks what the map draws,
		     the one picked on a plate as the rail's view is; each count opens
		     its passages. -->
		<div class="flex shrink-0 gap-1 border-y border-white/10 px-1.5 py-0.5">
			{#each joins as j (j.sign)}
				{@const n = j.all ? together.all : together.any}
				{@const off = waiting || n === 0}
				{@const picked = join === (j.all ? 'all' : 'any')}
				<div
					class={[
						'flex min-w-0 flex-1 items-center rounded transition-colors duration-150',
						picked && 'bg-white/[0.08]'
					]}
				>
					<button
						type="button"
						class={[
							'grid size-11 shrink-0 cursor-pointer place-items-center rounded text-base leading-none md:size-9',
							'transition-colors duration-150 focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60',
							picked
								? 'text-white'
								: 'text-white/58 hover:bg-white/[0.06] hover:text-white focus-visible:text-white'
						]}
						aria-pressed={picked}
						aria-label={j.map}
						onclick={() => onjoin?.(j.all ? 'all' : 'any')}
						onpointerenter={(event) => {
							if (hovers(event)) showTip(j.map, event.currentTarget);
						}}
						onpointerleave={hideTip}
						onfocus={(event) => {
							if (event.currentTarget.matches(':focus-visible'))
								showTip(j.map, event.currentTarget);
						}}
						onblur={hideTip}
					>
						{j.sign}
					</button>
					<button
						id={`together-${j.all ? 'all' : 'any'}`}
						type="button"
						class={[
							'flex h-11 min-w-0 flex-1 items-center rounded px-1.5 text-sm tabular-nums md:h-9',
							'transition-colors duration-150 focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60',
							off
								? 'cursor-default text-white/35'
								: 'cursor-pointer text-white/58 hover:bg-white/[0.06] hover:text-white focus-visible:text-white'
						]}
						aria-disabled={off}
						aria-label={`${j.name}: ${waiting ? 'counting' : passagesOf(n)}`}
						onclick={() => {
							if (off) return;
							// Its list opened, its sign picked too — the owner's rule:
							// the map then draws what the list reads.
							if (!picked) onjoin?.(j.all ? 'all' : 'any');
							ontogether?.(j.all);
						}}
						onpointerenter={(event) => {
							if (hovers(event)) showTip(j.name, event.currentTarget);
						}}
						onpointerleave={hideTip}
						onfocus={(event) => {
							if (event.currentTarget.matches(':focus-visible'))
								showTip(j.name, event.currentTarget);
						}}
						onblur={hideTip}
					>
						<span class="truncate">{waiting ? '…' : passagesOf(n)}</span>
					</button>
				</div>
			{/each}
		</div>
	{/if}

	<!-- One grid for every row, its columns shared (`subgrid`): the counts
	     line up on their right edge, and so do the words before them. -->
	<ul
		class="grid min-h-0 flex-1 [scrollbar-width:thin] [scrollbar-color:rgb(255_255_255/0.35)_transparent] grid-cols-[auto_minmax(0,1fr)_auto_auto_auto] content-start gap-x-0.5 overflow-y-scroll overscroll-contain
		       px-1.5 pb-2"
	>
		{#each list as search (search.id)}
			{@const color = COLORS[search.color]}
			{@const on = isShown(search)}
			{@const blocked = !on && !room}
			<li class="relative col-span-full grid grid-cols-subgrid">
				<div class="col-span-full grid grid-cols-subgrid items-center">
					<button
						id={`swatch-${search.id}`}
						type="button"
						class={square}
						aria-label={`Colour of ${search.word}: ${search.color}`}
						aria-haspopup="menu"
						aria-expanded={picking === search.id}
						onclick={() => open(search.id)}
					>
						<span
							class="size-3 rounded-[2px] border-[1.5px] transition-colors duration-150"
							style:border-color={color}
							style:background-color={on ? color : 'transparent'}
						></span>
					</button>

					<!-- The word: named by the tip, as the rail names its icons — the
					     kind of search, and whether its finals are told apart — and
					     clicked, the search explained (`SearchInfo`). -->
					<button
						id={`word-${search.id}`}
						type="button"
						class={[
							'flex h-11 min-w-0 flex-1 cursor-pointer items-center justify-end gap-2 rounded px-2 transition-colors duration-150 md:h-9',
							'hover:bg-white/[0.06] focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60',
							on ? 'text-white' : 'text-white/45 hover:text-white focus-visible:text-white'
						]}
						aria-label={`${search.word}: ${named(search, false)} — what this search did`}
						onclick={() => {
							hideTip();
							oninfo?.(search.id);
						}}
						onpointerenter={(event) => {
							if (hovers(event)) showTip(named(search, false), event.currentTarget);
						}}
						onpointerleave={hideTip}
						onfocus={(event) => {
							if (event.currentTarget.matches(':focus-visible'))
								showTip(named(search, false), event.currentTarget);
						}}
						onblur={hideTip}
					>
						<!-- The word first, from the right, flush with every other row's;
						     its letter or method after it, to its left — never under it.
						     An equidistant row shows no skip, the owner's rule: its tip
						     names it. -->
						{#if rowLabel(search)}
							<span dir="ltr" lang="en" class="shrink-0 text-xs text-white/45 tabular-nums"
								>{rowLabel(search)}</span
							>
						{/if}
						<span
							dir="rtl"
							lang="he"
							class="min-w-0 truncate text-right font-hebrew text-lg leading-none"
						>
							{shownWord(search)}
						</span>
					</button>

					<!-- The count: how many hits, and the way to their passages. -->
					<button
						id={`count-${search.id}`}
						type="button"
						class={[
							'h-11 rounded px-2 text-right text-sm tabular-nums transition-colors duration-150 md:h-9',
							'focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60',
							search.passages === null
								? 'cursor-default text-white/35'
								: 'cursor-pointer text-white/58 hover:bg-white/[0.06] hover:text-white focus-visible:text-white'
						]}
						aria-disabled={search.passages === null}
						aria-label={search.passages === null
							? `${search.word}: searching`
							: `${totalOf(search)} of ${search.word}, in ${passagesOf(search.passages)}`}
						onclick={() => {
							if (search.passages !== null) onresults?.(search.id);
						}}
					>
						{search.passages === null ? '…' : count(search.total ?? 0)}
					</button>

					<!-- The eye shows or hides the search's marks on the map. -->
					<button
						type="button"
						class={[
							square,
							blocked
								? 'cursor-default text-white/25 hover:bg-transparent'
								: on
									? 'text-white hover:text-white'
									: 'text-white/45 hover:text-white focus-visible:text-white'
						]}
						aria-pressed={on}
						aria-disabled={blocked}
						aria-label={`Show ${search.word} on the map${blocked ? ` — ${MAX_SHOWN} shown at most` : ''}`}
						onclick={() => {
							if (!blocked) ontoggle(search.id);
						}}
						onpointerenter={(event) => {
							if (hovers(event)) showTip(named(search, blocked), event.currentTarget);
						}}
						onpointerleave={hideTip}
						onfocus={(event) => {
							if (event.currentTarget.matches(':focus-visible')) {
								showTip(named(search, blocked), event.currentTarget);
							}
						}}
						onblur={hideTip}
					>
						<svg
							viewBox="0 0 256 256"
							class="size-4.5"
							fill="none"
							stroke="currentColor"
							stroke-width="14"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path
								d="M24 128c26-48 62-72 104-72s78 24 104 72c-26 48-62 72-104 72S50 176 24 128Z"
							/>
							<circle cx="128" cy="128" r="34" />
							{#if !on}<path d="M48 40 208 216" />{/if}
						</svg>
					</button>

					<button
						type="button"
						class={[square, 'text-white/58 hover:text-white focus-visible:text-white']}
						aria-label={`Remove ${search.word}`}
						onclick={() => onremove(search.id)}
					>
						<svg viewBox="0 0 256 256" class="size-4 fill-current" aria-hidden="true">
							<path
								d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
							/>
						</svg>
					</button>
				</div>

				{#if picking === search.id}
					<!-- The palette opens under its row, inside the panel. -->
					<div
						bind:this={palette}
						role="menu"
						tabindex="-1"
						aria-label={`Colour of ${search.word}`}
						class="col-span-full mx-1 mb-2 grid grid-cols-4 gap-1 rounded border border-white/15 bg-black p-2"
						onkeydown={onPaletteKeydown}
					>
						{#each names as name (name)}
							{@const chosen = name === search.color}
							<!-- A colour another search on the map wears is not offered
							     to one on the map: two would read as one. -->
							{@const worn =
								on && list.some((m) => m.id !== search.id && isShown(m) && m.color === name)}
							<button
								type="button"
								role="menuitemradio"
								aria-checked={chosen}
								aria-disabled={worn}
								aria-label={worn ? `${name}, worn by another search` : name}
								title={name}
								tabindex="-1"
								class={[
									'grid h-11 place-items-center rounded transition-colors duration-150 md:h-9',
									'focus-visible:bg-white/[0.08] focus-visible:outline-1 focus-visible:outline-white/60',
									worn ? 'cursor-default' : 'cursor-pointer hover:bg-white/[0.08]',
									chosen && 'bg-white/[0.1]'
								]}
								onclick={() => {
									if (worn) return;
									onrecolor(search.id, name);
									close(true);
								}}
							>
								<!-- Worn by another search on the map: its true colour, struck
								     through — dimmed, it read as another colour. -->
								<span
									class={[
										'relative size-4 rounded-[2px]',
										chosen && 'ring-1 ring-white ring-offset-2 ring-offset-black'
									]}
									style:background-color={COLORS[name]}
								>
									{#if worn}<span
											class="absolute top-1/2 left-1/2 h-[1.5px] w-5 -translate-1/2 -rotate-45 rounded-full bg-white/85
											       ring-1 ring-black"
										></span>{/if}
								</span>
							</button>
						{/each}
					</div>
				{/if}
			</li>
		{/each}
	</ul>

	<!-- Under the list, fixed however long it scrolls: every search taken away. -->
	<footer class="shrink-0 border-t border-white/10">
		<!-- Edge to edge: the safe area is its own padding, outside its height. -->
		<button
			type="button"
			class={[
				'box-content flex h-11 w-full cursor-pointer items-center justify-center gap-2 pb-[env(safe-area-inset-bottom)] text-sm md:h-9',
				'text-white/58 transition-colors duration-150 hover:bg-white/[0.06] hover:text-white',
				'focus-visible:text-white focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60'
			]}
			aria-label="Reset: remove every search"
			onclick={onclear}
		>
			<svg
				viewBox="0 0 256 256"
				class="size-4"
				fill="none"
				stroke="currentColor"
				stroke-width="14"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<path d="M40 56v56h56" />
				<path d="M48 160a88 88 0 1 0 8-78L40 112" />
			</svg>
			Reset
		</button>
	</footer>
</aside>

<div
	bind:this={tip}
	popover="manual"
	dir="ltr"
	lang="en"
	aria-hidden="true"
	class="pointer-events-none fixed inset-auto m-0 rounded border border-white/15 bg-black px-2 py-1 text-xs
	       tracking-wide whitespace-nowrap text-white opacity-100 transition-opacity delay-200 duration-100
	       starting:opacity-0"
></div>
