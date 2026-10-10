/**
 * Pont vers le module WebAssembly.
 *
 * Tout le calcul vit en Rust : corpus, disposition, camera, repli du zoom,
 * elagage, detection de clic. Ce fichier ne fait que transmettre les gestes et
 * peindre la liste de commandes produite.
 *
 * Deux choses lui reviennent en propre, et une seule raison a cela : le
 * faconnage de l'hebreu pointe — bidi et marques combinantes — vit dans le
 * navigateur, et rien d'autre ne le fait aussi bien.
 *
 *   1. mesurer la largeur des fragments que Rust reclame ;
 *   2. tracer le texte.
 */

/** Doit couvrir l'hebreu avec niqqud. Noto Serif Hebrew est charge par app.css. */
export const HEBREW_FONT =
	'"Noto Serif Hebrew Variable", "Taamey Frank CLM", "SBL Hebrew", "David", serif';

/** Taille de reference pour la mesure : la largeur est proportionnelle. */
const REFERENCE = 100;

const KIND_RECT = 0;
const KIND_TEXT = 1;
const KIND_CLIP = 2;
const KIND_UNCLIP = 3;

/** Signature des exports du module. Tous les flottants sont des pixels CSS. */
interface Wasm {
	memory: WebAssembly.Memory;
	init(width: number, height: number): number;
	resize(width: number, height: number): void;
	reset(): void;
	pan(dx: number, dy: number): void;
	zoom(x: number, y: number, factor: number): void;
	hover(x: number, y: number): number;
	select(x: number, y: number): number;
	selection_ptr(): number;
	focus(): void;
	focus_ptr(): number;
	build_frame(): number;
	frame_ptr(): number;
	frame_stride(): number;
	pending_ptr(): number;
	pending_len(): number;
	set_ratio(offset: number, length: number, ratio: number): void;
	clear_ratios(): void;
	text_ptr(): number;
	text_len(): number;
}

export interface Selection {
	book: number;
	chapter: number;
	verse: string;
}

/** Ou se trouve le centre de la vue. Alimente le fil d'Ariane. */
export interface Focus {
	section: string;
	book: string;
	chapter: string;
}

export interface TanakhMap {
	/** Redemande une image. Sans effet si une est deja programmee. */
	draw(): void;
	/** Revient a la vue d'ensemble. */
	reset(): void;
	/** Detache les ecouteurs et libere l'observateur de taille. */
	destroy(): void;
	readonly books: number;
}

export interface MapOptions {
	/** Appele au clic : le verset touche, ou `null` si le clic tombe a vide. */
	onSelect?: (selection: Selection | null) => void;
	/**
	 * Appele a chaque image : ou l'on se trouve. Les bandes de titre restent
	 * dans la carte et ne recouvrent donc jamais rien ; quand elles sortent de
	 * l'ecran, c'est le fil d'Ariane qui dit ou l'on est.
	 */
	onFocus?: (focus: Focus | null) => void;
	/** Chemin du module, prefixe par `base` en cas de sous-repertoire. */
	wasmUrl?: string;
}

