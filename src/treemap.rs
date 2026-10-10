//! Disposition du Tanakh : trois sections, puis les livres, puis les chapitres.
//!
//! **L'ordre canonique est preserve partout.** Un treemap « squarified »
//! classique trie par taille pour obtenir de belles proportions ; ici ce serait
//! faux — on ne melange pas l'ordre des livres du Tanakh. On emploie donc un
//! decoupage en bandes (*strip treemap*), qui garde l'ordre et laisse seulement
//! le choix du moment ou l'on change de rangee.
//!
//! L'ordre de lecture est celui d'une page : **de droite a gauche dans une
//! rangee, puis rangee suivante en dessous**. Un element qui vient apres un
//! autre est donc soit a sa gauche sur la meme ligne, soit plus bas — jamais
//! au-dessus. Les rangees sont toujours horizontales : une colonne verticale
//! romprait cette continuite.
//!
//! La disposition est calculee une fois dans un espace monde fixe ; la camera
//! la projette ensuite. Zoomer ne recalcule jamais la geometrie.

use crate::corpus;

/// Espace monde. Proportions d'une fenetre paysage ordinaire.
pub const WORLD: Rect = Rect { x: 0.0, y: 0.0, w: 1600.0, h: 1000.0 };

/// Les trois sections, dans l'ordre : Torah, Nevi'im, Ketouvim.
/// Les trois sections et leurs bornes, en index de livre : 5 de la Torah,
/// 21 des Prophetes, 13 des Ecrits.
pub const SECTIONS: [(&str, usize, usize); 3] =
    [("תורה", 0, 5), ("נביאים", 5, 26), ("כתובים", 26, 39)];


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// Retrecit vers le centre, par un facteur.
    pub fn shrink(&self, factor: f32) -> Rect {
        let (w, h) = (self.w * factor, self.h * factor);
        Rect { x: self.x + (self.w - w) / 2.0, y: self.y + (self.h - h) / 2.0, w, h }
    }

    pub fn inset(&self, pad: f32) -> Rect {
        let pad = pad.min(self.w / 2.0).min(self.h / 2.0).max(0.0);
        Rect { x: self.x + pad, y: self.y + pad, w: self.w - 2.0 * pad, h: self.h - 2.0 * pad }
    }
}

pub struct Layout {
    /// Une bande par section, empilees de haut en bas.
    pub sections: [Rect; 3],
    pub books: Vec<Rect>,
    pub chapters: Vec<Rect>,
    /// Bandes de titre, en coordonnees monde. Le titre s'y cale **en bas**,
    /// juste au-dessus du premier chapitre : l'espace qui l'en separe reste
    /// alors le meme a tous les zooms.
    pub section_bands: [Rect; 3],
    pub book_bands: Vec<Rect>,
}

/// Hauteur de la bande de titre d'une section, en unites monde.
///
/// Elle est plus haute que celle d'un livre, afin que son nom reste au-dessus
/// de celui des livres dans la hierarchie — la couleur seule n'y suffirait pas si
/// un titre de livre etait plus gros.
///
/// Celle d'une section se lit en deux parts : l'air qui la precede — c'est
/// **elle** qui separe une portion de la precedente, le titre etant cale en bas
/// de sa bande — puis la ligne du titre lui-meme.
const SECTION_LEAD: f32 = 90.0;
const SECTION_TITLE: f32 = 28.0;
pub const SECTION_BAND: f32 = SECTION_LEAD + SECTION_TITLE;

/// Rapport voulu entre un numero de chapitre et le titre du livre qui le porte.
pub const CHAPTER_OVER_BOOK: f32 = 0.5;

/// Part de la largeur d'une case que l'encre d'un numero peut occuper, et part
/// de sa hauteur. Le debordement est ainsi impossible dans les deux sens.
pub const NUMBER_WIDTH: f32 = 0.92;
pub const NUMBER_HEIGHT: f32 = 0.72;

