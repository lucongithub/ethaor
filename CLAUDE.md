# ethaor

Le Tanakh en carte zoomable, sans fond. **Rust compile en WebAssembly, affiche
dans une page.** Critere unique : la vitesse.

Reference d'affichage : https://github.com/Peeter95/code-map — la taille d'une
boite suit son volume, et en zoomant son contenu apparait dedans.

## Repartition du travail

**Tout le calcul est en Rust** : corpus, disposition, camera, repli du zoom,
elagage, detection de clic. Le module produit une liste de commandes ; la page
ne fait que la peindre.

**Deux choses restent au navigateur**, pour une seule raison : le faconnage de
l'hebreu pointe — bidi et marques combinantes — y vit, et rien d'autre ne le
fait aussi bien.

1. mesurer la largeur des fragments que Rust reclame ;
2. tracer le texte.

Rust dessine d'abord avec une estimation, inscrit les fragments inconnus dans
`pending`, la page les mesure et redemande une image. Chaque fragment n'est
mesure qu'une fois ; la convergence prend une image.

### Pourquoi pas un serveur

Un geste de zoom doit repondre sous 16 ms. Un aller-retour reseau, meme en
local, c'est 20 a 100 ms. Actix n'aura de sens que le jour ou le dictionnaire
demandera des donnees que l'on ne peut pas embarquer.

### Pourquoi pas three.js

WebGL imposerait un atlas de glyphes et un moteur de texte maison : pas de bidi,
pas de marques combinantes. Les 1 000 462 points de niqqud du corpus ne
s'afficheraient pas. Canvas 2D donne le faconnage du navigateur gratuitement.

## Structure

Meme disposition que check8ge : la caisse Rust a la racine, l'application
SvelteKit dans `web/`.

```
Cargo.toml, build.rs, src/      module WebAssembly — tout le calcul
web/                            SvelteKit 2 + Svelte 5 + Tailwind 4
  src/lib/ethaor.ts             pont vers le module : gestes, peinture, mesures
  src/lib/components/           TanakhMap.svelte
  src/routes/+page.svelte
  static/ethaor.wasm            copie par `npm run wasm`
  static/fonts/Baloo2-*.ttf     interface seulement (voir Polices)
  scripts/copy-wasm.mjs
```

`adapter-node`, comme check8ge : `npm start` lance `node build`.
`ssr = false` — le canevas et le module n'ont rien a faire sur le serveur.

Le CSS passe par Tailwind partout. `app.css` ne porte que l'import Tailwind,
les polices et deux jetons de theme (`--font-sans`, `--font-hebrew`).

## Construire

```sh
cd web
npm install
npm run wasm       # cargo build --release puis copie dans static/
npm run dev        # 127.0.0.1:8095
npm run build
npm run preview    # meme port que dev (vite preview n'herite pas de `server`)
npm run host       # 0.0.0.0:8080
npm start          # node build, 127.0.0.1:8095
```

Le corpus est soude au module : un seul telechargement, ~1 Mo compresse, aucune
requete ensuite.

## Polices

**Baloo 2 ne contient aucun glyphe hebreu** — verifie dans sa table `cmap` :
0 consonne sur 27, 0 marque sur 55. Elle ne sert qu'a l'interface.

L'hebreu pointe utilise **Noto Serif Hebrew** (`@fontsource-variable`), qui
couvre le niqqud et la cantillation.

`ethaor.ts` attend `document.fonts.ready` avant la premiere mesure : une largeur
relevee sur une police de repli serait mise en cache et fausse pour de bon.

## Principe directeur

**Tout ce qui peut etre calcule a la compilation ne doit pas l'etre a
l'execution.** Le corpus est fige : `build.rs` lit les sources HTML, construit
texte et index, et les soude au binaire. `corpus.rs` ne fait plus que de
l'indexation `O(1)` sur deux blocs `include_bytes!`.

Mesure : `build_frame` pour la vue d'ensemble (1 749 commandes) prend **0,03 ms**.

## Donnees

Source : `archive/t002/t/` — Mechon Mamre, HTML en **cp1255**, pas UTF-8.
`t01.htm`..`t35.htm`, un fichier par livre.

Mesures verifiees — ne pas les reestimer :

| | |
|---|---|
| Livres | 35 (`עזרא / נחמיה` est un seul titre dans la source) |
| Chapitres | 929 |
| Versets | 23 200 |
| Texte embarque | 4 653 Ko |
| Module wasm | 4,9 Mo, ~1,0 Mo gzip |

Pieges de la source, traites dans `build.rs` :

- L'octet **0xCA** n'est pas attribue en cp1255 standard ; Mechon-Mamre l'emploie
  pour `U+05BA`. Sans correctif, 414 signes sont perdus.
- Le premier `<H1>` de `t01.htm` est une page de garde, sans verset.
- Nettoyage : on retire `{פ} {ס} {ר} {ש}` et toute ponctuation. Le maqaf joint
  deux mots distincts, il devient donc une **espace**, pas une suppression.

## Disposition


