# Chargement d'ethaor — compte rendu (15 octobre 2026)

Objectif : réduire au maximum le temps de téléchargement et d'affichage, en
préparant l'arrivée de dictionnaires et de précalculs de recherche.

## Résultat

Mesures dans Chrome sans interface, réseau bridé (préréglages DevTools), cache
vide, médiane de 3 essais, site servi avec compression Brotli à la volée
(`web/scripts/bench-load.mjs`). « Avant » = dernier commit (`7ee5c1e`), où tout
arrivait d'un bloc.

### Sur PC

| réseau | avant : tout | carte affichée | texte complet |
|---|---|---|---|
| 3G | 20,99 s | **2,81 s** (×7,5) | **8,33 s** (×2,5) |
| 4G lente | 10,34 s | **1,39 s** (×7,4) | **4,13 s** (×2,5) |
| 4G | 2,11 s | **0,43 s** (×4,9) | **1,25 s** (×1,7) |
| fibre | 0,33 s | **0,13 s** (×2,6) | **0,24 s** (×1,4) |

### Sur téléphone (processeur bridé ×4)

| réseau | carte affichée | texte complet |
|---|---|---|
| 3G | 2,84 s | 8,39 s |
| 4G lente | 1,43 s | 4,24 s |
| 4G | 0,50 s | 1,40 s |
| fibre | 0,25 s | 0,44 s |

La version précédente n'a pas été mesurée sur téléphone.

## Ce qui a été fait

1. **Paquet compact (étape A)** — `src/pack.rs`.
   - On n'envoie que ce qui ne se recalcule pas. Le navigateur reconstruit lui-même :
     - le flux de lettres de l'ELS (513 Ko compressés) ;
     - les positions de l'index ;
     - les numéros de chapitres et de versets (une règle + 2 exceptions) ;
     - les versets réécrits de l'autre édition (seules les insertions voyagent).
   - Texte en 1 octet par caractère au lieu de 2 en UTF-8.
   - Le build vérifie que le dépliage redonne le corpus **octet pour octet** :
     une erreur fait échouer le build.
2. **Chargement en deux temps (étape B)**.
   - **Le module**, code + carte : 69 Ko compressés au lieu de 1,7 Mo. Il
     affiche la vue d'ensemble seul.
   - **Le texte** est un fichier à part, demandé **après** la première image.
     Mesuré : plus rapide que de le télécharger en parallèle.
   - En attendant le texte, la carte reste complète (les numéros), sans image
     noire. C'est vérifié sur 9 tailles d'écran, jusqu'à 40 crans de zoom.
3. **Codec maison** pour le texte — `src/pack/codec.rs`, construit par un
   sous-agent après un banc d'essai complet.
   - Modèle de contexte, calcul entier seulement, sans dépendance.
   - **489 Ko**, 20 % de moins que Brotli, zstd ou LZMA.
   - Décodé dans un **Web Worker**, qui réutilise le module déjà compilé : la
     carte ne se fige jamais.
   - Décodé **au fil du téléchargement** : sur réseau lent, le décodage finit en
     même temps que le téléchargement.
4. **Forme du texte choisie automatiquement** selon le débit mesuré par la page
   (`SLOW_LINK`, 300 Ko/s).
   - **Réseau lent** : le codec maison, le moins d'octets possible.
   - **Réseau rapide** : gzip (791 Ko), décodé nativement par le navigateur.
     Sur téléphone, c'est le processeur et non le réseau qui freinait le codec :
     en 4G, 1,40 s au lieu de 2,58 s.
   - Le choix automatique tombe sur la meilleure forme dans 7 cas sur 8 ; seul
     écart : 0,2 s en 4G sur PC.
5. **Polices**.
   - Baloo réduite au latin, un seul poids, en woff : 21 Ko au lieu de 3 × 410 Ko.
   - La page n'attend plus que la police hébraïque avant sa première image.