/// Part de la case qu'occupe un numero, **sachant la largeur de ses chiffres**.
///
/// `numeral` est la largeur du numero le plus large du Tanakh — trois lettres —
/// pour une police de taille 1. Tant que la page n'a pas mesure, c'est une
/// estimation ; des que la mesure arrive, la disposition est refaite avec elle.
///
/// C'est ce qui garde le rapport numero/titre exact dans le navigateur : si les
/// chiffres hebreux sont plus larges que l'estimation, c'est la largeur qui
/// borne le numero, et la bande de titre doit se resserrer d'autant.
pub fn number_fill(numeral: f32) -> f32 {
    (NUMBER_WIDTH / numeral.max(0.1)).min(NUMBER_HEIGHT)
}

/// Bande de titre, pas et largeur du bloc de chapitres, **deduits ensemble** de
/// la boite d'un livre. Renvoie `(pas, bande, colonnes)`.
///
/// Les rangees sont imposees : `CHAPTER_ROWS`. Le reste est noue — le titre
/// remplit sa bande, le numero remplit sa case, le numero vaut la moitie du
/// titre, et ce que la bande ne prend pas se partage entre les rangees. Fixer
/// l'une fixe les autres, on les resout donc par point fixe.
///
/// Les colonnes ne viennent **pas** de la largeur de la boite : elles viennent
/// du livre le plus dense, qui doit remplir ses rangees. C'est ce qui donne aux
/// 150 Psaumes sept rangees de 22 plutot que quatre de 38 — a la largeur de la
/// boite, ils tiendraient en quatre. Le bloc n'occupe alors qu'une moitie de la
/// boite en largeur, et c'est voulu : c'est la hauteur qui est rare.
///
/// Mesure, boite de 56,7 : sous l'estimation de largeur des chiffres, un pas de
/// 6,8 et une bande de 7,3 ; sous des chiffres mesures a 0,58 par lettre — ce
/// que donne une serif hebraique — un pas de 6,6 et une bande de 6,6. Le
/// rapport numero/titre reste 0,500 dans les deux cas.
fn chapters_fit(inner: Rect, densest: usize, fill: f32) -> (f32, f32, usize) {
    let rows = CHAPTER_ROWS;
    let columns = densest.div_ceil(rows).max(1);
    let (mut pitch, mut band) = (0.0, 0.0);

    // Le pas decoule de ce que la bande laisse ; la bande decoule du pas. Huit
    // passes suffisent largement a stabiliser le couple.
    for _ in 0..8 {
        pitch = ((inner.h - band) / rows as f32).max(0.1).min(inner.w / columns as f32);
        band = ((fill / CHAPTER_OVER_BOOK) * (pitch - 2.0 * CHAPTER_GAP))
            .clamp(0.0, inner.h * 0.5);
    }

    (pitch.min((inner.h - band) / rows as f32), band, columns)
}

/// Nombre de colonnes de livres, pour les trois sections.
///
/// C'est la largeur qu'imposent les cinq livres de la Torah ; les autres
/// sections la reprennent, et toutes les boites de livres sont alors de meme
/// taille d'un bout a l'autre du Tanakh. Les 21 Prophetes tiennent ainsi en
/// cinq rangees, les 13 Ecrits en trois.
///
/// Sept colonnes ont ete essayees — six rangees de livres, boites de 221,7 x
/// 87,0, cases un cinquieme plus grosses — puis abandonnees : la grille des
/// livres n'est pas un reglage de la taille des cases, et la Torah y perdait sa
/// rangee pleine.
const BOOK_COLUMNS: usize = 5;

/// Marge exterieure du monde. Elle n'intervient qu'au bord, jamais entre deux
/// sections.
const WORLD_MARGIN: f32 = 10.0;

