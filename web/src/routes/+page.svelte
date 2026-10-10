<script lang="ts">
	import TanakhMap from '$components/TanakhMap.svelte';
</script>

<svelte:head>
	<title>ethaor</title>
	<meta
		name="description"
		content="Le Tanakh en carte zoomable : trois sections, les livres dans l’ordre, les versets un par ligne."
	/>
</svelte:head>

<main class="h-dvh w-screen bg-black">
	<TanakhMap />
</main>
