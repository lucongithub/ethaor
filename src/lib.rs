//! ethaor — le Tanakh en carte zoomable, sans fond.
//!
//! Tout le calcul vit ici, compile en WebAssembly : corpus, disposition,
//! camera, repli du zoom, elagage, detection de clic. La page ne fait que
//! peindre la liste de commandes produite a chaque image. Rien ne transite par
//! le reseau pendant l'interaction.
//!
//! Une seule chose reste au navigateur : **mesurer et tracer le texte**. C'est
//! la que vit le faconnage de l'hebreu pointe — bidi, marques combinantes — et
//! aucune implementation maison ne le ferait aussi bien. Rust demande les
//! largeurs qui lui manquent, les met en cache, et n'y revient plus.

mod corpus;
mod hier;
mod treemap;

use std::collections::HashMap;

use treemap::{Layout, Rect, WORLD};

// ---------------------------------------------------------------- reglages

/// Part de la cellule d'une lettre qu'occupe le monde qu'elle contient.
const NEST: f32 = 0.004;

/// Le point n'apparait qu'une fois la lettre plus large que l'ecran : on peut
/// donc zoomer pleinement une lettre sans rien voir arriver.
const NEST_AFTER: f32 = 1.0;

/// En deca, le texte ne serait qu'une bavure grise.
const MIN_TEXT: f32 = 4.0;

/// Plancher propre aux numeros de chapitre, plus bas que celui du texte
/// courant : un numero compte une a trois lettres, connues d'avance et lues
/// comme une forme, la ou une ligne de verset doit se dechiffrer.
///
/// C'est ce qui garantit la promesse « les chapitres des le premier cran » : au
/// cran suivant le cadrage, un numero mesure 3,4 px sur une fenetre de 730 px
/// de haut. Au plancher du texte courant il aurait fallu deux crans — et c'est
/// une forme connue d'avance que l'on reconnait a cette taille, pas un mot a
/// dechiffrer.
const MIN_NUMBER: f32 = 3.3;

/// Hierarchie des blancs : une section est en blanc pur, un livre un ton
/// en dessous, un numero de chapitre un rien plus bas encore. Le rang se lit
/// donc a l'intensite, sans qu'aucun cadre ne soit necessaire.
const BOOK_INK: f32 = 0.78;
const CHAPTER_INK: f32 = 0.58;

/// Part de la bande de titre occupee par le texte.
///
/// Le titre est dimensionne par **la bande**, qui est en unites monde : il
/// grandit donc avec le zoom, exactement comme la boite et les chapitres, et
/// l'espace entre le haut du cadre et le premier chapitre garde la meme allure.
/// La bande etant de hauteur constante, tous les livres portent la meme taille
/// de titre a un zoom donne.
const TITLE_FILL: f32 = 1.0;

/// Plafond, en part de la hauteur de fenetre. Passe ce point la bande a quitte
/// l'ecran — on est entre dans le livre — et le titre n'a plus a grandir : il
/// n'est la que pour rappeler ou l'on se trouve.
const TITLE_CEILING: f32 = 0.085;

/// Hauteur de ligne minimale pour qu'un chapitre montre ses versets.
///
/// Juste au-dessus du plancher du texte : a l'interligne de 1,30, une ligne de
/// 5,5 px porte des caracteres de 4,2 px, soit `MIN_TEXT`. Exiger davantage ne
/// gagnait rien en lisibilite et repoussait les versets de deux crans de zoom.
const READABLE_LINE: f32 = 5.5;

/// Marge interieure d'une case, en **part de son cote**.
///
/// Fixe a 2 unites monde, elle ne laissait que 5,9 unites de texte dans une
/// case de 9,9 — 40 % de l'encre jetee, en largeur comme en hauteur, et des
/// versets qui n'apparaissaient que six crans de zoom plus tard. La marge d'une
/// case doit suivre la case, comme tout le reste du rendu.
const CELL_PAD: f32 = 0.06;

/// Au-dela de ce multiple du cadrage initial, les numeros de chapitre
/// apparaissent.
///
/// Au lancement la carte n'affiche donc que les sections et les livres, en
/// grand ; les numeros arrivent des le premier cran de zoom. Le seuil est
/// relatif au cadrage et non a une taille de case : il tombe ainsi au meme
/// endroit quelle que soit la fenetre, et apres un repli comme avant.
const CHAPTER_FROM: f32 = 1.1;

/// Les titres de section sont deux tiers plus grands que ceux des livres.
const SECTION_OVER_BOOK: f32 = 1.25;

/// Tailles d'un niveau. Chaque monde dessine a les siennes.
#[derive(Clone, Copy)]
struct Scale {
    show_chapters: bool,
    chapter: f32,
    book: f32,
    section: f32,
}

/// Largeur estimee d'une lettre, le temps que le navigateur mesure vraiment.
const GUESS: f32 = 0.42;

/// Largeur estimee du numero le plus large — trois lettres — dont part la
/// disposition. `build_frame` la refait des que la page a mesure la vraie.
const ESTIMATED_NUMERAL: f32 = 3.0 * GUESS;

/// En deca de cette largeur, le monde est dessine comme un simple point : ses
/// bandes seraient de toute facon sous le demi-pixel.
const STRUCTURE_FROM: f32 = 40.0;

/// Le monde ne descend jamais sous cette largeur a l'ecran : en de-zoomant, il
/// reste toujours au moins un point blanc, centre, qui represente le Tanakh.
/// Le de-zoom est donc borne, contrairement au zoom.
const MIN_WORLD_PX: f32 = 1.5;

// ------------------------------------------------------------------ sortie

/// Un enregistrement de la liste de commandes, en mots de 4 octets.
const STRIDE: usize = 10;

