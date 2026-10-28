<script lang="ts" module>
	export type View = 'map' | 'book';
	export const SEARCHES = [
		'Consecutive letters',
		'Equidistant letters',
		'First letters',
		'Last letters',
		'Guematria'
	] as const;
	export type Search = (typeof SEARCHES)[number];
</script>

<script lang="ts">
	// The sidebar: a rail of icons on the left, always this narrow — it does
	// not open, so it never covers the map nor takes more of the row. It starts
	// under the top bar, which holds the mark: the bar's rule runs whole across
	// the page and the rail's meets it from below, so no two rules cross. Each
	// icon is named in a tip on hover or keyboard focus. Its words are English,
	// like the rest of the chrome.
	//
	// Icons: Phosphor, light weight (MIT), drawn inline — one stroke, one
	// weight, nothing to load.

	let {
		view,
		onview,
		onsearch
	}: {
		view: View;
		onview: (next: View) => void;
		onsearch: (name: Search) => void;
	} = $props();

	const ICON = {
		book: 'M232,50H160a38,38,0,0,0-32,17.55A38,38,0,0,0,96,50H24a6,6,0,0,0-6,6V200a6,6,0,0,0,6,6H96a26,26,0,0,1,26,26,6,6,0,0,0,12,0,26,26,0,0,1,26-26h72a6,6,0,0,0,6-6V56A6,6,0,0,0,232,50ZM96,194H30V62H96a26,26,0,0,1,26,26V204.31A37.86,37.86,0,0,0,96,194Zm130,0H160a37.87,37.87,0,0,0-26,10.32V88a26,26,0,0,1,26-26h66ZM160,90h40a6,6,0,0,1,0,12H160a6,6,0,0,1,0-12Zm46,38a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,128Zm0,32a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,160Z',
		search:
			'M208,34H48A14,14,0,0,0,34,48V208a14,14,0,0,0,14,14H208a14,14,0,0,0,14-14V48A14,14,0,0,0,208,34Zm-13.52,88L134,61.52V46h15.52L210,106.48V122ZM134,78.48,177.52,122H134ZM210,48V89.52L166.48,46H208A2,2,0,0,1,210,48ZM48,46h74v76H46V48A2,2,0,0,1,48,46Zm58.48,164L46,149.52V134H61.52L122,194.48V210ZM122,177.52,78.48,134H122ZM46,208V166.48L89.52,210H48A2,2,0,0,1,46,208Zm162,2H134V192h0V134h76v74A2,2,0,0,1,208,210Z',
		map: 'M227.69,51.27a6,6,0,0,0-5.15-1.09L160.7,65.64l-62-31a6,6,0,0,0-4.14-.45l-64,16A6,6,0,0,0,26,56V200a6,6,0,0,0,7.46,5.82L95.3,190.36l62,31a6,6,0,0,0,4.14.45l64-16A6,6,0,0,0,230,200V56A6,6,0,0,0,227.69,51.27ZM102,49.71l52,26V206.29l-52-26Zm-64,11,52-13V179.32l-52,13ZM218,195.32l-52,13V76.68l52-13Z'
	};

	// One button per view, the map first: the one shown is pressed, the other
	// switches to its own. Pressing the view one is in changes nothing.
	const views: { view: View; label: string; icon: string }[] = [
		{ view: 'map', label: 'Map mode', icon: ICON.map },
		{ view: 'book', label: 'Book mode', icon: ICON.book }
	];

	const item = [
		'group relative flex h-10 w-full items-center rounded px-[0.6875rem]',
		'transition-colors duration-150',
		'focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60'
	];
	// Pressed: full white on a faint plate — rank in the white, as the map's.
	const pressed = 'cursor-default bg-white/[0.1] text-white';
	const idle = 'cursor-pointer text-white/58 hover:bg-white/[0.06] hover:text-white';

	// The name an icon shows beside it, on hover or keyboard focus: a name,
	// never an instruction. Over the map for the moment it is read.
	const tip = [
		'pointer-events-none absolute top-1/2 left-full z-40 ml-2 -translate-y-1/2 whitespace-nowrap',
		'rounded border border-white/15 bg-black px-2 py-1 text-xs tracking-wide text-white',
		'opacity-0 transition-opacity duration-100 group-hover:opacity-100 group-hover:delay-200',
		'group-focus-visible:opacity-100'
	];

	// The searches, in a menu beside the rail: names only, no icon. Picking
	// one closes the menu and hands its name to the page; all but Guematria
	// are wired yet.
	const searches = SEARCHES;

	let open = $state(false);
	let trigger = $state<HTMLButtonElement>();
	let menu = $state<HTMLDivElement>();

	function items(): HTMLButtonElement[] {
		return [...(menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])];
	}

	// Opened, the first item takes the focus: the arrows go on from there.
	function show() {
		open = true;
		queueMicrotask(() => items()[0]?.focus());
	}

	function hide(refocus: boolean) {
		open = false;
		if (refocus) trigger?.focus();
	}

	// The menu's own keys: arrows walk the items, wrapping; Home and End jump;
	// Escape and Tab close. Escape stops here, or the window would also take
	// it back to the overview.
	function onMenuKeydown(event: KeyboardEvent) {
		const all = items();
		const at = all.indexOf(document.activeElement as HTMLButtonElement);
		const go = (k: number) => all[(k + all.length) % all.length]?.focus();
		if (event.key === 'ArrowDown') go(at + 1);
		else if (event.key === 'ArrowUp') go(at - 1);
		else if (event.key === 'Home') go(0);
		else if (event.key === 'End') go(all.length - 1);
		else if (event.key === 'Escape') hide(true);
		else if (event.key === 'Tab') return hide(false);
		else return;
		event.preventDefault();
		event.stopPropagation();
	}

	// A press anywhere else closes it — the trigger toggles on its own.
	function onWindowPointerdown(event: PointerEvent) {
		const target = event.target as Node;
		if (open && !menu?.contains(target) && !trigger?.contains(target)) hide(false);
	}

	const entry = [
		'flex w-full cursor-pointer items-center rounded px-3 py-2 text-left text-sm whitespace-nowrap text-white/70',
		'transition-colors duration-150 outline-none',
		'hover:bg-white/[0.06] hover:text-white focus-visible:bg-white/[0.06] focus-visible:text-white',
		'aria-disabled:cursor-default aria-disabled:text-white/30 aria-disabled:hover:bg-transparent aria-disabled:hover:text-white/30'
	];
