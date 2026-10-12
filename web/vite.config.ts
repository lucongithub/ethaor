import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		host: '127.0.0.1',
		port: 8095,
		strictPort: false
	},
	// `vite preview` does not inherit `server`: without this it would fall back
	// to its default port, 4173.
	preview: {
		host: '127.0.0.1',
		port: 8095,
		strictPort: false
	}
});
