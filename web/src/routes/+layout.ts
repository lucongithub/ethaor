// L'application est entierement client : le canevas et le module WebAssembly
// n'ont rien a faire sur le serveur. On evite ainsi un rendu jete aussitot.
export const ssr = false;
export const prerender = true;