`treemap.rs` — **l'ordre canonique prime sur l'esthetique du pavage.** Un treemap
« squarified » trie par taille ; ce serait faux ici. Decoupage en rangees qui
conserve l'ordre.

**39 livres** : 5 pour la Torah, 21 pour les Prophetes, 13 pour les Ecrits.
`build.rs` tient l'ordre canonique dans `ORDER` et y classe ce que la source
livre.

Quatre livres sont scindes (`SPLITS`). Mechon-Mamre numerote Samuel de 1 a 55,
Rois de 1 a 47, Chroniques de 1 a 65 et Ezra/Nehemie de 1 a 23, **sans aucun
redemarrage a `א`** : on coupe donc au rang traditionnel — 31, 22, 29, 10 — et
les totaux concordent exactement. La seconde moitie est renumerotee avec
`hebrew_numeral`, ou 15 et 16 s'ecrivent `טו` et `טז`, jamais `יה` ni `יו`.

**Les livres sont poses sur une grille a 5 colonnes**, celle qu'imposent les
cinq livres de la Torah. Les 21 Prophetes tiennent en cinq rangees, les 13
Ecrits en trois — neuf rangees pour les 39 livres.

Sept colonnes ont ete essayees et mesurees : six rangees de livres, boites de
221,7 x 87,0, cases de 8,9 au lieu de 6,3, et la case a l'ecran passait de 5,74
a 6,49 px. Abandonnees : **la grille des livres n'est pas un reglage de la taille
des cases.** Elle dit l'ordre du Tanakh, la Torah y a sa rangee pleine, et la vue
d'ensemble — celle ou les chapitres ne s'affichent pas — ne doit pas bouger pour
une raison qui n'est pas la sienne.

La hauteur d'une section n'est pas proportionnelle a son volume de texte mais
calculee pour que **toutes les boites de livres soient identiques** : a hauteur
de rangee commune, une section de `n` rangees mesure `bandeau + n x rangee`.
Mesure : 163,1 x 56,7 pour les 39 livres.

**La largeur d'une colonne vient du bloc de chapitres, pas du monde**
(`BOOK_SLACK`). La boite valait 312 unites pour un bloc de 145 : plus de la
moitie etait du vide a gauche des chapitres, et le titre d'un livre flottait au
milieu de rien. Elle vaut desormais ce que demande le bloc le plus large — les
sept rangees de 22 de Tehilim — plus un dixieme d'air : 163,1, dont le bloc
occupe 91 %.

L'ordre du calcul s'inverse donc pour la largeur : la verticale d'abord — hauteur
de rangee, bande, pas — puis la colonne taillee sur le pas. Si le pas change, la
largeur suit, et le rapport reste le meme.

Le monde garde ses proportions : l'aire des livres est centree dedans, ce qui
laisse du noir a gauche et a droite plutot qu'un vide dans chaque boite. Mesure :
836 unites de carte sur les 1 600 du monde, et 887 chapitres a l'ecran au premier
cran de zoom contre 827 avant — la carte s'est resserree, pas appauvrie.

Le prix est sur les titres : le plus long — דברי הימים א — doit tenir dans 159
unites au lieu de 308, donc la taille de la vue d'ensemble tombe de 24,8 a
16,4 px. C'est la contrepartie directe de colonnes deux fois plus etroites.

**Les sections sont jointives ; ce qui les separe est l'air de leur bandeau.**
Le titre de section est cale en bas de sa bande, donc juste au-dessus de ses
livres : la part haute de la bande (`SECTION_LEAD`) est vide, et c'est elle,
et elle seule, qui tient lieu de respiration entre deux portions. La regler ici
plutot que par une marge autour de la section garde les boites de livres
identiques d'une section a l'autre — c'est leur hauteur commune qui decoule du
reste, pas l'inverse.

**L'air d'une portion est le seul reglage de la carte, et il se paie en
cases.** La hauteur du monde est fixe : ce que prennent les trois bandeaux, les
rangees de livres ne l'ont pas. Trois valeurs ont ete mesurees pour
`SECTION_LEAD`, a cinq colonnes : 30 donnait des boites de 83,3 et des cases de
9,9 ; 60, des boites de 70,0 ; 90 — la valeur retenue, le triple de la premiere
— des boites de 56,7 et des cases de 7,9. La ligne du titre, elle, est restee a
28 : elle n'est pas de l'air, et la rogner n'aurait rien separe.

Le cout est connu et accepte : un cran de zoom de plus pour atteindre les
versets, et un tiers d'encre en moins a la vue d'ensemble. C'est le seul endroit
ou l'on peut rendre de la hauteur aux cases sans toucher a la grille des livres :
a 60, les boites font 70,0 et les cases 7,9.

Mesure : 4,0 unites monde entre deux rangees d'un meme bloc, 122,0 entre deux
sections — 4 de gouttiere, 90 d'air, 28 pour la ligne du titre. Trente fois la
gouttiere : une portion ne se confond avec rien.

