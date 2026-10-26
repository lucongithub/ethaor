<script lang="ts">
	// The searches kept on the map, in a row under the top bar: one chip each,
	// its colour and its word, alone, centred. The word shows or hides its marks,
	// the colour opens the palette, the cross takes the search away. The row
	// is there only while a search is; its words are Hebrew, the rest English.
	import { COLORS, type Color, type Marked } from '$lib/marks';

	let {
		list,
		ontoggle,
		onremove,
		onrecolor
	}: {
		list: Marked[];
		ontoggle: (slot: number) => void;
		onremove: (slot: number) => void;
		onrecolor: (slot: number, color: Color) => void;
	} = $props();

	const names = Object.keys(COLORS) as Color[];

	// The slot whose palette is open, if any.
	let picking = $state<number | null>(null);
	let palette = $state<HTMLDivElement>();

	function open(slot: number) {
		picking = picking === slot ? null : slot;
		if (picking !== null) {
			queueMicrotask(() =>
				palette?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus()
			);
		}
	}

	function close(refocus: boolean) {
		const slot = picking;
		picking = null;
		if (refocus && slot !== null) document.getElementById(`swatch-${slot}`)?.focus();
	}

	// A press anywhere but in the palette or on its swatch closes it.
	function onWindowPointerdown(event: PointerEvent) {
		if (picking === null) return;
		const target = event.target as Element;
		if (!palette?.contains(target) && !target.closest?.(`#swatch-${picking}`)) close(false);
	}

	// The palette's keys: arrows walk the colours, Escape closes — and goes
	// no further, or the window would take it back to the overview.
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

	// The name a chip shows under it, on hover or keyboard focus — the rail's
	// tip, turned to open downwards.
	const tip = [
		'pointer-events-none absolute top-full left-1/2 z-40 mt-2 -translate-x-1/2 whitespace-nowrap',
		'rounded border border-white/15 bg-black px-2 py-1 text-xs tracking-wide text-white',
		'opacity-0 transition-opacity duration-100 group-hover:opacity-100 group-hover:delay-200',
		'group-focus-visible:opacity-100'
	];

	const control = [
		'grid h-8 shrink-0 cursor-pointer place-items-center transition-colors duration-150',
		'hover:bg-white/[0.06] focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-white/60'
	];
</script>

<svelte:window onpointerdown={onWindowPointerdown} />

<div
	dir="ltr"
	lang="en"
	role="toolbar"
	aria-label="Searches"
	class="flex min-h-11 shrink-0 flex-wrap items-center gap-2 border-b border-white/10 py-1.5 pr-[env(safe-area-inset-right)]
	       pl-[calc(0.25rem+env(safe-area-inset-left))] sm:pl-[calc(0.5rem+env(safe-area-inset-left))]"
>
	{#each list as search (search.slot)}
		{@const color = COLORS[search.color]}
		<div class="relative flex h-8 items-stretch rounded border border-white/15">
			<button
				id={`swatch-${search.slot}`}
				type="button"
				class={[control, 'w-8 rounded-l']}
				aria-label={`Colour of ${search.word}: ${search.color}`}
				aria-haspopup="menu"
				aria-expanded={picking === search.slot}
				onclick={() => open(search.slot)}
			>
				<span
					class="size-2.5 rounded-full border-[1.5px] transition-colors duration-150"
					style:border-color={color}
					style:background-color={search.shown ? color : 'transparent'}
				></span>
			</button>

			<button
				type="button"
				class={[
					control,
					'group relative border-x border-white/10 px-2.5',
					search.shown ? 'text-white' : 'text-white/40 hover:text-white/70'
				]}
				aria-pressed={search.shown}
				aria-label={`${search.kind}${search.exact ? ', exact letters' : ''}: ${search.word}, ${search.shown ? 'shown' : 'hidden'}`}
				onclick={() => ontoggle(search.slot)}
			>
				<span dir="rtl" lang="he" class="font-hebrew text-base leading-none">{search.word}</span>
				<!-- Its name, as the rail names its icons: the kind of search, and
				     whether its finals are told apart — so two searches of one
				     word, folded and exact, never read as one. -->
				<span class={tip} aria-hidden="true">
					{search.kind}{search.exact ? ' · Exact letters (final forms)' : ''}
				</span>
			</button>

			<button
				type="button"
				class={[control, 'w-8 rounded-r text-white/58 hover:text-white focus-visible:text-white']}
				aria-label={`Remove ${search.word}`}
				onclick={() => onremove(search.slot)}
			>
				<svg viewBox="0 0 256 256" class="size-3.5 fill-current" aria-hidden="true">
					<path
						d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
					/>
				</svg>
			</button>

			{#if picking === search.slot}
				<div
					bind:this={palette}
					role="menu"
					tabindex="-1"
					aria-label={`Colour of ${search.word}`}
					class="absolute top-full left-0 z-40 mt-2 grid w-max grid-cols-6 gap-1 rounded border border-white/15 bg-black p-2"
					onkeydown={onPaletteKeydown}
				>
					{#each names as name (name)}
						{@const on = name === search.color}
						<button
							type="button"
							role="menuitemradio"
							aria-checked={on}
							aria-label={name}
							title={name}
							tabindex="-1"
							class={[
								'grid size-7 cursor-pointer place-items-center rounded transition-colors duration-150',
								'hover:bg-white/[0.08] focus-visible:bg-white/[0.08] focus-visible:outline-1 focus-visible:outline-white/60',
								on && 'bg-white/[0.1]'
							]}
							onclick={() => {
								onrecolor(search.slot, name);
								close(true);
							}}
						>
							<span
								class={[
									'size-3.5 rounded-full',
									on && 'ring-1 ring-white ring-offset-2 ring-offset-black'
								]}
								style:background-color={COLORS[name]}
							></span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	{/each}
</div>