</script>

<svelte:window onpointerdown={onWindowPointerdown} />

<aside
	id="rail"
	dir="ltr"
	lang="en"
	aria-label="Sidebar"
	class="z-30 flex h-full w-12 shrink-0 flex-col border-r border-white/10 bg-black pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)] sm:w-14"
>
	<nav aria-label="Views" class="flex flex-col gap-0.5 px-1 pt-2 sm:px-2">
		{#each views as v (v.view)}
			{@const on = v.view === view}
			<button
				type="button"
				class={[item, on ? pressed : idle]}
				aria-label={v.label}
				aria-pressed={on}
				onclick={() => {
					if (!on) onview(v.view);
				}}
			>
				<svg viewBox="0 0 256 256" class="size-5 shrink-0 fill-current" aria-hidden="true">
					<path d={v.icon} />
				</svg>
				<span class={tip} aria-hidden="true">{v.label}</span>
			</button>
		{/each}
	</nav>

	<div class="relative mx-1 mt-0.5 sm:mx-2">
		<button
			bind:this={trigger}
			type="button"
			class={[item, open ? 'cursor-pointer bg-white/[0.1] text-white' : idle]}
			aria-label="Search"
			aria-haspopup="menu"
			aria-expanded={open}
			aria-controls="search-menu"
			onclick={() => (open ? hide(false) : show())}
		>
			<svg viewBox="0 0 256 256" class="size-5 shrink-0 fill-current" aria-hidden="true">
				<path d={ICON.search} />
			</svg>
			{#if !open}
				<span class={tip} aria-hidden="true">Search</span>
			{/if}
		</button>

		{#if open}
			<div
				bind:this={menu}
				id="search-menu"
				role="menu"
				aria-label="Search"
				tabindex="-1"
				class="absolute top-0 left-full z-40 ml-2 flex min-w-48 flex-col gap-0.5 rounded border border-white/15 bg-black p-1"
				onkeydown={onMenuKeydown}
			>
				{#each searches as name (name)}
					<button
						type="button"
						role="menuitem"
						tabindex="-1"
						class={entry}
						onclick={() => {
							hide(true);
							onsearch(name);
						}}
					>
						{name}
					</button>
				{/each}
			</div>
		{/if}
	</div>
</aside>
