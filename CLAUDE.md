# affut.sh

Jeu idle de capture de créatures, en Rust, qui tourne dans un terminal et dans
un navigateur. Deux publics : les amis de l'auteur en SSH sur darkforge, et
n'importe qui sur la page GitHub Pages.

## Ton et rédaction

L'utilisateur (tely) est francophone. Vouvoiement, ton neutre et professionnel,
pas d'emoji, **jamais de tiret long** dans les réponses. Les textes du jeu sont
en français, en minuscules, d'un ton sobre et un peu pince-sans-rire. Les
commentaires de code expliquent POURQUOI, pas quoi, et sont en français.

## Architecture

Une seule caisse, `affut`, avec une bibliothèque et deux binaires.

- `src/lib.rs` (~8 600 lignes) : tout le jeu. Tables de données en tête
  (CREATURES, BIOMES, TRAPS, BAITS, LABS, ACHS, NEWS), état sérialisé `State`,
  logique dans `impl Game`, rendu ratatui en bas, module `web` sous
  `#[cfg(target_arch = "wasm32")]`, tests à la fin.
- `src/main.rs` : lance la version terminal.
- `src/sync_main.rs` : serveur de synchronisation et de classement (tiny_http).
  Sauvegardes sur `/s/<jeton>`, classement sur `/lb`.
- `docs/index.html` : client navigateur, charge le wasm, gère le clavier, la
  session partagée, le pseudo, la favicon qui clignote.
- `docs/classement.html` : page du palmarès.

Abstractions de plateforme : `GKey`, `now_ms()`, `save_raw()` / `load_raw()`,
et `cfg!(target_arch = "wasm32")` là où le comportement diffère.

## Déployer

Les deux cibles se déploient ensemble, toujours.

```sh
# web : bump obligatoire de V dans docs/index.html, sinon le navigateur
# garde l'ancien wasm en cache
sed -i '' "s/const V = '20260821XX'/const V = '20260821XY'/" docs/index.html
wasm-pack build --release --target web --out-dir docs/pkg && rm -f docs/pkg/.gitignore
git add -A && git commit && git push          # GitHub Pages publie docs/
# puis attendre que Pages serve la nouvelle version avant d'annoncer

# terminal et serveur de classement
./deploy/deploy.sh tely@vps.tely.info
```

L'agent SSH doit être chargé : `export SSH_AUTH_SOCK=/tmp/affut_agent_sock`.
darkforge répond sur le **port 443** (voir `~/.ssh/config`), le port 22 est
fermé. Le jeu SSH des invités écoute sur 2322.

## Journal des versions

`VERSION` et la table `NEWS` en tête de `lib.rs`. **Toute version déployée qui
change quelque chose de visible pour un joueur doit avoir son entrée**, et la
taille du tableau (`[(&str, &str, &[&str]); N]`) se met à jour. Le panneau
s'ouvre tout seul au premier lancement suivant une mise à jour. Les retours de
joueurs sont crédités par leur pseudo.

## Tests

`cargo test` couvre quatre familles, toutes écrites en français :

- la carte : accessibilité à pied de chaque biome et de chaque porte,
  traversée du village, densité d'obstacles,
- les tables : cohérence créatures / biomes / recettes / prix,
- l'équilibre : rythme des légendes sur trente jours, cadence tolérée,
- l'interface : largeur du bestiaire en 80 colonnes, libellés des boutons,
  concordance entre ce qu'un panneau affiche et les formules du jeu.

Les bugs rapportés par les joueurs deviennent des invariants, pas des
correctifs ponctuels. `cargo test` ne reconstruit pas le binaire release :
lancer `cargo build --release` avant tout essai avec le vrai terminal.

## Vérifier à la main

- Terminal : `pty` + `pyte` en Python, avec `TIOCSWINSZ` **obligatoire**, sinon
  ratatui dessine dans un terminal de taille nulle et l'écran reste vide.
- Navigateur : Chrome headless par CDP, ou `node --input-type=module --check`
  sur le script extrait de `index.html` pour une simple vérification de syntaxe.

## Classement et anticheat

Le serveur ne fait jamais confiance au client : il reborne chaque statistique
et **recalcule le score lui-même**. Les plafonds se déduisent des moyens de la
partie (`captures_max_par_s`, `shiny_max_par_capture`, `especes_financables`)
plutôt que de constantes fixes, parce que trois faux positifs successifs ont
écarté un joueur honnête. Un écartement passé reste inscrit dans
`ecarte_un_jour`, qui ouvre un trophée scellé.

Sécurité : pseudos en alphabet latin uniquement, normalisation anti-homoglyphe,
corps limité à 4 ko, quota de 500 entrées, un changement de pseudo par semaine,
retrait par `hidden` et jamais par suppression de fichier.

## Vocabulaire du jeu, à ne pas mélanger

| terme | ce que c'est |
|---|---|
| trophées | les succès, bâtiment ╡ t ╞ |
| reliques | la monnaie de prestige gagnée en migrant |
| licence de piégeage | le niveau de labo, perdu à la migration |
| permis de piégeage | l'objet du marchand ou du puits, conservé |

Deux homonymies ont déjà rendu des règles incompréhensibles. Nommer les choses
distinctement est une correction de bug à part entière.

## Horloge

Une journée de village dure **deux heures réelles** (`JOUR_JEU_MS`). Jour,
nuit, saisons et lune suivent cette horloge. Tout ce qui produit (pièges,
contrats, troc, légendes, rattrapage hors-ligne, séries quotidiennes, foire)
reste au temps réel : accélérer cela multiplierait l'économie par douze.

## Rejouabilité hors-ligne

Météo, contrats, troc, marchand, traces et légendes sont tirés par
`splitmix64` sur un index de fenêtre, donc rejouables à l'identique au retour
du joueur. Ne jamais introduire d'aléa non reproductible dans ces systèmes.

## Communauté

tely (l'auteur), ook / ookook (Thomas, le plus gros captureur et le meilleur
rapporteur de bugs), Drasaerys, Foura, Merk, ookChromé. Leurs retours arrivent
par captures WhatsApp ou Discord relayées dans la conversation.

## Refusé, ne pas reproposer

Le relevé manuel des pièges : explicitement rejeté comme contraire à l'esprit
idle du jeu.