**L'air a deux valeurs, une par niveau de detail** (`SECTION_LEAD_WIDE` et
`SECTION_LEAD_TIGHT`, sa moitie). En vue d'ensemble, ou l'on ne lit que des
titres, c'est lui qui dit que le Tanakh se divise en trois. Des que les
chapitres paraissent, la lecture se fait dans un livre, la division est acquise,
et le meme vide ne fait plus que repousser le texte : on le reduit de moitie.

Les deux dispositions sont construites au demarrage et **echangees** au passage
du seuil (`set_detail`) ; rien n'est recalcule en cours de zoom, seule la mise
en page des versets, qui depend des cases, est jetee. Le monde garde ses
proportions, donc la camera reste valable : c'est la carte qui se reorganise,
pas la vue qui saute.

Mesure : boites de 163,1 x 56,7 et cases de 6,3 en vue d'ensemble, 223,2 x 76,7
et cases de **8,8** au niveau des chapitres — un tiers de plus. Les numeros
passent de 3,4 a 4,7 px au premier cran, et les versets arrivent au dixieme cran
au lieu du onzieme.

**Le bas de la carte reprend cette meme air.** La marge basse du monde vaut
`SECTION_LEAD`, et non la marge generale : sinon la derniere rangee des Ecrits
s'arretait a 13 unites du bord quand une portion en recoit 32, et le bas de page
paraissait coupe. Le haut n'a rien a corriger — la premiere section porte deja
son propre `SECTION_LEAD` au-dessus de son titre. Mesure : 92,0 unites de blanc
sous le dernier livre, exactement l'air qui precede un titre de portion.

**La bande de titre d'un livre n'est pas reglee, elle est deduite**
(`chapters_fit`). Bande, pas et nombre de rangees sont noues : le titre remplit
sa bande, le numero remplit sa case, le numero vaut la moitie du titre, et ce
que la bande ne prend pas se partage entre les rangees. Fixer l'une fixe les
deux autres. On les resout donc par point fixe, et l'on retient le nombre de
rangees qui donne **la plus grosse case** — c'est la case qui porte le texte,
et plus elle est grande, plus tot les versets se lisent.

**La largeur des chiffres entre dans le calcul.** Un numero est borne par la
hauteur de sa case et par sa largeur ; si les chiffres hebreux sont plus larges
que l'estimation, c'est la largeur qui decide, et la bande doit se resserrer
d'autant pour que le rapport tienne. `build_frame` refait donc la disposition
**une fois**, quand la page a mesure les numeros — pas a chaque zoom, une seule
fois au demarrage, et le rapport vaut alors exactement un demi dans le
navigateur et pas seulement sous l'estimation.

Mesure, boite de 312,0 x 56,7 : sous l'estimation (0,42 par lettre), une bande
de 8,6, un pas de 7,0 et des cases de 6,0 ; sous des chiffres mesures a 0,58 par
lettre — ce que donne une serif hebraique — la bande tombe a 6,6, le pas a 6,6 et
la case monte a 6,3. Rapport numero/titre : 0,500 dans les deux cas. Une bande
plus haute donne un titre plus gros et des cases plus petites : les deux reglages
sont le meme.


La bande n'est **jamais rognee** a une part de la section. Une borne a `h / 2`
donnait a la Torah, seule a n'avoir qu'une rangee, des boites bien plus hautes
que celles des autres sections, et les rangees se chevauchaient.

**Un seul pas pour les 929 chapitres du Tanakh.** Une case a donc exactement la
meme taille partout, d'un bout a l'autre du corpus — et par consequent un numero
de chapitre aussi. **C'est une regle, pas un reglage** : aucune taille de case ni
de numero ne doit dependre du livre, du zoom ou de ce qui est a l'ecran.

Tout en decoule :

- le **nombre de rangees** n'est pas fixe : `chapters_fit` essaie toutes les
  hauteurs de bloc jusqu'a `CHAPTER_ROWS_MAX` et garde celle qui donne la plus
  grosse case. Peu de rangees etirent le bloc en bande et butent sur la largeur
  — les 150 Psaumes ne tiennent pas en deux rangees de 25 — et beaucoup de
  rangees l'ecrasent en hauteur ; l'optimum est au croisement, et il se deplace
  avec la hauteur d'une boite. Le figer a quatre laissait un tiers de la boite
  vide des que les bandeaux de section ont rendu leur air ;
- le **pas** vaut donc la hauteur du corps divisee par ces rangees ;
- le **nombre de cases par rangee** vaut `ceil(150 / 7)`, soit 22 : il vient du
  livre le plus dense qui doit remplir ses rangees, et non de la largeur de la
  boite, qui en accepterait 46. Un bloc n'occupe donc qu'une moitie de boite en
  largeur, et c'est voulu — dans une boite de 312 x 57, c'est la hauteur qui est
  rare, jamais la largeur ;
- un livre prend le nombre de rangees qui donne a son bloc **la forme de son
  corps**, et se repartit a **parts egales** entre elles — sans quoi un livre de
  50 chapitres donnait une rangee pleine de 45 et un moignon de 5 ;