/// Les trois sections sont **jointives** : ce qui les separe est exactement ce
/// qui separe deux colonnes de livres, soit `2 x BOOK_GAP`. Le bandeau de titre
/// suffit a les distinguer ; une respiration plus large creusait un vide de 114
/// unites monde la ou les livres n'en comptent que 6, et volait aux rangees la
/// hauteur dont les titres ont besoin.
const BOOK_GAP: f32 = 2.0;
/// Air que la boite d'un livre garde au-dela de son bloc de chapitres, en part
/// de la largeur de ce bloc.
///
/// **C'est elle qui fixe la largeur d'une colonne de livres**, et non l'inverse.
/// La boite valait 312 unites pour un bloc de 145 : plus de la moitie etait du
/// vide a gauche des chapitres, et le titre d'un livre flottait au milieu de
/// rien. A un dixieme, elle vaut 159 et le bloc en occupe 91 %.
///
/// La colonne suit donc le pas : si le pas change, la largeur de boite change
/// avec lui, et le rapport reste le meme.
const BOOK_SLACK: f32 = 0.1;

/// Respiration autour d'une case de chapitre. **Tres petite** : c'est le pas qui
/// tient le rythme, et la case doit en occuper presque tout.
///
/// A 0,5, l'intervalle mangeait 1 unite d'un pas de 6,6, soit 15 % de l'encre,
/// ce qui se voit des que les rangees sont nombreuses et le pas serre. A 0,15,
/// la case passe de 5,7 a 6,3 sans qu'on cesse de distinguer deux chapitres :
/// le fond etant noir, l'intervalle n'a pas besoin d'etre large, seulement non
/// nul.
const CHAPTER_GAP: f32 = 0.15;

/// Rangees de chapitres du livre le plus dense : **sept pour les 150 Psaumes**,
/// soit sept rangees de 22.
///
/// C'est ce compte qui fixe tout le reste : les colonnes valent
/// `ceil(150 / 7)`, le pas vaut la hauteur du corps divisee par sept, et la
/// bande en decoule. Les autres livres se repartissent dans la meme grille.
///
/// Le prix est connu : a boite constante, chaque rangee de plus retire au pas
/// ce qu'elle prend en hauteur. Mesure, boite de 56,7 : cinq rangees donnaient
/// une case de 8,5, six une de 7,2, sept une de 6,3.
const CHAPTER_ROWS: usize = 7;

/// Hauteur reservee au titre, en haut d'une boite.
///
/// **Constante**, et non proportionnelle a la boite : une bande proportionnelle
/// est inexistante pour un petit livre, et son titre vient alors mordre sur les
/// chapitres. Elle n'est rognee que si la boite est trop basse pour l'accueillir.
///
/// La borne a un tiers ne sert plus qu'aux boites trop basses : la bande des
/// livres est desormais deduite du rapport titre/numero par `book_band`, et
/// vaut 9,4 unites pour une boite de 45,6, soit un cinquieme de sa hauteur.
fn header(h: f32, band: f32) -> f32 {
    band.min(h * 0.33)
}