const KIND_RECT: f32 = 0.0;
const KIND_TEXT: f32 = 1.0;
/// Ouvre puis referme un decoupage. Le texte d'un chapitre ne doit jamais
/// mordre sur ses voisins, meme si une largeur mal mesuree le fait deborder.
const KIND_CLIP: f32 = 2.0;
const KIND_UNCLIP: f32 = 3.0;

// ------------------------------------------------------------------ camera

#[derive(Clone, Copy)]
struct Camera {
    z: f32,
    x: f32,
    y: f32,
}

impl Camera {
    fn project(&self, r: Rect) -> Rect {
        Rect { x: r.x * self.z + self.x, y: r.y * self.z + self.y, w: r.w * self.z, h: r.h * self.z }
    }

    fn to_world(&self, px: f32, py: f32) -> (f32, f32) {
        ((px - self.x) / self.z, (py - self.y) / self.z)
    }

    /// Re-enracine la camera dans `cell`, qui contient une copie du monde.
    /// L'image ne bouge pas ; seule l'echelle redescend, ce qui empeche les
    /// flottants de perdre leur precision.
    fn wrap_in(&mut self, cell: Rect) {
        let z = self.z;
        let z2 = cell.w * z / WORLD.w;
        self.x = cell.x * z + self.x;
        self.y = cell.y * z + self.y + (cell.h * z - WORLD.h * z2) / 2.0;
        self.z = z2;
    }
}

// ------------------------------------------------------------------- etat

/// Une ligne mise en page : une tranche de verset, sa boite et sa taille de
/// caractere. L'unite du rendu n'est pas le verset mais la ligne — un verset
/// long en occupe plusieurs.
#[derive(Clone, Copy)]
struct Line {
    rect: Rect,
    text: &'static str,
    verse: usize,
    size: f32,
}

struct App {
    layout: Layout,
    camera: Camera,
    width: f32,
    height: f32,
    /// Nombre de Tanakhs traverses. La descente est a sens unique : une fois
    /// le nouveau monde apparu, l'ancien n'existe plus.
    level: u32,
    /// Largeur d'un fragment pour une police de taille 1, mesuree par la page.
    ratios: HashMap<(u32, u32), f32>,
    /// Fragments dont la page doit encore donner la largeur.
    pending: Vec<u32>,
    commands: Vec<f32>,
    /// Diagnostic : points retenus a la derniere image.
    last_nests: u32,
    /// Une espace du corpus, pour en mesurer la largeur une fois pour toutes.
    space: Option<&'static str>,
    /// Mise en page des chapitres deja consultes. Invalidee des qu'une mesure
    /// arrive de la page.
    lines: HashMap<usize, Vec<Line>>,
    /// Taille des numeros de chapitre, en unites monde. Calculee une fois,
    /// jetee des qu'une mesure arrive de la page.
    number: Option<f32>,
    /// Largeur du numero le plus large, telle que la disposition courante l'a
    /// supposee. Une mesure qui la dementit refait la disposition.
    numeral: f32,
    selection: [u32; 4],
    focus: [u32; 6],
}

static mut APP: Option<App> = None;

#[allow(static_mut_refs)]
fn app() -> &'static mut App {
    unsafe { APP.as_mut().expect("init() n'a pas ete appele") }
}

impl App {
    fn fit_scale(&self) -> f32 {
        (self.width / WORLD.w).min(self.height / WORLD.h)
    }

    fn fit(&mut self) {
        self.camera.z = self.fit_scale();
        self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
        self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
    }

    /// Largeur du fragment pour une police de taille 1.
    ///
    /// Tant que la page ne l'a pas mesure, on repond une estimation et on
    /// inscrit le fragment dans `pending` : l'image suivante sera juste.
    fn ratio(&mut self, fragment: &'static str) -> f32 {
        let key = key_of(fragment);
        if let Some(&ratio) = self.ratios.get(&key) {
            return ratio;
        }
        if self.pending.len() < 4096 {
            self.pending.push(key.0);
            self.pending.push(key.1);
        }
        hier::letter_count(fragment).max(1) as f32 * GUESS
    }

    /// Largeur d'une espace, mesuree une fois sur une vraie espace du corpus.
    fn space_ratio(&mut self) -> f32 {
        let space = *self.space.get_or_insert_with(|| {
            let text = corpus::text();
            let at = text.find(' ').unwrap_or(0);
            &text[at..at + 1]
        });
        self.ratio(space)
    }

    /// Largeur d'un fragment, **deduite de ses mots** et non mesuree telle
    /// quelle.
    ///
    /// C'est ce qui rend la mise en page stable : mesurer la ligne entiere
    /// ferait dependre les fragments a mesurer du decoupage, lequel depend des
    /// mesures. Chaque nouvelle mise en page reclamerait de nouvelles mesures,
    /// qui invalideraient la mise en page — sans fin. Les mots, eux, ne
    /// changent jamais.
    fn width_of(&mut self, text: &'static str, size: f32) -> f32 {
        let space = self.space_ratio();
        let mut total = 0.0;
        let mut count = 0usize;

        for word in text.split(' ') {
            total += self.ratio(word);
            count += 1;
        }

        (total + count.saturating_sub(1) as f32 * space) * size
    }

    fn chapter_at(&self, x: f32, y: f32) -> Option<usize> {
        self.layout.chapters.iter().position(|r| r.contains(x, y))
    }

