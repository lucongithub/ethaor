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
	// `vite preview` n'herite pas de `server` : sans cela il repartirait sur
	// son port par defaut, 4173.
	preview: {
		host: '127.0.0.1',
		port: 8095,
		strictPort: false
	}
});
