<script lang="ts">
	// The consecutive-letter search: a word asked for, typed or tapped on a
	// keyboard of the whole alphabet, then run — an ELS at skip 0. A modal
	// like the passage, the map inert beneath; its chrome is English, the word
	// and its keys Hebrew.
	import { erase, insert, KEYS, letters } from '$lib/letters';

	let {
		title,
		onrun,
		onclose
	}: {
		title: string;
		/** The word, and whether its final forms are told apart. */
		onrun: (word: string, exact: boolean) => void;
		onclose: () => void;
	} = $props();

	let dialog = $state<HTMLDialogElement | null>(null);
	let input = $state<HTMLInputElement | null>(null);
	let word = $state('');
	// Exact letters (final forms): a final matches only itself. Off, the
	// search folds them, as it always did.
	let exact = $state(false);
	// Where the next letter goes: the input's selection, kept while a key
	// holds the focus.
	let from = 0;
	let to = 0;

	// Opened at once. A mouse takes the input, ready to type; a finger keeps
	// off it, or the phone's own keyboard would cover the one drawn here.
	$effect(() => {
		if (!dialog || dialog.open) return;
		dialog.showModal();
		if (matchMedia('(pointer: fine)').matches) input?.focus();
		else dialog.focus();
	});

	function keep() {
		from = input?.selectionStart ?? word.length;
		to = input?.selectionEnd ?? from;
	}

	// The input's own typing filtered to letters, the caret kept where it was
	// less whatever was dropped before it.
	function onInput() {
		if (!input) return;
		const caret = letters(input.value.slice(0, input.selectionStart ?? input.value.length)).length;
		word = letters(input.value);
		input.value = word;
		input.setSelectionRange(caret, caret);
		keep();
	}

	function place(next: { word: string; caret: number }) {
		word = next.word;
		from = to = next.caret;
		if (input) {
			input.value = word;
			if (document.activeElement === input) input.setSelectionRange(from, to);
		}
	}

	function run() {
		if (!word) return;
		onrun(word, exact);
		close();
	}

	// Closed here or by the browser (Escape), the page hears it once, from
	// the dialog's own `close`.
	function close() {
		dialog?.close();
	}

	// Escape is the dialog's: it closes it, and goes no further — the window
	// would take it back to the overview.
	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') event.stopPropagation();
	}

	// A click on the backdrop — the dialog itself, never its content — is a
	// way out.
	function onClick(event: MouseEvent) {
		if (event.target === dialog) close();
	}

	const key = [
		'grid h-11 cursor-pointer place-items-center rounded border border-white/10 font-hebrew text-xl text-white/90',
		'transition-colors duration-100 select-none sm:h-12 sm:text-2xl',
		'hover:border-white/25 hover:bg-white/6 hover:text-white active:bg-white/12',
		'focus-visible:border-white/40 focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-1 focus-visible:outline-white/60'
	];
</script>

<dialog
	bind:this={dialog}
	dir="ltr"
	lang="en"
	tabindex="-1"
	aria-labelledby="letter-search-title"
	onclick={onClick}
	{onclose}
	onkeydown={onKeydown}
	class="m-auto w-[calc(100%-2rem)] max-w-lg flex-col overflow-hidden rounded-xl border border-white/15
	       bg-black p-0 text-white transition-[opacity,scale] duration-200 ease-out outline-none
	       backdrop:bg-black/80 open:flex motion-reduce:transition-none starting:scale-[0.98] starting:opacity-0"