- le bloc est **cale a droite**, d'ou part la lecture, et centre verticalement.

Un livre qui ne remplit pas sa rangee s'arrete donc ou il finit, au pas de tous
les autres. Trois chapitres occupent 20 unites sur 159, et c'est juste : les
etirer sur toute la largeur donnait trois cases perdues au milieu du vide.

**Le nombre de rangees est celui qui rapproche le bloc de la forme de sa
boite**, et non le minimum. Prendre le minimum — tout sur une rangee des que la
largeur l'accepte — mettait 34 livres sur 39 en longue bande, et faisait
voisiner deux dispositions sans rapport : Shemot alignait ses 40 chapitres sur
une rangee pendant que Bereshit, juste a cote, en posait 50 sur deux, pour la
seule raison que 50 depassait les colonnes disponibles — 45 a l'epoque, 22
aujourd'hui, puisqu'elles viennent desormais du livre le plus dense.

Le pas etant commun a tout le Tanakh, l'encre posee est la **meme quel que soit
le decoupage** : seule la forme du bloc change. La forme la plus proche de celle
du corps est la plus compacte, la mieux centree, et celle qui laisse le moins
d'air entre deux livres. A rapport egal on prend le plus de rangees : deux
rangees de dix valent mieux qu'une de vingt.

Mesure, chiffres mesures : pas de 6,6 unites monde, cases de 6,3, identiques
pour les 929 chapitres. Les 150 Psaumes tiennent en **7 rangees de 22**, Isaie en
5 de 14, la Genese en 4 de 13, Shemot en 4 de 10 ; 29 livres sur 39 passent a
deux rangees ou plus, et les 10 qui gardent une seule rangee comptent au plus 5
chapitres — a deux rangees, leur bloc serait bien plus etroit que haut par
rapport a sa boite.

**Sept rangees pour Tehilim se paient sur la case** (`CHAPTER_ROWS`). A boite
constante, chaque rangee de plus retire au pas ce qu'elle prend en hauteur :
mesure, cinq rangees donnaient des cases de 8,5, six de 7,2, sept de 6,3. La
respiration entre cases a ete resserree de 0,5 a 0,15 pour en rendre une part
(5,7 — 6,3), et c'est tout ce qu'il y avait a rendre : le reste est de la hauteur
de boite, qui ne depend que de la grille des livres et de l'air des portions.

Deux autres repartitions ont ete essayees et mesurees :

- **une case maximale** par livre alignait tout sur une seule rangee des qu'un
  livre comptait moins de 23 chapitres, et donnait une taille de case par
  livre ;
- un **rangement carre** (25 chapitres en 5 x 5) tient dans un carre de 26
  unites au milieu d'une boite large de 300, avec des cases de 3 unites, et les
  150 Psaumes tombent a 0,2 unite par case. Le rendre grand demanderait un monde
  en portrait, ou la carte n'occupe plus que 600 des 1400 pixels de large d'un
  ecran paysage — essaye, puis abandonne.

Les livres, eux, remplissent leur bande : c'est la largeur des cinq premiers qui
fixe la colonne.

### Les versets

**Tous les versets d'un chapitre partagent la meme taille de caractere.** Un
verset trop long passe sur plusieurs lignes au lieu d'etre comprime : seul le
nombre de lignes varie. Une respiration (`gap_k`) separe les versets, de sorte
qu'on voie ou l'un finit et ou l'autre commence.

`lay_out` choisit la taille en partant d'une ligne par verset puis en
resserrant tant que la colonne deborde — le nombre de lignes ne peut que
croitre avec la taille, donc quelques passes suffisent. Le resultat est mis en
cache par chapitre et invalide des qu'une mesure arrive de la page.

L'unite du rendu n'est donc pas le verset mais **la ligne** : `Line` porte sa
tranche de corpus, sa boite et sa taille. `wrap` coupe aux mots en gardant des
tranches contigues, pour que seuls un decalage et une longueur traversent la
frontiere.

Chaque ligne est calee sur le bord droit, a partir de la largeur mesuree ; on
ne delegue jamais l'alignement.

**La marge d'une case suit la case** (`CELL_PAD`, 6 % du cote). Fixee a 2 unites
monde, elle ne laissait que 5,9 unites de texte dans une case de 9,9 : 40 % de
l'encre jetee, en largeur comme en hauteur, et des versets qui n'apparaissaient
que six crans de zoom plus tard. Tout le rendu etant proportionnel, sa marge
doit l'etre aussi.

**Le plancher de lisibilite d'une ligne est celui du texte** (`READABLE_LINE` =
5,5 px) : a l'interligne de 1,30, une telle ligne porte des caracteres de 4,2 px,
soit `MIN_TEXT`. Exiger 7 px ne gagnait rien en lisibilite et repoussait les
versets de deux crans.

