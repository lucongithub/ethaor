<script lang="ts">
	// The operator's logs, live: `/logs/?token=…` once, then `/logs/` alone
	// (see `logs-view.ts`). Black, white, monospaced — read, not decorated.
	import { onMount, tick } from 'svelte';
	import { follow, keep, KEY, rows, takeToken, type Row, type State } from '$lib/logs-view';

	const LEVELS = ['error', 'warn', 'info', 'debug', 'trace'];
	// Each level its own ink, read at a glance down the column; the gravest
	// bold too. Written whole so Tailwind finds the classes.
	const INK: Record<string, string> = {
		ERROR: 'font-bold text-red-400',
		WARN: 'font-bold text-amber-300',
		INFO: 'text-sky-400',
		DEBUG: 'text-emerald-400',
		TRACE: 'text-violet-400'
	};

	let token = $state<string | null>(null);
	let typed = $state('');
	let level = $state('info');
	let lines = $state.raw<Row[]>([]);
	// The next line's own number: never reused, so a row keeps its key.
	let next = 0;
	let status = $state<State>({ kind: 'connecting' });
	let lagged = $state(0);
	let pinned = $state(true);
	let list: HTMLElement | undefined = $state();
	let stop: AbortController | null = null;

	function store(value: string | null) {
		try {
			if (value) sessionStorage.setItem(KEY, value);
			else sessionStorage.removeItem(KEY);
		} catch {
			// No storage (private window): the token lives as long as the page.
		}
	}

	function start() {
		stop?.abort();
		if (!token) return;
		stop = new AbortController();
		lagged = 0;
		void follow({
			fetch: (...a) => fetch(...a),
			token,
			level,
			signal: stop.signal,
			onLines: async (added, fresh) => {
				lines = keep(lines, rows(added, next), fresh);
				next += added.length;
				if (pinned) {
					await tick();
					list?.scrollTo({ top: list.scrollHeight });
				}
			},
			onLagged: (files) => (lagged += files),
			onState: (s) => {
				status = s;
				// A token refused is forgotten: the field asks again.
				if (s.kind === 'refused' && s.status === 401) {
					store(null);
					token = null;
				}
			}
		});
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		const value = typed.trim();
		if (!value) return;
		token = value;
		typed = '';
		store(value);
		start();
	}

	function forget() {
		stop?.abort();
		store(null);
		token = null;
		lines = [];
		status = { kind: 'connecting' };
	}

	function scrolled() {
		if (list) pinned = list.scrollHeight - list.scrollTop - list.clientHeight < 40;
	}

	onMount(() => {
		const { token: given, clean } = takeToken(new URL(location.href));
		// Out of the address before anything else: the history keeps `/logs/`.
		if (clean !== null) history.replaceState(history.state, '', clean);
		if (given) store(given);
		try {
			token = given ?? sessionStorage.getItem(KEY);
		} catch {
			token = given;
		}
		start();
		return () => stop?.abort();
	});
</script>

<svelte:head>
	<title>ethaor logs</title>
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex" />
</svelte:head>

<main
	lang="en"
	dir="ltr"
	class="flex h-dvh w-screen flex-col bg-black font-mono text-xs text-white"
>
	{#if token === null}
		<form onsubmit={submit} class="m-auto flex flex-col gap-3 p-4">
			<label for="token">Logs token (ETHAOR_LOGS_TOKEN)</label>
			<input
				id="token"
				type="password"
				autocomplete="off"
				bind:value={typed}
				class="w-80 border border-white bg-black px-2 py-1 text-white"
			/>
			<button type="submit" class="border border-white px-2 py-1">Follow the logs</button>
			{#if status.kind === 'refused'}
				<p>{status.message}{status.incident ? ` (${status.incident})` : ''}</p>
			{/if}
		</form>
	{:else}
		<header class="flex flex-wrap items-center gap-4 border-b border-white/30 px-3 py-2">
			<span>ethaor logs</span>
			<label>
				level
				<select
					bind:value={level}
					onchange={start}
					class="border border-white/50 bg-black px-1 text-white"
				>
					{#each LEVELS as l (l)}
						<option value={l}>{l}</option>
					{/each}
				</select>
			</label>
			<span>{lines.length} lines</span>
			<span>
				{#if status.kind === 'live'}live
				{:else if status.kind === 'connecting'}connecting…
				{:else if status.kind === 'retrying'}retrying: {status.why}
				{:else}{status.status}
					{status.code}: {status.message}{status.incident ? ` (${status.incident})` : ''}
				{/if}
			</span>
			{#if lagged > 0}<span>{lagged} files missed</span>{/if}
			{#if !pinned}<span>paused: scroll to the bottom to follow</span>{/if}
			<button onclick={forget} class="ml-auto border border-white/50 px-2">Forget the token</button>
		</header>
		<ol bind:this={list} onscroll={scrolled} class="flex-1 overflow-auto px-3 py-2">
			{#each lines as line (line.id)}
				<!-- Off screen, a row is not laid out nor painted. -->
				<li
					class="break-all whitespace-pre-wrap [contain-intrinsic-size:auto_1rem] [content-visibility:auto]"
				>
					<span class="text-white/60">{line.time}</span>
					<span class={INK[line.level] ?? ''}>{line.level.padEnd(5)}</span>
					{line.message}
					<span class="text-white/60">{line.fields}</span>
				</li>
			{/each}
		</ol>
	{/if}
</main>