>
	<header class="flex shrink-0 items-center gap-3 py-2 ps-5 pe-2 sm:ps-6 sm:pe-3">
		<h2 id="letter-search-title" class="flex-1 truncate text-base tracking-wide text-white/90">
			{title}
		</h2>
		<button
			type="button"
			aria-label="Close"
			class="grid size-10 shrink-0 cursor-pointer place-items-center rounded text-white/58 transition-colors duration-150
			       hover:bg-white/[0.06] hover:text-white focus-visible:text-white
			       focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60"
			onclick={close}
		>
			<svg viewBox="0 0 256 256" class="size-5 fill-current" aria-hidden="true">
				<path
					d="M204.24,195.76a6,6,0,1,1-8.48,8.48L128,136.49,60.24,204.24a6,6,0,0,1-8.48-8.48L119.51,128,51.76,60.24a6,6,0,0,1,8.48-8.48L128,119.51l67.76-67.75a6,6,0,0,1,8.48,8.48L136.49,128Z"
				/>
			</svg>
		</button>
	</header>

	<form
		class="flex flex-col gap-5 px-5 pt-2 pb-5 sm:gap-6 sm:px-6 sm:pb-6"
		onsubmit={(event) => {
			event.preventDefault();
			run();
		}}
	>
		<!-- The word: Hebrew, from the right, a rule under it as a line to
		     write on. The erase key stands at its end, the left. -->
		<div
			class="flex items-center gap-2 border-b border-white/25 transition-colors duration-150 focus-within:border-white/70"
		>
			<button
				type="button"
				aria-label="Erase a letter"
				disabled={!word}
				class="grid size-10 shrink-0 cursor-pointer place-items-center rounded text-white/58 transition-colors duration-150
				       hover:bg-white/[0.06] hover:text-white focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-2
				       focus-visible:outline-white/60 disabled:cursor-default disabled:text-white/20 disabled:hover:bg-transparent"
				onclick={() => place(erase(word, from, to))}
			>
				<!-- Phosphor backspace, light. -->
				<svg viewBox="0 0 256 256" class="size-5 fill-current" aria-hidden="true">
					<path
						d="M216,42H68.53a14.06,14.06,0,0,0-12,6.8L9.65,124.91a6,6,0,0,0,0,6.18L56.5,207.2a14.06,14.06,0,0,0,12,6.8H216a14,14,0,0,0,14-14V56A14,14,0,0,0,216,42Zm2,158a2,2,0,0,1-2,2H68.53a2,2,0,0,1-1.71-1L21.8,128,66.82,55a2,2,0,0,1,1.71-1H216a2,2,0,0,1,2,2Zm-53.76-98.24L144.49,128l19.75,19.76a6,6,0,1,1-8.48,8.48L136,136.49l-19.76,19.75a6,6,0,0,1-8.48-8.48L127.51,128l-19.75-19.76a6,6,0,0,1,8.48-8.48L136,119.51l19.76-19.75a6,6,0,0,1,8.48,8.48Z"
					/>
				</svg>
			</button>
			<input
				bind:this={input}
				dir="rtl"
				lang="he"
				aria-label="Word"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
				value={word}
				oninput={onInput}
				onselect={keep}
				onkeyup={keep}
				onpointerup={keep}
				onblur={keep}
				class="h-16 min-w-0 flex-1 bg-transparent text-right font-hebrew text-4xl tracking-wide text-white caret-white
				       outline-none selection:bg-white/25 sm:text-5xl"
			/>
		</div>

		<!-- The alphabet in its order, from the right, each final after its
		     letter: three rows of nine. A key writes at the caret and keeps the
		     focus, so a phone's keyboard never opens over this one. -->
		<div
			dir="rtl"
			lang="he"
			class="grid grid-cols-9 gap-1 sm:gap-1.5"
			role="group"
			aria-label="Hebrew letters"
		>
			{#each KEYS as letter (letter)}
				<button
					type="button"
					class={key}
					onpointerdown={(event) => event.preventDefault()}
					onclick={() => place(insert(word, from, to, letter))}
				>
					{letter}
				</button>
			{/each}
		</div>

		<!-- The option, then the actions: side by side where they fit, the
		     option above them on a phone. -->
		<div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
			<button
				type="button"
				role="switch"
				aria-checked={exact}
				class="group -ml-1.5 flex h-10 cursor-pointer items-center gap-2.5 rounded px-1.5 text-sm transition-colors duration-150
				       focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60
				       {exact ? 'text-white' : 'text-white/58 hover:text-white'}"
				onclick={() => (exact = !exact)}
			>
				<span
					class="relative h-4.5 w-8 shrink-0 rounded-full border transition-colors duration-150
					       {exact ? 'border-white/60 bg-white/[0.12]' : 'border-white/25 group-hover:border-white/40'}"
					aria-hidden="true"
				>
					<span
						class="absolute top-1/2 left-0.5 size-3 -translate-y-1/2 rounded-full transition-[translate,background-color] duration-150
						       motion-reduce:transition-none
						       {exact ? 'translate-x-3.5 bg-white' : 'bg-white/58 group-hover:bg-white/80'}"
					></span>
				</span>
				Exact letters (final forms)
			</button>

			<div class="ml-auto flex items-center gap-2">
				<button
					type="button"
					disabled={!word}
					class="h-10 cursor-pointer rounded px-4 text-sm text-white/58 transition-colors duration-150
				       hover:bg-white/[0.06] hover:text-white focus-visible:text-white focus-visible:outline-1 focus-visible:outline-offset-2
				       focus-visible:outline-white/60 disabled:cursor-default disabled:text-white/20 disabled:hover:bg-transparent"
					onclick={() => place({ word: '', caret: 0 })}
				>
					Clear
				</button>
				<button
					type="submit"
					disabled={!word}
					class="h-10 cursor-pointer rounded border border-white/15 px-5 text-sm tracking-wide text-white transition-colors duration-150
				       hover:border-white/30 hover:bg-white/[0.06] focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white/60
				       disabled:cursor-default disabled:border-white/10 disabled:text-white/20 disabled:hover:bg-transparent"
				>
					Run
				</button>
			</div>
		</div>
	</form>
</dialog>