6. **Préchargement** du module et de la police hébraïque dans le HTML
   (`web/scripts/preload.mjs`), que Cloudflare transforme en *103 Early Hints*.

### Mesures du codec (texte des versets, décodage dans V8, PC)

| codec | taille | décodage |
|---|---|---|
| maison (modèle de contexte) | 499 Ko | ~340 ms |
| LZMA (lzma-rust2) | 626 Ko | 29 ms |
| zstd (ruzstd) | 635 Ko | 13 ms |
| Brotli (brotli-decompressor) | 635 Ko | 13 ms |
| gzip, décodé nativement | 771 Ko | 11 ms |

Mesuré ensuite dans Chrome : sur téléphone, le codec maison décode environ
165 Ko compressés par seconde, soit ~9 fois moins vite que sur PC. C'est ce
qui a conduit au choix automatique entre codec et gzip.

### Écarté, mesures à l'appui

- **SIMD wasm** : +10 % seulement, et un navigateur sans SIMD ne chargerait plus
  du tout le module.
- **LZMA ou zstd embarqués** : leur décodeur alourdirait le module de 24 à 43 Ko,
  sur le chemin du premier affichage.
- **Compression laissée à l'hébergeur** : on n'en maîtrise pas le niveau.
- **Séparer consonnes et signes** : plus gros (933 Ko contre 607 Ko).
- **Bloquer le zoom tant que le texte manque** : inutile, aucune image n'est
  vide sans lui.
- **Optimiser le module pour la taille** (`opt-level=s`) : −7 Ko seulement, pour
  un code plus lent.

## Outils laissés dans le dépôt

- `web/scripts/bench-load.mjs` : chargement dans Chrome, réseau et processeur
  bridés (`CPU=4`, `TEXT=codec|gzip`).
- `web/scripts/bench-els.mjs`, `web/scripts/tune-els.mjs` : calibrage du modèle
  de coût de la recherche ELS.

## Suite : tuiles par chapitre (principe de Google Maps)

Critère ajouté au banc : **le premier verset lisible**, pour un visiteur qui
zoome de 16 crans dès l'affichage de la carte (un cran toutes les 150 ms,
`DIVE=16`).

**Principe retenu.**
- Les tuiles **complètent** le flux complet, elles ne le remplacent pas : le
  codec apprend sur tout le corpus, et découpé en centaines de morceaux, chaque
  morceau repartirait de zéro.
- Une tuile est la tranche d'un chapitre dans le texte non compressé
  (`ethaor-text.plain.bin`), demandée par une requête partielle (*Range*). Un
  chapitre moyen pèse ~2,6 Ko : un seul aller-retour réseau.
- Rust nomme les chapitres visibles qui deviendront lisibles d'ici 3 crans
  (`TILE_AHEAD`) ; les chapitres voisins sont groupés en une seule requête.
- Chaque chapitre arrive directement à sa place finale, dans n'importe quel
  ordre (`corpus::Landing`) : un tampon à la taille finale, rempli de zéros, et
  des bornes de versets atomiques.

**Résultat (PC, premier verset lisible)**

| réseau | sans tuiles | avec tuiles | limite (carte + geste) |
|---|---|---|---|
| 3G | 8,41 s | **4,30 s** | ~4,3 s |
| 4G lente | 4,23 s | **2,89 s** | ~2,9 s |
| 4G | 2,04 s | 1,99 s | ~1,95 s |
| fibre | 1,70 s | 1,61 s | ~1,6 s |

Le visiteur n'attend plus le texte, seulement son propre geste : c'est la
limite physique.

**Banc passé en HTTP/2** (comme un CDN). La carte s'affiche alors en 2,61 s en
3G, 1,29 s en 4G lente, 0,35 s en 4G et 0,11 s en fibre ; le premier verset
arrive en 4,08 / 2,77 / 1,83 / 1,62 s.

**Ce qui reste sur le chemin critique** (~122 Ko transférés en 3G) :

| poste | taille transférée |
|---|---|
| module | 67 Ko |
| police hébraïque | 18 Ko |
| JavaScript | ~33 Ko |
| CSS | ~3 Ko |

