<script lang="ts">
	// ethaor's mark: two nuns facing each other, like brackets, a point
	// between them — `[·]`. The Tanakh sets two inverted nuns around Numbers
	// 10:35–36 the same way, the one place its own text is bracketed by
	// letters; brackets are how the map sets the other edition's reading
	// apart; and the point is the text they hold, as a single point of light.
	//
	// Set in the Tanakh's own face, Noto Serif Hebrew — the letters of the
	// mark are the letters of the text — light, so its line is the menu
	// icons' line (Phosphor light). Weight 280, not 200: in the same ink the
	// serif's hairlines read greyer than the icons' even line, measured on
	// screen, so the mark is a touch heavier to look as bright. The left nun is mirrored,
	// the right one reads as written. Nuns and point take the colour they are
	// given — the icons' own at rest, in the menu, white on hover.
	let { class: classes = '' }: { class?: string } = $props();
</script>

<span
	role="img"
	aria-label="Ethaor"
	dir="ltr"
	class={[
		'inline-flex items-center justify-center gap-[0.1em] font-hebrew leading-none font-[280] select-none',
		classes
	]}
>
	<span class="inline-block -scale-x-100" aria-hidden="true">נ</span>
	<svg viewBox="0 0 4 4" class="size-[0.15em] shrink-0 fill-current" aria-hidden="true"
		><circle cx="2" cy="2" r="1.6" /></svg
	>
	<span class="inline-block" aria-hidden="true">נ</span>
</span>