pub fn build(numeral: f32) -> Layout {
    // --- les trois bandes
    //
    // Leur hauteur n'est pas proportionnelle au volume de texte mais calculee
    // pour que **toutes les boites de livres soient identiques**, d'une section
    // a l'autre. A hauteur de rangee commune `row`, une section qui porte `n`
    // rangees mesure `bandeau + n x row`.
    let rows: Vec<usize> =
        SECTIONS.iter().map(|&(_, first, last)| (last - first).div_ceil(BOOK_COLUMNS)).collect();
    let total_rows: usize = rows.iter().sum();

    // Le bas de la carte reprend **l'air qui precede un titre de portion**, de
    // sorte que la derniere rangee de livres respire comme les autres. Sans
    // cela, les Ecrits s'arretaient a 21 unites du bord quand une portion en
    // recoit 94 : le bas de page paraissait coupe.
    //
    // Le haut n'a pas besoin du meme traitement : la premiere section porte
    // deja son propre `SECTION_LEAD` au-dessus de son titre.
    let full = Rect {
        x: WORLD_MARGIN,
        y: WORLD_MARGIN,
        w: WORLD.w - 2.0 * WORLD_MARGIN,
        h: WORLD.h - WORLD_MARGIN - SECTION_LEAD,
    };
    let fixed = 3.0 * SECTION_BAND;
    let row = ((full.h - fixed) / total_rows as f32).max(1.0);

    // --- la largeur d'une colonne de livres
    //
    // Elle n'est pas donnee par le monde mais **par le bloc de chapitres**. La
    // hauteur d'une boite, elle, est deja fixee : c'est `row` moins les deux
    // marges de `BOOK_GAP`, et c'est d'elle que viennent le pas et la bande.
    // On resout donc la verticale d'abord, puis on taille la colonne pour que
    // le bloc le plus large — les sept rangees de 22 de Tehilim — y tienne avec
    // un dixieme d'air.
    //
    // Le monde garde ses proportions : l'aire des livres est simplement
    // centree dedans, ce qui laisse du noir a gauche et a droite plutot qu'un
    // vide a l'interieur de chaque boite.
    let densest =
        (0..corpus::book_count()).map(|book| corpus::chapters_of(book).len()).max().unwrap_or(1);
    let inner = Rect { h: row - 4.0 * BOOK_GAP, ..full };
    let (pitch, band, columns) = chapters_fit(inner, densest, number_fill(numeral));

    let cell = columns as f32 * pitch * (1.0 + BOOK_SLACK) + 4.0 * BOOK_GAP;
    let wide = (cell * BOOK_COLUMNS as f32).min(full.w);
    let area = Rect { x: (WORLD.w - wide) / 2.0, w: wide, ..full };

    let mut sections = [Rect::ZERO; 3];
    let mut section_bands = [Rect::ZERO; 3];
    let mut y = area.y;
    for (i, &count) in rows.iter().enumerate() {
        let h = SECTION_BAND + count as f32 * row;
        sections[i] = Rect { x: area.x, y, w: area.w, h };
        y += h;
    }

    // --- les livres, dans l'ordre, a l'interieur de leur bande
    let mut books = vec![Rect::ZERO; corpus::book_count()];
    for (i, &(_, start, end)) in SECTIONS.iter().enumerate() {
        let band = sections[i];
        // Pas de part proportionnelle ici : la hauteur de section est calculee
        // pour contenir le bandeau **entier** plus ses rangees, et le rogner
        // rendrait les boites de livres inegales d'une section a l'autre — la
        // Torah, qui n'a qu'une rangee, heritait de 84 unites quand les autres
        // en recevaient 64.
        let head = SECTION_BAND.min(band.h - 1.0);
        section_bands[i] = Rect { h: head, ..band };
        let body = Rect { y: band.y + head, h: (band.h - head).max(1.0), ..band };

        for (book, rect) in (start..end).zip(grid(end - start, body, Some(BOOK_COLUMNS), false)) {
            books[book] = rect.inset(BOOK_GAP);
        }
    }

    // --- les chapitres, dans l'ordre, a l'interieur de leur livre
    //
    // **Un seul pas pour les 929 chapitres du Tanakh.** Une case a donc la
    // meme taille partout, et par consequent un numero de chapitre aussi.
    // C'est le livre le plus dense — les 150 Psaumes, quatre rangees de 38 —
    // qui le fixe, en largeur comme en hauteur.
    //
    // Un livre qui ne remplit pas sa rangee part du bord droit et s'arrete ou
    // il finit : l'espacement reste celui de tous les autres, au lieu d'etirer
    // trois chapitres sur 300 unites.
    let mut chapters = vec![Rect::ZERO; corpus::chapter_count()];
    let mut book_bands = vec![Rect::ZERO; corpus::book_count()];

    let bodies: Vec<Rect> = (0..corpus::book_count())
        .map(|book| {
            let inner = books[book].inset(BOOK_GAP);
            let head = header(inner.h, band);
            book_bands[book] = Rect { h: head, ..inner };
            Rect { y: inner.y + head, h: (inner.h - head).max(1.0), ..inner }
        })
        .collect();


    for book in 0..corpus::book_count() {
        let range = corpus::chapters_of(book);
        let body = bodies[book];
        // Nombre de rangees : celui qui donne au bloc **la forme de son corps**.
        //
        // Prendre le minimum — tout sur une rangee des que la largeur l'accepte
        // — mettait 34 livres sur 39 en longue bande, et faisait voisiner deux
        // dispositions sans rapport : Shemot alignait ses 40 chapitres sur une
        // rangee pendant que Bereshit, juste a cote, en posait 50 sur deux,
        // pour la seule raison que 50 depassait les 45 colonnes disponibles.
        //
        // Le pas etant commun a tout le Tanakh, l'encre posee est la meme quel
        // que soit le decoupage : seule la forme du bloc change. On retient
        // donc celle qui ressemble le plus a la boite — la plus compacte, la
        // mieux centree, et celle qui laisse le moins d'air entre deux livres.
        //
        // A rapport egal, le plus de rangees : deux rangees de dix valent mieux
        // qu'une de vingt, qui s'etire en travers de la boite.
        // La forme visee est celle du bloc le plus dense — `colonnes x pas` —
        // et non celle de la boite : les blocs sont bornes en largeur par les
        // colonnes, donc c'est dans cette fenetre-la qu'ils doivent s'inscrire.
        let shape = columns as f32 * pitch / body.h;
        let drift = |rows: usize| {
            let ratio = range.len().div_ceil(rows) as f32 / rows as f32;
            (ratio / shape).max(shape / ratio)
        };
        let rows = (1..=CHAPTER_ROWS.min(range.len()))
            .rev()
            .filter(|&rows| range.len().div_ceil(rows) <= columns)
            .min_by(|&a, &b| drift(a).total_cmp(&drift(b)))
            .unwrap_or_else(|| range.len().div_ceil(columns));

        // Rangees egales : on repartit a parts egales entre elles. Sans cela un
        // livre de 50 chapitres donnait une rangee pleine de 45 et un moignon
        // de 5.
        let columns = range.len().div_ceil(rows);

        // Le bloc est centre verticalement dans le corps ; horizontalement il
        // est cale a droite, d'ou part la lecture.
        let top = body.y + (body.h - rows as f32 * pitch) / 2.0;
        let right = body.x + body.w;

        for (i, chapter) in range.enumerate() {
            let slot = Rect {
                x: right - (i % columns + 1) as f32 * pitch,
                y: top + (i / columns) as f32 * pitch,
                w: pitch,
                h: pitch,
            };
            chapters[chapter] = slot.inset(CHAPTER_GAP);
        }
    }

    Layout { sections, books, chapters, section_bands, book_bands }
}