## Suite : deux modules, la recherche dans un worker

Les futurs modules de recherche viendront s'ajouter : la recherche est sortie
du module de la carte.

- **Un seul crate, deux compilations** (fonctionnalités Cargo `map` et
  `search`).
  - `ethaor.wasm`, la carte : **52 Ko** gzip au lieu de 73.
  - `ethaor-search.wasm`, la recherche : le codec et l'ELS, 48 Ko.
- **Le module de recherche vit dans un worker**, avec sa propre copie du
  corpus. Il est chargé après la première image. Ni le décodage ni une
  recherche ne figent jamais la carte.
- **L'API de recherche de la page est asynchrone** :
  `search(mot, saut_min, saut_max)`.
- **Le build refuse un module** qui exporterait ce qui ne lui revient pas.

| réseau (HTTP/2, PC) | carte avant | carte après | premier verset avant | premier verset après |
|---|---|---|---|---|
| 3G | 2,61 s | **2,42 s** | 4,08 s | **3,91 s** |
| 4G lente | 1,29 s | **1,21 s** | 2,77 s | **2,68 s** |
| 4G | 0,35 s | 0,39 s | 1,83 s | 1,83 s |
| fibre | 0,11 s | 0,14 s | 1,62 s | 1,61 s |

En 4G et en fibre, les écarts sont du bruit de mesure.

Vérifié dans V8 : le module de recherche décode le texte au fil de l'eau et le
redonne identique ; l'ELS retrouve `תורה` (saut 49) à la lettre 5.

## Retrait du codec maison

Depuis les tuiles, le codec maison ne changeait plus rien pour la lecture :
le premier verset arrivait au même moment avec lui ou avec gzip. Il ne
servait plus qu'à faire arriver le texte complet plus tôt sur les réseaux
lents, au prix de :

- plusieurs secondes de calcul sur téléphone ;
- un second fichier texte à publier ;
- ~640 lignes de code spécialisé.

Il a été retiré : le texte complet voyage en gzip, décodé nativement par le
navigateur. Le module de recherche passe de 124 à 95 Ko brut.

| mesure (HTTP/2, PC) | avec codec | gzip seul |
|---|---|---|
| carte, 3G | 2,42 s | 2,43 s |
| premier verset, 3G | 3,91 s | 3,91 s |
| premier verset, 4G lente | 2,68 s | 2,71 s |
| texte complet, 3G | 7,96 s | 11,08 s |
| texte complet, 4G lente | 3,95 s | 5,46 s |
| texte complet, fibre | 0,25 s | 0,21 s |

Le texte complet ne sert qu'à la recherche ELS et à lire ailleurs que là où
l'on plonge : en 3G, la recherche est prête ~3 s plus tard. La lecture n'est
pas touchée.

Vérifié dans V8 : le gzip ouvert redonne le texte octet pour octet, et la
recherche trouve toujours `תורה` (saut 49) à la lettre 5. 245 tests verts.

## Le texte livre par livre, le plus proche de la vue d'abord

Idée de l'utilisateur, à la Google Earth.
- La carte, jusqu'aux numéros de chapitres, est immédiate : elle est dans le
  module.
- Ensuite, le texte arrive **par livres**, toujours le livre le plus proche du
  centre de la vue en premier (`next_book`). L'ordre se recalcule à chaque
  envoi, donc il suit le visiteur s'il se déplace.
- Les tuiles de plongée passent en priorité haute, les livres en priorité
  basse.

**Mise en œuvre.**
- Un seul fichier, `ethaor-text.books.bin` : 39 livres compressés chacun à
  part, puis les lectures. Total 821 Ko, contre 791 Ko d'un seul bloc (+4 %).
- Chaque bloc est demandé par une requête partielle (*Range*). Leurs positions
  sont livrées avec le code de la page (JSON) : aucune requête pour les
  connaître.
- Le worker de recherche reçoit les mêmes livres.