**But : plonger vite dans le texte.** Mesure, fenetre de 1536 x 730 : la vue
d'ensemble montre les livres, le premier cran les numeros de chapitre, et les
versets apparaissent au **dixieme** cran, a 29 fois le cadrage — neuvieme sur une
fenetre de 1080 px de haut, onzieme sur une de 620. Avant — marge de case
fixe, plancher a 7 px et cran de 1,22 — il en fallait **vingt et un**, a 65 fois
le cadrage. Quatre de ces treize crans sont le prix des sept rangees de Tehilim,
deux celui de l'air des portions.

### Les etiquettes

**Les titres sont dimensionnes par leur bande**, qui est en unites monde : ils
grandissent donc avec le zoom, exactement comme la boite et les chapitres. Une
taille fixe en pixels creusait un espace de plus en plus grand entre le cadre et
le premier chapitre — c'est un **padding constant** qu'il faut, au sens du web.

`uniform_title` retient la bande **mediane** parmi celles qui sont a l'ecran,
jamais la plus petite : un seul livre minuscule ecraserait tous les titres de la
vue d'ensemble. Tous les titres partagent ainsi une taille unique a un zoom
donne.

Le titre est cale **en bas de sa bande**, donc juste au-dessus du premier
chapitre : c'est ce qui rend l'espace stable. Mesure : rapport ecart/taille
constant a 0,14 sur quatre niveaux de zoom, jusqu'au plafond `TITLE_CEILING` —
passe ce point la bande a quitte l'ecran, on est entre dans le livre, et le
titre n'a plus a grandir.

Les versets suivent la meme logique : leur mise en page est en unites monde, et
le rapport interligne/taille reste a 1,85 quel que soit le zoom.

**Un titre reste dans sa bande, jamais epingle par-dessus le contenu.** La
bande lui est reservee, donc il ne recouvre rien. Epingler un titre le faisait
mordre sur les chapitres, sur les versets, et parfois sur un autre titre — un
voile sombre le rendait lisible mais masquait quand meme.

Quand la bande sort de l'ecran, c'est le **fil d'Ariane** qui prend le relais :
`focus` renvoie section, livre et chapitre sous le centre de la vue, et la page
les affiche dans sa propre rangee, au-dessus du canevas. Rien n'est recouvert,
rien n'est perdu. Mesure : zero chevauchement de texte sur 25 niveaux de zoom,
decoupage compris.

Chaque titre est decoupe **en largeur seulement**. La bande d'un petit livre est
plus basse que la ligne de texte ; decouper aussi en hauteur coupait la tete des
lettres et donnait l'illusion de tailles differentes, alors que tous les titres
partagent la meme. Le debordement se fait vers le haut, dans la gouttiere entre
livres, jamais sur les chapitres : le titre est ancre au bas de sa bande.

Mesure : zero titre rogne en hauteur. Deux le restent en largeur a la vue
d'ensemble — עובדיה et שיר השירים, les plus petites boites — et redeviennent
entiers des le premier cran de zoom.

La bande de titre reservee par `treemap` est de **hauteur constante**
(`BOOK_BAND`, `SECTION_BAND`), non proportionnelle a la boite : une bande
proportionnelle est inexistante pour un petit livre, et son titre venait alors
se superposer aux chapitres. Mesure : zero chevauchement titre/numero.

**Un numero de chapitre a la meme taille partout** (`number_size`), puisque
toutes les cases ont la meme taille. Une taille par livre a ete essayee, pour
montrer les numeros deux crans plus tot : elle donnait dix tailles differentes a
l'ecran en meme temps. C'est exclu.

**Un numero vaut la moitie d'un titre de livre** (`CHAPTER_OVER_BOOK` = 0,5).
C'est de ce rapport que `book_band` deduit la hauteur de bande, donc le pas,
donc la taille de case : la case est taillee pour ce corps-la, et non l'inverse.

**Et il se lit en entier ou pas du tout** : les deux bornes qui suivent — 72 %
de la hauteur de case, 92 % de sa largeur pour le numero le plus large du Tanakh,
trois lettres — ne font plus que tenir l'invariant si une mesure venue de la page
rend les chiffres plus larges que l'estimation. Le debordement reste impossible
par construction. Mesure : une seule taille de numero par image sur quarante
crans de zoom, zero chevauchement de texte, zero image vide.

