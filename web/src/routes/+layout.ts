// The application is entirely client-side: the canvas and the WebAssembly
// module have no business on the server. This avoids a render thrown away at
// once.
export const ssr = false;
export const prerender = true;