**Boucle de réglage : nombre de livres en vol.** Un seul à la fois payait un
aller-retour réseau par livre : 24 s en 3G pour le texte complet.

| texte complet | 2 | 4 | 6 | **8 (retenu)** | gzip d'un bloc |
|---|---|---|---|---|---|
| 3G | 15,1 s | 12,3 s | 12,1 s | **12,0 s** | 11,1 s |
| 4G lente | 7,8 s | 6,2 s | 6,0 s | **5,9 s** | 5,5 s |
| 4G | 2,6 s | 1,6 s | 1,4 s | **1,36 s** | 1,17 s |
| fibre | 0,68 s | 0,40 s | 0,33 s | **0,26 s** | 0,21 s |

**Le critère de l'idée : le livre regardé arrive presque aussitôt.**

| réseau | carte | livre le plus proche de la vue arrivé |
|---|---|---|
| 3G | 2,42 s | **2,76 s** (+0,34 s) |
| 4G lente | 1,21 s | **1,38 s** (+0,17 s) |
| 4G | 0,36 s | **0,44 s** |
| fibre | 0,13 s | **0,14 s** |

Avec le fichier unique, ce livre pouvait attendre jusqu'à 11 s en 3G, selon sa
place dans le Tanakh. Le premier verset en plongée reste limité par le geste
(3,9 s en 3G), quel que soit le réglage. Le texte complet, nécessaire à la
recherche ELS, arrive ~0,9 s plus tard qu'en un seul bloc en 3G : c'est le prix
de la compression livre par livre.

247 tests verts, dont deux nouveaux :
- les livres arrivent dans n'importe quel ordre, et les lectures attendent
  qu'ils soient tous là ;
- le prochain livre demandé est le plus proche de la vue, il suit la vue quand
  elle se déplace, et chaque livre n'est demandé qu'une fois.

## Dernière phase — livrée

1. **Vérifier.**
   - 251 tests verts ; `npm run check`, clippy (sur les trois cibles) et
     Prettier : aucune remarque.
   - Efficacité du codec verrouillée :
     - plafond de taille serré (502 000 octets) ;
     - le build refuse un codec qui ne bat plus gzip d'un tiers ;
     - banc de vitesse en release : ~380 ms de décodage natif.
   - Cassures volontaires : retirer la marge du décodage au fil de l'eau
     rougit. Avec un réglage du modèle dégradé (vitesse d'apprentissage 1), le
     texte grossit au-delà du plafond, qui l'arrête.
   - La boucle de réglage a trouvé mieux : vitesse d'apprentissage du
     mélangeur 6 au lieu de 3, soit −1,2 Ko.
2. **Vérifier en vrai.**
   - Dans Chrome, plongée jusqu'aux versets puis clic : texte de Sophonie 2–3
     affiché, fil d'Ariane jusqu'au verset, passage ouvert au clic.
   - Les deux formes du texte (`codec`, `gzip`) donnent le premier verset dans
     le même temps.
3. **Banc lisible** : la forme du texte choisie apparaît dans le tableau.
4. **Documentation** : CLAUDE.md à jour (transport, tuiles, deux modules,
   efficacité du codec, outils, tableau des tests).
5. **Nettoyage** : copie temporaire de l'ancienne version supprimée, après
   avoir vérifié que ses liens vers `node_modules` et `archive` étaient
   retirés et les originaux intacts.
6. **Reste à décider**
   - Le commit.
   - Faut-il versionner les fichiers générés dans `web/src/lib/` ?
     - lesquels : `ethaor*.wasm` et `ethaor-text*.bin`, ~4 Mo dont 2,4 Mo pour
       le texte non compressé ;
     - `npm run build` les régénère de toute façon ;
     - mais un déploiement qui construit depuis git sans Rust en aurait besoin.
   - Le premier déploiement Cloudflare (`npx wrangler login`, puis
     `npm run deploy`), suivi d'une mesure réelle sur l'URL publique.