Les numeros apparaissent **des le premier cran** (`MIN_NUMBER` = 3,5 px, plus bas
que le plancher du texte courant : un numero compte une a trois lettres connues
d'avance, lues comme une forme, la ou une ligne de verset doit se dechiffrer).
Mesure, fenetre de 1536 x 730, chiffres mesures : masques au cadrage — la vue
d'ensemble ne montre que les livres — puis 4,7 px au cran suivant, ou l'air des
portions s'est resserre ; 4,0 px sur une fenetre de 620 px de haut, 7,0 sur une
de 1080. La promesse tient sur toute la plage de fenetres usuelles, et le
plancher (3,5 px) ne decide plus de rien.

Le numero est **centre dans sa case** : `emit_text` cale sur le bord droit, on
lui passe donc une boite qui s'arrete au milieu du nombre.

**Au lancement, la carte ne montre que les sections et les livres.** Tant que
les numeros de chapitre sont masques, la boite d'un livre est libre : son titre
l'occupe entierement, en largeur comme en hauteur, et s'y centre.

Les numeros arrivent **des le premier cran de zoom** (`CHAPTER_FROM`, exprime en
multiple du cadrage initial et non en pixels : le seuil tombe ainsi au meme
endroit quelle que soit la fenetre, et apres un repli comme avant).

**Le seuil ne suffit pas sur petit ecran** : `chapters_ready` exige en plus que
le numero passe son plancher. Sur un telephone, le premier cran franchissait le
seuil, le titre d'un livre rentrait dans sa bande — trois pixels — et les
numeros valaient un pixel et demi : l'image etait **entierement noire**. Le
basculement attend donc la lisibilite, et il se decide toujours sur la
disposition des chapitres, celle qui sera en place apres : la juger sur la
disposition courante ferait osciller le niveau d'une image a l'autre.

**Le cadrage a un plancher, pour que les niveaux tombent au meme cran partout**
(`base_scale`). Le cadrage naturel est le monde entier dans la fenetre ; sur une
fenetre etroite il devenait trois fois plus petit que sur un portable, donc des
numeros au quatrieme cran au lieu du premier et des titres de livres de 5 px. On
prefere desormais **rogner la carte** : le cadrage ne descend jamais sous
l'echelle qui met un numero juste au-dessus de son plancher au premier cran.

Le prix est connu : sur une fenetre etroite, la vue de depart ne montre plus
toute la largeur de la carte. On la retrouve en de-zoomant d'un cran — les trois
portions — et en glissant.

**Le pas de la molette est calcule, non fixe** (`zoom_step`, exporte vers la
page) : il vaut ce qu'il faut pour que la plongee du cadrage jusqu'a un chapitre
plein ecran demande toujours `DIVE_NOTCHES` crans. Sur un ecran paysage le
cadrage est deja proportionnel a la hauteur, donc le calcul rend 1,40
exactement ; sur les fenetres etroites, ou le cadrage a ete plancheise, il monte
a 1,47.

Mesure, neuf fenetres de 320 x 568 a 3 840 x 2 160 : **numeros au premier cran
sur les neuf**, titres de livres de 12,1 px au minimum (contre 4,5 avant), zero
image sans texte, zero chevauchement. Les versets, eux, arrivent entre le
septieme et le douzieme cran : ils dependent d'un plancher en pixels, donc un
grand ecran les montre plus tot. Jamais plus tard. Le titre
rentre alors dans sa bande pour leur laisser la place.

C'est la seule marche de taille de toute la plage de zoom, et elle est voulue :
c'est le changement de niveau de detail. Mesure, fenetre de 1536 x 730 : 16,4 px
au lancement, 6,8 au cran suivant, puis croissance reguliere. Le titre **remplit alors sa bande**
(`TITLE_FILL` = 1,0) : une part plus faible creusait la marche sans rien gagner,
le pave de glyphes etant de toute facon plus court que la ligne qui le porte.

**Un titre de section est un quart plus grand qu'un titre de livre**
(`SECTION_OVER_BOOK` = 1,25). Juste assez pour se lire comme un titre au-dessus
des livres — le blanc pur contre `BOOK_INK` fait le reste. Plus gros, il mangeait
l'air qui separe les portions. Il n'a pas de plafond propre : celui des titres de
livres borne deja les deux, et le rapport reste donc exact a tous les zooms.
Mesure : 16,4 et 20,5 px au cadrage initial, rapport 1,25 jusqu'au zoom de
lecture. Trois tailles par image une fois les chapitres visibles, et trois
seulement : 3,4 pour les numeros, 6,8 pour les livres, 8,5 pour les sections.

**Une case montre toujours quelque chose** : son texte des qu'il est lisible,
son numero sinon. Deux failles donnaient des cases vides, et donc des numeros
qui apparaissaient ici mais pas la :

- un seuil de taille minimale sur la case, qui privait les petites de leur
  numero sans rien mettre a la place ;
- une case en mode texte dont les lignes tombaient toutes sous `MIN_TEXT`.

On verifie donc apres coup que du texte a ete pose ; sinon on retombe sur le
numero. Mesure : zero case vide, du cadrage initial jusqu'au zoom de lecture.

## Zoom sans fin

Au coeur de chaque lettre, `NEST` reserve la place du Tanakh entier — **les
sept dixiemes de sa cellule**, et non un point.

**Aucun aplat n'est jamais peint.** Le rendu ne sort que du texte et des
decoupages : il n'y a plus de carre blanc, ni au zoom ni au de-zoom. C'etait le
pansement d'un trou, et le trou a ete bouche des deux cotes — un monde naissant
large de mille pixels d'un cote, un plancher de de-zoom de l'autre.

- Rien du monde contenu n'est dessine avant le repli : on peut zoomer
  pleinement une lettre sans rien voir arriver.
- **Des que la lettre est depassee, `wrap_in` re-enracine la camera** et le
  niveau augmente. Il n'y a jamais deux mondes a l'ecran : `build_frame`
  dessine le monde courant, et c'est tout. Ni fondu, ni liste de points, ni
  camera imbriquee.
- A cet instant le nouveau Tanakh fait **sept dixiemes de l'ecran**, donc ses
  titres de portion sont deja lisibles : le passage d'un texte a l'autre n'a
  pas d'etat intermediaire. A `NEST` = 0,004, il naissait large de six pixels
  et mettait une quinzaine de crans a devenir lisible — d'ou l'aplat blanc.
  Mesure : repli au trentieme cran, monde large de 1 250 px, 42 titres a
  l'ecran a 17,6 et 22,0 px, zero aplat.
- **Re-enraciner tot est une question de precision, pas d'esthetique.**
  Attendre que le point recouvre l'ecran laissait une trentaine de crans ou la
  camera manipulait des coordonnees voisines de 1e8 en f32 : l'ulp y depasse le
  pixel, et le point — large de quelques pixels — derivait du centre jusqu'a
  800 px mesures. Replie dans le monde du point, tout redevient petit et exact.
  Mesure : ecart au centre 0 px sur toute la croissance du point.
- Le zoom suit d'abord **le chapitre** tant qu'il ne remplit pas l'ecran, puis
  la lettre. Les fonds etant noirs, le vide entre deux cases ne se distingue en
  rien du reste : sans cette attraction, zoomer a cote menait a un ecran noir
  dont on ne savait pas sortir. Mesure : 20 images vides sur 160 crans avant,
  zero apres.
- Ensuite la place du monde contenu est au centre de la lettre alors que le
  zoom garde le curseur immobile : sans aide elle deriverait hors de l'ecran.
  Une **attraction faible** (14 % par cran, renforcee a l'approche) ramene la
  vue vers elle.
- Apres le repli, le monde est plus petit que le cadrage : la regle du de-zoom
  le **centre**, ce qui le place exactement au milieu et l'y maintient pendant
  qu'il grandit. Le cadrage libre revient quand le monde remplit l'ecran.

**La descente est a sens unique.** Des que le nouveau Tanakh occupe l'ecran,
l'ancien cesse d'exister : aucun chemin de retour n'est garde, et de-zoomer ne
ramene pas au monde precedent.

**Un cran de de-zoom, et la carte n'est plus que les trois portions, en grand,
a egale distance** (`PORTIONS_UNDER`). Les trois niveaux sont ainsi a **un cran
l'un de l'autre**, de part et d'autre du cadrage :

| zoom | ce qu'on voit |
|---|---|
| un cran sous le cadrage | les trois portions seules, en grand |
| cadrage | les portions et les 39 livres |
| un cran au-dessus | les chapitres |

Le seuil est juste sous 1 et non a 1 : le cadrage lui-meme doit rester du cote
des livres, y compris aux arrondis pres. Attendre que les titres de livres
passent sous le plancher du texte demandait cinq crans, pendant lesquels la
carte n'etait qu'une grille de mots de plus en plus pales.

Deux regles changent a ce niveau.

Leur **place** : dans le monde, elle vient du volume des sections — une rangee de
livres pour la Torah, cinq pour les Prophetes — ce qui les met a des distances
tres inegales. Une fois les livres partis, cette inegalite ne dit plus rien ;
`draw_world` les repartit a parts egales sur la hauteur de l'ecran.

Leur **taille** : elle est prise sur le tiers du monde projete
(`SECTION_CARD`), et non plus sur la bande de la section. Suivre la bande les
rendait minuscules au moment meme ou ils devenaient le seul contenu de l'image —
trois mots de 4 px sur une fenetre vide. Ils naissent donc grands, puis
**rapetissent et se rapprochent a chaque cran de de-zoom, sans fin** : la carte
s'eloigne au lieu de disparaitre.

Une taille figee en pixels d'ecran a ete essayee : on lisait bien les trois
mots, mais de-zoomer ne faisait plus rien du tout.

Leur **ecart** vaut un quart de la hauteur du monde (`CARD_GAP`), soit 2,1 fois
la taille des mots. A un tiers, les trois mots tenaient les coins de l'ecran et
ne se lisaient plus comme un groupe ; a un sixieme, ils etaient trop serres.

Et **les trois partagent un seul bord droit**, celui du mot le plus large : c'est
la lecture qui le veut, et trois mots centres chacun pour soi partaient dans
trois directions. Le pave ainsi forme est, lui, centre sur le monde.

Le niveau se reconnait **au zoom seul, et du bon cote du cadrage**. Tres zoome
dans un livre, les bandes de titre ont aussi quitte l'ecran : juger sur elles
faisait peindre les trois portions en geant par-dessus les versets. Verifie : sur
cinquante crans de zoom, les seules images a trois textes sont les trois numeros
de chapitre d'un petit livre — א, ב, ג a 70 puis 98 px — jamais la carte.

**Le de-zoom n'est donc plus borne par la lisibilite** (`floor_scale`), puisque
les trois titres ne rapetissent plus : il ne reste qu'un garde-fou technique pour
que l'echelle ne tombe pas a zero. La carte reste **centree** — en dessous du
cadrage, la camera se recentre a chaque cran.

Mesure, de-zoom depuis le cadrage sur une fenetre de 1 536 x 730 : les 42 titres
du cadrage (16,4 et 20,5 px) cedent **des le premier cran** la place a trois mots
de **60,8 px** ecartes de 130,4 px, puis 43,5 et 93,1, puis 31,0 et 66,5, puis
22,2 et 47,5 — et ainsi de suite jusqu'au garde-fou technique. A chaque cran les
deux ecarts restent egaux au dixieme de pixel, les trois bords droits sont
confondus au pixel, et aucun aplat n'est emis.

L'attraction se renforce a l'approche (`0,14 + 0,62 x proximite`). Sans cela le
point devrait grandir bien au-dela de l'ecran pour en couvrir les quatre coins,
et les flottants y perdraient leur precision : mesure, le repli se produit vers
un zoom de 1e6 au lieu de 5e7.

`locate` tolere les interstices — respiration entre versets, intervalle entre
cases, marges entre livres — en retenant la ligne, la case ou le chapitre le
plus proche plutot que celui qui contient le point. Sans cela, viser entre deux
boites ne descendait jamais, et depuis que les cases sont espacees pour remplir
leur boite, un clic entre deux chapitres d'un petit livre ne selectionnait rien.

**La tolerance s'arrete au livre.** Un point hors de toute boite de livre ne
designe rien : cliquer dans l'air qui separe deux portions, ou dans le blanc du
bas, ne selectionne pas le chapitre le plus proche. `hover` suit la meme regle,
pour que le pointeur annonce ce qu'un clic fera. Mesure : 195 points sur 195
d'une boite de livre designent un chapitre — y compris chez un livre de trois
chapitres, dont les cases sont a cent unites l'une de l'autre — et zero dans
l'air entre portions comme dans le blanc du bas.

Sans ce repli, atteindre une lettre demanderait un zoom de l'ordre de 1e6 et les
`f32` perdraient toute precision — c'est pourquoi il intervient des le
depassement de la lettre, et non plus tard.

## Interface

Monochrome : fond noir, structures en degrades de blanc. Pas de couleur.

**Aucun cadre, aucun fond.** Ni trait de contour ni aplat : sections, livres et
chapitres partagent le meme noir, et seules les lettres portent l'information.

Le rang se lit alors a **l'intensite du blanc** : une section en blanc pur, un
livre un ton en dessous (`BOOK_INK`), un numero de chapitre un rien plus bas
encore (`CHAPTER_INK`). Mesure : 1,00 / 0,78 / 0,58.

Les trois parties du Tanakh se distinguent par l'**air au-dessus de leur titre**
(`SECTION_LEAD`) et par ce titre lui-meme, en blanc pur. Mesure : 122,0 unites
monde entre deux sections, contre 4,0 entre deux rangees de livres.

**Il n'y a aucun aplat.** Mesure : zero commande de rectangle sur soixante
crans de zoom, repli compris, et zero image sans texte.

**Le survol ne change rien au dessin** ; il ne sert qu'a la forme du pointeur.
Verifie : la liste de commandes est identique au bit pres, avec et sans survol.

Les etiquettes de structure (titres, numeros) sont dessinees en coordonnees
monde, donc elles grandissent avec le zoom — mais `label` les supprime des que
leur boite depasse l'ecran : au-dela, ce ne serait qu'un glyphe geant flottant
par-dessus le niveau courant.

**Une seule exception, au dernier etat du de-zoom** : quand la carte n'est plus
que les trois noms de portion, leur taille et leur ecart ne viennent plus de
leur bande mais du monde projete (`SECTION_CARD`, `CARD_GAP`). La bande d'une
section vaut ce que vaut son volume de livres ; une fois les livres partis, elle
ne dit plus rien, et c'est le monde entier qui sert de mesure. Les trois mots
grandissent et rapetissent donc toujours avec le zoom, comme tout le reste.

Gestes : molette ou pincement pour zoomer, glisser pour deplacer, clic pour
selectionner, Echap pour la vue d'ensemble. **Un cran de molette vaut 1,4** : de
la vue d'ensemble au texte d'un chapitre il y a une dizaine de crans, et c'est
deja long ; a 1,22 il en fallait le double, ce qui donnait l'impression de ne
jamais arriver au texte. Le pave tactile envoie des fractions de cran, donc le
reglage fin reste possible. `touch-action: none` sur le canvas,
sans quoi le navigateur mobile intercepte tout.

`select` renvoie le chemin complet (livre, chapitre, verset) : c'est le point
d'accroche pour afficher demain des informations sur un mot ou une lettre.

## Interdits

- Alignement de texte delegue au moteur : on calcule la position soi-meme.
- Parsing du HTML a l'execution : c'est le travail de `build.rs`.
- Allocation par lettre, par mot ou par verset dans la boucle d'image.
- Recalcul de la disposition au zoom.
