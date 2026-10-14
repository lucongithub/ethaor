import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess({
		typescript: true
	}),

	kit: {
		// Static: the app has no server code — the search, like everything
		// else, runs in the visitor's browser — so `build/` is plain files any
		// host or CDN can serve, with nothing to keep running.
		adapter: adapter(),
		alias: {
			$components: 'src/lib/components',
			$lib: 'src/lib',
			$routes: 'src/routes'
		}
	}
};

export default config;
