<script lang="ts" module>
	export type View = 'map' | 'book';
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
		onview
	}: {
		view: View;
		onview: (next: View) => void;
	} = $props();

	const ICON = {
		book: 'M232,50H160a38,38,0,0,0-32,17.55A38,38,0,0,0,96,50H24a6,6,0,0,0-6,6V200a6,6,0,0,0,6,6H96a26,26,0,0,1,26,26,6,6,0,0,0,12,0,26,26,0,0,1,26-26h72a6,6,0,0,0,6-6V56A6,6,0,0,0,232,50ZM96,194H30V62H96a26,26,0,0,1,26,26V204.31A37.86,37.86,0,0,0,96,194Zm130,0H160a37.87,37.87,0,0,0-26,10.32V88a26,26,0,0,1,26-26h66ZM160,90h40a6,6,0,0,1,0,12H160a6,6,0,0,1,0-12Zm46,38a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,128Zm0,32a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,160Z',
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
</script>

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
</aside>