export async function createTanakhMap(
	canvas: HTMLCanvasElement,
	options: MapOptions = {}
): Promise<TanakhMap> {
	const { onSelect, onFocus, wasmUrl = '/ethaor.wasm' } = options;

	const context = canvas.getContext('2d', { alpha: false });
	if (!context) throw new Error('canvas 2d indisponible');
	const ctx = context;

	// La police doit etre chargee **avant** la premiere mesure : une largeur
	// relevee sur une police de repli serait mise en cache et fausse pour de bon.
	if (document.fonts) {
		try {
			await document.fonts.load(`16px ${HEBREW_FONT}`, 'א');
			await document.fonts.ready;
		} catch {
			// Police indisponible : le repli systeme fera l'affaire.
		}
	}

	const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl), {});
	const wasm = instance.exports as unknown as Wasm;

	// Si la police definitive arrive apres les premieres mesures, celles-ci ont
	// ete relevees sur une police de repli : il faut les oublier, sinon le texte
	// reste mal cadre pour toute la session.
	document.fonts?.ready.then(() => {
		wasm.clear_ratios();
		draw();
	});

	const decoder = new TextDecoder();
	const decoded = new Map<number, string>();

	/** Fragment du corpus, lu directement dans la memoire du module. */
	function fragment(offset: number, length: number): string {
		const key = offset * 1024 + length;
		let text = decoded.get(key);
		if (text === undefined) {
			const bytes = new Uint8Array(wasm.memory.buffer, wasm.text_ptr() + offset, length);
			text = decoder.decode(bytes);
			decoded.set(key, text);
		}
		return text;
	}

	// ----------------------------------------------------------------- rendu

	let width = 0;
	let height = 0;
	let dpr = 1;

	function measure() {
		const rect = canvas.getBoundingClientRect();
		dpr = Math.min(window.devicePixelRatio || 1, 2);
		width = Math.max(1, rect.width);
		height = Math.max(1, rect.height);
		canvas.width = Math.round(width * dpr);
		canvas.height = Math.round(height * dpr);
	}

	function paint() {
		const words = wasm.build_frame();
		const stride = wasm.frame_stride();
		const commands = new Float32Array(wasm.memory.buffer, wasm.frame_ptr(), words);

		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.fillStyle = '#000';
		ctx.fillRect(0, 0, width, height);

		// Le navigateur pose le texte sur son bord droit et applique le bidi.
		ctx.textAlign = 'right';
		ctx.textBaseline = 'alphabetic';
		ctx.direction = 'rtl';

		// Profondeur de decoupage ouverte, pour la refermer proprement en fin
		// d'image si une commande manquait.
		let clips = 0;

		for (let i = 0; i < words; i += stride) {
			const kind = commands[i];
			const x = commands[i + 1];
			const y = commands[i + 2];
			const w = commands[i + 3];
			const h = commands[i + 4];

			// Le texte d'un chapitre est enferme dans sa boite : une largeur mal
			// mesuree le ferait sinon mordre sur ses voisins.
			if (kind === KIND_CLIP) {
				ctx.save();
				ctx.beginPath();
				ctx.rect(x, y, w, h);
				ctx.clip();
				clips++;
				continue;
			}

			if (kind === KIND_UNCLIP) {
				if (clips > 0) {
					ctx.restore();
					clips--;
				}
				continue;
			}

			// Aplat sans cadre : les boites ne sont dessinees que par leur
			// teinte.
			if (kind === KIND_RECT) {
				const fill = commands[i + 5];
				if (fill > 0.003) {
					ctx.fillStyle = `rgba(255,255,255,${fill})`;
					ctx.fillRect(x, y, w, h);
				}
				continue;
			}

			ctx.font = `${commands[i + 6]}px ${HEBREW_FONT}`;
			ctx.fillStyle = `rgba(255,255,255,${commands[i + 5]})`;
			ctx.fillText(fragment(commands[i + 7], commands[i + 8]), x, y);
		}

		while (clips > 0) {
			ctx.restore();
			clips--;
		}

		reportFocus();
		measurePending();
	}

	let lastFocus = '';
	function reportFocus() {
		if (!onFocus) return;

		wasm.focus();
		const view = new Uint32Array(wasm.memory.buffer, wasm.focus_ptr(), 6);
		const read = (i: number) => (view[i + 1] ? fragment(view[i], view[i + 1]) : '');

		const section = read(0);
		if (!section) {
			if (lastFocus) {
				lastFocus = '';
				onFocus(null);
			}
			return;
		}

		const next: Focus = { section, book: read(2), chapter: read(4) };
		const key = `${next.section}|${next.book}|${next.chapter}`;
		if (key === lastFocus) return;

		lastFocus = key;
		onFocus(next);
	}

	/**
	 * Rust dessine d'abord avec une estimation, puis inscrit les fragments dont
	 * il ignore la largeur. On les mesure et on redemande une image : elle sera
	 * juste. Chaque fragment n'est mesure qu'une fois.
	 */
	function measurePending() {
		const count = wasm.pending_len();
		if (count === 0) return;

		// Copie : `set_ratio` peut faire croitre la memoire du module et
		// invalider la vue.
		const pairs = Array.from(new Uint32Array(wasm.memory.buffer, wasm.pending_ptr(), count));

		ctx.font = `${REFERENCE}px ${HEBREW_FONT}`;
		for (let i = 0; i < pairs.length; i += 2) {
			const offset = pairs[i];
			const length = pairs[i + 1];
			const ratio = ctx.measureText(fragment(offset, length)).width / REFERENCE;
			wasm.set_ratio(offset, length, ratio);
		}

		draw();
	}

	let frame: number | null = null;
	function draw() {
		frame ??= requestAnimationFrame(() => {
			frame = null;
			paint();
		});
	}

	// ---------------------------------------------------------------- gestes

	const pointers = new Map<number, { x: number; y: number }>();
	let pinch = 0;
	let moved = false;

	const local = (event: PointerEvent) => {
		const rect = canvas.getBoundingClientRect();
		return { x: event.clientX - rect.left, y: event.clientY - rect.top };
	};

	const spread = () => {
		const [a, b] = [...pointers.values()];
		return Math.hypot(a.x - b.x, a.y - b.y);
	};

	const midpoint = () => {
		const [a, b] = [...pointers.values()];
		return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
	};

	function onPointerDown(event: PointerEvent) {
		canvas.setPointerCapture(event.pointerId);
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		moved = false;
		if (pointers.size === 2) pinch = spread();
	}

	function onPointerMove(event: PointerEvent) {
		if (!pointers.has(event.pointerId)) {
			const { x, y } = local(event);
			canvas.style.cursor = wasm.hover(x, y) < 0 ? 'default' : 'pointer';
			draw();
			return;
		}

		const previous = pointers.get(event.pointerId)!;
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });

		if (pointers.size === 2) {
			const now = spread();
			if (pinch > 0 && now > 0) {
				const rect = canvas.getBoundingClientRect();
				const mid = midpoint();
				wasm.zoom(mid.x - rect.left, mid.y - rect.top, now / pinch);
			}
			pinch = now;
			moved = true;
			draw();
			return;
		}

		const dx = event.clientX - previous.x;
		const dy = event.clientY - previous.y;
		if (Math.abs(dx) + Math.abs(dy) > 3) moved = true;
		wasm.pan(dx, dy);
		draw();
	}

	function onPointerUp(event: PointerEvent) {
		if (!pointers.has(event.pointerId)) return;
		const single = pointers.size === 1;
		pointers.delete(event.pointerId);
		pinch = pointers.size === 2 ? spread() : 0;

		if (single && !moved) {
			const { x, y } = local(event);
			onSelect?.(selectionAt(x, y));
		}
		draw();
	}

	function onWheel(event: WheelEvent) {
		event.preventDefault();
		const rect = canvas.getBoundingClientRect();
		// Un cran vaut 1,4 : il faut une dizaine de crans pour passer de la vue
		// d'ensemble au texte d'un chapitre, et c'est deja long. A 1,22,
		// l'ancien pas, il en fallait vingt et un. Le pave tactile envoie des
		// fractions de cran, donc le reglage fin reste possible.
		const steps = event.deltaMode === 1 ? -event.deltaY : -event.deltaY / 100;
		wasm.zoom(event.clientX - rect.left, event.clientY - rect.top, Math.pow(1.4, steps));
		draw();
	}

	/**
	 * Point d'accroche pour la suite : Rust renvoie le chemin complet, de quoi
	 * afficher demain ce que l'on sait d'un mot ou d'une lettre.
	 */
	function selectionAt(x: number, y: number): Selection | null {
		if (wasm.select(x, y) === 0) return null;

		const view = new Uint32Array(wasm.memory.buffer, wasm.selection_ptr(), 4);
		const [book, chapter, offset, length] = view;
		if (length === 0) return null;

		return { book, chapter, verse: fragment(offset, length) };
	}

	function onResize() {
		measure();
		wasm.resize(width, height);
		draw();
	}

	// ---------------------------------------------------------------- depart

	measure();
	const books = wasm.init(width, height);

	canvas.addEventListener('pointerdown', onPointerDown);
	canvas.addEventListener('pointermove', onPointerMove);
	canvas.addEventListener('pointerup', onPointerUp);
	canvas.addEventListener('pointercancel', onPointerUp);
	canvas.addEventListener('wheel', onWheel, { passive: false });

	const observer = new ResizeObserver(onResize);
	observer.observe(canvas);

	draw();

	return {
		books,
		draw,
		reset() {
			wasm.reset();
			draw();
		},
		destroy() {
			observer.disconnect();
			canvas.removeEventListener('pointerdown', onPointerDown);
			canvas.removeEventListener('pointermove', onPointerMove);
			canvas.removeEventListener('pointerup', onPointerUp);
			canvas.removeEventListener('pointercancel', onPointerUp);
			canvas.removeEventListener('wheel', onWheel);
			if (frame !== null) cancelAnimationFrame(frame);
		}
	};
}