/// Grille reguliere : **toutes les cases ont exactement la meme largeur et la
/// meme hauteur.**
///
/// Un decoupage en rangees donne bien a chaque chapitre la meme aire, mais pas
/// la meme forme : la derniere rangee, moins remplie, etire ses cases sur toute
/// la largeur. Ici le nombre de colonnes est fixe d'avance, et la derniere
/// rangee laisse simplement du vide a sa gauche.
///
/// On retient le nombre de colonnes qui rend les cases **les plus carrees
/// possible**. Un chapitre n'a pas de sens de lecture propre : le carre est la
/// forme qui se compare le mieux d'une case a l'autre, et qui tire le meilleur
/// parti de la place. Les boites de livres, elles, visent toujours le rectangle
/// d'or.
///
/// Les cases se remplissent de droite a gauche, puis vers le bas — l'ordre de
/// lecture.
fn grid(count: usize, area: Rect, fixed: Option<usize>, square: bool) -> Vec<Rect> {
    let mut out = vec![Rect::ZERO; count];
    if count == 0 || area.w <= 0.0 || area.h <= 0.0 {
        return out;
    }

    // Nombre de cases par rangee.
    //
    // On ne force pas une grille carree — 25 chapitres en 5 x 5 tiendraient
    // dans un carre de 26 unites au milieu d'une boite large de 300, avec des
    // cases de 3 unites. On ne prend pas non plus la plus grosse case possible,
    // qui aligne tout sur une seule rangee des que le livre compte moins de 23
    // chapitres.
    //
    // On retient **l'arrangement le plus equilibre parmi ceux qui ne coutent
    // presque rien en taille de case** : au plus un cinquieme du meilleur cote.
    // Une rangee de 21 cases de 14,3 unites cede ainsi la place a deux rangees
    // de 13,2, et un livre de trois chapitres garde sa rangee unique, dont les
    // cases sont quatre fois plus grandes que si on l'equilibrait.
    const KEEP: f32 = 0.80;

    let side_of = |columns: usize| {
        let rows = count.div_ceil(columns);
        (area.w / columns as f32).min(area.h / rows as f32)
    };

    let columns = match fixed {
        // Le compte impose ne se rabat **pas** sur le nombre d'elements : une
        // rangee de cinq livres dans une grille de sept garde des boites de la
        // largeur des autres, et laisse deux emplacements vides. Sans cela la
        // Torah heritait de boites un tiers plus larges que celles des autres
        // sections, et la case de chapitre n'etait plus unique.
        Some(n) => n.max(1),
        None => {
            let best = (1..=count).map(side_of).fold(0.0_f32, f32::max);
            // Le plus equilibre d'abord, la plus grosse case pour trancher.
            (1..=count)
                .filter(|&c| side_of(c) >= best * KEEP)
                .min_by(|&a, &b| {
                    let balance = |c: usize| {
                        let r = count.div_ceil(c) as f32;
                        let c = c as f32;
                        (c / r).max(r / c)
                    };
                    balance(a)
                        .total_cmp(&balance(b))
                        .then(side_of(b).total_cmp(&side_of(a)))
                })
                .unwrap_or(1)
        }
    };
    let rows = count.div_ceil(columns);

    let mut w = area.w / columns as f32;
    let mut h = area.h / rows as f32;

    // Cases **carrees**, quitte a laisser du vide. Etirer une case pour remplir
    // la boite la rendrait plate des qu'un livre compte peu de chapitres — un
    // livre d'un seul chapitre occuperait toute la largeur sur une hauteur
    // derisoire. Le carre tient toujours : son cote est le plus petit des deux,
    // donc la grille ne deborde jamais.
    //
    // Les livres, eux, remplissent leur bande : c'est la largeur des cinq
    // premiers qui fixe la colonne.
    //
    // **Le vide restant passe entre les cases**, et non autour du bloc : la
    // grille s'etale alors sur toute la place disponible, jusqu'a la gouttiere
    // du livre voisin. Une seule colonne, ou une seule rangee, n'a pas
    // d'intervalle ou repartir le vide : on la centre.
    let (mut dx, mut dy) = (0.0, 0.0);
    let (mut step_x, mut step_y) = (w, h);
    if square {
        let side = w.min(h);
        step_x = if columns > 1 { (area.w - side) / (columns - 1) as f32 } else { 0.0 };
        step_y = if rows > 1 { (area.h - side) / (rows - 1) as f32 } else { 0.0 };
        if columns == 1 {
            dx = (area.w - side) / 2.0;
        }
        if rows == 1 {
            dy = (area.h - side) / 2.0;
        }
        w = side;
        h = side;
    }

    for (i, slot) in out.iter_mut().enumerate() {
        let column = i % columns;
        let row = i / columns;
        *slot = Rect {
            // De droite a gauche, en partant du bord droit.
            x: area.x + area.w - dx - w - column as f32 * step_x,
            y: area.y + dy + row as f32 * step_y,
            w,
            h,
        };
    }

    out
}



