<script lang="ts" module>
	export type View = 'map' | 'book';
</script>

<script lang="ts">
	import Logo from './Logo.svelte';

	// The sidebar: a rail of icons on the left, always this narrow — it does
	// not open, so it never covers the map nor takes more of the row. Each icon
	// is named in a tip on hover or keyboard focus. Its words are English, like
	// the rest of the chrome.
	//
	// Icons: Phosphor, light weight (MIT), drawn inline — one stroke, one
	// weight, nothing to load.

	let {
		view,
		onview,
		onhome
	}: {
		view: View;
		onview: (next: View) => void;
		/** The mark is the way home: the opening, as Escape gives it. */
		onhome: () => void;
	} = $props();

	const ICON = {
		book: 'M232,50H160a38,38,0,0,0-32,17.55A38,38,0,0,0,96,50H24a6,6,0,0,0-6,6V200a6,6,0,0,0,6,6H96a26,26,0,0,1,26,26,6,6,0,0,0,12,0,26,26,0,0,1,26-26h72a6,6,0,0,0,6-6V56A6,6,0,0,0,232,50ZM96,194H30V62H96a26,26,0,0,1,26,26V204.31A37.86,37.86,0,0,0,96,194Zm130,0H160a37.87,37.87,0,0,0-26,10.32V88a26,26,0,0,1,26-26h66ZM160,90h40a6,6,0,0,1,0,12H160a6,6,0,0,1,0-12Zm46,38a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,128Zm0,32a6,6,0,0,1-6,6H160a6,6,0,0,1,0-12h40A6,6,0,0,1,206,160Z',
		map: 'M227.69,51.27a6,6,0,0,0-5.15-1.09L160.7,65.64l-62-31a6,6,0,0,0-4.14-.45l-64,16A6,6,0,0,0,26,56V200a6,6,0,0,0,7.46,5.82L95.3,190.36l62,31a6,6,0,0,0,4.14.45l64-16A6,6,0,0,0,230,200V56A6,6,0,0,0,227.69,51.27ZM102,49.71l52,26V206.29l-52-26Zm-64,11,52-13V179.32l-52,13ZM218,195.32l-52,13V76.68l52-13Z'
	};

	// The mode button names where it goes, not where one is: on the map it
	// opens the book, in the book it goes back to the map.
	const toggle = $derived(
		view === 'map'
			? { label: 'Book mode', icon: ICON.book, next: 'book' as const }
			: { label: 'Map mode', icon: ICON.map, next: 'map' as const }
	);

	const item = [
		'group relative flex h-10 w-full cursor-pointer items-center rounded px-[0.6875rem]',
		'text-white/58 transition-colors duration-150 hover:bg-white/[0.06] hover:text-white',
		'focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60'
	];

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
	<!-- The top cell continues the breadcrumb's row and its rule, and holds
	     the mark, on the icons' column below it. -->
	<div
		class="flex h-[calc(var(--nav)+env(safe-area-inset-top))] shrink-0 items-center border-b border-white/10 px-1 pt-[env(safe-area-inset-top)] sm:px-2"
	>
		<!-- The mark leads home: the map's opening, as Escape gives it. It
		     brightens on hover like the items. -->
		<button
			type="button"
			class="group flex h-10 w-full cursor-pointer items-center rounded px-[0.6875rem]
			       focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
			aria-label="Ethaor — back to the overview"
			onclick={onhome}
		>
			<Logo
				class="h-5 w-5 text-[1.25rem] text-white/58 transition-colors duration-150 group-hover:text-white"
			/>
		</button>
	</div>

	<nav aria-label="Views" class="flex flex-col gap-0.5 px-1 pt-2 sm:px-2">
		<button
			type="button"
			class={item}
			aria-label={toggle.label}
			onclick={() => onview(toggle.next)}
		>
			<svg viewBox="0 0 256 256" class="size-5 shrink-0 fill-current" aria-hidden="true">
				<path d={toggle.icon} />
			</svg>
			<span class={tip} aria-hidden="true">{toggle.label}</span>
		</button>
	</nav>
</aside>