    /// Mise en page d'un chapitre, en coordonnees monde.
    ///
    /// **Tous les versets ont la meme taille de caractere.** Un verset trop
    /// long passe sur plusieurs lignes au lieu d'etre comprime : seul le
    /// nombre de lignes varie, jamais la taille. Une respiration separe les
    /// versets, de sorte qu'on voie ou l'un finit et ou l'autre commence.
    ///
    /// Calcule une fois par chapitre, puis mis en cache : la disposition ne
    /// depend que du monde, pas du zoom.
    fn chapter_lines(&mut self, chapter: usize) -> Vec<Line> {
        if let Some(lines) = self.lines.get(&chapter) {
            return lines.clone();
        }
        let laid = self.lay_out(chapter);
        self.lines.insert(chapter, laid.clone());
        laid
    }

    fn lay_out(&mut self, chapter: usize) -> Vec<Line> {
        let cell = column(self.layout.chapters[chapter]);
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);

        // Hauteur d'une ligne et respiration entre versets, en multiples de la
        // taille de caractere.
        let line_k = 1.30;
        let gap_k = 0.55;

        // On part d'une ligne par verset, puis on resserre tant que la colonne
        // deborde. Le nombre de lignes ne peut que croitre quand la taille
        // croit, donc quelques passes suffisent.
        let mut size = cell.h / (count as f32 * line_k + (count - 1) as f32 * gap_k);
        for _ in 0..5 {
            let total = self.column_height(chapter, cell.w, size, line_k, gap_k);
            if total <= cell.h {
                break;
            }
            size *= cell.h / total;
        }

        let mut out = Vec::new();
        let mut y = cell.y;

        for verse in verses {
            let text = corpus::verse_text(verse);
            for slice in self.wrap(text, cell.w, size) {
                out.push(Line {
                    rect: Rect { x: cell.x, y, w: cell.w, h: size * line_k },
                    text: slice,
                    verse,
                    size,
                });
                y += size * line_k;
            }
            y += size * gap_k;
        }

