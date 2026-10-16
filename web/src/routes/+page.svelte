<script lang="ts">
	import TanakhMap from '$components/TanakhMap.svelte';
</script>

<svelte:head>
	<title>ethaor</title>
	<meta
		name="description"
		content="The Tanakh as a zoomable map: three sections, the books in order, the verses one per line."
	/>
</svelte:head>

<main class="h-dvh w-screen bg-black">
	<TanakhMap />
</main>
