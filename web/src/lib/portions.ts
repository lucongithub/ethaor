// The three portions' names, alone: the book view's first level and the
// map's opening show them at one size, so switching views moves nothing.
// That size is Tailwind's type scale, stepped up at each breakpoint — read
// off the map's opening on common screens and rounded to the nearest step.
// The book view lays its names with these classes; the bridge measures them
// and hands the size to the module (`set_portion_size`). One string, so the
// two cannot drift apart. The `min-[…rem]:` steps are in `rem`: in `px`,
// Tailwind sorts them before `2xl:` and they never apply.
export const PORTION_TEXT =
	'text-5xl xl:text-6xl 2xl:text-7xl min-[120rem]:text-8xl min-[160rem]:text-9xl';