        out
    }

    /// Hauteur qu'occuperait le chapitre a cette taille de caractere.
    fn column_height(&mut self, chapter: usize, width: f32, size: f32, line_k: f32, gap_k: f32) -> f32 {
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);
        let mut lines = 0usize;

        for verse in verses {
            let drawn = self.width_of(corpus::verse_text(verse), size);
            let needed = if width > 0.0 { (drawn / width).ceil() as usize } else { 1 };
            lines += needed.max(1);
        }

        lines as f32 * size * line_k + (count - 1) as f32 * size * gap_k
    }

    /// Decoupe un verset en lignes qui tiennent dans `width`, aux mots.
    ///
    /// Chaque ligne reste une tranche du corpus : seuls un decalage et une
    /// longueur traversent la frontiere avec la page.
    fn wrap(&mut self, verse: &'static str, width: f32, size: f32) -> Vec<&'static str> {
        if width <= 0.0 || size <= 0.0 || self.width_of(verse, size) <= width {
            return vec![verse];
        }

        let words: Vec<&'static str> = verse.split(' ').collect();
        let space = self.space_ratio();

        let mut out = Vec::new();
        let mut start = 0usize; // debut de la ligne courante, en octets
        let mut at = 0usize; // debut du mot courant
        let mut run = 0.0f32;

        for word in &words {
            let w = self.ratio(word) * size;
            let first = at == start;
            let advance = if first { w } else { w + space * size };

            if !first && run + advance > width {
                out.push(&verse[start..at - 1]); // sans l'espace de fin
                start = at;
                run = w;
            } else {
                run += advance;
            }
            at += word.len() + 1;
        }

        out.push(&verse[start..]);
        out
    }

    /// Cellules des lettres d'une ligne, de droite a gauche.
    ///
    /// Le texte est dessine d'un seul tenant ; ce decoupage ne sert qu'a situer
    /// une lettre — pour le point qu'elle contient, et pour le clic.
    fn letter_cells(&mut self, text: &'static str, row: Rect, size: f32) -> Vec<Rect> {
        let drawn = self.width_of(text, size);
        if drawn <= 0.0 {
            return Vec::new();
        }

        let parts: Vec<&'static str> = text.split(' ').flat_map(hier::letters).collect();
        let total: f32 = parts.iter().map(|p| self.ratio(p)).sum();
        if total <= 0.0 {
            return Vec::new();
        }

        let mut right = row.x + row.w;
        parts
            .iter()
            .map(|part| {
                let w = drawn * self.ratio(part) / total;
                right -= w;
                Rect { x: right, w, ..row }
            })
            .collect()
    }

    /// Chapitre le plus proche d'un point, meme si celui-ci tombe dans un
    /// interstice entre deux boites.
    fn nearest_chapter(&self, x: f32, y: f32) -> Option<usize> {
        self.layout
            .chapters
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| gap_to(**a, x, y).total_cmp(&gap_to(**b, x, y)))
            .map(|(i, _)| i)
    }

    /// Livre dont la boite contient un point.
    fn book_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..corpus::book_count()).find(|&b| self.layout.books[b].contains(x, y))
    }

    /// Chemin complet sous un point du monde.
    ///
    /// Si le point tombe entre deux cases, on retient **la plus proche du meme
    /// livre**. Les cases sont espacees pour occuper toute la largeur de leur
    /// boite : chez un livre de trois chapitres, l'intervalle depasse cent
    /// unites monde, et une recherche stricte ne rendait rien la plupart du
    /// temps. La tolerance s'arrete au livre — cliquer dans l'air qui separe
    /// deux portions ne selectionne rien.
    fn locate(&mut self, x: f32, y: f32) -> Option<(usize, usize, usize, Option<Rect>)> {
        let chapter = match self.chapter_at(x, y) {
            Some(chapter) => chapter,
            None => {
                let book = self.book_at(x, y)?;
                corpus::chapters_of(book)
                    .min_by(|&a, &b| {
                        gap_to(self.layout.chapters[a], x, y)
                            .total_cmp(&gap_to(self.layout.chapters[b], x, y))
                    })?
            }
        };
        self.locate_in(chapter, x, y)
    }

    fn locate_in(
        &mut self,
        chapter: usize,
        x: f32,
        y: f32,
    ) -> Option<(usize, usize, usize, Option<Rect>)> {
        let book = (0..corpus::book_count()).find(|&b| {
            let r = corpus::chapters_of(b);
            chapter >= r.start && chapter < r.end
        })?;

        // Ligne la plus proche verticalement, et non celle qui contient le
        // point : les respirations entre versets laissent des interstices ou le
        // curseur tombe souvent, et une recherche stricte n'y trouverait rien.
        let lines = self.chapter_lines(chapter);
        let hit = lines.into_iter().min_by(|a, b| {
            vertical_gap(a.rect, y).total_cmp(&vertical_gap(b.rect, y))
        });

        let Some(line) = hit else { return Some((book, chapter, usize::MAX, None)) };

        // Meme tolerance horizontalement : le texte est cale a droite, donc la
        // gauche de la bande est souvent vide.
        let cell = self
            .letter_cells(line.text, line.rect, line.size)
            .into_iter()
            .min_by(|a, b| horizontal_gap(*a, x).total_cmp(&horizontal_gap(*b, x)));

        Some((book, chapter, line.verse, cell))
    }

    fn nest_at(&mut self, x: f32, y: f32) -> Option<Rect> {
        self.locate(x, y)?.3.map(|cell| cell.shrink(NEST))
    }

    /// Point vise par le zoom, avec la lettre qui le contient. Tolere les
    /// interstices entre boites : viser entre deux livres doit rester
    /// productif.
    fn target_near(&mut self, x: f32, y: f32) -> Option<(Rect, Rect)> {
        let chapter = self.chapter_at(x, y).or_else(|| self.nearest_chapter(x, y))?;
        let cell = self.locate_in(chapter, x, y)?.3?;
        Some((cell, cell.shrink(NEST)))
    }

    fn zoom_at(&mut self, px: f32, py: f32, factor: f32) {
        let (bx, by) = self.camera.to_world(px, py);
        self.camera.z *= factor;
        let (ax, ay) = self.camera.to_world(px, py);
        self.camera.x += (ax - bx) * self.camera.z;
        self.camera.y += (ay - by) * self.camera.z;

        // Descente. Le point se trouve au centre de la lettre, alors que le
        // zoom garde le curseur immobile : sans aide, le point derive hors de
        // l'ecran et devient impossible a atteindre. On attire donc doucement
        // la vue vers lui des qu'on a depasse la lettre. Le geste reste libre —
        // la correction est faible et ne s'applique qu'en approche.
        if factor > 1.0 {
            let (wx, wy) = self.camera.to_world(px, py);

            // Tant qu'un chapitre ne remplit pas l'ecran, c'est **lui** que la
            // vue suit. Les fonds etant noirs, le vide entre les cases ne se
            // distingue en rien du reste : sans cette attraction, zoomer a cote
            // menait a un ecran noir dont on ne savait pas sortir.
            let aimed = self
                .chapter_at(wx, wy)
                .or_else(|| self.nearest_chapter(wx, wy))
                .map(|c| self.camera.project(self.layout.chapters[c]));

            if let Some(cell) = aimed
                && cell.w < self.width
            {
                let k = 0.18;
                self.camera.x += (px - (cell.x + cell.w / 2.0)) * k;
                self.camera.y += (py - (cell.y + cell.h / 2.0)) * k;
            }

            let (wx, wy) = self.camera.to_world(px, py);
            if let Some((letter, nest)) = self.target_near(wx, wy) {
                let cell = self.camera.project(letter);
                let screen = self.camera.project(nest);

                if cell.w > self.width * NEST_AFTER {
                    // La lettre est depassee : le nouveau Tanakh apparait, et
                    // l'ancien disparait au meme instant.
                    //
                    // On re-enracine **ici**, et non quand le point recouvre
                    // l'ecran. Attendre coutait une trentaine de crans passes
                    // a manipuler des coordonnees de l'ordre de 1e8 en f32 :
                    // le point, large de quelques pixels, derivait alors de
                    // plus en plus loin du centre (jusqu'a 800 px mesures).
                    // Une fois la camera rendue au monde du point, tous les
                    // nombres redeviennent petits et le cadrage exact.
                    self.camera.wrap_in(nest);
                    self.level += 1;
                } else if screen.w > 2.0 {
                    // Avant cela, une attraction douce vers le curseur : le
                    // point est au centre de sa lettre et deriverait sinon hors
                    // de l'ecran.
                    let closeness = (cell.w / self.width).clamp(0.0, 1.0);
                    let k = 0.14 + 0.62 * closeness;
                    self.camera.x += (px - (screen.x + screen.w / 2.0)) * k;
                    self.camera.y += (py - (screen.y + screen.h / 2.0)) * k;
                }
            }
        }

        // Plancher : le monde ne descend jamais sous un pixel de large.
        self.camera.z = self.camera.z.max(MIN_WORLD_PX / WORLD.w);

        // En dessous du cadrage, on recentre : la vue converge vers un point
        // unique plutot que de deriver hors champ.
        if self.camera.z < self.fit_scale() {
            self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
            self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
        }
    }

    // ---------------------------------------------------------------- rendu

    fn visible(&self, r: Rect) -> bool {
        r.x < self.width && r.y < self.height && r.x + r.w > 0.0 && r.y + r.h > 0.0
    }

    /// Un monde reduit a un point blanc. C'est le seul aplat du rendu : tous
    /// les fonds sont noirs, et seules les lettres portent l'information.
    fn dot(&mut self, r: Rect) {
        let side = r.w.min(r.h).max(MIN_WORLD_PX);
        let x = r.x + (r.w - side) / 2.0;
        let y = r.y + (r.h - side) / 2.0;
        self.commands
            .extend_from_slice(&[KIND_RECT, x, y, side, side, 1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Restreint le dessin a un rectangle, jusqu'a `unclip`.
    fn clip(&mut self, r: Rect) {
        self.commands
            .extend_from_slice(&[KIND_CLIP, r.x, r.y, r.w, r.h, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    fn unclip(&mut self) {
        self.commands
            .extend_from_slice(&[KIND_UNCLIP, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Titre d'un livre ou d'une section, pose **dans sa bande**.
    ///
    /// Il n'est jamais epingle par-dessus le contenu : la bande lui est
    /// reservee, donc il ne recouvre rien. Quand elle sort de l'ecran, le fil
    /// d'Ariane prend le relais — l'information n'est jamais perdue, et jamais
    /// superposee.
    ///
    /// Il reste decoupe a sa boite : trop long pour un petit livre, il est
    /// rogne plutot que d'aller mordre sur son voisin.
    fn title(
        &mut self,
        content: &'static str,
        box_: Rect,
        band: Rect,
        size: f32,
        full: bool,
        alpha: f32,
    ) {
        if alpha <= 0.01 || size < MIN_TEXT || !self.visible(band) {
            return;
        }

        let line = size * 1.35;
        let left = box_.x.max(0.0);
        let right = (box_.x + box_.w).min(self.width);
        if right - left < 2.0 {
            return;
        }

        // Cale en bas de la bande, donc juste au-dessus du premier chapitre :
        // l'espace qui les separe garde la meme allure a tous les zooms.
        //
        // Sauf quand la boite est libre — numeros de chapitre encore masques :
        // le titre s'y centre alors, au lieu de deborder de sa bande vers le
        // haut et d'aller mordre sur le titre de section.
        let area = if full {
            Rect { x: left, y: box_.y, w: right - left, h: box_.h }
        } else {
            Rect { x: left, y: band.y + band.h - line, w: right - left, h: line }
        };

        // Decoupe **en largeur seulement**. La bande d'un petit livre est plus
        // basse que la ligne de texte ; decouper aussi en hauteur coupait la
        // tete des lettres et donnait l'illusion de tailles differentes, alors
        // que tous les titres partagent la meme. Le debordement se fait vers le
        // haut, dans la gouttiere entre livres, jamais sur les chapitres : le
        // titre est ancre au bas de sa bande.
        self.clip(Rect { x: box_.x, y: 0.0, w: box_.w, h: self.height });
        self.emit_text(content, area, size, alpha);
        self.unclip();
    }

    /// Pose un fragment, cale sur le bord droit de sa boite, a une taille
    /// imposee. C'est ce qui permet a tous les versets d'un chapitre de
    /// partager la meme taille quelle que soit leur longueur.
    /// Le plancher de lisibilite est au choix de l'appelant : un titre et une
    /// ligne de verset s'arretent a `MIN_TEXT`, un numero de chapitre a
    /// `MIN_NUMBER`.
    fn emit_text(&mut self, content: &'static str, area: Rect, size: f32, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        let (offset, len) = key_of(content);
        self.commands.extend_from_slice(&[
            KIND_TEXT,
            area.x + area.w, // bord droit : le texte est pose dessus
            area.y + area.h / 2.0 + size * 0.36,
            area.w,
            area.h,
            alpha,
            size,
            offset as f32,
            len as f32,
            0.0,
        ]);
    }

    /// Largeur du numero le plus large du Tanakh, pour une police de taille 1.
    fn widest_numeral(&mut self) -> f32 {
        (0..corpus::chapter_count())
            .map(|c| self.ratio(corpus::chapter_label(c)))
            .fold(0.3_f32, f32::max)
    }

    fn build_frame(&mut self) {
        self.commands.clear();
        self.pending.clear();

        // La disposition suppose une largeur de chiffres ; si la page en a
        // mesure une autre, on la refait avec la vraie. Une seule fois, au
        // debut : les mesures ne changent plus ensuite. C'est a ce prix que le
        // rapport numero/titre vaut exactement un demi dans le navigateur, et
        // pas seulement sous l'estimation.
        let numeral = self.widest_numeral();
        if (numeral - self.numeral).abs() > self.numeral * 0.005 {
            self.numeral = numeral;
            self.layout = treemap::build(numeral);
            self.lines.clear();
            self.number = None;
        }

        let camera = self.camera;

        // Un seul monde est dessine, toujours. Le relais vers le suivant ne se
        // joue plus ici : `zoom_at` re-enracine la camera des que la lettre
        // depasse l'ecran, de sorte que le monde courant *est* deja le nouveau.
        // Tant que ce monde tient dans quelques pixels, rien n'y est lisible et
        // le dessin ressort vide : on pose alors le point blanc qui le
        // represente, et qui grandira jusqu'a devenir le Tanakh.
        let before = self.commands.len();
        self.draw_world(camera, 1.0);
        if self.commands.len() == before {
            self.dot(camera.project(WORLD));
        }

        // `draw_world` pose deja son propre point quand le monde est trop
        // petit pour montrer la moindre bande : le releve ne peut donc pas se
        // fier au garde-fou ci-dessus. Il regarde le resultat — une image
        // reduite a un seul rectangle est un point.
        let point = self.commands.len() == STRIDE && self.commands[0] == KIND_RECT;
        self.last_nests = u32::from(point);
    }

    fn book_title_size(&mut self, camera: Camera, show_chapters: bool) -> f32 {
        if show_chapters {
            return self.uniform_title(camera);
        }

        let widest = (0..corpus::book_count())
            .map(|book| self.ratio(corpus::book_name(book)))
            .fold(0.3_f32, f32::max);

        let box_ = self.layout.books[0];
        let world = (box_.h * 0.6).min(box_.w * 0.88 / widest);
        (world * camera.z).min(self.height * TITLE_CEILING)
    }

    fn uniform_title(&mut self, camera: Camera) -> f32 {
        let mut bands: Vec<f32> = Vec::new();

        for book in 0..corpus::book_count() {
            let rect = camera.project(self.layout.books[book]);
            if self.visible(rect) && rect.w >= 2.0 {
                bands.push(camera.project(self.layout.book_bands[book]).h);
            }
        }

        if bands.is_empty() {
            return 0.0;
        }

        bands.sort_by(f32::total_cmp);
        let median = bands[bands.len() / 2];
        (median * TITLE_FILL).min(self.height * TITLE_CEILING)
    }

    /// Taille des numeros de chapitre, en unites monde.
    ///
    /// **La meme partout.** Toutes les cases du Tanakh partagent un pas unique,
    /// donc une taille unique : un numero ne change pas de corps selon le livre
    /// qu'on regarde. Une taille par livre avait ete essayee pour montrer les
    /// numeros un cran plus tot ; elle donnait dix tailles differentes a
    /// l'ecran en meme temps.
    ///
    /// **Un numero se lit en entier ou pas du tout** : on prend le plus large
    /// du Tanakh — trois lettres — et la plus grande taille qui le fait tenir
    /// dans une case, largeur comprise. Le debordement est donc impossible par
    /// construction.
    ///
    /// Mise en cache, jetee des qu'une mesure arrive de la page.
    fn number_size(&mut self) -> f32 {
        if let Some(size) = self.number {
            return size;
        }
        let cell = self.layout.chapters[0];
        let widest = self.widest_numeral();
        // **La moitie d'un titre de livre.** Le titre remplit sa bande, dont
        // `chapters_fit` a deduit la hauteur a partir de ce rapport et de la
        // largeur des chiffres : la case est donc taillee pour accueillir ce
        // corps-la. Les deux bornes qui suivent ne font que tenir l'invariant —
        // un numero se lit en entier ou pas du tout.
        let half = self.layout.book_bands[0].h * treemap::CHAPTER_OVER_BOOK;
        let size = half
            .min(cell.h * treemap::NUMBER_HEIGHT)
            .min(cell.w * treemap::NUMBER_WIDTH / widest);
        self.number = Some(size);
        size
    }

    /// Tailles d'un niveau, deduites de **sa** camera.
    ///
    /// Elles ne peuvent pas etre calculees une fois pour l'image : un monde
    /// imbriqué est dessine a une tout autre echelle, et reprendre les tailles
    /// du niveau au-dessus donnait des titres absurdes des l'apparition du
    /// Tanakh suivant, puis de pire en pire en continuant a zoomer.
    fn scale_of(&mut self, camera: Camera) -> Scale {
        let show_chapters = camera.z > self.fit_scale() * CHAPTER_FROM;
        let book = self.book_title_size(camera, show_chapters);

        let number = self.number_size();

        Scale {
            show_chapters,
            chapter: number * camera.z,
            book,
            // Pas de plafond propre : celui des titres de livres borne deja les
            // deux, et le rapport reste donc exact a tous les zooms.
            section: book * SECTION_OVER_BOOK,
        }
    }

    fn draw_world(&mut self, camera: Camera, alpha: f32) {
        if alpha <= 0.004 {
            return;
        }

        let scale = self.scale_of(camera);

        // Trop petit pour que la moindre bande soit visible : on pose un point.
        //
        // Sans cela il resterait un vide entre le moment ou le monde est trop
        // petit pour montrer sa structure et celui ou ses bandes depassent le
        // demi-pixel : tout serait elague et l'ecran deviendrait noir. Le point
        // grandit avec le monde, jusqu'a ceder la place a la structure.
        let span = WORLD.w * camera.z;
        if span < STRUCTURE_FROM {
            let x = WORLD.x * camera.z + camera.x;
            let y = WORLD.y * camera.z + camera.y + (WORLD.h * camera.z - span) / 2.0;
            let side = span.max(MIN_WORLD_PX);
            self.dot(Rect { x, y, w: side, h: side });
            return;
        }

        for section in 0..3 {
            let band = camera.project(self.layout.sections[section]);
            if !self.visible(band) {
                continue;
            }

            // Le titre de section d'abord : il fixe le plancher sous lequel les
            // titres de livres viendront s'epingler, pour qu'ils ne se
            // recouvrent jamais.
            let strip = camera.project(self.layout.section_bands[section]);
            self.title(corpus::section_name(section), band, strip, scale.section, false, alpha);

            let (first, last) = (treemap::SECTIONS[section].1, treemap::SECTIONS[section].2);
            for book in first..last {
                self.draw_book(book, camera, scale, alpha);
            }
        }
    }

    fn draw_book(&mut self, book: usize, camera: Camera, scale: Scale, alpha: f32) {
        let rect = camera.project(self.layout.books[book]);
        if !self.visible(rect) || rect.w < 0.7 {
            return;
        }
        for chapter in corpus::chapters_of(book) {
            let cell = camera.project(self.layout.chapters[chapter]);
            if !self.visible(cell) || cell.w < 0.7 || cell.h < 0.7 {
                continue;
            }

            // Hauteur de ligne, estimee sans materialiser la mise en page :
            // 929 chapitres par image, ce serait autant de vecteurs jetes.
            let count = corpus::verses_of(chapter).len().max(1);
            let line = column(self.layout.chapters[chapter]).h / count as f32 * camera.z;

            // Une case montre **toujours quelque chose** : son texte des qu'il
            // est lisible, son numero sinon. Sans cette garantie, une case trop
            // petite pour son texte et trop petite pour le seuil d'etiquette
            // restait vide, et l'on voyait des numeros apparaitre ici, pas la.
            let mut drawn = false;

            if line >= READABLE_LINE {
                let before = self.commands.len();
                self.clip(cell);
                for line in self.chapter_lines(chapter) {
                    let row = camera.project(line.rect);
                    if !self.visible(row) {
                        continue;
                    }
                    self.draw_line(line, row, camera.z, alpha);
                }
                self.unclip();

                // Le decoupage compte pour deux commandes : au-dela, du texte a
                // bien ete pose.
                drawn = self.commands.len() > before + 2 * STRIDE;
            }

            if !drawn && scale.show_chapters && scale.chapter >= MIN_NUMBER {
                // Centre dans sa case : `emit_text` cale sur le bord droit, on
                // lui donne donc une boite qui s'arrete au milieu du nombre.
                let label = corpus::chapter_label(chapter);
                let ink = self.ratio(label) * scale.chapter;
                let area = Rect { w: (cell.w + ink) / 2.0, ..cell };
                self.emit_text(label, area, scale.chapter, alpha * CHAPTER_INK);
            }
        }

        let strip = camera.project(self.layout.book_bands[book]);
        let full = !scale.show_chapters;
        self.title(corpus::book_name(book), rect, strip, scale.book, full, alpha * BOOK_INK);
    }

    /// Une ligne de texte, calee sur le bord droit de sa boite.
    ///
    /// La taille vient de la mise en page, pas de la boite : tous les versets
    /// d'un chapitre partagent la meme, un verset long occupant plusieurs
    /// lignes plutot que des caracteres plus petits.
    fn draw_line(&mut self, line: Line, row: Rect, scale: f32, alpha: f32) {
        let size = line.size * scale;
        if size >= MIN_TEXT {
            self.emit_text(line.text, row, size, alpha);
        }
    }

}

/// Colonne de texte d'une case : la case moins sa marge.
fn column(cell: Rect) -> Rect {
    cell.inset(cell.w * CELL_PAD)
}

/// Distance d'un point a un rectangle ; nulle s'il est dedans.
fn gap_to(r: Rect, x: f32, y: f32) -> f32 {
    let dx = horizontal_gap(r, x);
    let dy = vertical_gap(r, y);
    dx * dx + dy * dy
}

/// Distance verticale d'un point a un rectangle ; nulle s'il est dedans.
fn vertical_gap(r: Rect, y: f32) -> f32 {
    (r.y - y).max(y - (r.y + r.h)).max(0.0)
}

/// Distance horizontale d'un point a un rectangle ; nulle s'il est dedans.
fn horizontal_gap(r: Rect, x: f32) -> f32 {
    (r.x - x).max(x - (r.x + r.w)).max(0.0)
}

fn key_of(fragment: &'static str) -> (u32, u32) {
    (corpus::offset_of(fragment) as u32, fragment.len() as u32)
}

// ----------------------------------------------------------------- interface

/// Prepare l'etat. Renvoie le nombre de livres, qui sert de verification.
#[unsafe(no_mangle)]
pub extern "C" fn init(width: f32, height: f32) -> u32 {
    let mut app = App {
        layout: treemap::build(ESTIMATED_NUMERAL),
        camera: Camera { z: 1.0, x: 0.0, y: 0.0 },
        width: width.max(1.0),
        height: height.max(1.0),
        level: 0,
        ratios: HashMap::new(),
        pending: Vec::new(),
        commands: Vec::new(),
        last_nests: 0,
        space: None,
        lines: HashMap::new(),
        number: None,
        numeral: ESTIMATED_NUMERAL,
        selection: [0; 4],
        focus: [0; 6],
    };
    app.fit();

    unsafe { APP = Some(app) };
    corpus::book_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn resize(width: f32, height: f32) {
    let app = app();
    let refit = (app.camera.z - app.fit_scale()).abs() < f32::EPSILON;
    app.width = width.max(1.0);
    app.height = height.max(1.0);
    if refit {
        app.fit();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset() {
    let app = app();
    app.level = 0;
    app.fit();
}

#[unsafe(no_mangle)]
pub extern "C" fn pan(dx: f32, dy: f32) {
    let app = app();
    app.camera.x += dx;
    app.camera.y += dy;
}

#[unsafe(no_mangle)]
pub extern "C" fn zoom(x: f32, y: f32, factor: f32) {
    app().zoom_at(x, y, factor);
}

/// Chapitre sous le curseur, ou -1. Ne sert qu'a la forme du pointeur : le
/// survol ne change rien au dessin.
#[unsafe(no_mangle)]
pub extern "C" fn hover(x: f32, y: f32) -> i32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    // Meme tolerance que `locate`, pour que le pointeur annonce ce qu'un clic
    // selectionnera reellement.
    app.locate(wx, wy).map_or(-1, |(_, chapter, _, _)| chapter as i32)
}

/// Remplit la selection : livre, chapitre, verset, lettre (offset/longueur
/// lisibles via `selection_ptr`). Renvoie 1 si quelque chose a ete touche.
#[unsafe(no_mangle)]
pub extern "C" fn select(x: f32, y: f32) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);

    match app.locate(wx, wy) {
        Some((book, chapter, verse, _)) => {
            let text = if verse == usize::MAX { "" } else { corpus::verse_text(verse) };
            let (offset, len) =
                if text.is_empty() { (0, 0) } else { key_of(text) };
            app.selection = [book as u32, chapter as u32, offset, len];
            1
        }
        None => {
            app.selection = [0, 0, 0, 0];
            0
        }
    }
}

/// Remplit `focus_ptr` : section, livre et chapitre sous le centre de la vue,
/// chacun en (decalage, longueur) dans le corpus. Longueur nulle = rien.
///
/// C'est ce qui alimente le fil d'Ariane : il dit toujours ou l'on se trouve,
/// y compris quand les bandes de titre ont quitte l'ecran.
#[unsafe(no_mangle)]
pub extern "C" fn focus() {
    let app = app();
    let (wx, wy) = app.camera.to_world(app.width / 2.0, app.height / 2.0);

    let chapter = app.chapter_at(wx, wy).or_else(|| app.nearest_chapter(wx, wy));
    app.focus = [0; 6];

    let Some(chapter) = chapter else { return };
    let Some(book) = (0..corpus::book_count()).find(|&b| {
        let r = corpus::chapters_of(b);
        chapter >= r.start && chapter < r.end
    }) else {
        return;
    };

    let section = treemap::SECTIONS
        .iter()
        .position(|&(_, first, last)| book >= first && book < last)
        .unwrap_or(0);

    let put = |slot: &mut [u32], text: &'static str| {
        let (offset, len) = key_of(text);
        slot[0] = offset;
        slot[1] = len;
    };

    put(&mut app.focus[0..2], corpus::section_name(section));
    put(&mut app.focus[2..4], corpus::book_name(book));
    put(&mut app.focus[4..6], corpus::chapter_label(chapter));
}

#[unsafe(no_mangle)]
pub extern "C" fn focus_ptr() -> *const u32 {
    app().focus.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn selection_ptr() -> *const u32 {
    app().selection.as_ptr()
}

/// Construit la liste de commandes de l'image. Renvoie son nombre de mots.
#[unsafe(no_mangle)]
pub extern "C" fn build_frame() -> u32 {
    let app = app();
    app.build_frame();
    app.commands.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn frame_ptr() -> *const f32 {
    app().commands.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn frame_stride() -> u32 {
    STRIDE as u32
}

/// Fragments dont la page doit mesurer la largeur : des paires (offset, taille).
#[unsafe(no_mangle)]
pub extern "C" fn pending_ptr() -> *const u32 {
    app().pending.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn pending_len() -> u32 {
    app().pending.len() as u32
}

/// Oublie toutes les largeurs mesurees.
///
/// A appeler quand la police change — au chargement de la police definitive,
/// par exemple : des largeurs relevees sur une police de repli seraient fausses
/// pour de bon.
#[unsafe(no_mangle)]
pub extern "C" fn clear_ratios() {
    let app = app();
    app.ratios.clear();
    app.lines.clear();
    app.number = None;
}

/// Enregistre une largeur mesuree, pour une police de taille 1.
#[unsafe(no_mangle)]
pub extern "C" fn set_ratio(offset: u32, len: u32, ratio: f32) {
    let app = app();
    app.ratios.insert((offset, len), ratio);
    // La mise en page depend des largeurs : elle doit etre refaite.
    app.lines.clear();
    app.number = None;
}

/// Le corpus, en UTF-8, directement dans la memoire du module.
/// Diagnostic : etat de la camera et profondeur de descente.
/// Position de la camera, en pixels. Sert aux sondes : sans elle, un releve
/// doit deviner le cadrage et se trompe des qu'on a zoome.
#[unsafe(no_mangle)]
pub extern "C" fn cam_x() -> f32 {
    app().camera.x
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_y() -> f32 {
    app().camera.y
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_z() -> f32 {
    app().camera.z
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_fit() -> f32 {
    app().fit_scale()
}

#[unsafe(no_mangle)]
pub extern "C" fn depth() -> u32 {
    app().level
}

/// Diagnostic : largeur a l'ecran du point vise, ou -1 s'il n'y en a pas.
#[unsafe(no_mangle)]
pub extern "C" fn probe_nest(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.nest_at(wx, wy) {
        Some(nest) => app.camera.project(nest).w,
        None => -1.0,
    }
}

/// Diagnostic : largeur a l'ecran de la cellule de lettre visee.
#[unsafe(no_mangle)]
pub extern "C" fn probe_letter(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.locate(wx, wy) {
        Some((_, _, _, Some(cell))) => app.camera.project(cell).w,
        _ => -1.0,
    }
}

/// Diagnostic : 1 si l'ancien monde a cede la place au suivant.
#[unsafe(no_mangle)]
pub extern "C" fn probe_fade() -> f32 {
    let app = app();
    let camera = app.camera;
    let _ = camera;
    if app.last_nests == 0 { 0.0 } else { 1.0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_points() -> u32 {
    app().last_nests
}

/// Diagnostic : geometrie d'un livre dans le monde (0=x 1=y 2=w 3=h).
#[unsafe(no_mangle)]
pub extern "C" fn probe_book(book: u32, field: u32) -> f32 {
    let r = app().layout.books[book as usize];
    match field {
        0 => r.x,
        1 => r.y,
        2 => r.w,
        _ => r.h,
    }
}

/// Diagnostic : geometrie d'un chapitre dans le monde (0=x 1=y 2=w 3=h).
#[unsafe(no_mangle)]
pub extern "C" fn probe_chapter(chapter: u32, field: u32) -> f32 {
    let r = app().layout.chapters[chapter as usize];
    match field {
        0 => r.x,
        1 => r.y,
        2 => r.w,
        _ => r.h,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn text_ptr() -> *const u8 {
    corpus::text().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn text_len() -> u32 {
    corpus::text().len() as u32
}
