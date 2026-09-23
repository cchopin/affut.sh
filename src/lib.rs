// affut.sh — un comptoir de capture pour gens patients
// TUI ratatui/crossterm, même stack que late.sh

use std::collections::VecDeque;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style}};
#[cfg(target_arch = "wasm32")]
use ratatui_core::{buffer::Buffer, layout::Rect, style::{Color, Style}};

/* touches abstraites : le natif traduit crossterm, le web traduit KeyboardEvent */
#[derive(Clone, Copy, PartialEq)]
pub enum GKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    PageUp,
    PageDown,
    Char(char),
}

/* ================================================================ données */

const RAR_LABEL: [&str; 5] = ["commun", "peu commun", "rare", "épique", "légendaire"];
/* colonnes du bestiaire, calées sur le contenu réel (glyphe ≤ 5, nom ≤ 22,
   libellé de rareté ≤ 10) pour que la ligne entière tienne dans un terminal
   de 80 : au-delà, reflow_rows replie « stock » et l'alignement saute. c'est
   ce qui impose « ×132 » plutôt que « · pris ×132 », sept colonnes de moins ;
   la ligne de légende en tête du panneau dit ce que chaque colonne contient. */
const DEX_W_GLYPH: usize = 6;
const DEX_W_NAME: usize = 23;
const DEX_W_MOON: usize = 2;
const DEX_W_RAR: usize = 20;
const DEX_W_RANK: usize = 4;
const DEX_W_SEX: usize = 3;
const RAR_W: [f64; 5] = [100.0, 28.0, 5.0, 0.7, 0.08];
const RAR_VAL: [f64; 5] = [3.0, 9.0, 30.0, 150.0, 900.0];
/* la chance agit linéairement (plafonnée), plus jamais exponentiellement */
const RAR_LUCK_FACT: [f64; 5] = [0.0, 0.35, 0.6, 0.8, 1.0];
const LUCK_CAP: f64 = 6.0;
/* le brocanteur reprend un piège à la moitié de son prix : de quoi se
   débarrasser des vieux pièges en bois sans en faire une source d'écus */
const TRAP_RESALE: f64 = 0.5;

struct BiomeDef {
    name: &'static str,
    cost: f64,
    mult: f64,
    desc: &'static str,
}
/* biomes 0..WILDB : ceux de la carte. le dernier, « curiosités », n'a ni
   terrain ni pièges — on n'y entre pas, ses espèces s'obtiennent au troc. */
const WILDB: usize = 11;
const CURIO_B: usize = 11;
/* les légendes errantes n'appartiennent à aucun biome : elles traversent le
   monde entier, y compris les terres qu'on n'a pas encore les moyens d'ouvrir. */
const LEGEND_B: usize = 12;
/* ce qui vit au fond du puits : une seule espèce, qu'aucune autre voie ne donne */
const PUITS_B: usize = 13;
/* les prix du désert aux ruines suivent le revenu des pièges, bien plus élevé
   depuis que chaque palier double au lieu d'ajouter 40% : sans cela la partie
   durerait trois fois moins longtemps. la forêt, la rivière, le marais, la
   montagne et le lac gardent leurs prix d'origine — ce sont les seuls qu'un
   joueur avait déjà en vue, et le début de partie profite ainsi du gain sans
   en payer le prix.

   la hausse monte jusqu'au récif puis REDESCEND aux ruines, au lieu de croître
   jusqu'au bout : à faire porter tout l'effort par le dernier biome, on passait
   80% de la partie après le volcan contre 64% à l'origine. avec cette courbe,
   69% — la longueur totale est la même, mais elle n'est plus concentrée dans un
   seul mur final. */
const BIOMES: [BiomeDef; 14] = [
    BiomeDef { name: "forêt",    cost: 0.0,         mult: 1.0, desc: "des sous-bois humides où tout bruisse. le point de départ de toute traque." },
    BiomeDef { name: "marais",   cost: 2500.0,      mult: 1.6, desc: "de la vase, des bulles, des choses qui clignent des yeux sous la surface." },
    BiomeDef { name: "montagne", cost: 20000.0,     mult: 2.5, desc: "des cimes venteuses. les pièges y gèlent mais les prises valent le détour." },
    BiomeDef { name: "désert",   cost: 400000.0,    mult: 4.0, desc: "des dunes à perte de vue. tout ce qui y survit vaut cher." },
    BiomeDef { name: "glacier",  cost: 3000000.0,   mult: 6.5, desc: "un silence bleu et parfait. les espèces y sont rares et magnifiques." },
    BiomeDef { name: "abysses",  cost: 25000000.0,  mult: 10.0, desc: "là où la lumière renonce. le fond du bestiaire, littéralement." },
    BiomeDef { name: "volcan",   cost: 85000000.0,  mult: 13.0, desc: "la montagne qui fume. huit espèces y vivent, aucune n'a froid." },
    BiomeDef { name: "récif",    cost: 260000000.0, mult: 16.0, desc: "un jardin sous la surface, plus peuplé qu'il n'y paraît. douze espèces s'y cachent." },
    BiomeDef { name: "ruines",   cost: 440000000.0, mult: 20.0, desc: "ce qu'il reste d'avant. dix espèces s'y accrochent, dont certaines depuis trop longtemps." },
    BiomeDef { name: "rivière",  cost: 900.0,       mult: 1.3, desc: "elle descend de la montagne et traverse tout. neuf espèces la remontent, personne ne sait pourquoi." },
    BiomeDef { name: "lac",      cost: 45000.0,     mult: 3.2, desc: "là où la rivière s'arrête et réfléchit. sept espèces y tournent en rond depuis des siècles." },
    BiomeDef { name: "curiosités", cost: f64::INFINITY, mult: 10.0, desc: "des espèces qu'aucun piège n'attrape. elles changent de mains, jamais de gré." },
    BiomeDef { name: "légendes",   cost: f64::INFINITY, mult: 18.0, desc: "elles ne vivent nulle part et passent partout. on ne les croise qu'en silhouette, une fois de temps en temps." },
    BiomeDef { name: "le puits",   cost: f64::INFINITY, mult: 22.0, desc: "sous la place, l'eau descend plus bas que les fondations. quelque chose y attend d'être appelé." },
];

struct CreatureDef {
    b: usize,
    r: usize,
    g: &'static str,
    n: &'static str,
    lore: &'static str,
}
const CREATURES: [CreatureDef; 133] = [
    // ---- forêt (13)
    CreatureDef { b: 0, r: 0, g: "(o.o)", n: "mulotin",          lore: "un rongeur curieux qui entasse des graines dans les pièges eux-mêmes." },
    CreatureDef { b: 0, r: 0, g: "~(°>",  n: "sourivole",        lore: "moitié souris, moitié feuille morte. plane mal, atterrit pire." },
    CreatureDef { b: 0, r: 0, g: ".ø.",   n: "champillon",       lore: "un champignon qui marche. lentement, mais il marche." },
    CreatureDef { b: 0, r: 0, g: "=ö=",   n: "bourdonel",        lore: "bourdonne en permanence, même endormi. surtout endormi." },
    CreatureDef { b: 0, r: 0, g: "(:>",   n: "hérissou",         lore: "roule en boule au moindre bruit. se déroule pour les baies." },
    CreatureDef { b: 0, r: 1, g: "\\|/",  n: "cerfeuil",         lore: "un petit cervidé dont les bois fleurissent au printemps." },
    CreatureDef { b: 0, r: 1, g: "/\\.",  n: "renardou cendré",  lore: "sa fourrure sent le feu de camp éteint. personne ne sait pourquoi." },
    CreatureDef { b: 0, r: 1, g: ".w.",   n: "papillotte",       lore: "un papillon qui se prend pour une feuille. les feuilles votent contre." },
    CreatureDef { b: 0, r: 2, g: "{|}",   n: "sylvestre",        lore: "un esprit d'arbre qui déteste être dérangé mais adore les appâts." },
    CreatureDef { b: 0, r: 2, g: "*.*",   n: "lucioleau",        lore: "clignote en morse. les messages sont rarement polis." },
    CreatureDef { b: 0, r: 2, g: "(-)",   n: "taupinard",        lore: "creuse des tunnels qui débouchent toujours dans un piège. troublant." },
    CreatureDef { b: 0, r: 3, g: "(*)",   n: "dryadelle",        lore: "gardienne des clairières. se laisse capturer uniquement par curiosité." },
    CreatureDef { b: 0, r: 4, g: "\\VV/", n: "grand cornu",      lore: "le patron de la forêt. les autres espèces s'inclinent sur son passage." },
    // ---- marais (11)
    CreatureDef { b: 1, r: 0, g: "~o~",   n: "vasouille",        lore: "une bulle de vase avec des yeux. pop." },
    CreatureDef { b: 1, r: 0, g: "(o)~",  n: "crapotin",         lore: "croasse faux. les autres crapauds l'évitent." },
    CreatureDef { b: 1, r: 0, g: "_@/",   n: "limacet",          lore: "laisse une trace luisante qui épelle son propre nom." },
    CreatureDef { b: 1, r: 0, g: "-=+",   n: "moustiflard",      lore: "trop gros pour voler discrètement, trop têtu pour arrêter." },
    CreatureDef { b: 1, r: 1, g: "(°°)",  n: "grenouillard",     lore: "un vieux sage amphibien. donne des conseils que personne ne demande." },
    CreatureDef { b: 1, r: 1, g: "~~~o",  n: "sangsurelle",      lore: "s'attache facilement. au sens propre, hélas." },
    CreatureDef { b: 1, r: 1, g: "@~",    n: "tourbillard",      lore: "un petit tourbillon de tourbe. poli, mais collant." },
    CreatureDef { b: 1, r: 2, g: ".::.",  n: "brumelin",         lore: "un morceau de brouillard devenu autonome un soir d'octobre." },
    CreatureDef { b: 1, r: 2, g: "!*!",   n: "feufollet",        lore: "attire les voyageurs vers les pièges. techniquement un collègue." },
    CreatureDef { b: 1, r: 3, g: "}~{",   n: "hydrelle",         lore: "trois têtes, un seul avis : contre." },
    CreatureDef { b: 1, r: 4, g: "~S~",   n: "basilombre",       lore: "son regard fige la vase elle-même. ne le fixez pas trop longtemps." },
    // ---- montagne (10)
    CreatureDef { b: 2, r: 0, g: "[o]",   n: "cailloutin",       lore: "un galet qui a décidé d'avoir des jambes. respectable." },
    CreatureDef { b: 2, r: 0, g: "(u)",   n: "marmotton",        lore: "dort huit mois par an. les quatre autres, il mange." },
    CreatureDef { b: 2, r: 0, g: "/\\_",  n: "chamoisel",        lore: "défie la gravité par principe et les chasseurs par sport." },
    CreatureDef { b: 2, r: 0, g: "\\v/",  n: "aiglonet",         lore: "un rapace de poche. vise mal mais avec conviction." },
    CreatureDef { b: 2, r: 1, g: "<*>",   n: "cristalpin",       lore: "pousse comme un cristal, pique comme un pin." },
    CreatureDef { b: 2, r: 1, g: ")(",    n: "bouquetin de brume", lore: "on ne voit jamais que ses cornes dépasser du nuage." },
    CreatureDef { b: 2, r: 2, g: "[#]",   n: "golemite",         lore: "un tas de pierres qui se souvient d'avoir été une montagne." },
    CreatureDef { b: 2, r: 2, g: "\\W/",  n: "condorage",        lore: "ses colères déclenchent des avalanches. ses joies aussi." },
    CreatureDef { b: 2, r: 3, g: "s^s",   n: "wyvernelle",       lore: "une dragonne de taille modeste et d'ego considérable." },
    CreatureDef { b: 2, r: 4, g: "/M\\",  n: "titan des cimes",  lore: "quand il s'assoit, les cartes doivent être redessinées." },
    // ---- désert (12)
    CreatureDef { b: 3, r: 0, g: "(=)",   n: "scarabinet",       lore: "pousse une boule de sable partout. c'est son projet de vie." },
    CreatureDef { b: 3, r: 0, g: "~s",    n: "serpentile",       lore: "écrit des poèmes dans le sable en rampant. illisibles." },
    CreatureDef { b: 3, r: 0, g: "^..^",  n: "fennecot",         lore: "ses oreilles captent la radio. il préfère le jazz." },
    CreatureDef { b: 3, r: 0, g: "|#|",   n: "cactille",         lore: "un cactus timide. les épines, c'est de la gêne." },
    CreatureDef { b: 3, r: 0, g: "-*-",   n: "rosable",          lore: "une rose des sables qui bourgeonne. personne n'arrose, elle insiste." },
    CreatureDef { b: 3, r: 1, g: "-E<",   n: "scorpiard",        lore: "brille sous la lune et le sait parfaitement." },
    CreatureDef { b: 3, r: 1, g: "\\_/",  n: "vautourin",        lore: "patiente au-dessus des pièges. il a compris le concept." },
    CreatureDef { b: 3, r: 2, g: ".?.",   n: "mirageon",         lore: "existe-t-il vraiment ? le piège dit oui. le doute demeure." },
    CreatureDef { b: 3, r: 2, g: "=A=",   n: "dunataure",        lore: "mi-homme mi-dune. entièrement insaisissable, ou presque." },
    CreatureDef { b: 3, r: 2, g: "^!^",   n: "chacalin",         lore: "rit tout seul dans les dunes. on préfère ne pas savoir de quoi." },
    CreatureDef { b: 3, r: 3, g: "[:]",   n: "sphinxel",         lore: "pose une énigme avant chaque capture. le piège ne répond jamais, ça l'agace." },
    CreatureDef { b: 3, r: 4, g: "OOO~",  n: "ver des sables",   lore: "le désert n'est pas vide. il digère." },
    // ---- glacier (11)
    CreatureDef { b: 4, r: 0, g: "(v)",   n: "pingolin",         lore: "glisse sur le ventre par efficacité, pas par jeu. enfin, un peu par jeu." },
    CreatureDef { b: 4, r: 0, g: "*o*",   n: "frimousse",        lore: "une boule de neige avec un visage. fond au printemps, revient vexée." },
    CreatureDef { b: 4, r: 0, g: "(\\_",  n: "lièvrelin",        lore: "blanc sur blanc. on ne capture souvent que ses empreintes." },
    CreatureDef { b: 4, r: 0, g: ":3=",   n: "morsille",         lore: "des défenses imposantes, un caractère de peluche." },
    CreatureDef { b: 4, r: 0, g: ".:.",   n: "neigelin",         lore: "un flocon trop gros pour fondre, trop léger pour tomber." },
    CreatureDef { b: 4, r: 1, g: "vvv",   n: "stalactin",        lore: "tombe du plafond des grottes sur les pièges. par solidarité." },
    CreatureDef { b: 4, r: 1, g: "|-|",   n: "rennelune",        lore: "ne touche jamais vraiment le sol. vérifiez ses empreintes." },
    CreatureDef { b: 4, r: 2, g: "[Y]",   n: "yétillon",         lore: "un yéti junior. floute lui-même les photos, c'est de famille." },
    CreatureDef { b: 4, r: 2, g: "≈≈≈",   n: "aurorelle",        lore: "un ruban d'aurore boréale qui a pris goût au sol." },
    CreatureDef { b: 4, r: 3, g: "|>o",   n: "givrecorne",       lore: "sa corne givre l'air. les collectionneurs givrent d'envie." },
    CreatureDef { b: 4, r: 4, g: "~O~",   n: "léviathan blanc",  lore: "la banquise, c'est son dos. réfléchissez-y." },
    // ---- abysses (11)
    CreatureDef { b: 5, r: 0, g: "-o)",   n: "lanternet",        lore: "sa lampe frontale est en panne un jour sur deux. il fait avec." },
    CreatureDef { b: 5, r: 0, g: "(((",   n: "méduselle",        lore: "transparente et fière de l'être. difficile à compter." },
    CreatureDef { b: 5, r: 0, g: "}={",   n: "crabique",         lore: "marche de côté même dans ses rêves." },
    CreatureDef { b: 5, r: 0, g: "~~>",   n: "anguiliss",        lore: "un éclair au ralenti. électrise les conversations, littéralement." },
    CreatureDef { b: 5, r: 1, g: "(8)",   n: "poulpinet",        lore: "ouvre les pièges de l'intérieur. reste dedans par confort." },
    CreatureDef { b: 5, r: 1, g: ">:)",   n: "nocturnix",        lore: "sourit dans le noir. c'est précisément le problème." },
    CreatureDef { b: 5, r: 1, g: "_o_",   n: "fondrille",        lore: "vit encore plus bas que le fond. remonte pour les grandes occasions." },
    CreatureDef { b: 5, r: 2, g: ".-.",   n: "spectrelle",       lore: "le fantôme d'un poisson qui refuse d'admettre quoi que ce soit." },
    CreatureDef { b: 5, r: 2, g: "[ ]",   n: "néantin",          lore: "un morceau de rien, soigneusement encadré." },
    CreatureDef { b: 5, r: 3, g: "{X}",   n: "krakenot",         lore: "un kraken de poche. les navires miniatures le redoutent." },
    CreatureDef { b: 5, r: 4, g: "(Ω)",   n: "ancien des profondeurs", lore: "il était là avant les biomes. il sera là après vous." },
    // ---- volcan (8)
    CreatureDef { b: 6, r: 0, g: "(∴)",   n: "bracille",         lore: "un lézard qui dort sur les braises. les braises apprécient." },
    CreatureDef { b: 6, r: 0, g: "-·-",   n: "cendrillot",       lore: "fait des tas de cendres bien rangés. les défait. recommence." },
    CreatureDef { b: 6, r: 0, g: ")))",   n: "fumerol",          lore: "un serpentin de fumée qui a des opinions." },
    CreatureDef { b: 6, r: 1, g: "[≈]",   n: "magmite",          lore: "une flaque de lave apprivoisée. ne pas caresser." },
    CreatureDef { b: 6, r: 1, g: "*!*",   n: "soufrelin",        lore: "sent l'œuf pourri et en joue." },
    CreatureDef { b: 6, r: 2, g: "{#}",   n: "pyroclaste",       lore: "il explose par politesse, pour prévenir." },
    CreatureDef { b: 6, r: 3, g: "~§~",   n: "salamandragore",   lore: "sa morsure brûle, son lore aussi." },
    CreatureDef { b: 6, r: 4, g: "(♦)",   n: "cœur-de-forge",    lore: "le volcan bat à son rythme. littéralement." },
    // ---- récif (12)
    CreatureDef { b: 7, r: 0, g: ":o:",   n: "corailleau",       lore: "un bout de corail qui a pris la mer au sérieux." },
    CreatureDef { b: 7, r: 0, g: "°o°",   n: "bulleret",         lore: "produit des bulles carrées. refuse d'expliquer." },
    CreatureDef { b: 7, r: 0, g: "?~",    n: "hippocampette",    lore: "se déplace uniquement à la verticale, par principe." },
    CreatureDef { b: 7, r: 0, g: ".*.",   n: "étoilette",        lore: "compte ses branches en boucle. cinq. toujours cinq." },
    CreatureDef { b: 7, r: 0, g: "=o=",   n: "clownard",         lore: "vit dans une anémone, en colocation conflictuelle." },
    CreatureDef { b: 7, r: 1, g: "}i{",   n: "languste",         lore: "joue de ses antennes comme d'un violon." },
    CreatureDef { b: 7, r: 1, g: "<^>",   n: "raiettine",        lore: "plane sous l'eau. l'eau n'a rien remarqué." },
    CreatureDef { b: 7, r: 1, g: "~e~",   n: "murénia",          lore: "sourit beaucoup trop pour quelqu'un qui vit dans un trou." },
    CreatureDef { b: 7, r: 2, g: "(@)",   n: "nautilange",       lore: "porte sa maison en spirale. déménage sans le savoir." },
    CreatureDef { b: 7, r: 2, g: ">=>",   n: "barracuml",        lore: "rapide, nerveux, incapable de nager en ligne droite." },
    CreatureDef { b: 7, r: 3, g: "(o)",   n: "perlamère",        lore: "sa perle vaut une fortune. elle le sait. elle négocie." },
    CreatureDef { b: 7, r: 4, g: "≈C≈",   n: "chantecoral",      lore: "le chant du récif. l'entendre, c'est déjà l'avoir cherché trop longtemps." },
    // ---- ruines (10)
    CreatureDef { b: 8, r: 0, g: "[.]",   n: "gravelin",         lore: "un caillou taillé qui se souvient d'avoir été une colonne." },
    CreatureDef { b: 8, r: 0, g: "...",   n: "poussiéreux",      lore: "il est littéralement de la poussière. motivée." },
    CreatureDef { b: 8, r: 1, g: "}{",    n: "liergne",          lore: "du lierre qui grimpe sur ce qui n'existe plus." },
    CreatureDef { b: 8, r: 1, g: "#:#",   n: "mosaïquin",        lore: "des tesselles qui se recomposent la nuit." },
    CreatureDef { b: 8, r: 1, g: "!i!",   n: "chandelmoine",     lore: "une bougie qui fait des rondes. par habitude." },
    CreatureDef { b: 8, r: 2, g: "|o|",   n: "gardogol",         lore: "il garde une porte. la porte n'existe plus. il garde quand même." },
    CreatureDef { b: 8, r: 2, g: "<?>",   n: "oraclyphe",        lore: "il prédit le passé avec une précision remarquable." },
    CreatureDef { b: 8, r: 3, g: ".^.",   n: "spectrarque",      lore: "l'ancien maître des lieux. très à cheval sur l'étiquette." },
    CreatureDef { b: 8, r: 3, g: "{t}",   n: "chronolithe",      lore: "le temps passe autour de lui, jamais à travers." },
    CreatureDef { b: 8, r: 4, g: "/#\\",  n: "bâtisseur oublié", lore: "il a construit les ruines. neuves, à l'époque." },
    /* rivière : neuf espèces, du courant vif aux berges */
    CreatureDef { b: 9, r: 0, g: "~o~",  n: "vairounet",     lore: "remonte le courant par principe. n'a jamais dit lequel." },
    CreatureDef { b: 9, r: 0, g: "-<><", n: "ablette grise", lore: "vit en banc. chaque membre est persuadé de mener la troupe." },
    CreatureDef { b: 9, r: 0, g: ",o,",  n: "galetin",       lore: "se fait passer pour un caillou. très convaincant, jusqu'à ce qu'il nage." },
    CreatureDef { b: 9, r: 1, g: "~>~",  n: "flèche d'eau",  lore: "traverse un gué avant qu'on ait fini de le regarder." },
    CreatureDef { b: 9, r: 1, g: "(oo)", n: "loutron",       lore: "collectionne des objets brillants au fond. refuse de dire où." },
    CreatureDef { b: 9, r: 1, g: "=^=",  n: "martin-pêche",  lore: "plonge mieux que vous ne piégez. il le sait." },
    CreatureDef { b: 9, r: 2, g: "~§~",  n: "anguillon",     lore: "on croit l'avoir. on a un nœud." },
    CreatureDef { b: 9, r: 2, g: "<=>",  n: "écrevisse d'or",lore: "recule pour avancer. tout un programme." },
    CreatureDef { b: 9, r: 3, g: "~▲~",  n: "silure ancien", lore: "vit sous le pont depuis avant le pont." },
    /* lac : sept espèces, là où le courant s'arrête */
    CreatureDef { b: 10, r: 0, g: "o~o",  n: "bulleau",      lore: "monte à la surface pour éclater. recommence. inlassablement." },
    CreatureDef { b: 10, r: 0, g: "~w~",  n: "nénufaron",    lore: "une feuille qui a appris à nager, et qui en fait trop." },
    CreatureDef { b: 10, r: 1, g: "(°)",  n: "carpaillon",   lore: "vous regarde depuis le fond avec une patience de fonctionnaire." },
    CreatureDef { b: 10, r: 2, g: "<~>",  n: "ondine grise", lore: "on ne la voit qu'au reflet. jamais dans l'eau." },
    CreatureDef { b: 10, r: 2, g: "≈o≈",  n: "brumelac",     lore: "se confond avec la brume du matin. dort le reste du temps." },
    CreatureDef { b: 10, r: 3, g: "~©~",  n: "nautile pâle", lore: "une spirale qui tourne dans le mauvais sens. personne n'ose corriger." },
    CreatureDef { b: 10, r: 4, g: "<@>",  n: "gardien du lac", lore: "au fond, quelque chose de très vieux compte les jours. il en manque quatre." },
    /* curiosités : introuvables dans la nature, elles ne s'obtiennent qu'au troc */
    CreatureDef { b: 11, r: 1, g: "<o>",  n: "troqueline",  lore: "n'appartient jamais deux fois à la même personne. c'est sa façon de voyager." },
    CreatureDef { b: 11, r: 2, g: "[+]",  n: "colporel",    lore: "dort dans les sacoches. se réveille exactement au moment de l'échange." },
    CreatureDef { b: 11, r: 2, g: "%~%",  n: "pacotille",   lore: "sans valeur, paraît-il. tout le monde en veut une." },
    CreatureDef { b: 11, r: 3, g: "<*>",  n: "curiosa",     lore: "vient d'un biome que personne n'a cartographié. elle refuse d'en parler." },
    CreatureDef { b: 11, r: 3, g: "(:)",  n: "porcelin",    lore: "une figurine qui respire. les collectionneurs se l'arrachent, elle s'en moque." },
    CreatureDef { b: 11, r: 4, g: "=@=",  n: "chimérel",    lore: "trois marchands jurent l'avoir vendu le même jour. aucun ne ment." },
    /* légendes errantes : aucun piège ne les prend, elles n'apparaissent qu'en
       silhouette, n'importe où sur la carte, et se tentent à la main */
    CreatureDef { b: 12, r: 3, g: "^v^",  n: "arpenteur",     lore: "il traverse la carte d'un bout à l'autre chaque nuit. personne ne sait ce qu'il compte." },
    CreatureDef { b: 12, r: 3, g: "(~~)", n: "brumaille",     lore: "une brume qui a pris l'habitude d'avoir une forme. elle y tient." },
    CreatureDef { b: 12, r: 3, g: "<^>",  n: "veilleur pâle", lore: "il se poste là où l'on va passer, puis attend. il attend depuis longtemps." },
    CreatureDef { b: 12, r: 3, g: "}o{",  n: "colporteur des vents", lore: "il transporte des odeurs d'un biome à l'autre. c'est ainsi qu'on sait qu'il est venu." },
    CreatureDef { b: 12, r: 4, g: "*^*",  n: "aube-errante",  lore: "elle n'apparaît qu'entre deux instants. les horloges du village avancent d'une seconde après son passage." },
    CreatureDef { b: 12, r: 4, g: "<=>",  n: "faucheur de sel", lore: "il suit les côtes et les vieilles routes. là où il s'arrête, plus rien ne pousse." },
    CreatureDef { b: 12, r: 4, g: "[o]",  n: "sentinelle creuse", lore: "une carapace sans habitant, qui se déplace pourtant. on a renoncé à savoir." },
    CreatureDef { b: 12, r: 4, g: "~*~",  n: "mirage du nord", lore: "vu au glacier, au désert et au marais le même soir. les trois témoins sont formels." },
    CreatureDef { b: 12, r: 4, g: "(*)",  n: "œil du monde",  lore: "il regarde le joueur, jamais le piège. c'est déjà une réponse." },
    CreatureDef { b: 12, r: 4, g: "}*{",  n: "ombre-lierre",  lore: "elle pousse sur les traces des autres légendes. là où elle est, une autre est passée." },
    CreatureDef { b: 12, r: 4, g: "<*>",  n: "porte-écailles", lore: "chaque écaille vient d'une espèce différente. aucune ne manque à personne." },
    CreatureDef { b: 12, r: 4, g: "*@*",  n: "premier piégé", lore: "la toute première prise du tout premier traqueur. elle s'est échappée depuis, et n'a pas vieilli." },
    /* le puits : elle ne se capture pas, elle accepte de remonter */
    CreatureDef { b: 13, r: 4, g: "(0)",  n: "puisard", lore: "on l'a descendu au bout d'une corde, il y a très longtemps, et on a remis la margelle. il remonte quand ça lui chante, et seulement si on lui a donné quelque chose." },
];

/* espèces qui comptent dans le bestiaire : les curiosités en sont exclues,
   pour que le pourcentage et « bestiaire complet » gardent leur sens. */
/* jour de foire : une journée sur quatre, le village s'anime */
fn un_couple() -> u32 {
    1
}
fn fair_day() -> bool {
    ((now_ms() / 86_400_000.0) as u64) % 4 == 0
}

/* chance de shiny maximale qu'un total de gains permet d'atteindre : le bonus
   se paie au labo (chasse nocturne), et la nuit étoilée double la mise. sert au
   serveur du classement — 33 shinies pour 4 500 prises supposent un labo que
   186 000 écus gagnés ne financent pas. */
pub fn shiny_max_par_capture(ecus_gagnes: f64) -> f64 {
    let (mut cumul, mut niveau) = (0.0, 0u32);
    while niveau < LABS[LAB_ECLAT].max {
        let cout = LABS[LAB_ECLAT].base * LABS[LAB_ECLAT].mult.powi(niveau as i32);
        if cumul + cout > ecus_gagnes {
            break;
        }
        cumul += cout;
        niveau += 1;
    }
    /* la nuit étoilée double la chance, mais n'occupe qu'une fenêtre météo sur
       six : on retient 1,5, soit bien plus que la moyenne réelle (~1,17) sans
       aller jusqu'à supposer une nuit étoilée perpétuelle. */
    SHINY_BASE * (1.0 + niveau as f64 * 0.10) * 1.5
}

/* les biomes de la carte, du moins cher au plus cher : l'ordre d'affichage
   suit la progression du joueur, pas l'ordre interne de la table (rivière et
   lac ont été ajoutés après coup et coûtent pourtant presque rien). */
pub fn biomes_par_prix() -> Vec<usize> {
    let mut v: Vec<usize> = (0..WILDB).collect();
    v.sort_by(|&a, &b| BIOMES[a].cost.partial_cmp(&BIOMES[b].cost).unwrap_or(std::cmp::Ordering::Equal));
    v
}
/* (prix d'ouverture, nombre d'espèces) de chaque biome, du moins cher au plus
   cher — le serveur du classement s'en sert pour vérifier qu'un bestiaire
   annoncé était finançable. une seule table, jamais deux à resynchroniser. */
/* les espèces hors biome — curiosités du troc et légendes errantes — ne
   s'attrapent dans aucune terre et n'entrent dans aucun palier de prix. le
   classement doit donc les tolérer au-dessus du bestiaire finançable. */
pub fn especes_hors_biome() -> usize {
    CREATURES.iter().filter(|c| c.b >= WILDB).count()
}
pub fn curiosites_max() -> usize {
    CREATURES.iter().filter(|c| c.b == CURIO_B).count()
}
pub fn legendes_max() -> usize {
    CREATURES.iter().filter(|c| c.b >= LEGEND_B).count()
}
/* ce que valent au palmarès les espèces qu'aucun piège n'attrape : le troc
   coûte des doublons et ne se revend pas, l'approche d'une légende se joue en
   une seule fois. sans ligne au score, les deux ne paieraient jamais. */
pub const PTS_CURIOSITE: f64 = 500.0;
pub const PTS_LEGENDE: f64 = 1000.0;

pub fn paliers_bestiaire() -> Vec<(f64, f64)> {
    biomes_par_prix()
        .into_iter()
        .map(|b| (BIOMES[b].cost, biome_creatures(b).count() as f64))
        .collect()
}

fn wild_species() -> impl Iterator<Item = usize> {
    (0..CREATURES.len()).filter(|&i| CREATURES[i].b < WILDB)
}
fn wild_total() -> usize {
    CREATURES.iter().filter(|c| c.b < WILDB).count()
}

/* les légendes errantes, avec leurs poids : les épiques paraissent trois fois
   plus souvent que les légendaires. */
fn tirage_legende() -> usize {
    let pool: Vec<(usize, u32)> = (0..CREATURES.len())
        .filter(|&i| CREATURES[i].b == LEGEND_B)
        .map(|i| (i, if CREATURES[i].r >= 4 { 1 } else { 3 }))
        .collect();
    let total: u32 = pool.iter().map(|&(_, w)| w).sum();
    let mut tir = rand::thread_rng().gen_range(0..total);
    for (i, w) in &pool {
        if tir < *w {
            return *i;
        }
        tir -= w;
    }
    pool[0].0
}

fn biome_creatures(b: usize) -> impl Iterator<Item = usize> {
    (0..CREATURES.len()).filter(move |&i| CREATURES[i].b == b)
}

struct TrapDef { n: &'static str, cost: f64, itv: f64, luck: f64, succ: f64 }
const TRAPS: [TrapDef; 6] = [
    TrapDef { n: "piège en bois",   cost: 40.0,      itv: 60.0, luck: 0.0,  succ: 0.45 },
    TrapDef { n: "cage en fer",     cost: 800.0,     itv: 42.0, luck: 0.35, succ: 0.58 },
    TrapDef { n: "piège à ressort", cost: 8000.0,    itv: 30.0, luck: 0.90, succ: 0.70 },
    TrapDef { n: "piège chromé",    cost: 60000.0,   itv: 21.0, luck: 1.80, succ: 0.80 },
    TrapDef { n: "piège à plasma",  cost: 400000.0,  itv: 15.0, luck: 3.20, succ: 0.88 },
    TrapDef { n: "piège quantique", cost: 2500000.0, itv: 10.0, luck: 5.50, succ: 0.95 },
];

struct BaitDef { n: &'static str, cost: f64, desc: &'static str }
const BAITS: [BaitDef; 5] = [
    BaitDef { n: "baies sauvages",    cost: 15.0,   desc: "vitesse du piège +15%" },
    BaitDef { n: "viande fumée",      cost: 60.0,   desc: "chance +0,12" },
    BaitDef { n: "nectar doré",       cost: 300.0,  desc: "valeur des prises ×1,3 · chance +0,08" },
    BaitDef { n: "truffe des brumes", cost: 1200.0, desc: "poids des raretés rare+ ×1,8" },
    BaitDef { n: "essence lunaire",   cost: 4000.0, desc: "chance de shiny ×3 · chance +0,1" },
];
const BAIT_BAIES: usize = 0;
const BAIT_VIANDE: usize = 1;
const BAIT_NECTAR: usize = 2;
const BAIT_TRUFFE: usize = 3;
const BAIT_ESSENCE: usize = 4;

struct LabDef { n: &'static str, max: u32, base: f64, mult: f64, desc: &'static str }
const LABS: [LabDef; 15] = [
    LabDef { n: "affûtage",        max: 10, base: 600.0,  mult: 2.4, desc: "des mâchoires mieux huilées : +6% de vitesse par niveau." },
    LabDef { n: "flair",           max: 15, base: 900.0,  mult: 2.3, desc: "l'instinct du traqueur : +0,08 de chance par niveau." },
    LabDef { n: "négoce",          max: 15, base: 800.0,  mult: 2.3, desc: "l'art de la marge : +5% aux prix de vente par niveau." },
    LabDef { n: "horlogerie",      max: 11, base: 1200.0, mult: 2.1, desc: "progression hors-ligne simulée au retour : 2 h de base, +2 h par niveau. les pièges, eux, ne s'usent jamais." },
    LabDef { n: "chasse nocturne", max: 10, base: 3000.0, mult: 2.4, desc: "sortir aux bonnes heures : +10% de chance de shiny par niveau." },
    LabDef { n: "auto-vente",      max: 1,  base: 15000.0, mult: 1.0, desc: "revend les doublons dès la capture, selon vos filtres (à la boutique)." },
    LabDef { n: "conservation",    max: 10, base: 5000.0, mult: 2.0, desc: "de meilleures vitrines : la cagnotte du musée accumule +2 h par niveau (4 h de base)." },
    LabDef { n: "ailes du musée",  max: 6,  base: 12000.0, mult: 2.4, desc: "on pousse les murs : +1 salle d'exposition par niveau." },
    LabDef { n: "grands enclos",   max: 3,  base: 10000.0, mult: 2.8, desc: "plus de place pour les couples : +1 enclos par niveau." },
    LabDef { n: "lignées",         max: 5,  base: 8000.0, mult: 2.4, desc: "registres d'élevage : +4% de chance qu'une naissance monte d'un rang (25% de base)." },
    LabDef { n: "traqueur",        max: 5,  base: 4000.0, mult: 2.2, desc: "meilleure endurance : le repos entre deux battues diminue de 30 s par niveau (5 min de base)." },
    LabDef { n: "courtage",        max: 5,  base: 6000.0, mult: 2.3, desc: "carnet d'adresses : les primes de contrats augmentent de 15% par niveau." },
    LabDef { n: "licence de piégeage", max: 6, base: 3000.0, mult: 2.6, desc: "l'administration est tatillonne : 2 pièges posés autorisés de base, +1 par niveau." },
    LabDef { n: "appel des légendes", max: 5, base: 25000.0, mult: 2.7, desc: "des appeaux qui portent loin : +2,6 points de chance qu'une légende errante paraisse à chaque tirage (20% de base, et vos battues s'y ajoutent)." },
    LabDef { n: "approche silencieuse", max: 5, base: 35000.0, mult: 2.7, desc: "on apprend à ne plus faire craquer les branches : +5% de réussite face à une légende." },
];
const LAB_AFFUTAGE: usize = 0;
const LAB_FLAIR: usize = 1;
const LAB_NEGOCE: usize = 2;
const LAB_HORLOGE: usize = 3;
const LAB_ECLAT: usize = 4;
const LAB_AUTOVENTE: usize = 5;
const LAB_CONSERVATION: usize = 6;
const LAB_AILES: usize = 7;
const LAB_ENCLOS: usize = 8;
const LAB_LIGNEES: usize = 9;
const LAB_TRAQUEUR: usize = 10;
const LAB_COURTAGE: usize = 11;
const LAB_LICENCE: usize = 12;
const LAB_APPEL: usize = 13;
const LAB_APPROCHE: usize = 14;
const MERCH_ITEMS: usize = 5;

struct AchDef { n: &'static str, d: &'static str, r: f64 }
const ACH_666: usize = 27;
const ACHS: [AchDef; 33] = [
    AchDef { n: "première prise",         d: "capturer une créature",                r: 50.0 },
    AchDef { n: "braconnier du dimanche", d: "capturer 100 créatures",               r: 500.0 },
    AchDef { n: "main verte",             d: "capturer 1 000 créatures",             r: 5000.0 },
    AchDef { n: "force de la nature",     d: "capturer 10 000 créatures",            r: 50000.0 },
    AchDef { n: "ça brille",              d: "capturer un shiny",                    r: 1000.0 },
    AchDef { n: "aimant à paillettes",    d: "capturer 25 shinies",                  r: 25000.0 },
    AchDef { n: "chasseur de mythes",     d: "capturer une créature légendaire",     r: 3000.0 },
    AchDef { n: "carnet de terrain",      d: "découvrir 10 espèces",                 r: 300.0 },
    AchDef { n: "encyclopédiste",         d: "découvrir 30 espèces",                 r: 3000.0 },
    AchDef { n: "bestiaire complet",      d: "découvrir toutes les espèces",             r: 100000.0 },
    AchDef { n: "les pieds dans la vase", d: "débloquer le marais",                  r: 200.0 },
    AchDef { n: "au fond du gouffre",     d: "débloquer les abysses",                r: 200000.0 },
    AchDef { n: "premier magot",          d: "gagner 10 000 écus au total",          r: 1000.0 },
    AchDef { n: "fortune faite",          d: "gagner 1 000 000 d'écus au total",     r: 50000.0 },
    AchDef { n: "ingénierie douteuse",    d: "posséder un piège quantique",          r: 30000.0 },
    AchDef { n: "nouveau départ",         d: "effectuer une migration",              r: 0.0 },
    AchDef { n: "nomade",                 d: "effectuer 5 migrations",               r: 0.0 },
    AchDef { n: "forêt domestiquée",      d: "découvrir les 10 espèces de la forêt", r: 2000.0 },
    AchDef { n: "beau spécimen",          d: "capturer une créature de rang S",      r: 2000.0 },
    AchDef { n: "œil du maître",          d: "obtenir 10 espèces en rang S",         r: 20000.0 },
    AchDef { n: "battue éclair",          d: "mener une battue",                     r: 300.0 },
    AchDef { n: "fournisseur",            d: "livrer 5 contrats",                    r: 5000.0 },
    AchDef { n: "l'insaisissable",        d: "capturer une légende errante",         r: 10000.0 },
    AchDef { n: "éleveur",                d: "obtenir une naissance à l'enclos",     r: 2000.0 },
    AchDef { n: "conservateur",           d: "remplir les 6 salles du musée",        r: 15000.0 },
    AchDef { n: "oiseau de nuit",         d: "capturer une espèce nocturne",         r: 1000.0 },
    AchDef { n: "assidu",                 d: "chasser 7 jours d'affilée",            r: 2000.0 },
    AchDef { n: "le compte est bon",     d: "capturer 666 créatures",               r: 666.0 },
    AchDef { n: "panthéon",               d: "capturer les 12 légendes errantes",    r: 120000.0 },
    /* scellés : ni le nom ni la condition ne s'affichent avant de les gagner */
    AchDef { n: "sympathy for the devil",  d: "avoir donné une bête au puits",        r: 666.0 },
    AchDef { n: "bad is good",             d: "six offrandes au puits",               r: 6666.0 },
    AchDef { n: "it is a good day to die", d: "soixante-six offrandes au puits",      r: 66666.0 },
    AchDef { n: "unleash the beast",       d: "six cent soixante-six offrandes",      r: 666666.0 },
];
/* les trophées que rien n'annonce : le panneau les tait tant qu'ils dorment */
const ACH_SCELLES: [usize; 4] = [29, 30, 31, 32];
const ACH_BETE: usize = 32;

const SHINY_BASE: f64 = 1.0 / 512.0;

const SAISONS: [&str; 4] = ["printemps", "été", "automne", "hiver"];
const METEOS: [&str; 6] = ["ciel clair", "pluie", "brume", "canicule", "tempête", "nuit étoilée"];
/* espèces qui ne sortent que la nuit (21 h – 7 h) */
/* espèces qui ne sortent qu'entre 21 h et 7 h — davantage dans les lieux
   sombres (abysses, ruines) que sur les crêtes ou dans le volcan */
const NOCTURNES: [usize; 20] = [
    1, 9,          // forêt : sourivole, lucioleau
    21, 23,        // marais : feufollet, basilombre
    28,            // montagne : cristalpin
    39, 41,        // désert : scorpiard, mirageon
    52, 54,        // glacier : rennelune, aurorelle
    57, 62, 64,    // abysses : lanternet, nocturnix, spectrelle
    70,            // volcan : fumerol
    83,            // récif : murénia
    92, 95, 96,    // ruines : chandelmoine, spectrarque, chronolithe
    106,           // rivière : silure ancien
    111, 113,      // lac : brumelac, gardien du lac
];
/* journal des versions — la plus récente en tête. VERSION sert de repère
   « déjà lu » : quand elle change, la pastille ● réapparaît dans la barre. */
const VERSION: &str = "1.27";
const NEWS: [(&str, &str, &[&str]); 28] = [
    (
        "1.27",
        "23 septembre 2026",
        &[
            "l'enclos conseille : le sélecteur est trié par intérêt et non plus par ordre interne, chaque ligne dit le rang du petit et ce que porte déjà le registre, une flèche ↑ marque les couples qui feraient progresser le bestiaire, et un bouton « meilleur couple » installe directement le plus utile.",
            "l'auto-vente épargne autant de couples que vous voulez, jusqu'à quatre par espèce, au lieu d'un seul. un couple unique ne permettait qu'une reproduction, les parents étant consommés : l'enclos manquait de stock.",
        ],
    ),
    (
        "1.26",
        "23 septembre 2026",
        &[
            "le journal se lit en deux colonnes : les prises à gauche, tout le reste à droite. une éclosion, une légende, une battue ou une offrande au puits ne se font plus chasser par trois captures automatiques.",
            "et sur la version navigateur, la favicon clignote tant qu'une légende rôde ou que le puits a soif, avec un rappel dans le titre de l'onglet. de quoi jouer dans un autre onglet sans rien rater.",
        ],
    ),
    (
        "1.25",
        "23 septembre 2026",
        &[
            "le puits rassasié ne luit plus : la lueur rouge annonce désormais une offrande possible, et pas seulement une heure de la nuit.",
        ],
    ),
    (
        "1.24",
        "23 septembre 2026",
        &[
            "le village a son horloge : une journée y dure deux heures réelles, soit douze jours pendant que vous en vivez un. le jour, la nuit, les saisons et la lune suivent cette horloge et non plus celle de votre ordinateur : les vingt espèces nocturnes ◦ sortent cinquante minutes toutes les deux heures, donc elles deviennent accessibles à qui ne joue jamais après 21 h. les pièges, les contrats, le troc et les légendes restent au temps réel : rien de ce qui produit n'a été accéléré.",
            "la barre du bas et le tableau de bord affichent l'heure qu'il est au village.",
            "quelque chose dort au fond du puits, et il arrive qu'une offrande le réveille. c'est la seule façon de le rencontrer.",
        ],
    ),
    (
        "1.23",
        "23 septembre 2026",
        &[
            "la fontaine du village n'a pas toujours été une fontaine. certaines nuits, très tard, l'eau recule et la margelle bat d'une lueur rouge. approchez-vous, vous verrez bien.",
            "quatre trophées de plus au tableau. ils n'ont pas de nom tant qu'ils ne sont pas gagnés, et personne n'en parle. idée de ook.",
        ],
    ),
    (
        "1.22",
        "10 septembre 2026",
        &[
            "une silhouette de légende reste maintenant vingt minutes au lieu de dix. avec un tirage toutes les dix minutes, il y a une légende à chercher sur la carte près de huit heures par jour : une session normale devrait en croiser, sans qu'un passage éclair une fois par jour suffise.",
        ],
    ),
    (
        "1.21",
        "10 septembre 2026",
        &[
            "les légendes paraissent deux fois plus souvent : 20 chances sur 100 par tirage au lieu de 10, soit une trentaine de silhouettes par jour. les battues pèsent le double elles aussi (+2 points chacune, jusqu'à +16) et l'appel du labo passe à +2,6 points par niveau. en une semaine, le joueur le plus assidu en avait croisé sept espèces sur douze, et le plus gros captureur aucune : elles étaient trop rares pour qui ne guette pas la carte.",
            "à l'enclos, on choisit le rang des parents. par défaut ce sont toujours les plus bas, mais le petit naissant au meilleur rang de ses parents, sacrifier un couple [A] ou [S] pour viser un beau spécimen — shiny compris — devient possible. suggéré par Drasaerys.",
        ],
    ),
    (
        "1.20",
        "8 septembre 2026",
        &[
            "les curiosités et les légendes se vendent enfin à la boutique : la liste ne parcourait que les biomes, si bien qu'un doublon restait coincé en réserve sans emploi. la découverte, elle, reste acquise au bestiaire — vendre un doublon ne coûte aucun point au palmarès.",
            "une curiosité vaut désormais dix fois sa valeur de base au lieu d'une : de quoi rendre un doublon intéressant, sans que le troc rapporte plus qu'il ne coûte.",
        ],
    ),
    (
        "1.19",
        "8 septembre 2026",
        &[
            "le troc rapporte enfin au palmarès : chaque curiosité vaut 500 points et chaque légende errante 1000. elles ne se revendent pas et ne comptent pas parmi les espèces, il n'y avait donc aucune raison d'en chercher une deuxième. merci à ook pour la remarque.",
            "le classement affiche deux colonnes de plus, curiosités et légendes, avec leur total à atteindre.",
        ],
    ),
    (
        "1.18",
        "6 septembre 2026",
        &[
            "le labo annonçait des effets qu'il n'appliquait pas : la colonne de droite promettait +8% de négoce par niveau au lieu de +5, +15% de chance de shiny au lieu de +10, +0,06 de flair au lieu de +0,08 et 35% de montée de rang à l'enclos au lieu de 25. les descriptions, elles, étaient justes : ce sont bien elles que le jeu applique. merci à Drasaerys pour l'œil.",
            "la licence de piégeage compte enfin les licences achetées au marchand dans le nombre de pièges autorisés qu'elle affiche.",
        ],
    ),
    (
        "1.17",
        "3 septembre 2026",
        &[
            "les légendes se tirent maintenant toutes les 10 minutes au lieu de toutes les 30, à 10 chances sur 100 par tirage : le rythme quotidien ne change pas, mais une battue se fait sentir tout de suite au lieu d'attendre la demi-heure suivante.",
            "en échange, la silhouette ne reste que ces dix minutes-là. il faut être présent pour la croiser — c'est le but.",
            "les battues ajoutent +1 point par tirage pendant une heure, cumulables jusqu'à +8, et l'appel du labo +1,3 point par niveau. tout à fond, on passe de 14 à plus de 40 silhouettes par jour.",
        ],
    ),
    (
        "1.16",
        "3 septembre 2026",
        &[
            "douze légendes errantes rejoignent le bestiaire. aucun piège ne les prend : elles ne s'obtiennent qu'en approchant une silhouette ✧, et elles n'appartiennent à aucun biome.",
            "les silhouettes paraissent désormais partout, y compris sur les terres que vous n'avez pas encore ouvertes : une légende peut vous attendre au volcan bien avant que vous ayez les moyens d'y poser un piège.",
            "chaque battue remue le terrain : pendant une heure, les légendes ont +3 chances sur 100 de paraître, et les battues se cumulent jusqu'à +24. le tableau de bord affiche la pression en cours.",
            "deux nouvelles compétences au labo : l'appel des légendes (leur fréquence) et l'approche silencieuse (votre réussite face à elles).",
            "et ce panneau s'ouvre tout seul au premier lancement après une mise à jour, plutôt que d'attendre que vous remarquiez la pastille.",
        ],
    ),
    (
        "1.15",
        "3 septembre 2026",
        &[
            "les listes bouclent : dans un panneau, le haut depuis la première entrée saute à la dernière, et le bas depuis la dernière revient au début. les menus d'espèces s'allongeant à chaque biome ouvert, la fin de liste n'est plus à vingt appuis.",
        ],
    ),
    (
        "1.14",
        "2 septembre 2026",
        &[
            "les curiosités obtenues au troc ne font plus sortir du classement : elles étaient comptées comme des espèces de biome, et le serveur refusait un bestiaire plus large que ce que la partie pouvait s'offrir.",
        ],
    ),
    (
        "1.13",
        "2 septembre 2026",
        &[
            "le musée sait se garnir seul : une option choisit les spécimens les mieux payés de la réserve et suit vos nouvelles prises, sans passer les salles une par une.",
            "les montants abrégés sont tronqués et non plus arrondis : avec 2 999 500 écus, le prix d'un biome à 3 M ne s'affiche plus comme s'il était payable.",
        ],
    ),
    (
        "1.12",
        "22 août 2026",
        &[
            "les biomes s'affichent enfin dans l'ordre des prix, au tableau de bord comme au bestiaire et au comptoir : la rivière (900 écus) et le lac (45 k) traînaient derrière les ruines à 440 M.",
            "les traces et les légendes ne tombent plus au milieu de l'eau : depuis que le lac et la rivière ne se traversent plus, elles y étaient hors d'atteinte. tout point d'apparition rejoint la berge la plus proche.",
            "le classement croise désormais quatre valeurs qui se tiennent entre elles — captures, espèces, shinies et écus gagnés. améliorer sa chance de shiny se paie au labo : en déclarer plus que sa cagnotte ne l'autorise sort du tableau, comme découvrir des biomes qu'on n'a pas les moyens d'ouvrir.",
            "la vitesse de capture retrouve une limite réaliste : elle autorisait 18 000 prises à l'heure, huit fois ce que huit pièges peuvent produire.",
        ],
    ),
    (
        "1.11",
        "21 août 2026",
        &[
            "une naissance se raconte : l'enclos annonce le rang des deux parents et celui du petit, dit s'il dépasse sa lignée, s'il brille, ou s'il inaugure une espèce au bestiaire.",
            "et il prévient dès que le petit est né, au lieu de vous laisser le découvrir en passant.",
            "les contrats affichent leur rapport face à la boutique (×N, vert ou rouge) : la prime vaut environ deux fois la vente des mêmes bêtes, mais la livraison prend vos plus bas rangs — avec un stock de beaux spécimens, elle devient perdante.",
            "le classement ne se laisse plus arranger : un shiny toutes les 80 prises au plus, et surtout, on ne peut pas avoir découvert les espèces de biomes qu'on n'avait pas les moyens d'ouvrir. les deux valeurs se tiennent, baisser l'une fait sauter l'autre.",
        ],
    ),
    (
        "1.10",
        "21 août 2026",
        &[
            "le marchand ambulant se voit enfin : son étal reste dessiné sur la place même quand il est sur les routes, et vous y lisez l'heure de son retour. son arrivée est annoncée au journal.",
            "ses passages ne tombent plus à heures fixes : trois à cinq visites par jour, tirées au sort, de deux à quatre heures chacune — il est là près d'une heure sur deux.",
            "sa licence de piégeage suit désormais le prix du labo (40 % de moins que votre prochain palier) au lieu d'un tarif fixe qui n'avait de sens à aucun moment de la partie ; son plan de piège coûte bien la moitié du prix neuf.",
            "les traces disent ce qu'elles ont donné : la piste s'annonce avant la prise, et l'aide détaille les trois issues possibles.",
            "le conseil de session explique ce qu'il conseille : « le temps avantage la rivière, mais vous n'y avez aucun piège posé ».",
            "bancs et lampadaires ne barrent plus le passage : le mobilier est décoratif.",
        ],
    ),
    (
        "1.9",
        "21 août 2026",
        &[
            "le classement ne croit plus votre navigateur sur parole : le score est recalculé sur le serveur, à partir des mêmes composantes qu'avant. rien ne change pour vous, le chiffre affiché reste le même.",
            "une partie dont les chiffres ne tiennent pas debout sort du tableau : plus d'espèces que le temps de jeu n'en permet, des trophées que les gains n'ont pas payés, des écus tombés plus vite que le meilleur piège du jeu ne peut en rapporter. elle y revient d'elle-même dès qu'elle redevient plausible.",
            "trafiquer sa sauvegarde reste possible — c'est votre partie, elle vit dans votre navigateur. le classement, lui, n'en tiendra pas compte.",
        ],
    ),
    (
        "1.8",
        "21 août 2026",
        &[
            "un palier de piège se sent enfin : chaque modèle rapporte environ le double du précédent, au lieu d'un petit +40% pour un prix multiplié par six à vingt. les prix, eux, n'ont pas bougé.",
            "au labo, affûtage passe à +6% de vitesse et flair à +0,08 de chance par niveau : le second rapportait +1,2% de revenu pour 900 écus, personne ne pouvait le prendre au sérieux.",
            "du désert aux ruines, les prix suivent ce nouveau revenu, sinon la partie durerait trois fois moins longtemps. la forêt, la rivière, le marais, la montagne et le lac ne bougent pas : le début de partie profite du gain sans en payer le prix.",
            "un emplacement de piège coûte désormais le prix du piège qu'on y met, et non plus celui du biome. le quatrième emplacement des ruines valait deux fois le biome lui-même : on ne l'achetait jamais.",
            "la boutique reprend vos pièges à la moitié de leur prix — de quoi solder les vieux pièges en bois. jamais un piège posé, jamais le dernier.",
            "la battue passe en haut du panneau d'un biome, et « vendre tous les doublons » en tête de la boutique : deux gestes qu'on répète à chaque passage, ils n'ont plus à attendre sous le reste.",
            "les panneaux défilent à la molette et au trackpad.",
            "dans les menus, q va enfin à gauche : zqsd n'y marchait que dans trois directions sur quatre. Échap ferme le panneau.",
            "le bestiaire ne se décale plus : deux caractères y occupaient deux cases au lieu d'une, la lune des espèces nocturnes ◦ et l'étoile des shinies ⋆. les colonnes tiennent maintenant dans une fenêtre étroite, et la lune reste visible sur les espèces déjà capturées.",
            "la fenêtre des trophées s'intitulait « succès » : elle porte enfin le nom du bâtiment. elle liste toujours vos succès et leurs récompenses.",
        ],
    ),
    (
        "1.7",
        "21 août 2026",
        &[
            "nouvelle carte : l'eau descend enfin dans le bon sens. la rivière naît de la montagne, remplit le lac, contourne le village et se jette dans la mer — le récif borde la côte, les abysses sont au large.",
            "deux biomes de plus : la rivière (neuf espèces) et le lac (sept). le bestiaire passe à 114 espèces, votre pourcentage baisse d'autant — c'est du contenu en plus, pas une perte.",
            "le village a été redessiné : rues pavées, place centrale, fontaine, bancs, lampadaires, et des ponts là où les routes franchissent la rivière.",
            "on ne marche plus sur l'eau : la rivière et le lac ne se franchissent qu'aux ponts, et l'on y pose ses pièges depuis la berge.",
            "la fontaine ne bouche plus le passage : la rangée des portes est dégagée, on va de la boutique au labo en ligne droite.",
            "on circule beaucoup mieux dans les biomes : le décor est resté dense, mais l'essentiel ne barre plus la route.",
            "le lac a pris une forme de lac : une nappe arrondie, plus un rectangle.",
            "l'auto-vente met aussi de côté les spécimens réclamés par le troc, comme elle le fait déjà pour les commandes du comptoir.",
            "le comptoir de troc s'ouvre avec r (t[r]oc) plutôt qu'avec x.",
            "vingt espèces nocturnes ◦ au lieu de sept, et pas le même nombre selon les lieux : trois dans les abysses et les ruines, une seule sur les crêtes ou dans le volcan.",
        ],
    ),
    (
        "1.6",
        "21 août 2026",
        &[
            "comptoir de troc (touche r) : des collectionneurs échangent leurs curiosités contre vos doublons. six espèces qu'aucun piège n'attrape.",
            "marchand ambulant : il passe plusieurs fois par jour sur la place, avec une malle qui change — breloques de chance, licences, œufs de curiosité.",
            "jour de foire, un jour sur quatre : le marchand reste, le troc double ses demandes, la chance monte et les prix baissent.",
            "le marchand passe désormais quatre fois par jour en moyenne, à des heures qui ne se répètent jamais — et son étal reste visible sur la place même quand il est sur les routes, avec l'heure de son retour.",
            "les curiosités ne comptent pas dans le pourcentage du bestiaire : votre progression ne recule pas.",
            "la barre du bas s'adapte enfin aux petits écrans.",
        ],
    ),
    (
        "1.5",
        "21 août 2026",
        &[
            "palmarès : choisissez un pseudo et comparez-vous aux autres piégeurs — touche p, ou la page classement.",
            "un succès de plus, à la 666ᵉ prise. quelque chose s'ouvre sur la place du village.",
            "la place ne porte plus de trèfle vert : il laissait croire à un événement.",
            "affichage : le cadre tient désormais dans toutes les fenêtres, et les repères ne se posent plus sur la dernière ligne.",
            "une icône pour l'onglet, et une police unifiée partout.",
        ],
    ),
    (
        "1.4",
        "20 août 2026",
        &[
            "session partagée entre vos ordinateurs : un lien, la même partie, synchronisée toute seule.",
            "un seul appareil joue à la fois — les autres se mettent en pause plutôt que de diverger.",
        ],
    ),
    (
        "1.3",
        "20 août 2026",
        &[
            "le jeu tourne aussi dans le navigateur, avec la même progression hors-ligne.",
            "taille du texte réglable avec + et −.",
        ],
    ),
    (
        "1.2",
        "19 août 2026",
        &[
            "sexes ♂♀ et élevage à l'enclos : un couple donne une naissance, parfois d'un rang supérieur.",
            "traces au sol à suivre, conseil de session, et série de connexion avec un élan qui grandit.",
        ],
    ),
    (
        "1.1",
        "19 août 2026",
        &[
            "grande refonte de l'équilibrage : la progression se compte désormais en jours, pas en heures.",
            "98 espèces réparties sur neuf biomes, licence de piégeage, migration payante.",
        ],
    ),
    (
        "1.0",
        "18 août 2026",
        &["ouverture du comptoir."],
    ),
];

const RANK_NAMES: [&str; 4] = ["C", "B", "A", "S"];
const RANK_MULT: [f64; 4] = [1.0, 1.5, 2.2, 4.0];
/* position de la légende errante dans chaque biome */
const LEGEND_SPOTS: [(usize, usize); 11] = [(16, 26), (16, 48), (52, 8), (95, 10), (16, 7), (95, 64), (52, 68), (95, 34), (16, 68), (30, 50), (52, 29)];
/* les légendes se tirent souvent et faiblement plutôt que rarement et fort :
   un tirage toutes les 10 minutes rend la pression des battues sensible tout
   de suite, au lieu d'attendre la demi-heure suivante. la silhouette tient
   deux fenêtres, soit vingt minutes : assez pour qu'une session la croise,
   trop peu pour qu'on la trouve en passant une fois par jour. les chances se comptent pour mille, la finesse du dixième de
   point servant à répartir l'effet des battues et du labo. */
const LEGEND_WINDOW_MS: f64 = 600_000.0;
const LEGEND_LINGER: u64 = 2;
const LEGEND_BASE_PM: u64 = 200;
const LEGEND_MAX_PM: u64 = 450;
const LAB_APPEL_PM: u64 = 26;
/* une battue attire les légendes une heure durant, et l'effet se cumule
   jusqu'à huit battues — de quoi presque doubler la chance de base. */
const HUNT_BUFF_MS: f64 = 3_600_000.0;
const HUNT_BUFF_PM: u64 = 20;
const HUNT_BUFF_MAX_PM: u64 = 160;
/* durée de couvaison à l'enclos, par rareté (minutes) */
const PEN_MIN: [f64; 5] = [30.0, 60.0, 150.0, 420.0, 1080.0];

/* =================================================================== état */

#[derive(Serialize, Deserialize, Clone)]
struct Placement {
    trap: usize,
    bait: Option<usize>,
    next_at: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct BiomeState {
    slots: usize,
    pl: Vec<Option<Placement>>,
    #[serde(default)]
    hunt_at: f64, // prochaine battue autorisée
}

/* réserve par espèce : compteurs par rang (C,B,A,S), par sexe, normaux et shinies */
#[derive(Serialize, Deserialize, Clone, Default)]
struct InvE {
    m: [u64; 4],
    f: [u64; 4],
    sm: [u64; 4],
    sf: [u64; 4],
}
impl InvE {
    fn tn(&self) -> u64 { self.m.iter().sum::<u64>() + self.f.iter().sum::<u64>() }
    fn ts(&self) -> u64 { self.sm.iter().sum::<u64>() + self.sf.iter().sum::<u64>() }
    fn tm(&self) -> u64 { self.m.iter().sum() }
    fn tf(&self) -> u64 { self.f.iter().sum() }
    fn nr(&self, r: usize) -> u64 { self.m[r] + self.f[r] }
    fn sr(&self, r: usize) -> u64 { self.sm[r] + self.sf[r] }
}
/* registre du bestiaire : totaux à vie + meilleur rang vu (0 = jamais, sinon rang+1) */
#[derive(Serialize, Deserialize, Clone, Default)]
struct DexE {
    n: u64,
    s: u64,
    best: u8,
    bests: u8,
    #[serde(default)]
    mf: u8, // bit 0 : mâle observé · bit 1 : femelle observée
}
#[derive(Serialize, Deserialize, Clone)]
struct MusE {
    ci: usize,
    rank: usize,
    shiny: bool,
    #[serde(default)]
    sex: u8,
}
#[derive(Serialize, Deserialize, Clone)]
struct Pen {
    ci: usize,
    r1: usize,
    r2: usize,
    ready_at: f64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct State {
    ecus: f64,
    total_earned: f64,
    run_earned: f64,
    captures: u64,
    shinies: u64,
    attempts: u64,
    trophies: u32,
    migrations: u32,
    traps: Vec<u32>,
    baits: Vec<u64>,
    biomes: Vec<Option<BiomeState>>,
    #[serde(default)]
    inv2: Vec<InvE>,
    #[serde(default)]
    dex2: Vec<DexE>,
    #[serde(default)]
    contracts_window: u64,
    #[serde(default)]
    contracts_done: Vec<bool>,
    #[serde(default)]
    contracts: Vec<(usize, u64, f64)>,
    /* troc : (espèce demandée, quantité, curiosité offerte), figé par fenêtre */
    #[serde(default)]
    trades_window: u64,
    #[serde(default)]
    trades: Vec<(usize, u64, usize)>,
    #[serde(default)]
    trades_done: Vec<bool>,
    #[serde(default)]
    trades_made: u64,
    /* marchand ambulant : achats déjà faits (clé fenêtre×8+rang), breloques et
       licences acquises auprès de lui */
    #[serde(default)]
    merchant_done: Vec<u64>,
    #[serde(default)]
    charms: u32,
    /* le puits : offrandes consenties, et la nuit de la dernière */
    #[serde(default)]
    sacrifices: u32,
    #[serde(default)]
    well_night: f64,
    #[serde(default)]
    licences: u32,
    #[serde(default)]
    museum: Vec<Option<MusE>>,
    #[serde(default)]
    museum_at: f64,
    #[serde(default)]
    museum_pool: f64,
    #[serde(default)]
    pens: Vec<Option<Pen>>,
    #[serde(default)]
    legends_tried: Vec<u64>,
    #[serde(default)]
    hunts_done: u64,
    #[serde(default)]
    contracts_delivered: u64,
    #[serde(default)]
    legends_caught: u64,
    /* horodatage des battues récentes : chacune attire les légendes une heure
       durant, et l'effet se cumule */
    #[serde(default)]
    hunts_at: Vec<f64>,
    /* fenêtres où une légende s'est montrée : une fois parue, elle reste
       jusqu'au bout même si la pression des battues retombe */
    #[serde(default)]
    legends_open: Vec<u64>,
    #[serde(default)]
    pen_born: u64,
    #[serde(default)]
    last_day: u32,
    #[serde(default)]
    streak: u32,
    #[serde(default)]
    traces_done: Vec<u64>,
    /* dernière version dont les nouveautés ont été lues */
    #[serde(default)]
    news_seen: String,
    lab: Vec<u32>,
    autosell: Vec<bool>,
    /* combien de couples ♂♀ la vente épargne par espèce : l'enclos a besoin
       de stock, et un seul couple ne permet qu'une reproduction */
    #[serde(default = "un_couple")]
    autokeep: u32,
    /* le musée se garnit tout seul avec les spécimens les plus rentables */
    #[serde(default)]
    museum_auto: bool,
    ach: Vec<bool>,
    last_seen: f64,
}
impl Default for State {
    fn default() -> Self {
        let mut biomes = vec![None; BIOMES.len()];
        biomes[0] = Some(BiomeState { slots: 2, pl: vec![None, None], hunt_at: 0.0 });
        let mut traps = vec![0; 6];
        traps[0] = 1;
        State {
            ecus: 30.0,
            total_earned: 0.0,
            run_earned: 0.0,
            captures: 0,
            shinies: 0,
            attempts: 0,
            trophies: 0,
            migrations: 0,
            traps,
            baits: vec![0; 5],
            biomes,
            inv2: vec![InvE::default(); CREATURES.len()],
            dex2: vec![DexE::default(); CREATURES.len()],
            contracts_window: 0,
            contracts_done: vec![false; 3],
            contracts: vec![],
            trades_window: 0,
            trades: vec![],
            trades_done: vec![],
            trades_made: 0,
            merchant_done: vec![],
            charms: 0,
            sacrifices: 0,
            well_night: -1.0,
            licences: 0,
            museum: vec![None; 12],
            museum_at: 0.0,
            museum_pool: 0.0,
            pens: vec![None; 6],
            legends_tried: vec![],
            hunts_done: 0,
            contracts_delivered: 0,
            legends_caught: 0,
            hunts_at: vec![],
            legends_open: vec![],
            pen_born: 0,
            last_day: 0,
            streak: 0,
            traces_done: vec![],
            news_seen: String::new(),
            lab: vec![0; LABS.len()],
            autosell: vec![false; 5],
            autokeep: 1,
            museum_auto: false,
            ach: vec![false; ACHS.len()],
            last_seen: now_ms(),
        }
    }
}
impl State {
    fn normalize(&mut self) {
        self.traps.resize(6, 0);
        self.baits.resize(5, 0);
        if self.biomes.len() < BIOMES.len() {
            self.biomes.resize(BIOMES.len(), None);
        }
        self.lab.resize(LABS.len(), 0);
        self.autosell.resize(5, false);
        self.autokeep = self.autokeep.clamp(1, 4);
        self.ach.resize(ACHS.len(), false);
        self.inv2.resize(CREATURES.len(), InvE::default());
        self.dex2.resize(CREATURES.len(), DexE::default());
        self.contracts_done.resize(3, false);
        self.museum.resize(12, None);
        self.pens.resize(6, None);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64
}
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}
#[cfg(not(target_arch = "wasm32"))]
fn clock_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
#[cfg(target_arch = "wasm32")]
fn clock_hms() -> String {
    let d = js_sys::Date::new_0();
    format!("{:02}:{:02}:{:02}", d.get_hours(), d.get_minutes(), d.get_seconds())
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}
/* ── l'horloge du village ────────────────────────────────────────────────
   une journée au village dure deux heures réelles : une journée de travail en
   couvre quatre, et la nuit ne dépend plus du fuseau horaire du joueur. seuls
   les cycles d'ambiance la suivent — jour, nuit, saisons, lune. tout ce qui
   produit (pièges, contrats, troc, légendes, hors-ligne) reste au temps réel,
   sinon l'économie entière serait multipliée par douze. */
const JOUR_JEU_MS: f64 = 2.0 * 3_600_000.0;
/* l'heure qu'il est au village, de 0 à 24 */
fn heure_jeu(ms: f64) -> f64 {
    (ms.rem_euclid(JOUR_JEU_MS)) / JOUR_JEU_MS * 24.0
}
fn is_night_at(ms: f64) -> bool {
    let h = heure_jeu(ms);
    h >= 21.0 || h < 7.0
}
/* la nuit en cours, comptée de 7 h à 7 h : la pleine lune ne change pas de
   numéro à minuit pile */
fn nuit_index(ms: f64) -> f64 {
    ((ms - 7.0 / 24.0 * JOUR_JEU_MS) / JOUR_JEU_MS).floor()
}
fn pleine_lune(ms: f64) -> bool {
    (nuit_index(ms) as i64).rem_euclid(8) == 3
}
/* le puits a soif des deux premières heures du jour — dix minutes réelles,
   douze fois par jour — et toute la nuit de pleine lune */
fn puits_assoiffe(ms: f64) -> bool {
    heure_jeu(ms) < 2.0 || (pleine_lune(ms) && is_night_at(ms))
}
/* le jour réel : ce qui limite les offrandes, pour que douze nuits par jour ne
   multiplient pas d'autant ce que le puits rend */
fn jour_reel(ms: f64) -> f64 {
    (ms / 86_400_000.0).floor()
}

fn season_at(ms: f64) -> usize {
    ((ms / JOUR_JEU_MS) as u64 % 4) as usize
}
/* météo déterministe par créneau de 20 min — la simulation hors-ligne la rejoue à l'identique */
fn weather_at(ms: f64) -> usize {
    let w = (ms / 1_200_000.0) as u64;
    if is_night_at(ms) && splitmix(w ^ 0xABCD) % 100 < 30 {
        return 5; // nuit étoilée
    }
    match splitmix(w) % 100 {
        0..=34 => 0,  // ciel clair
        35..=54 => 1, // pluie
        55..=69 => 2, // brume
        70..=84 => 3, // canicule
        _ => 4,       // tempête
    }
}
fn weather_luck(w: usize, biome: usize) -> f64 {
    match (w, biome) {
        (1, 3) => 0.3,  // pluie au désert : aubaine
        (2, _) => 0.25, // brume : partout
        (4, 2) => 0.8,  // tempête en montagne
        (3, 3) => 0.5,  // canicule au désert
        (5, _) => 0.1,  // nuit étoilée
        _ => 0.0,
    }
}
fn weather_itv_mult(w: usize, biome: usize) -> f64 {
    match (w, biome) {
        (1, 0) | (1, 1) => 0.8, // pluie : forêt et marais plus actifs
        (3, 0) => 1.15,         // canicule : forêt endormie
        (3, 3) => 0.85,         // canicule : désert grouillant
        _ => 1.0,
    }
}
fn weather_succ_mod(w: usize, biome: usize) -> f64 {
    if w == 4 && biome == 2 { -0.15 } else { 0.0 }
}
fn weather_shiny_mult(w: usize) -> f64 {
    if w == 5 { 2.0 } else { 1.0 }
}
fn season_luck(sea: usize, biome: usize) -> f64 {
    match (sea, biome) {
        (0, 0) | (0, 1) => 0.15, // printemps : forêt, marais
        (1, 2) | (1, 3) => 0.15, // été : montagne, désert
        (3, 4) | (3, 5) => 0.15, // hiver : glacier, abysses
        _ => 0.0,
    }
}
fn season_desc(sea: usize) -> &'static str {
    match sea {
        0 => "forêt et marais +0,15 chance",
        1 => "montagne et désert +0,15 chance",
        2 => "25% des appâts épargnés",
        _ => "glacier et abysses +0,15 chance",
    }
}
fn weather_desc(w: usize) -> &'static str {
    match w {
        1 => "forêt/marais +25% vitesse · désert +0,3 chance",
        2 => "+0,25 chance partout",
        3 => "désert +0,5 chance et +15% vitesse · forêt ralentie",
        4 => "montagne : réussite −15% mais chance +0,8",
        5 => "shiny ×2 · +0,1 chance",
        _ => "aucun effet",
    }
}

fn save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    // chaque clé invitée porte AFFUT_PLAYER=<pseudo> (authorized_keys) : un monde par joueur
    let player: Option<String> = std::env::var("AFFUT_PLAYER")
        .ok()
        .map(|p| p.chars().filter(|c| c.is_ascii_alphanumeric()).take(24).collect::<String>())
        .filter(|p| !p.is_empty());
    match player {
        Some(p) => std::path::Path::new(&home).join(format!(".affutsh2-{}.json", p)),
        None => std::path::Path::new(&home).join(".affutsh2.json"),
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn save_raw(json: &str) {
    let _ = std::fs::write(save_path(), json);
}
#[cfg(not(target_arch = "wasm32"))]
fn load_raw() -> Option<String> {
    std::fs::read_to_string(save_path()).ok()
}
#[cfg(target_arch = "wasm32")]
fn web_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
#[cfg(target_arch = "wasm32")]
fn save_raw(json: &str) {
    if let Some(st) = web_storage() {
        let _ = st.set_item("affutsh_save2", json);
    }
}
#[cfg(target_arch = "wasm32")]
fn load_raw() -> Option<String> {
    web_storage()?.get_item("affutsh_save2").ok().flatten()
}

/* ================================================================ couleurs */

#[derive(Clone, Copy, PartialEq)]
enum C {
    Text, Dim, Dimmer, Blue, Gold, GoldDark, Green, Red, White, Purple, Marsh, Ice, Abyss, Shiny, Sel,
}
fn rarity_color(r: usize) -> C {
    match r {
        0 => C::Text,
        1 => C::Green,
        2 => C::Blue,
        3 => C::Purple,
        _ => C::Gold,
    }
}

struct Theme { truecolor: bool }
impl Theme {
    fn detect() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let ct = std::env::var("COLORTERM").unwrap_or_default();
            Theme { truecolor: ct.contains("truecolor") || ct.contains("24bit") }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Theme { truecolor: true }
        }
    }
    fn style(&self, c: C, panel_bg: bool) -> Style {
        let fg = if self.truecolor {
            match c {
                C::Text => Color::Rgb(194, 205, 220),
                C::Dim => Color::Rgb(126, 138, 155),
                C::Dimmer => Color::Rgb(84, 95, 110),
                C::Blue => Color::Rgb(122, 201, 255),
                C::Gold => Color::Rgb(255, 196, 92),
                C::GoldDark => Color::Rgb(214, 160, 75),
                C::Green => Color::Rgb(131, 214, 145),
                C::Red => Color::Rgb(255, 133, 133),
                C::White => Color::Rgb(248, 251, 255),
                C::Purple => Color::Rgb(199, 146, 234),
                C::Marsh => Color::Rgb(95, 179, 155),
                C::Ice => Color::Rgb(191, 224, 255),
                C::Abyss => Color::Rgb(122, 138, 176),
                C::Shiny => {
                    if (now_ms() as u64 / 700) % 2 == 0 { Color::Rgb(122, 232, 255) } else { Color::Rgb(248, 251, 255) }
                }
                C::Sel => Color::Rgb(26, 30, 38),
            }
        } else {
            Color::Indexed(match c {
                C::Text => 252,
                C::Dim => 246,
                C::Dimmer => 241,
                C::Blue => 111,
                C::Gold => 214,
                C::GoldDark => 172,
                C::Green => 114,
                C::Red => 210,
                C::White => 231,
                C::Purple => 176,
                C::Marsh => 72,
                C::Ice => 153,
                C::Abyss => 103,
                C::Shiny => if (now_ms() as u64 / 700) % 2 == 0 { 87 } else { 231 },
                C::Sel => 16,
            })
        };
        let mut st = Style::new().fg(fg);
        if c == C::Sel {
            st = st.bg(if self.truecolor { Color::Rgb(255, 196, 92) } else { Color::Indexed(214) });
        } else if panel_bg {
            st = st.bg(if self.truecolor { Color::Rgb(30, 35, 44) } else { Color::Indexed(235) });
        }
        st
    }
}

/* =================================================================== monde */

const MAPW: usize = 114;
const MAPH: usize = 80;

#[derive(Clone, Copy)]
struct Cell { ch: char, c: C, solid: bool }

struct WorldMap {
    cells: Vec<Vec<Cell>>,
    doors: Vec<(usize, usize, Zone)>,
    /* la rivière serpente : on retient ses cases plutôt qu'un rectangle */
    river: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Zone {
    Biome(usize),
    Boutique,
    Labo,
    Bestiaire,
    Succes,
    Musee,
    Enclos,
    Troc,
}

const ZONE_RECTS: [(usize, usize, usize, usize, usize); 10] = [
    (1, 16, 30, 22, 0),   // forêt — le versant boisé, sous le glacier
    (1, 40, 30, 18, 1),   // marais — les basses terres où l'eau stagne
    (34, 1, 38, 15, 2),   // montagne — la crête, au nord
    (78, 1, 34, 17, 3),   // désert — l'arrière-pays sec du nord-est
    (1, 1, 30, 13, 4),    // glacier — les hauteurs gelées
    (80, 50, 32, 28, 5),  // abysses — le large, au-delà du récif
    (34, 58, 38, 20, 6),  // volcan — le sud fumant
    (80, 22, 32, 25, 7),  // récif — la côte, là où la rivière se jette
    (1, 60, 30, 18, 8),   // ruines — l'ouest oublié
    (39, 17, 28, 11, 10), // lac (rive comprise : on pêche depuis le bord) — au pied de la montagne, source de la rivière
];
const LABEL_POS: [(usize, usize); 11] = [(11, 17), (11, 41), (45, 2), (89, 2), (10, 2), (91, 51), (45, 59), (91, 23), (10, 61), (38, 57), (47, 19)];

impl WorldMap {
    fn put(&mut self, x: usize, y: usize, ch: char, c: C, solid: bool) {
        if x < MAPW && y < MAPH {
            self.cells[y][x] = Cell { ch, c, solid };
        }
    }
    fn text(&mut self, x: usize, y: usize, s: &str, c: C, solid: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i, y, ch, c, solid);
        }
    }
    fn scatter(&mut self, x: usize, y: usize, w: usize, h: usize, glyphs: &[char], c: C, density: f64, solid: bool, rng: &mut StdRng) {
        for yy in y..(y + h).min(MAPH) {
            for xx in x..(x + w).min(MAPW) {
                if self.cells[yy][xx].ch == ' ' && rng.gen::<f64>() < density {
                    let g = glyphs[rng.gen_range(0..glyphs.len())];
                    self.put(xx, yy, g, c, solid);
                }
            }
        }
    }
    fn building(&mut self, x: usize, y: usize, w: usize, h: usize, label: &str, zone: Zone, glyph: char) {
        for xx in x..x + w {
            self.put(xx, y, '═', C::Dim, true);
            self.put(xx, y + h - 1, '═', C::Dim, true);
        }
        for yy in y..y + h {
            self.put(x, yy, '║', C::Dim, true);
            self.put(x + w - 1, yy, '║', C::Dim, true);
        }
        self.put(x, y, '╔', C::Dim, true);
        self.put(x + w - 1, y, '╗', C::Dim, true);
        self.put(x, y + h - 1, '╚', C::Dim, true);
        self.put(x + w - 1, y + h - 1, '╝', C::Dim, true);
        for yy in y + 1..y + h - 1 {
            for xx in x + 1..x + w - 1 {
                self.put(xx, yy, '▒', C::Dimmer, true);
            }
        }
        let lx = x + (w - label.chars().count()) / 2;
        self.text(lx, y + h / 2, label, C::Gold, true);
        let dx = x + w / 2;
        self.text(dx - 2, y + h - 1, &format!("╡ {} ╞", glyph), C::Gold, false);
        self.doors.push((dx, y + h - 1, zone));
        self.doors.push((dx, y + h, zone));
    }

    fn build() -> Self {
        let mut w = WorldMap {
            cells: vec![vec![Cell { ch: ' ', c: C::Dimmer, solid: false }; MAPW]; MAPH],
            doors: vec![],
            river: vec![],
        };
        let mut rng = StdRng::seed_from_u64(1337);

        /* ── les terres ──────────────────────────────────────────────────
           l'altitude est au nord (glacier, montagne), l'eau descend vers le
           sud-est : lac au pied de la montagne, rivière, puis la mer — récif
           le long de la côte, abysses au large. */
        /* chaque biome se peuple en deux passes : une poignée d'éléments qui
           barrent vraiment la route, puis un décor abondant mais traversable.
           on garde l'ambiance sans transformer la marche en labyrinthe. */
        w.scatter(1, 1, 30, 13, &['▲'], C::Ice, 0.03, true, &mut rng);              // glacier
        w.scatter(1, 1, 30, 13, &['*', '·', '*'], C::Ice, 0.11, false, &mut rng);
        w.scatter(34, 1, 38, 15, &['▲', '∆'], C::Dim, 0.04, true, &mut rng);        // montagne
        w.scatter(34, 1, 38, 15, &['^', '/', '·', '^'], C::Dim, 0.11, false, &mut rng);
        w.scatter(78, 1, 34, 17, &['∙', '≈', '·'], C::GoldDark, 0.12, false, &mut rng); // désert
        w.scatter(1, 16, 30, 22, &['♣', '♠'], C::Green, 0.05, true, &mut rng);      // forêt
        w.scatter(1, 16, 30, 22, &['♣', '.', '♠', '.'], C::Green, 0.13, false, &mut rng);
        w.scatter(1, 40, 30, 18, &['~', '~', 'o', '"'], C::Marsh, 0.14, false, &mut rng); // marais
        w.scatter(1, 60, 30, 18, &['#'], C::Dim, 0.03, true, &mut rng);             // ruines
        w.scatter(1, 60, 30, 18, &['[', ']', '.', ','], C::Dim, 0.10, false, &mut rng);
        w.scatter(34, 58, 38, 20, &['^'], C::Red, 0.03, true, &mut rng);            // volcan
        w.scatter(34, 58, 38, 20, &['∴', '*', '·', '∴'], C::Red, 0.11, false, &mut rng);
        w.scatter(80, 22, 32, 25, &['~', '≈', 'o', ':'], C::Blue, 0.14, false, &mut rng); // récif
        w.scatter(80, 50, 32, 28, &['▓'], C::Abyss, 0.03, true, &mut rng);          // abysses
        w.scatter(80, 50, 32, 28, &['▒', '●', '·', '▒'], C::Abyss, 0.12, false, &mut rng);

        /* le lac : une ellipse au pied de la montagne — une nappe d'eau n'a
           pas de coins. les berges dessinées d'un trait plus clair. */
        {
            let (cx, cy, rx, ry) = (52.5_f64, 22.0_f64, 13.0_f64, 4.6_f64);
            for y in 17..28 {
                for x in 38..68 {
                    let (dx, dy) = ((x as f64 - cx) / rx, (y as f64 - cy) / ry);
                    let d = dx * dx + dy * dy;
                    if d > 1.0 {
                        continue;
                    }
                    let g = if d > 0.72 { '~' } else if rng.gen::<f64>() < 0.45 { '≈' } else { '~' };
                    w.put(x, y, g, C::Blue, true); // on pêche depuis la berge
                }
            }
        }

        /* ── la rivière ──────────────────────────────────────────────────
           née de la montagne, elle remplit le lac, en ressort, contourne le
           village par l'ouest, longe le marais et se jette dans la mer. */
        {
            const SOURCE: [(i32, i32); 2] = [(48, 13), (48, 18)];
            const COURS: [(i32, i32); 8] =
                [(46, 27), (40, 32), (33, 38), (32, 46), (34, 54), (50, 56), (68, 54), (80, 48)];
            let mut trace: Vec<(usize, usize)> = vec![];
            let mut trace_seg = |pts: &[(i32, i32)], out: &mut Vec<(usize, usize)>| {
                for pair in pts.windows(2) {
                    let ((x1, y1), (x2, y2)) = (pair[0], pair[1]);
                    let (mut x, mut y) = (x1, y1);
                    while x != x2 || y != y2 {
                        out.push((x as usize, y as usize));
                        // avance en diagonale douce : le cours serpente
                        if x != x2 && (y == y2 || (x - x2).abs() >= (y - y2).abs()) {
                            x += (x2 - x).signum();
                        } else {
                            y += (y2 - y).signum();
                        }
                    }
                    out.push((x2 as usize, y2 as usize));
                }
            };
            trace_seg(&SOURCE, &mut trace);
            trace_seg(&COURS, &mut trace);
            for &(x, y) in &trace {
                for dx in 0..2 {
                    if x + dx < MAPW && y < MAPH {
                        w.put(x + dx, y, '~', C::Blue, true);
                    }
                }
            }
            w.river = trace.iter().flat_map(|&(x, y)| [(x, y), (x + 1, y)]).collect();
        }

        /* ── le village ──────────────────────────────────────────────────
           une trame de rues pavées plutôt qu'un semis au hasard : deux axes
           est-ouest, trois nord-sud, une place au milieu. */
        for y in 30..=52 {
            for x in 34..=76 {
                if w.cells[y][x].ch == ' ' && rng.gen::<f64>() < 0.28 {
                    w.put(x, y, '·', C::Dimmer, false);
                }
            }
        }
        let mut rue_h = |w: &mut WorldMap, y: usize| {
            for x in 34..=76 {
                w.put(x, y, '░', C::Dimmer, false);
            }
        };
        rue_h(&mut w, 37);
        rue_h(&mut w, 45);
        for &x in &[38, 57, 72] {
            for y in 30..=52 {
                w.put(x, y, '░', C::Dimmer, false);
            }
        }
        // la place : entre la première et la deuxième rangée, jamais sur un mur
        for y in 37..=39 {
            for x in 46..=64 {
                w.put(x, y, '░', C::Dimmer, false);
            }
        }
        /* routes : elles relient le village à chaque biome et rouvrent le
           passage partout où elles rencontrent un obstacle */
        let mut path = |w: &mut WorldMap, x1: usize, y1: usize, x2: usize, y2: usize| {
            let (mut x, mut y) = (x1 as i32, y1 as i32);
            // une route se voit : elle efface le décor qu'elle traverse
            while x != x2 as i32 {
                w.put(x as usize, y as usize, '░', C::Dimmer, false);
                x += (x2 as i32 - x).signum();
            }
            while y != y2 as i32 {
                w.put(x as usize, y as usize, '░', C::Dimmer, false);
                y += (y2 as i32 - y).signum();
            }
        };
        path(&mut w, 34, 37, 18, 30);   // village -> forêt (par la rue principale)
        path(&mut w, 34, 45, 18, 48);   // village -> marais
        path(&mut w, 16, 16, 16, 10);   // forêt -> glacier
        path(&mut w, 16, 58, 16, 64);   // marais -> ruines
        path(&mut w, 52, 30, 52, 27);   // village -> rive sud du lac
        path(&mut w, 39, 30, 39, 17);   // longe la rive ouest du lac
        path(&mut w, 39, 16, 52, 12);   // -> montagne
        path(&mut w, 76, 34, 92, 14);   // village -> désert
        path(&mut w, 76, 37, 92, 32);   // village -> récif
        path(&mut w, 92, 47, 92, 54);   // récif -> abysses
        path(&mut w, 46, 52, 46, 62);   // village -> volcan
        path(&mut w, 64, 52, 64, 62);

        /* ponts : les routes sont tracées après la rivière et rouvrent le
           passage là où elles la franchissent — on y pose un tablier */
        {
            let cases: Vec<(usize, usize)> = w.river.clone();
            for (x, y) in cases {
                if x < MAPW && y < MAPH && !w.cells[y][x].solid {
                    w.put(x, y, '═', C::Dim, false);
                }
            }
        }

        // bâtiments : trois rangées le long des deux axes
        w.building(36, 31, 16, 6, "boutique", Zone::Boutique, 'o');
        w.building(58, 31, 16, 6, "labo", Zone::Labo, 'l');
        w.building(36, 40, 11, 5, "bestiaire", Zone::Bestiaire, 'b');
        w.building(49, 40, 13, 5, "troc", Zone::Troc, 'r');
        w.building(64, 40, 12, 5, "trophées", Zone::Succes, 't');
        w.building(40, 47, 13, 5, "musée", Zone::Musee, 'm');
        w.building(58, 47, 13, 5, "enclos", Zone::Enclos, 'e');

        /* la fontaine et le mobilier occupent le bas de la place : la rangée
           des portes (y=37) reste entièrement dégagée, si bien qu'on va d'un
           bâtiment à l'autre en ligne droite */
        w.text(53, 38, "╭─╮", C::Blue, true);
        w.text(53, 39, "╰─╯", C::Blue, true);
        /* le mobilier se tient au-dessus des façades, jamais dans un passage :
           les interstices entre bâtiments (x=47-48 et x=62-63) restent libres */
        for &(bx, by) in &[(50, 39), (58, 39)] {
            w.put(bx, by, '▬', C::Dim, false); // décoratif : un banc ne barre pas la route
        }
        for &(lx, ly) in &[(50, 38), (58, 38)] {
            w.put(lx, ly, '╽', C::GoldDark, false); // décoratif : on passe devant
        }

        w
    }

    fn solid(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= MAPW as i32 || y >= MAPH as i32 {
            return true;
        }
        self.cells[y as usize][x as usize].solid
    }

    fn zone_at(&self, x: usize, y: usize) -> Option<Zone> {
        for &(dx, dy, z) in &self.doors {
            if (dx as i32 - x as i32).abs() <= 1 && (dy as i32 - y as i32).abs() <= 1 {
                return Some(z);
            }
        }
        /* depuis la berge : une case au bord de l'eau donne accès au biome,
           sinon la rivière serait inatteignable une fois rendue infranchissable */
        if self.river.iter().any(|&(rx, ry)| {
            (rx as i32 - x as i32).abs() <= 1 && (ry as i32 - y as i32).abs() <= 1
        }) {
            return Some(Zone::Biome(9));
        }
        for &(zx, zy, zw, zh, b) in &ZONE_RECTS {
            if x >= zx && x < zx + zw && y >= zy && y < zy + zh {
                return Some(Zone::Biome(b));
            }
        }
        None
    }
}

/* ================================================================ panneaux */

#[derive(Clone)]
enum SellQty { One, Keep1, All }

#[derive(Clone)]
enum Action {
    Open(PanelKind),
    Close,
    CloseAll,
    BuyTrap(usize),
    SellTrap(usize),
    BuyBait(usize, u64),
    BuyLab(usize),
    Unlock(usize),
    BuySlot(usize),
    Place(usize, usize, usize),
    SetBait(usize, usize, Option<usize>),
    SetAutokeep(u32),
    Remove(usize, usize),
    Sell(usize, bool, SellQty),
    SellDupes,
    ToggleAutosell(usize),
    Migrate,
    DoReset,
    Hunt(usize),
    Deliver(usize),
    MuseumAdd(usize, usize, bool),
    MuseumRemove(usize),
    MuseumCollect,
    MuseumAuto,
    ToggleMuseumAuto,
    PenStart(usize, usize, Option<usize>),
    PenCollect(usize),
    Trade(usize),
    MerchBuy(usize),
    Sacrifice(usize),
    LegendTry(usize, u64, Option<usize>),
    TraceFollow(u64, usize),
    Nothing,
}

#[derive(Clone)]
struct Contract {
    ci: usize,
    qty: u64,
    reward: f64,
}

#[derive(Clone)]
struct OfflineSummary {
    h: u64,
    m: u64,
    hit_cap: bool,
    caught: u64,
    shinies: u64,
    earned: f64,
    discoveries: Vec<usize>,
}

#[derive(Clone)]
enum PanelKind {
    Dashboard,
    Inventory,
    Contracts,
    Museum,
    MuseumPick(usize),
    Pens,
    PenPick(usize),
    Legend(usize, u64),
    Biome(usize),
    TrapPick(usize, usize),
    BaitPick(usize, usize),
    Unlock(usize),
    Shop,
    Lab,
    MigrConfirm,
    Dex,
    Creature(usize),
    Achs,
    Help,
    Journal,
    Board,
    News,
    Trade,
    Merchant,
    Puits,
    Offline(OfflineSummary),
    ResetConfirm,
}

struct Panel {
    kind: PanelKind,
    sel: usize,
    scroll: usize,
    inner: usize,
}
impl Panel {
    fn new(kind: PanelKind) -> Panel {
        Panel { kind, sel: 0, scroll: 0, inner: 20 }
    }
}

struct Row {
    segs: Vec<(String, C)>,
    btns: Vec<(String, C, Action)>,
    act: Option<Action>,
    indent: u16,
}
impl Row {
    fn text(s: impl Into<String>, c: C) -> Row {
        Row { segs: vec![(s.into(), c)], btns: vec![], act: None, indent: 0 }
    }
    fn header(s: &str) -> Row {
        Row {
            segs: vec![(format!("── {} ", s), C::Blue), ("─".repeat(40), C::Dimmer)],
            btns: vec![],
            act: None,
            indent: 0,
        }
    }
}
fn wrap_lines(s: &str, w: usize) -> Vec<String> {
    let w = w.max(20);
    let mut lines = vec![];
    let mut cur = String::new();
    for word in s.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > w {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
fn wrap_rows(s: &str, w: usize, c: C) -> Vec<Row> {
    wrap_lines(s, w).into_iter().map(|l| Row::text(l, c)).collect()
}
/* replie toute ligne plus large que le panneau : texte coupé aux espaces,
   boutons regroupés sur des lignes suivantes, filets d'en-tête raccourcis */
fn reflow_rows(rows: Vec<Row>, w: usize) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::with_capacity(rows.len());
    for r in rows {
        let avail = w.saturating_sub(r.indent as usize).max(24);
        let seg_len: usize = r.segs.iter().map(|(t, _)| t.chars().count()).sum();
        let btn_len: usize = r.btns.iter().map(|(l, _, _)| l.chars().count() + 3).sum();
        if seg_len + btn_len <= avail {
            out.push(r);
            continue;
        }
        // en-tête à filet : on raccourcit les '─' au lieu de replier
        if r.btns.is_empty() {
            if let Some((last, lc)) = r.segs.last().cloned() {
                if !last.is_empty() && last.chars().all(|ch| ch == '─') {
                    let fixed = seg_len - last.chars().count();
                    if fixed < avail {
                        let mut segs = r.segs.clone();
                        segs.pop();
                        segs.push(("─".repeat(avail - fixed), lc));
                        out.push(Row { segs, btns: vec![], act: r.act.clone(), indent: r.indent });
                        continue;
                    }
                }
            }
        }
        // replier le texte en lignes ≤ avail, en préservant les couleurs
        let mut lines: Vec<Vec<(String, C)>> = vec![vec![]];
        let mut cur = 0usize;
        for (text, c) in &r.segs {
            let mut rest: String = text.clone();
            loop {
                let space = avail - cur;
                let n = rest.chars().count();
                if n <= space {
                    if n > 0 {
                        lines.last_mut().unwrap().push((rest.clone(), *c));
                        cur += n;
                    }
                    break;
                }
                let hard = rest.char_indices().nth(space).map(|(i, _)| i).unwrap_or(rest.len());
                let cut = match rest[..hard].rfind(' ') {
                    Some(p) if p > 0 => p + 1,
                    _ if cur > 0 => 0, // rien ne tient sur cette ligne entamée : à la ligne
                    _ => hard,
                };
                if cut > 0 {
                    lines.last_mut().unwrap().push((rest[..cut].trim_end().to_string(), *c));
                    rest = rest[cut..].trim_start().to_string();
                }
                lines.push(vec![]);
                cur = 0;
                if rest.is_empty() {
                    break;
                }
            }
        }
        // répartir les boutons : d'abord en fin de dernière ligne, puis lignes dédiées
        let mut btn_groups: Vec<Vec<(String, C, Action)>> = vec![vec![]];
        let mut bw = lines.last().map(|l| l.iter().map(|(t, _)| t.chars().count()).sum::<usize>()).unwrap_or(0);
        for b in r.btns {
            let need = b.0.chars().count() + 3;
            if bw + need > avail && bw > 0 {
                btn_groups.push(vec![]);
                bw = 0;
            }
            btn_groups.last_mut().unwrap().push(b);
            bw += need;
        }
        let n_lines = lines.len();
        for (i, line) in lines.into_iter().enumerate() {
            let is_last = i + 1 == n_lines;
            out.push(Row {
                segs: line,
                btns: if is_last { btn_groups[0].clone() } else { vec![] },
                act: if i == 0 { r.act.clone() } else { None },
                indent: if i == 0 { r.indent } else { r.indent + 2 },
            });
        }
        for g in btn_groups.into_iter().skip(1) {
            if !g.is_empty() {
                out.push(Row { segs: vec![], btns: g, act: None, indent: r.indent + 2 });
            }
        }
    }
    out
}

/* liste à puce/numéro avec retrait suspendu, repliée à la largeur du panneau */
fn bullet_rows(prefix: &str, s: &str, w: usize, c: C) -> Vec<Row> {
    let pl = prefix.chars().count();
    wrap_lines(s, w.saturating_sub(pl))
        .into_iter()
        .enumerate()
        .map(|(i, l)| Row::text(format!("{}{}", if i == 0 { prefix.to_string() } else { " ".repeat(pl) }, l), c))
        .collect()
}
fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.to_string() } else { format!("{}{}", s, " ".repeat(w - n)) }
}

/* ==================================================================== jeu */

struct LogLine {
    t: String,
    segs: Vec<(String, C)>,
    /* les prises tombent en continu : elles ont leur colonne, pour ne pas
       chasser du journal une éclosion ou une légende */
    prise: bool,
}

/* une ligne du classement, telle que la sert /lb (version navigateur) */
#[derive(Deserialize, Clone, Default)]
struct BoardRow {
    #[serde(default)]
    pseudo: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    especes: f64,
    #[serde(default)]
    captures: f64,
    #[serde(default)]
    shinies: f64,
    #[serde(default)]
    ecus: f64,
    #[serde(default)]
    rangs: f64,
    #[serde(default)]
    trophees: f64,
    #[serde(default)]
    curiosites: f64,
    #[serde(default)]
    legendes: f64,
}

struct Game {
    s: State,
    world: WorldMap,
    px: i32,
    py: i32,
    panels: Vec<Panel>,
    logs: VecDeque<LogLine>,
    toasts: Vec<(String, f64)>,
    quit: bool,
    panel_w: usize,
    legend_seen: u64,
    /* classement : alimenté par le navigateur, jamais sauvegardé */
    board: Vec<BoardRow>,
    board_me: String,
    board_state: u8, // 0 = pas encore reçu, 1 = à jour, 2 = injoignable
    /* dernière venue du marchand déjà annoncée */
    merchant_seen: u64,
    /* naissances déjà signalées, pour ne pas répéter l'annonce à chaque tick */
    pens_seen: Vec<f64>,
    /* clin d'œil : le cercle qui s'ouvre sur la place à la 666e prise */
    pentacle_until: f64,
    /* dernier passage du garnissage automatique du musée */
    museum_auto_at: f64,
}

impl Game {
    fn new() -> (Game, bool) {
        // le repli vers l'ancien nom de fichier ne vaut que pour le propriétaire :
        // un joueur invité (AFFUT_PLAYER) démarre toujours son propre monde
        let raw = load_raw();
        let (mut s, fresh) = match raw {
            Some(raw) => (serde_json::from_str::<State>(&raw).unwrap_or_default(), false),
            None => (State::default(), true),
        };
        s.normalize();
        let game = Game {
            s,
            world: WorldMap::build(),
            px: 60,
            py: 38,
            panels: vec![],
            logs: VecDeque::new(),
            toasts: vec![],
            quit: false,
            panel_w: 74,
            legend_seen: 0,
            board: Vec::new(),
            board_me: String::new(),
            board_state: 0,
            merchant_seen: 0,
            pens_seen: vec![],
            pentacle_until: 0.0,
            museum_auto_at: 0.0,
        };
        (game, fresh)
    }

    fn log(&mut self, segs: Vec<(String, C)>) {
        self.log_ligne(segs, false);
    }
    /* le flot des captures : même colonne, même mémoire, autre file */
    fn log_prise(&mut self, segs: Vec<(String, C)>) {
        self.log_ligne(segs, true);
    }
    fn log_ligne(&mut self, segs: Vec<(String, C)>, prise: bool) {
        let t = clock_hms();
        self.logs.push_front(LogLine { t, segs, prise });
        self.logs.truncate(120);
    }
    fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), now_ms()));
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }
    fn save(&mut self) {
        self.s.last_seen = now_ms();
        if let Ok(json) = serde_json::to_string(&self.s) {
            save_raw(&json);
        }
    }

    /* ------------------------------------------------------------- bonus */

    fn completed_biomes(&self) -> usize {
        (0..WILDB).filter(|&b| biome_creatures(b).all(|i| self.s.dex2[i].n > 0)).count()
    }
    fn shiny_completed_biomes(&self) -> usize {
        (0..WILDB).filter(|&b| biome_creatures(b).all(|i| self.s.dex2[i].s > 0)).count()
    }
    fn global_luck(&self) -> f64 {
        self.s.lab[LAB_FLAIR] as f64 * 0.08
            + self.s.trophies as f64 * 0.008
            + self.completed_biomes() as f64 * 0.04
            + self.s.charms as f64 * 0.10
            + if fair_day() { 0.15 } else { 0.0 }
            + self.streak_bonus()
    }
    /* l'élan : bonus de chance qui grandit avec les jours de chasse consécutifs */
    fn streak_bonus(&self) -> f64 {
        (self.s.streak.min(10)) as f64 * 0.015
    }    fn sell_mult(&self) -> f64 {
        (1.0 + self.s.lab[LAB_NEGOCE] as f64 * 0.05)
            * (1.0 + self.s.trophies as f64 * 0.01)
            * (1.0 + self.shiny_completed_biomes() as f64 * 0.05)
    }    fn speed_mult(&self) -> f64 {
        1.0 + self.s.lab[LAB_AFFUTAGE] as f64 * 0.06
    }    fn shiny_chance(&self, bait: Option<usize>, at: f64) -> f64 {
        let mut c = SHINY_BASE * (1.0 + self.s.lab[LAB_ECLAT] as f64 * 0.10) * weather_shiny_mult(weather_at(at));
        if bait == Some(BAIT_ESSENCE) {
            c *= 3.0;
        }
        c.min(1.0 / 128.0)
    }
    fn offline_cap_ms(&self) -> f64 {
        (2.0 + self.s.lab[LAB_HORLOGE] as f64 * 2.0) * 3600.0 * 1000.0
    }
    fn trap_interval(&self, tier: usize, bait: Option<usize>, biome: usize, at: f64) -> f64 {
        let mut itv = TRAPS[tier].itv / self.speed_mult();
        if bait == Some(BAIT_BAIES) {
            itv *= 0.8;
        }
        itv *= weather_itv_mult(weather_at(at), biome);
        itv * 1000.0
    }    fn creature_value(&self, ci: usize, shiny: bool) -> f64 {
        let c = &CREATURES[ci];
        let mut v = RAR_VAL[c.r] * BIOMES[c.b].mult * self.sell_mult();
        if shiny {
            v *= 15.0;
        }
        v.floor().max(1.0)
    }
    fn creature_value_r(&self, ci: usize, shiny: bool, rank: usize) -> f64 {
        (self.creature_value(ci, shiny) * RANK_MULT[rank]).floor().max(1.0)
    }
    fn biome_luck(&self, biome: usize, at: f64) -> f64 {
        self.global_luck() + weather_luck(weather_at(at), biome) + season_luck(season_at(at), biome)
    }
    fn roll_rank(&self, luck: f64) -> usize {
        let r = rand::thread_rng().gen::<f64>();
        let lk = luck.min(LUCK_CAP);
        let ps = (0.008 * (1.0 + lk * 0.15)).min(0.025);
        let pa = (0.06 * (1.0 + lk * 0.2)).min(0.12);
        if r < ps { 3 } else if r < ps + pa { 2 } else if r < ps + pa + 0.25 { 1 } else { 0 }
    }    fn take_lowest(&mut self, ci: usize, shiny: bool, mut q: u64) -> f64 {
        let mut v = 0.0;
        for r in 0..4 {
            let val = self.creature_value_r(ci, shiny, r);
            loop {
                if q == 0 {
                    return v;
                }
                let iv = &mut self.s.inv2[ci];
                let (a, b) = if shiny { (&mut iv.sm, &mut iv.sf) } else { (&mut iv.m, &mut iv.f) };
                let src = if a[r] >= b[r] { a } else { b };
                if src[r] == 0 {
                    break;
                }
                src[r] -= 1;
                q -= 1;
                v += val;
            }
        }
        v
    }
    /* retire le meilleur spécimen d'un sexe donné ; renvoie son rang */
    fn take_best_sex(&mut self, ci: usize, shiny: bool, sex: u8) -> Option<usize> {
        for r in (0..4).rev() {
            let iv = &mut self.s.inv2[ci];
            let bucket = match (shiny, sex) {
                (false, 0) => &mut iv.m,
                (false, _) => &mut iv.f,
                (true, 0) => &mut iv.sm,
                (true, _) => &mut iv.sf,
            };
            if bucket[r] > 0 {
                bucket[r] -= 1;
                return Some(r);
            }
        }
        None
    }
    /* remet un spécimen en réserve */
    fn give_back(&mut self, ci: usize, shiny: bool, sex: u8, rank: usize) {
        let iv = &mut self.s.inv2[ci];
        match (shiny, sex) {
            (false, 0) => iv.m[rank] += 1,
            (false, _) => iv.f[rank] += 1,
            (true, 0) => iv.sm[rank] += 1,
            (true, _) => iv.sf[rank] += 1,
        }
    }
    /* vend tout sauf le meilleur mâle et la meilleure femelle ; renvoie (qté, valeur) */
    fn sell_except_pair(&mut self, ci: usize) -> (u64, f64) {
        let bm = self.take_best_sex(ci, false, 0);
        let bf = self.take_best_sex(ci, false, 1);
        let q = self.s.inv2[ci].tn();
        let v = self.take_lowest(ci, false, q);
        if let Some(r) = bm {
            self.give_back(ci, false, 0, r);
        }
        if let Some(r) = bf {
            self.give_back(ci, false, 1, r);
        }
        (q, v)
    }
    /* retire le meilleur spécimen tous sexes confondus ; renvoie (rang, sexe) */
    /* retire le spécimen non-shiny de plus bas rang */
    fn take_lowest_one(&mut self, ci: usize) -> Option<(usize, u8)> {
        for r in 0..4 {
            let iv = &mut self.s.inv2[ci];
            if iv.m[r] > 0 {
                iv.m[r] -= 1;
                return Some((r, 0));
            }
            if iv.f[r] > 0 {
                iv.f[r] -= 1;
                return Some((r, 1));
            }
        }
        None
    }
    fn take_best(&mut self, ci: usize, shiny: bool) -> Option<(usize, u8)> {
        for r in (0..4).rev() {
            let iv = &mut self.s.inv2[ci];
            let (m, f) = if shiny { (&mut iv.sm, &mut iv.sf) } else { (&mut iv.m, &mut iv.f) };
            if m[r] > 0 || f[r] > 0 {
                if m[r] >= f[r] {
                    m[r] -= 1;
                    return Some((r, 0));
                } else {
                    f[r] -= 1;
                    return Some((r, 1));
                }
            }
        }
        None
    }
    /* enregistre un spécimen (capture, naissance, légende) : sexe tiré à pile ou face */
    fn add_specimen(&mut self, ci: usize, shiny: bool, rank: usize) -> (bool, bool, u8) {
        let sex: u8 = if rand::thread_rng().gen::<bool>() { 0 } else { 1 };
        let is_new = self.s.dex2[ci].n == 0;
        let new_shiny = shiny && self.s.dex2[ci].s == 0;
        self.s.dex2[ci].n += 1;
        if shiny {
            self.s.dex2[ci].s += 1;
            self.s.dex2[ci].bests = self.s.dex2[ci].bests.max(rank as u8 + 1);
        }
        self.s.dex2[ci].best = self.s.dex2[ci].best.max(rank as u8 + 1);
        self.s.dex2[ci].mf |= 1 << sex;
        self.give_back(ci, shiny, sex, rank);
        self.s.captures += 1;
        if shiny {
            self.s.shinies += 1;
        }
        (is_new, new_shiny, sex)
    }
    fn lab_cost(&self, k: usize) -> f64 {
        (LABS[k].base * LABS[k].mult.powi(self.s.lab[k] as i32)).floor()
    }
    /* un emplacement coûte ce que coûte le piège qu'on y mettra : poser un
       piège de plus et améliorer un piège d'un palier rapportent à peu près
       autant (+1/n du revenu contre ×2 sur un piège sur n), ils doivent donc
       coûter à peu près autant. adossé au prix du biome, le 4e emplacement des
       ruines valait deux fois le biome lui-même — 1500 h d'économies pour un
       seul piège, si bien qu'on ne l'achetait jamais. */
    fn slot_cost(&self, b: usize) -> f64 {
        let meilleur = (0..TRAPS.len()).rev().find(|&t| self.s.traps[t] > 0).unwrap_or(0);
        let base = TRAPS[meilleur].cost;
        if self.s.biomes[b].as_ref().map(|x| x.slots).unwrap_or(2) == 2 {
            base.floor()
        } else {
            (base * 2.5).floor()
        }
    }
    fn placed_count(&self, tier: usize) -> u32 {
        let mut n = 0;
        for bs in self.s.biomes.iter().flatten() {
            for pl in bs.pl.iter().flatten() {
                if pl.trap == tier {
                    n += 1;
                }
            }
        }
        n
    }
    fn trophy_gain(&self) -> u32 {
        (self.s.run_earned / 1_000_000.0).sqrt().floor() as u32
    }
    fn migration_cost(&self) -> f64 {
        100_000.0 * 2f64.powi(self.s.migrations.min(12) as i32)
    }
    fn trap_cap(&self) -> u32 {
        2 + self.s.lab[LAB_LICENCE] + self.s.licences
    }
    fn placed_total(&self) -> u32 {
        (0..6).map(|t| self.placed_count(t)).sum()
    }    fn gain(&mut self, n: f64) {
        self.s.ecus += n;
        self.s.total_earned += n;
        self.s.run_earned += n;
    }

    /* ----------------------------------------------------------- capture */

    fn roll_creature(&self, biome: usize, luck: f64, bait: Option<usize>, at: f64) -> usize {
        let night = is_night_at(at);
        let pool: Vec<usize> = biome_creatures(biome).collect();
        let weights: Vec<f64> = pool
            .iter()
            .map(|&i| {
                if NOCTURNES.contains(&i) && !night {
                    return 0.0;
                }
                let r = CREATURES[i].r;
                let lk = luck.min(LUCK_CAP);
                let mut w = RAR_W[r] * (1.0 + lk * RAR_LUCK_FACT[r]);
                if bait == Some(BAIT_TRUFFE) && r >= 2 {
                    w *= 1.8;
                }
                w
            })
            .collect();
        let total: f64 = weights.iter().sum();
        let mut r = rand::thread_rng().gen::<f64>() * total;
        for (k, &i) in pool.iter().enumerate() {
            r -= weights[k];
            if r <= 0.0 && weights[k] > 0.0 {
                return i;
            }
        }
        *pool.iter().rev().find(|&&i| !NOCTURNES.contains(&i)).unwrap()
    }    fn attempt(&mut self, biome: usize, slot: usize, at: f64, bonus_luck: f64, silent: bool) -> Option<(usize, bool, bool, bool, f64, usize)> {
        // renvoie (créature, shiny, nouveauté, nouveau shiny, auto-vente, rang)
        let pl = self.s.biomes[biome].as_ref()?.pl[slot].clone()?;
        let mut bait = None;
        if let Some(bt) = pl.bait {
            if self.s.baits[bt] > 0 {
                bait = Some(bt);
                // automne : 25% des appâts sont épargnés
                if !(season_at(at) == 2 && rand::thread_rng().gen::<f64>() < 0.25) {
                    self.s.baits[bt] -= 1;
                    if self.s.baits[bt] == 0 && !silent {
                        self.log(vec![(format!("appât épuisé ({}) · {}", BAITS[bt].n, BIOMES[biome].name), C::Dim)]);
                    }
                }
            }
        }
        self.s.attempts += 1;
        let succ = (TRAPS[pl.trap].succ + weather_succ_mod(weather_at(at), biome)).clamp(0.05, 0.99);
        if rand::thread_rng().gen::<f64>() > succ {
            return None;
        }
        let mut luck = TRAPS[pl.trap].luck + self.biome_luck(biome, at) + bonus_luck;
        if bait == Some(BAIT_VIANDE) {
            luck += 0.12;
        }
        if bait == Some(BAIT_NECTAR) {
            luck += 0.08;
        }
        if bait == Some(BAIT_ESSENCE) {
            luck += 0.1;
        }
        let ci = self.roll_creature(biome, luck, bait, at);
        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(bait, at);
        let rank = self.roll_rank(luck);
        let (is_new, new_shiny, sex) = self.add_specimen(ci, shiny, rank);

        let mut sold = 0.0;
        let r = CREATURES[ci].r;
        // l'auto-vente garde le meilleur couple ♂♀ ET ce que demandent les commandes
        // en cours (sinon les contrats deviendraient inlivrables) ; jamais les shinies
        if self.s.lab[LAB_AUTOVENTE] >= 1 && self.s.autosell[r] && !shiny {
            let keep = self.seuil_garde(ci);
            if self.s.inv2[ci].tn() > keep {
                let (_, v) = self.sell_surplus(ci);
                sold = (v * if bait == Some(BAIT_NECTAR) { 1.3 } else { 1.0 }).floor();
                self.gain(sold);
            }
        }
        Some((ci, shiny, is_new, new_shiny, sold, rank + sex as usize * 10))
    }    /* battue sans piège : une tentative à mains nues */
    fn bare_attempt(&mut self, biome: usize, at: f64) -> Option<(usize, bool, bool, bool, f64, usize)> {
        self.s.attempts += 1;
        let succ = (0.35 + weather_succ_mod(weather_at(at), biome)).clamp(0.05, 0.99);
        if rand::thread_rng().gen::<f64>() > succ {
            return None;
        }
        let luck = self.biome_luck(biome, at) + 0.2;
        let ci = self.roll_creature(biome, luck, None, at);
        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, at);
        let rank = self.roll_rank(luck);
        let (is_new, new_shiny, sex) = self.add_specimen(ci, shiny, rank);
        Some((ci, shiny, is_new, new_shiny, 0.0, rank + sex as usize * 10))
    }
    fn report_catch(&mut self, biome: usize, res: Option<(usize, bool, bool, bool, f64, usize)>) {
        let Some((ci, shiny, is_new, new_shiny, sold, rank_sex)) = res else { return };
        let (rank, sex) = (rank_sex % 10, rank_sex / 10);
        let c = &CREATURES[ci];
        let rank_c = match rank { 3 => C::Gold, 2 => C::Blue, 1 => C::Text, _ => C::Dimmer };
        let mut segs = vec![
            (format!("{} → ", BIOMES[biome].name), C::Dim),
            (format!("{}{}", c.n, if shiny { " ⋆" } else { "" }), if shiny { C::Shiny } else { rarity_color(c.r) }),
            (format!(" {}[{}]", if sex == 0 { "♂" } else { "♀" }, RANK_NAMES[rank]), rank_c),
            (format!(" ({}{})", RAR_LABEL[c.r], if shiny { " · shiny" } else { "" }), C::Dimmer),
        ];
        if sold > 0.0 {
            segs.push((format!(" · auto-vente +{} écus", fmt(sold)), C::GoldDark));
        }
        self.log_prise(segs);
        if is_new {
            self.log(vec![
                ("nouvelle espèce découverte : ".into(), C::Green),
                (c.n.to_string(), rarity_color(c.r)),
                (" !".into(), C::Green),
            ]);
            self.toast(format!("nouvelle espèce : {}", c.n));
        }
        if new_shiny && !is_new {
            self.toast(format!("shiny obtenu : {} ⋆", c.n));
        }
        if rank == 3 {
            self.toast(format!("rang S : {} !", c.n));
        }
        if c.r == 4 {
            self.toast(format!("légendaire capturé : {}", c.n));
        }
    }    fn tick(&mut self) {
        let now = now_ms();
        for b in 0..BIOMES.len() {
            let slots = match &self.s.biomes[b] {
                Some(bs) => bs.pl.len(),
                None => continue,
            };
            for i in 0..slots {
                let mut guard = 0;
                loop {
                    let Some(pl) = self.s.biomes[b].as_ref().and_then(|bs| bs.pl[i].clone()) else { break };
                    if pl.next_at > now || guard >= 50 {
                        break;
                    }
                    guard += 1;
                    let res = self.attempt(b, i, pl.next_at, 0.0, false);
                    self.report_catch(b, res);
                    let bait_ok = self.s.biomes[b].as_ref().and_then(|bs| bs.pl[i].as_ref()).and_then(|p| p.bait).filter(|&bt| self.s.baits[bt] > 0);
                    let itv = self.trap_interval(pl.trap, bait_ok, b, pl.next_at);
                    if let Some(bs) = self.s.biomes[b].as_mut() {
                        if let Some(p) = bs.pl[i].as_mut() {
                            p.next_at += itv;
                        }
                    }
                }
            }
        }
        // contrats : renouvellement toutes les 2 h
        let cw = (now / 7_200_000.0) as u64;
        if self.s.contracts_window != cw || self.s.contracts.is_empty() {
            self.s.contracts_window = cw;
            self.s.contracts = self.gen_contracts(cw);
            self.s.contracts_done = vec![false; 3];
            self.log(vec![("de nouveaux contrats sont affichés à la boutique.".into(), C::Blue)]);
        }
        // l'enclos : on prévient dès qu'un petit est né, une seule fois
        {
            let prets: Vec<(usize, f64)> = self
                .s
                .pens
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.as_ref().filter(|p| p.ready_at <= now).map(|p| (p.ci, p.ready_at)))
                .collect();
            self.s_pens_cleanup();
            for (ci, at) in prets {
                if !self.pens_seen.contains(&at) {
                    self.pens_seen.push(at);
                    self.log(vec![
                        ("l'enclos s'agite : ".into(), C::Green),
                        (format!("un petit {} est né", CREATURES[ci].n), rarity_color(CREATURES[ci].r)),
                        (" — allez le chercher.".into(), C::Dim),
                    ]);
                    self.toast(format!("naissance : {} vous attend", CREATURES[ci].n));
                }
            }
        }
        // le marchand : on prévient quand il s'installe, il ne reste pas longtemps
        if let Some((mw, mfin)) = self.merchant_now() {
            if self.merchant_seen != mw {
                self.merchant_seen = mw;
                let reste = mfin - now;
                self.log(vec![(
                    format!(
                        "le marchand ambulant déballe sa malle sur la place — il repart dans {} min.",
                        (reste / 60_000.0).ceil() as u64
                    ),
                    C::Gold,
                )]);
                self.toast("le marchand est là");
            }
        }
        // troc : les collectionneurs changent d'envie chaque jour
        let tw = (now / 86_400_000.0) as u64;
        if self.s.trades_window != tw || (self.s.trades.is_empty() && !self.s.dex2.iter().all(|d| d.n == 0)) {
            self.s.trades_window = tw;
            self.s.trades = self.gen_trades(tw);
            self.s.trades_done = vec![false; self.s.trades.len()];
            if !self.s.trades.is_empty() {
                self.log(vec![(
                    format!("le comptoir de troc affiche {} demande(s) du jour.", self.s.trades.len()),
                    C::Blue,
                )]);
            }
        }
        // rituel du jour : série de connexions consécutives -> élan + petit cadeau
        let day = (now / 86_400_000.0) as u32;
        if self.s.last_day != day {
            self.s.streak = if self.s.last_day + 1 == day { self.s.streak + 1 } else { 1 };
            self.s.last_day = day;
            let gift = (self.s.streak.min(10)) as u64;
            self.s.baits[BAIT_BAIES] += gift;
            self.log(vec![
                (format!("jour de chasse n° {} d'affilée : ", self.s.streak), C::Gold),
                (format!("élan +{} aujourd'hui · {} baies offertes", fmt2(self.streak_bonus()), gift), C::Green),
            ]);
            self.toast(format!("série : jour {} — élan +{}", self.s.streak, fmt2(self.streak_bonus())));
            if fair_day() {
                self.log(vec![(
                    "jour de foire : le marchand reste toute la journée, le troc double ses demandes, la chance sourit.".into(),
                    C::Gold,
                )]);
            }
            self.check_achievements();
        }
        // légende errante : annoncer son apparition (une fois par fenêtre)
        if let Some((w, b, _)) = self.legend_now() {
            if !self.s.legends_open.contains(&w) {
                self.s.legends_open.push(w);
                if self.s.legends_open.len() > 8 {
                    self.s.legends_open.remove(0);
                }
            }
            if w != self.legend_seen {
                self.legend_seen = w;
                let left = self.legend_left_min(w, now);
                self.log(vec![
                    ("✧ une silhouette immense rôde ".into(), C::Gold),
                    (format!("en {} — {} min pour la trouver (repérez le ✧ sur l'étiquette du biome) !", BIOMES[b].name, left), C::Gold),
                ]);
                self.toast(format!("✧ légende errante : {}", BIOMES[b].name));
            }
        }
        // musée : le revenu s'accumule (plafond extensible au labo)
        self.museum_accrue(now);
        /* garnissage automatique : inutile de le refaire à chaque image,
           les salles ne changent qu'au rythme des prises */
        if self.s.museum_auto && now - self.museum_auto_at > 3000.0 {
            self.museum_auto_at = now;
            self.museum_optimize();
        }
        self.toasts.retain(|&(_, t)| now - t < 3800.0);
        self.check_achievements();
        self.s.last_seen = now;
    }
    /* oublie les naissances déjà récupérées : la liste ne grandit pas */
    fn s_pens_cleanup(&mut self) {
        let vivants: Vec<f64> = self.s.pens.iter().flatten().map(|p| p.ready_at).collect();
        self.pens_seen.retain(|a| vivants.contains(a));
    }

    fn museum_accrue(&mut self, now: f64) {
        if self.s.museum_at == 0.0 {
            self.s.museum_at = now;
            return;
        }
        let dt = (now - self.s.museum_at).max(0.0);
        self.s.museum_at = now;
        let cap = self.museum_rate() * self.museum_cap_h() * 3_600_000.0;
        self.s.museum_pool = (self.s.museum_pool + self.museum_rate() * dt).min(cap);
    }
    /* garnit le musée avec les spécimens les plus rentables : ce qui est
       exposé retourne d'abord en réserve, puis on reprend les meilleurs.
       l'opération est donc idempotente, et le revenu étant la somme des
       pièces, prendre les N plus chères est bien l'optimum. */
    fn museum_optimize(&mut self) -> bool {
        let n = self.museum_slots().min(self.s.museum.len());
        self.museum_accrue(now_ms());
        let avant: Vec<Option<(usize, usize, bool)>> =
            self.s.museum.iter().take(n).map(|o| o.as_ref().map(|m| (m.ci, m.rank, m.shiny))).collect();
        for slot in 0..n {
            if let Some(m) = self.s.museum[slot].take() {
                self.give_back(m.ci, m.shiny, m.sex, m.rank);
            }
        }
        let mut lots: Vec<(f64, usize, bool)> = vec![];
        for ci in 0..CREATURES.len() {
            for shiny in [false, true] {
                for r in 0..4 {
                    let iv = &self.s.inv2[ci];
                    let dispo = if shiny { iv.sr(r) } else { iv.nr(r) };
                    if dispo == 0 {
                        continue;
                    }
                    let v = self.creature_value_r(ci, shiny, r);
                    for _ in 0..dispo.min(n as u64) {
                        lots.push((v, ci, shiny));
                    }
                }
            }
        }
        lots.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut pose = 0;
        for &(_, ci, shiny) in lots.iter().take(n) {
            if let Some((rank, sex)) = self.take_best(ci, shiny) {
                self.s.museum[pose] = Some(MusE { ci, rank, shiny, sex });
                pose += 1;
            }
        }
        let apres: Vec<Option<(usize, usize, bool)>> =
            self.s.museum.iter().take(n).map(|o| o.as_ref().map(|m| (m.ci, m.rank, m.shiny))).collect();
        avant != apres
    }

    fn museum_slots(&self) -> usize {
        6 + self.s.lab[LAB_AILES] as usize
    }
    fn museum_cap_h(&self) -> f64 {
        4.0 + self.s.lab[LAB_CONSERVATION] as f64 * 2.0
    }
    fn pen_slots(&self) -> usize {
        3 + self.s.lab[LAB_ENCLOS] as usize
    }
    fn pen_rankup(&self) -> f64 {
        0.25 + self.s.lab[LAB_LIGNEES] as f64 * 0.04
    }    fn hunt_cooldown_ms(&self) -> f64 {
        (300.0 - self.s.lab[LAB_TRAQUEUR] as f64 * 30.0) * 1000.0
    }
    /* écus par milliseconde générés par les salles occupées */
    fn museum_rate(&self) -> f64 {
        self.s.museum.iter().flatten().map(|m| self.creature_value_r(m.ci, m.shiny, m.rank) * 0.001 / 60_000.0).sum()
    }

    pub fn welcome(&mut self, fresh: bool) {
        let msg = if fresh {
            "bienvenue. un piège en bois vous attend en réserve — la forêt est à l'ouest.".into()
        } else {
            let poses: usize = self.s.biomes.iter().flatten().map(|b| b.pl.iter().flatten().count()).sum();
            format!(
                "bon retour. {} écus · {} captures · {} piège(s) en service.",
                fmt(self.s.ecus),
                fmt(self.s.captures as f64),
                poses
            )
        };
        self.log(vec![(msg, C::Green)]);
    }

    fn run_offline(&mut self) {
        let now = now_ms();
        let away = now - self.s.last_seen;
        if away < 15000.0 {
            self.reset_timers(now);
            return;
        }
        let capped = away.min(self.offline_cap_ms());
        let from = now - capped;
        let (mut caught, earned0, shinies0) = (0u64, self.s.total_earned, self.s.shinies);
        let mut discoveries = vec![];
        for b in 0..BIOMES.len() {
            let slots = match &self.s.biomes[b] {
                Some(bs) => bs.pl.len(),
                None => continue,
            };
            for i in 0..slots {
                let Some(pl0) = self.s.biomes[b].as_ref().and_then(|bs| bs.pl[i].clone()) else { continue };
                let mut t = pl0.next_at.max(from);
                let mut guard = 0;
                while t <= now && guard < 4000 {
                    guard += 1;
                    if let Some(res) = self.attempt(b, i, t, 0.0, true) {
                        caught += 1;
                        if res.2 {
                            discoveries.push(res.0);
                        }
                    }
                    let bait_ok = self.s.biomes[b].as_ref().unwrap().pl[i]
                        .as_ref()
                        .and_then(|p| p.bait)
                        .filter(|&bt| self.s.baits[bt] > 0);
                    t += self.trap_interval(pl0.trap, bait_ok, b, t);
                }
                if let Some(bs) = self.s.biomes[b].as_mut() {
                    if let Some(p) = bs.pl[i].as_mut() {
                        p.next_at = t;
                    }
                }
            }
        }
        self.museum_accrue(now);
        self.check_achievements();
        if caught > 0 {
            let sum = OfflineSummary {
                h: (capped / 3600000.0) as u64,
                m: ((capped as u64 % 3600000) / 60000),
                hit_cap: away > self.offline_cap_ms(),
                caught,
                shinies: self.s.shinies - shinies0,
                earned: self.s.total_earned - earned0,
                discoveries,
            };
            self.log(vec![(
                format!("retour de {} hors-ligne : {} captures", if sum.h > 0 { format!("{} h {} min", sum.h, sum.m) } else { format!("{} min", sum.m) }, fmt(caught as f64)),
                C::Green,
            )]);
            self.panels.push(Panel::new(PanelKind::Offline(sum)));
        }
        self.reset_timers(now);
    }    fn reset_timers(&mut self, now: f64) {
        for b in 0..BIOMES.len() {
            let Some(bs) = self.s.biomes[b].as_ref() else { continue };
            for i in 0..bs.pl.len() {
                let Some(pl) = self.s.biomes[b].as_ref().unwrap().pl[i].clone() else { continue };
                if pl.next_at < now || pl.next_at > now + 120000.0 {
                    let itv = self.trap_interval(pl.trap, pl.bait, b, now);
                    if let Some(p) = self.s.biomes[b].as_mut().unwrap().pl[i].as_mut() {
                        p.next_at = now + itv;
                    }
                }
            }
        }
    }    fn ach_done(&self, i: usize) -> bool {
        let s = &self.s;
        match i {
            0 => s.captures >= 1,
            1 => s.captures >= 100,
            2 => s.captures >= 1000,
            3 => s.captures >= 10000,
            4 => s.shinies >= 1,
            5 => s.shinies >= 25,
            6 => wild_species().any(|c| CREATURES[c].r == 4 && s.dex2[c].n > 0),
            7 => wild_species().filter(|&c| s.dex2[c].n > 0).count() >= 10,
            8 => wild_species().filter(|&c| s.dex2[c].n > 0).count() >= 30,
            9 => wild_species().all(|c| s.dex2[c].n > 0),
            10 => s.biomes[1].is_some(),
            11 => s.biomes[5].is_some(),
            12 => s.total_earned >= 10000.0,
            13 => s.total_earned >= 1000000.0,
            14 => s.traps[5] >= 1,
            15 => s.migrations >= 1,
            16 => s.migrations >= 5,
            17 => biome_creatures(0).all(|c| s.dex2[c].n > 0),
            18 => wild_species().any(|c| s.dex2[c].best >= 4),
            19 => wild_species().filter(|&c| s.dex2[c].best >= 4).count() >= 10,
            20 => s.hunts_done >= 1,
            21 => s.contracts_delivered >= 5,
            22 => s.legends_caught >= 1,
            23 => s.pen_born >= 1,
            24 => s.museum.iter().flatten().count() >= 6,
            25 => NOCTURNES.iter().any(|&c| s.dex2[c].n > 0),
            26 => s.streak >= 7,
            27 => s.captures >= 666,
            28 => (0..CREATURES.len()).filter(|&i| CREATURES[i].b == LEGEND_B).all(|i| s.dex2[i].n > 0),
            29 => s.sacrifices >= 1,
            30 => s.sacrifices >= 6,
            31 => s.sacrifices >= 66,
            32 => s.sacrifices >= 666,
            _ => false,
        }
    }    fn check_achievements(&mut self) {
        for i in 0..ACHS.len() {
            if !self.s.ach[i] && self.ach_done(i) {
                self.s.ach[i] = true;
                if ACHS[i].r > 0.0 {
                    self.gain(ACHS[i].r);
                }
                if i == ACH_BETE {
                    self.pentacle_until = now_ms() + 12_000.0;
                    self.log(vec![(
                        "le puits déborde. le cercle s'ouvre de lui-même, et quelque chose remonte avec l'eau.".into(),
                        C::Red,
                    )]);
                    self.toast("666 offrandes…");
                }
                if i == ACH_666 {
                    self.pentacle_until = now_ms() + 12_000.0;
                    self.log(vec![(
                        "un cercle s'ouvre au centre du village. personne ne l'a dessiné.".into(),
                        C::Red,
                    )]);
                    self.toast("666 prises…");
                }
                self.log(vec![
                    ("succès débloqué : ".into(), C::Green),
                    (ACHS[i].n.into(), C::Gold),
                    (if ACHS[i].r > 0.0 { format!(" (+{} écus)", fmt(ACHS[i].r)) } else { String::new() }, C::Dimmer),
                ]);
                self.toast(format!("succès : {}", ACHS[i].n));
            }
        }
    }

    /* ----------------------------------------------------------- actions */

    fn apply(&mut self, a: Action) {
        match a {
            Action::Open(kind) => self.panels.push(Panel::new(kind)),
            Action::Close => {
                self.panels.pop();
            }
            Action::CloseAll => self.panels.clear(),
            Action::BuyTrap(t) => {
                if self.s.ecus >= TRAPS[t].cost {
                    self.s.ecus -= TRAPS[t].cost;
                    self.s.traps[t] += 1;
                    self.log(vec![
                        (format!("acheté : {}", TRAPS[t].n), C::Text),
                        (format!(" (−{} écus)", fmt(TRAPS[t].cost)), C::Dimmer),
                    ]);
                }
            }
            Action::SellTrap(t) => {
                let libres = self.s.traps[t] - self.placed_count(t);
                let total: u32 = self.s.traps.iter().sum();
                if libres > 0 && total > 1 {
                    self.s.traps[t] -= 1;
                    let v = (TRAPS[t].cost * TRAP_RESALE).floor();
                    self.gain(v);
                    self.log(vec![
                        (format!("revendu : {}", TRAPS[t].n), C::Text),
                        (format!(" (+{} écus)", fmt(v)), C::GoldDark),
                    ]);
                }
            }
            Action::BuyBait(bt, q) => {
                let cost = BAITS[bt].cost * q as f64;
                if self.s.ecus >= cost {
                    self.s.ecus -= cost;
                    self.s.baits[bt] += q;
                    self.log(vec![
                        (format!("acheté : {}× {}", q, BAITS[bt].n), C::Text),
                        (format!(" (−{} écus)", fmt(cost)), C::Dimmer),
                    ]);
                }
            }
            Action::BuyLab(k) => {
                let cost = self.lab_cost(k);
                if self.s.lab[k] < LABS[k].max && self.s.ecus >= cost {
                    self.s.ecus -= cost;
                    self.s.lab[k] += 1;
                    self.log(vec![
                        (format!("labo : {} niveau {}", LABS[k].n, self.s.lab[k]), C::Blue),
                        (format!(" (−{} écus)", fmt(cost)), C::Dimmer),
                    ]);
                }
            }
            Action::Unlock(b) => {
                if self.s.biomes[b].is_none() && self.s.ecus >= BIOMES[b].cost {
                    self.s.ecus -= BIOMES[b].cost;
                    self.s.biomes[b] = Some(BiomeState { slots: 2, pl: vec![None, None], hunt_at: 0.0 });
                    self.log(vec![(format!("biome débloqué : {}", BIOMES[b].name), C::Green)]);
                    self.toast(format!("biome débloqué : {}", BIOMES[b].name));
                    self.check_achievements();
                    self.panels.pop();
                }
            }
            Action::BuySlot(b) => {
                let cost = self.slot_cost(b);
                if let Some(bs) = self.s.biomes[b].as_ref() {
                    if bs.slots < 4 && self.s.ecus >= cost {
                        self.s.ecus -= cost;
                        let bs = self.s.biomes[b].as_mut().unwrap();
                        bs.slots += 1;
                        bs.pl.push(None);
                        self.log(vec![
                            (format!("emplacement supplémentaire : {}", BIOMES[b].name), C::Text),
                            (format!(" (−{} écus)", fmt(cost)), C::Dimmer),
                        ]);
                    }
                }
            }
            Action::Place(b, i, t) => {
                if self.placed_total() >= self.trap_cap() {
                    self.log(vec![(format!("licence de piégeage : {} pièges posés maximum (voir le labo).", self.trap_cap()), C::Red)]);
                    return;
                }
                let itv = self.trap_interval(t, None, b, now_ms());
                if let Some(bs) = self.s.biomes[b].as_mut() {
                    bs.pl[i] = Some(Placement { trap: t, bait: None, next_at: now_ms() + itv });
                }
                self.log(vec![(format!("piège posé : {} → {}", TRAPS[t].n, BIOMES[b].name), C::Text)]);
                self.panels.pop();
            }
            Action::SetBait(b, i, bt) => {
                if let Some(bs) = self.s.biomes[b].as_mut() {
                    if let Some(p) = bs.pl[i].as_mut() {
                        p.bait = bt;
                    }
                }
                self.panels.pop();
            }
            Action::Remove(b, i) => {
                if let Some(bs) = self.s.biomes[b].as_mut() {
                    bs.pl[i] = None;
                }
            }
            Action::Sell(ci, shiny, qty) => {
                let have = if shiny { self.s.inv2[ci].ts() } else { self.s.inv2[ci].tn() };
                let (q, v) = match qty {
                    SellQty::One => {
                        let q = 1u64.min(have);
                        (q, self.take_lowest(ci, shiny, q))
                    }
                    SellQty::All => (have, self.take_lowest(ci, shiny, have)),
                    // garde le meilleur mâle et la meilleure femelle (le couple pour l'enclos)
                    SellQty::Keep1 => self.sell_except_pair(ci),
                };
                if q > 0 {
                    let v = v.floor();
                    self.gain(v);
                    self.log(vec![
                        (format!("vendu : {}× {}{}", q, CREATURES[ci].n, if shiny { " ⋆" } else { "" }), C::Text),
                        (format!(" (+{} écus)", fmt(v)), C::GoldDark),
                    ]);
                }
            }
            Action::SellDupes => {
                let mut total = 0.0;
                let mut n = 0u64;
                for ci in 0..CREATURES.len() {
                    if self.s.inv2[ci].tn() > self.seuil_garde(ci) {
                        let (q, v) = self.sell_surplus(ci);
                        total += v;
                        n += q;
                    }
                }
                if n > 0 {
                    total = total.floor();
                    self.gain(total);
                    self.log(vec![
                        (format!("vendu : {} doublons", fmt(n as f64)), C::Text),
                        (format!(" (+{} écus)", fmt(total)), C::GoldDark),
                    ]);
                }
            }
            Action::ToggleAutosell(r) => self.s.autosell[r] = !self.s.autosell[r],
            Action::SetAutokeep(n) => self.s.autokeep = n.clamp(1, 4),
            Action::Migrate => {
                let g = self.trophy_gain();
                let cost = self.migration_cost();
                if g >= 1 && self.s.ecus >= cost {
                    self.s.trophies += g;
                    self.s.migrations += 1;
                    self.s.ecus = 30.0;
                    self.s.run_earned = 0.0;
                    self.s.traps = vec![0; 6];
                    self.s.traps[0] = 1;
                    self.s.baits = vec![0; 5];
                    self.s.inv2 = vec![InvE::default(); CREATURES.len()];
                    self.s.museum = vec![None; 12];
                    self.s.museum_pool = 0.0;
                    self.s.pens = vec![None; 6];
                    self.s.contracts_done = vec![false; 3];
                    self.s.contracts = vec![];
                    self.s.lab = vec![0; LABS.len()];
                    self.s.autosell = vec![false; 5];
                    let mut biomes = vec![None; BIOMES.len()];
                    biomes[0] = Some(BiomeState { slots: 2, pl: vec![None, None], hunt_at: 0.0 });
                    self.s.biomes = biomes;
                    // filet de sécurité : garantit les tailles de tous les vecteurs,
                    // pour que la migration ne puisse plus jamais laisser un état bancal
                    self.s.normalize();
                    self.panels.clear();
                    self.log(vec![(format!("migration effectuée : +{} trophées. tout recommence, en mieux.", g), C::Gold)]);
                    self.toast(format!("migration : +{} trophées", g));
                    self.check_achievements();
                    self.save();
                }
            }
            Action::DoReset => {
                let _ = std::fs::remove_file(save_path());
                self.s = State::default();
                self.logs.clear();
                self.panels.clear();
            }
            Action::Hunt(b) => {
                let now = now_ms();
                let ok = self.s.biomes[b].as_ref().map(|bs| bs.hunt_at <= now).unwrap_or(false);
                if ok {
                    let slots = self.s.biomes[b].as_ref().unwrap().pl.len();
                    let placed = self.s.biomes[b].as_ref().unwrap().pl.iter().flatten().count();
                    let mut hits = 0;
                    if placed == 0 {
                        // à mains nues : une seule tentative, modeste mais toujours possible
                        let res = self.bare_attempt(b, now);
                        if res.is_some() {
                            hits += 1;
                        }
                        self.report_catch(b, res);
                    }
                    for i in 0..slots {
                        if self.s.biomes[b].as_ref().unwrap().pl[i].is_some() {
                            let res = self.attempt(b, i, now, 0.2, false);
                            if res.is_some() {
                                hits += 1;
                            }
                            self.report_catch(b, res);
                        }
                    }
                    let cd = self.hunt_cooldown_ms();
                    self.s.biomes[b].as_mut().unwrap().hunt_at = now + cd;
                    self.s.hunts_done += 1;
                    /* chaque battue remue le terrain : les légendes s'en
                       approchent pendant une heure, et l'effet s'accumule */
                    self.s.hunts_at.push(now);
                    self.s.hunts_at.retain(|&t| now - t < HUNT_BUFF_MS);
                    if self.s.hunts_at.len() > 32 {
                        self.s.hunts_at.remove(0);
                    }
                    self.log(vec![(format!("battue en {} : {} prise{}", BIOMES[b].name, hits, if hits > 1 { "s" } else { "" }), C::Green)]);
                    self.check_achievements();
                }
            }
            Action::Deliver(idx) => {
                let (_, contracts) = self.contracts_now();
                if idx < contracts.len() && !self.s.contracts_done[idx] {
                    let c = &contracts[idx];
                    if self.deliverable(c.ci) >= c.qty {
                        let (ci, qty, reward) = (c.ci, c.qty, c.reward);
                        // le meilleur couple ♂♀ n'est JAMAIS livré ; les shinies non plus
                        let bm = self.take_best_sex(ci, false, 0);
                        let bf = self.take_best_sex(ci, false, 1);
                        self.take_lowest(ci, false, qty);
                        if let Some(r) = bm {
                            self.give_back(ci, false, 0, r);
                        }
                        if let Some(r) = bf {
                            self.give_back(ci, false, 1, r);
                        }
                        self.s.contracts_done[idx] = true;
                        self.s.contracts_delivered += 1;
                        self.gain(reward);
                        self.log(vec![
                            (format!("contrat livré : {}× {}", qty, CREATURES[ci].n), C::Blue),
                            (format!(" (+{} écus)", fmt(reward)), C::GoldDark),
                        ]);
                        self.toast("contrat livré");
                        self.check_achievements();
                    }
                }
            }
            Action::MuseumAdd(slot, ci, shiny) => {
                if slot < self.museum_slots() && self.s.museum[slot].is_none() {
                    self.museum_accrue(now_ms());
                    if let Some((rank, sex)) = self.take_best(ci, shiny) {
                        self.s.museum[slot] = Some(MusE { ci, rank, shiny, sex });
                        self.log(vec![(format!("exposé au musée : {}{} [{}]", CREATURES[ci].n, if shiny { " ⋆" } else { "" }, RANK_NAMES[rank]), C::Blue)]);
                        self.check_achievements();
                        self.panels.pop();
                    }
                }
            }
            Action::MuseumRemove(slot) => {
                self.museum_accrue(now_ms());
                if let Some(m) = self.s.museum[slot].take() {
                    self.give_back(m.ci, m.shiny, m.sex, m.rank);
                    self.log(vec![(format!("retiré du musée : {}", CREATURES[m.ci].n), C::Dim)]);
                }
            }
            Action::MuseumAuto => {
                if self.museum_optimize() {
                    self.log(vec![("musée : les salles reprennent les spécimens les plus rentables.".into(), C::Blue)]);
                    self.toast("musée garni");
                } else {
                    self.toast("le musée expose déjà le mieux payé");
                }
                self.check_achievements();
            }
            Action::ToggleMuseumAuto => {
                self.s.museum_auto = !self.s.museum_auto;
                if self.s.museum_auto {
                    self.museum_auto_at = now_ms();
                    self.museum_optimize();
                    self.check_achievements();
                }
            }
            Action::Sacrifice(ci) => {
                if self.well_used_tonight() || !puits_assoiffe(now_ms()) || self.s.inv2[ci].tn() == 0 {
                    self.panels.pop();
                    return;
                }
                /* on donne son plus bas rang, jamais un shiny : le puits ne
                   veut pas de ce qui brille */
                let Some((rang, _)) = self.take_lowest_one(ci) else { return };
                self.s.well_night = jour_reel(now_ms());
                self.s.sacrifices += 1;
                let c = &CREATURES[ci];
                self.log(vec![
                    (format!("vous laissez {} [{}] glisser dans le puits. ", c.n, RANK_NAMES[rang]), C::Red),
                    ("l'eau se referme sans un bruit.".into(), C::Dimmer),
                ]);
                self.recompense_du_puits(ci, rang);
                self.check_achievements();
                self.panels.pop();
            }
            Action::MuseumCollect => {
                self.museum_accrue(now_ms());
                let v = self.s.museum_pool.floor();
                if v >= 1.0 {
                    self.s.museum_pool -= v;
                    self.gain(v);
                    self.log(vec![("recette du musée : ".into(), C::Blue), (format!("+{} écus", fmt(v)), C::GoldDark)]);
                }
            }
            Action::MerchBuy(item) => {
                let Some((w, _)) = self.merchant_now() else { return };
                let key = w * 8 + item as u64;
                let prix = self.merchant_price(item);
                if self.s.merchant_done.contains(&key) || self.s.ecus < prix {
                    return;
                }
                self.s.ecus -= prix;
                self.s.merchant_done.push(key);
                match item {
                    0 => {
                        let curios: Vec<usize> = (0..CREATURES.len()).filter(|&i| CREATURES[i].b == CURIO_B).collect();
                        let ci = curios[rand::thread_rng().gen_range(0..curios.len())];
                        let rank = self.roll_rank(self.global_luck());
                        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, now_ms());
                        let (is_new, _, _) = self.add_specimen(ci, shiny, rank);
                        self.log(vec![
                            ("l'œuf éclot : ".into(), C::Dim),
                            (
                                format!("{}{}", CREATURES[ci].n, if shiny { " ⋆" } else { "" }),
                                if shiny { C::Shiny } else { rarity_color(CREATURES[ci].r) },
                            ),
                            (if is_new { " — nouvelle curiosité !".into() } else { String::new() }, C::Green),
                        ]);
                    }
                    1 => {
                        self.s.charms += 1;
                        self.log(vec![("une breloque de plus : chance +0,10.".into(), C::Green)]);
                    }
                    2 => {
                        let b = self.best_bait();
                        self.s.baits[b] += 10;
                        self.log(vec![(format!("dix {} rejoignent votre besace.", BAITS[b].n), C::Green)]);
                    }
                    3 => {
                        self.s.licences += 1;
                        self.log(vec![(
                            format!("licence obtenue : {} emplacements de piège au total.", self.trap_cap()),
                            C::Green,
                        )]);
                    }
                    _ => {
                        let t = self.next_trap();
                        self.s.traps[t] += 1;
                        self.log(vec![(format!("un {} d'occasion entre en réserve.", TRAPS[t].n), C::Green)]);
                    }
                }
                self.toast("le marchand vous salue");
                self.check_achievements();
            }
            Action::Trade(i) => {
                let Some(&(want, qty, give)) = self.s.trades.get(i) else { return };
                if self.s.trades_done.get(i).copied().unwrap_or(true) || self.s.inv2[want].tn() < qty {
                    return;
                }
                self.take_lowest(want, false, qty); // les plus bas rangs partent en premier
                let rank = self.roll_rank(self.global_luck());
                let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, now_ms());
                let (is_new, _, sex) = self.add_specimen(give, shiny, rank);
                if let Some(d) = self.s.trades_done.get_mut(i) {
                    *d = true;
                }
                self.s.trades_made += 1;
                self.log(vec![
                    (format!("troc : {} ×{} contre ", CREATURES[want].n, qty), C::Dim),
                    (
                        format!(
                            "{}{} {}[{}]",
                            CREATURES[give].n,
                            if shiny { " ⋆" } else { "" },
                            if sex == 0 { "♂" } else { "♀" },
                            RANK_NAMES[rank]
                        ),
                        if shiny { C::Shiny } else { rarity_color(CREATURES[give].r) },
                    ),
                    (if is_new { " — nouvelle curiosité !".into() } else { String::new() }, C::Green),
                ]);
                self.toast(format!("troc : {}", CREATURES[give].n));
                self.check_achievements();
            }
            Action::PenStart(slot, ci, rang) => {
                let iv = &self.s.inv2[ci];
                let dispo = match rang {
                    Some(r) => iv.m[r] >= 1 && iv.f[r] >= 1,
                    None => iv.tm() >= 1 && iv.tf() >= 1,
                };
                if slot < self.pen_slots() && self.s.pens[slot].is_none() && dispo {
                    /* par défaut on consomme les plus bas rangs — mais le petit
                       naît au meilleur rang de ses parents, alors le joueur peut
                       vouloir sacrifier du beau pour viser un S, shiny compris */
                    let (rm, rf) = match rang {
                        Some(r) => (r, r),
                        None => (
                            (0..4).find(|&r| self.s.inv2[ci].m[r] > 0).unwrap(),
                            (0..4).find(|&r| self.s.inv2[ci].f[r] > 0).unwrap(),
                        ),
                    };
                    self.s.inv2[ci].m[rm] -= 1;
                    self.s.inv2[ci].f[rf] -= 1;
                    let dur = PEN_MIN[CREATURES[ci].r] * 60_000.0;
                    self.s.pens[slot] = Some(Pen { ci, r1: rm, r2: rf, ready_at: now_ms() + dur });
                    self.log(vec![(format!("enclos : un couple de {} (♂[{}] ♀[{}]) s'installe.", CREATURES[ci].n, RANK_NAMES[rm], RANK_NAMES[rf]), C::Green)]);
                    self.panels.pop();
                }
            }
            Action::PenCollect(slot) => {
                let now = now_ms();
                if let Some(pen) = self.s.pens[slot].clone() {
                    if pen.ready_at <= now {
                        self.s.pens[slot] = None;
                        let mut rank = pen.r1.max(pen.r2);
                        if rank < 3 && rand::thread_rng().gen::<f64>() < self.pen_rankup() {
                            rank += 1;
                        }
                        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, now) * 2.0;
                        let (is_new, _, sex) = self.add_specimen(pen.ci, shiny, rank);
                        self.s.pen_born += 1;
                        /* la naissance se raconte : d'où vient le petit et ce
                           qu'il vaut par rapport à ses parents */
                        let mieux = rank > pen.r1.max(pen.r2);
                        let mut segs = vec![
                            ("naissance à l'enclos — ".into(), C::Green),
                            (CREATURES[pen.ci].n.to_string(), rarity_color(CREATURES[pen.ci].r)),
                            (" : ".into(), C::Dim),
                            (format!("♂[{}] × ♀[{}]", RANK_NAMES[pen.r1], RANK_NAMES[pen.r2]), C::Dim),
                            (" donnent ".into(), C::Dim),
                            (
                                format!("{}[{}]{}", if sex == 0 { "♂" } else { "♀" }, RANK_NAMES[rank], if shiny { " ⋆" } else { "" }),
                                if shiny { C::Shiny } else if mieux { C::Gold } else { C::Text },
                            ),
                        ];
                        if mieux {
                            segs.push((" — le petit dépasse ses parents !".into(), C::Gold));
                        }
                        if shiny {
                            segs.push((" et il brille.".into(), C::Shiny));
                        }
                        if is_new {
                            segs.push((" première de son espèce au bestiaire.".into(), C::Green));
                        }
                        self.log(segs);
                        self.toast(format!(
                            "naissance : {} {}[{}]{}",
                            CREATURES[pen.ci].n,
                            if sex == 0 { "♂" } else { "♀" },
                            RANK_NAMES[rank],
                            if shiny { " ⋆" } else { "" }
                        ));
                        self.check_achievements();
                    }
                }
            }
            Action::LegendTry(_biome, window, bait) => {
                if self.s.legends_tried.contains(&window) {
                    self.panels.pop();
                } else {
                    self.s.legends_tried.push(window);
                    if self.s.legends_tried.len() > 24 {
                        self.s.legends_tried.remove(0);
                    }
                    let mut p = 0.25 + (self.global_luck() * 0.05).min(0.15)
                        + self.s.lab[LAB_APPROCHE] as f64 * 0.05;
                    let mut shiny_mult = 4.0;
                    if let Some(bt) = bait {
                        if self.s.baits[bt] > 0 {
                            self.s.baits[bt] -= 1;
                            match bt {
                                BAIT_VIANDE | BAIT_NECTAR => p += 0.10,
                                BAIT_TRUFFE => p += 0.15,
                                BAIT_ESSENCE => {
                                    p += 0.10;
                                    shiny_mult = 16.0;
                                }
                                _ => p += 0.05,
                            }
                        }
                    }
                    let now = now_ms();
                    if rand::thread_rng().gen::<f64>() < p {
                        let ci = tirage_legende();
                        let rank = self.roll_rank(self.global_luck() + 0.5).max(2);
                        let shiny = rand::thread_rng().gen::<f64>() < (SHINY_BASE * shiny_mult).min(0.05);
                        self.add_specimen(ci, shiny, rank);
                        self.s.legends_caught += 1;
                        self.log(vec![
                            ("légende errante capturée : ".into(), C::Gold),
                            (format!("{}{} [{}] !", CREATURES[ci].n, if shiny { " ⋆" } else { "" }, RANK_NAMES[rank]),
                             if shiny { C::Shiny } else { rarity_color(CREATURES[ci].r) }),
                        ]);
                        self.toast(format!("légende capturée : {}", CREATURES[ci].n));
                        self.check_achievements();
                    } else {
                        self.log(vec![("la silhouette s'évanouit dans les fourrés…".into(), C::Dim)]);
                        self.toast("la légende s'est enfuie");
                    }
                    self.panels.pop();
                }
            }
            Action::TraceFollow(key, biome) => {
                if !self.s.traces_done.contains(&key) {
                    self.s.traces_done.push(key);
                    if self.s.traces_done.len() > 40 {
                        self.s.traces_done.remove(0);
                    }
                    let now = now_ms();
                    let roll = rand::thread_rng().gen::<f64>();
                    if roll < 0.60 {
                        // la piste aboutit : capture ciblée
                        let luck = self.biome_luck(biome, now) + 0.1;
                        let ci = self.roll_creature(biome, luck, None, now);
                        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, now);
                        let rank = self.roll_rank(luck);
                        let (is_new, new_shiny, sex) = self.add_specimen(ci, shiny, rank);
                        // annoncé AVANT la prise : on lit d'abord la piste, puis ce qu'elle a donné
                        self.log(vec![(
                            format!("vous suivez les traces en {} : elles mènent droit au gîte, et vous surprenez son occupant.", BIOMES[biome].name),
                            C::Green,
                        )]);
                        self.report_catch(biome, Some((ci, shiny, is_new, new_shiny, 0.0, rank + sex as usize * 10)));
                    } else if roll < 0.85 {
                        self.log(vec![("vous suivez les traces : elles se perdent dans les fourrés. rien cette fois.".into(), C::Dim)]);
                    } else {
                        let gift = 1 + (rand::thread_rng().gen_range(0..2)) as u64;
                        let bt = [BAIT_VIANDE, BAIT_BAIES][rand::thread_rng().gen_range(0..2)];
                        self.s.baits[bt] += gift;
                        self.log(vec![(format!("au bout de la piste, une cache abandonnée : {}× {}.", gift, BAITS[bt].n), C::Gold)]);
                        self.toast(format!("trouvaille : {}× {}", gift, BAITS[bt].n));
                    }
                }
            }
            Action::Nothing => {}
        }
    }
    /* stock livrable : la réserve moins le meilleur couple ♂♀, intouchable */
    fn deliverable(&self, ci: usize) -> u64 {
        let iv = &self.s.inv2[ci];
        let reserved = (iv.tm() > 0) as u64 + (iv.tf() > 0) as u64;
        iv.tn().saturating_sub(reserved)
    }
    /* quantité encore due aux commandes ouvertes pour cette espèce */
    /* ce qu'il faut garder de côté : les commandes du comptoir ET les
       demandes du troc encore ouvertes — sinon l'auto-vente écoule sous vos
       pieds les spécimens qu'un collectionneur attend */
    fn contract_need(&self, ci: usize) -> u64 {
        let (_, cs) = self.contracts_now();
        let mut n = 0;
        for (i, c) in cs.iter().enumerate() {
            if c.ci == ci && !self.s.contracts_done.get(i).copied().unwrap_or(false) {
                n += c.qty;
            }
        }
        n + self.trade_need(ci)
    }
    fn trade_need(&self, ci: usize) -> u64 {
        self.s
            .trades
            .iter()
            .enumerate()
            .filter(|(i, &(want, _, _))| want == ci && !self.s.trades_done.get(*i).copied().unwrap_or(false))
            .map(|(_, &(_, qty, _))| qty)
            .sum()
    }
    /* vend le surplus au-delà du meilleur couple ♂♀ ET des commandes en cours */
    fn sell_surplus(&mut self, ci: usize) -> (u64, f64) {
        let need = self.contract_need(ci);
        /* on met de côté les meilleurs couples avant de solder le reste, puis
           on les remet : c'est le stock de l'enclos */
        let mut mis: Vec<(u8, usize)> = vec![];
        for sexe in 0..2u8 {
            for _ in 0..self.s.autokeep.max(1) {
                if let Some(r) = self.take_best_sex(ci, false, sexe) {
                    mis.push((sexe, r));
                }
            }
        }
        let q = self.s.inv2[ci].tn().saturating_sub(need);
        let v = self.take_lowest(ci, false, q);
        for (sexe, r) in mis {
            self.give_back(ci, false, sexe, r);
        }
        (q, v)
    }
    /* le seuil au-delà duquel une espèce a du surplus */
    fn seuil_garde(&self, ci: usize) -> u64 {
        2 * self.s.autokeep.max(1) as u64 + self.contract_need(ci)
    }
    /* les commandes sont générées au début de chaque créneau de 2 h et figées en
       sauvegarde : trois espèces DISTINCTES, choisies parmi celles que le joueur
       a déjà découvertes (repli sur les communs des biomes débloqués en tout
       début de partie) — jamais une espèce inconnue ni d'un biome verrouillé */
    /* les offres de troc tiennent une journée ; en jour de foire, elles doublent */
    /* les passages du marchand : trois à cinq par jour, à des heures tirées
       au sort et pour deux à quatre heures — jamais aux mêmes créneaux d'un
       jour sur l'autre, mais reproductible (le jeu doit pouvoir rejouer une
       absence hors-ligne à l'identique). */
    const MERCHANT_POS: (usize, usize) = (66, 38);
    fn merchant_visits(jour: u64) -> Vec<(f64, f64)> {
        let mut rng = StdRng::seed_from_u64(splitmix(jour ^ 0x1A2B_3C4D));
        let n = 3 + rng.gen_range(0..3u32); // 3, 4 ou 5 passages
        let tranche = 86_400_000.0 / n as f64;
        (0..n)
            .map(|i| {
                let debut = jour as f64 * 86_400_000.0 + i as f64 * tranche + rng.gen::<f64>() * tranche * 0.55;
                let duree = 2.0 * 3_600_000.0 + rng.gen::<f64>() * 2.0 * 3_600_000.0;
                (debut, debut + duree)
            })
            .collect()
    }
    /* renvoie (identifiant du passage, instant du départ) */
    fn merchant_now(&self) -> Option<(u64, f64)> {
        let now = now_ms();
        let jour = (now / 86_400_000.0) as u64;
        if fair_day() {
            // jour de foire : il tient boutique du matin au soir
            return Some((jour * 8 + 7, (jour + 1) as f64 * 86_400_000.0));
        }
        for d in [jour.saturating_sub(1), jour] {
            for (i, &(a, b)) in Self::merchant_visits(d).iter().enumerate() {
                if now >= a && now < b {
                    return Some((d * 8 + i as u64, b));
                }
            }
        }
        None
    }
    /* prochain passage, pour ne jamais laisser le joueur sans repère */
    fn merchant_next(&self) -> Option<f64> {
        let now = now_ms();
        let jour = (now / 86_400_000.0) as u64;
        (0..3)
            .flat_map(|k| Self::merchant_visits(jour + k))
            .map(|(a, _)| a)
            .filter(|&a| a > now)
            .fold(None, |acc: Option<f64>, a| Some(acc.map_or(a, |m| m.min(a))))
    }
    fn merchant_stock(&self, w: u64) -> Vec<usize> {
        let mut rng = StdRng::seed_from_u64(splitmix(w ^ 0x5EED_1234));
        let mut all: Vec<usize> = (0..MERCH_ITEMS).collect();
        let mut out = vec![];
        for _ in 0..3.min(all.len()) {
            out.push(all.remove(rng.gen_range(0..all.len())));
        }
        out.sort();
        out
    }
    fn merchant_price(&self, item: usize) -> f64 {
        let fair = if fair_day() { 0.75 } else { 1.0 };
        let p = match item {
            0 => 180_000.0,                               // œuf de curiosité
            1 => 60_000.0 * (1.0 + self.s.charms as f64), // breloque
            2 => BAITS[self.best_bait()].cost * 10.0 * 0.6, // lot d'appâts
            /* la licence doit rester une affaire face au labo, qui suit une
               progression géométrique : on s'adosse à son prochain palier.
               une fois le labo épuisé, elle devient le seul moyen d'aller
               plus loin — et se paie en conséquence. */
            3 => {
                if self.s.lab[LAB_LICENCE] < LABS[LAB_LICENCE].max {
                    self.lab_cost(LAB_LICENCE) * 0.6
                } else {
                    400_000.0 * 2f64.powi(self.s.licences as i32)
                }
            }
            // un piège d'occasion : la moitié du prix neuf, comme annoncé
            _ => TRAPS[self.next_trap()].cost * 0.5,
        };
        p * fair
    }
    fn best_bait(&self) -> usize {
        (0..BAITS.len()).rev().find(|&b| self.s.ecus >= BAITS[b].cost * 10.0).unwrap_or(0)
    }

    fn gen_trades(&self, w: u64) -> Vec<(usize, u64, usize)> {
        let mut rng = StdRng::seed_from_u64(splitmix(w ^ 0x7B0C_7B0C));
        let mut cand: Vec<usize> = wild_species()
            .filter(|&i| self.s.dex2[i].n > 0 && self.s.biomes[CREATURES[i].b].is_some())
            .collect();
        if cand.len() < 2 {
            return vec![]; // trop tôt : personne ne troque contre rien
        }
        let curios: Vec<usize> = (0..CREATURES.len()).filter(|&i| CREATURES[i].b == CURIO_B).collect();
        /* prix d'une curiosité, en « points de prise » — la valeur de base
           d'un spécimen sert d'unité, si bien qu'un rare coûte moins de pièces
           qu'un commun */
        const PTS: [f64; 5] = [0.0, 120.0, 320.0, 900.0, 2600.0];
        const VAL: [f64; 5] = [3.0, 9.0, 30.0, 150.0, 900.0];
        let n = if fair_day() { 6 } else { 3 };
        let mut out: Vec<(usize, u64, usize)> = vec![];
        let mut tries = 0;
        while out.len() < n && tries < 80 {
            tries += 1;
            let give = curios[rng.gen_range(0..curios.len())];
            /* on demande une espèce d'un calibre voisin : sinon une curiosité
               rare se paie en centaines de communs, et tout finit au plafond */
            let proches: Vec<usize> = cand
                .iter()
                .copied()
                .filter(|&i| CREATURES[i].r + 1 >= CREATURES[give].r)
                .collect();
            let pool = if proches.is_empty() { &cand } else { &proches };
            let want = pool[rng.gen_range(0..pool.len())];
            if out.iter().any(|&(w2, _, g2)| w2 == want || g2 == give) {
                continue;
            }
            let qty = (PTS[CREATURES[give].r] / VAL[CREATURES[want].r]).ceil().clamp(2.0, 40.0) as u64;
            out.push((want, qty, give));
        }
        out
    }

    fn gen_contracts(&self, w: u64) -> Vec<(usize, u64, f64)> {
        let mut rng = StdRng::seed_from_u64(splitmix(w ^ 0xC0117AC7));
        let mut cand: Vec<usize> = (0..CREATURES.len())
            .filter(|&i| self.s.biomes[CREATURES[i].b].is_some() && CREATURES[i].r <= 2 && self.s.dex2[i].n > 0)
            .collect();
        if cand.len() < 3 {
            cand = (0..CREATURES.len())
                .filter(|&i| self.s.biomes[CREATURES[i].b].is_some() && CREATURES[i].r == 0)
                .collect();
        }
        let mut out: Vec<(usize, u64, f64)> = vec![];
        for _ in 0..3 {
            let mut ci = cand[rng.gen_range(0..cand.len())];
            for _essai in 0..10 {
                if !out.iter().any(|c| c.0 == ci) {
                    break;
                }
                ci = cand[rng.gen_range(0..cand.len())];
            }
            let qty = match CREATURES[ci].r {
                0 => rng.gen_range(4..=8),
                1 => rng.gen_range(3..=5),
                _ => rng.gen_range(2..=3),
            };
            let reward = ((qty as f64 * self.creature_value(ci, false) * 1.8 + 25.0) * (1.0 + self.s.lab[LAB_COURTAGE] as f64 * 0.15)).floor();
            out.push((ci, qty, reward));
        }
        out
    }
    fn contracts_now(&self) -> (u64, Vec<Contract>) {
        let w = (now_ms() / 7_200_000.0) as u64;
        let list = self.s.contracts.iter().map(|&(ci, qty, reward)| Contract { ci, qty, reward }).collect();
        (w, list)
    }
    /* traces fraîches : par fenêtre de 10 min, ~1 biome débloqué sur 4 en porte */
    /* ramène un point d'apparition sur une case où l'on peut poser le pied :
       depuis que l'eau ne se traverse plus, une trace ou une légende tirée au
       milieu du lac serait à jamais hors d'atteinte. */
    fn spot_foulable(&self, x: usize, y: usize) -> (usize, usize) {
        if !self.world.solid(x as i32, y as i32) {
            return (x, y);
        }
        for r in 1..14i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue; // seulement le pourtour du carré
                    }
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 2 || ny < 2 || nx >= MAPW as i32 - 2 || ny >= MAPH as i32 - 2 {
                        continue;
                    }
                    if !self.world.solid(nx, ny) {
                        return (nx as usize, ny as usize);
                    }
                }
            }
        }
        (x, y)
    }

    fn traces_now(&self) -> Vec<(u64, usize, (usize, usize))> {
        let w = (now_ms() / 600_000.0) as u64;
        let mut out = vec![];
        for b in 0..BIOMES.len() {
            if self.s.biomes[b].is_none() {
                continue;
            }
            if splitmix(w ^ (b as u64) ^ 0x7124CE5) % 100 >= 22 {
                continue;
            }
            let key = w * 16 + b as u64;
            if self.s.traces_done.contains(&key) {
                continue;
            }
            let (lx, ly) = LEGEND_SPOTS[b];
            let offs: [(i32, i32); 3] = [(-6, 3), (5, -2), (-2, 6)];
            let (dx, dy) = offs[(splitmix(w ^ 0xF00 ^ b as u64) % 3) as usize];
            let x = (lx as i32 + dx).clamp(2, MAPW as i32 - 3) as usize;
            let y = (ly as i32 + dy).clamp(2, MAPH as i32 - 3) as usize;
            out.push((key, b, self.spot_foulable(x, y)));
        }
        out
    }
    /* légende errante : fenêtre de 30 min, 30% de chance, position fixe par biome */
    /* battues encore actives : chacune tient une heure */
    fn hunts_actives(&self, at: f64) -> usize {
        self.s.hunts_at.iter().filter(|&&t| at - t < HUNT_BUFF_MS).count()
    }
    /* chance qu'une légende paraisse à un tirage (toutes les 10 min), pour
       mille : 100 de base, la pression des battues, l'appel du labo. */
    fn legend_chance(&self, at: f64) -> u64 {
        let pression = (self.hunts_actives(at) as u64 * HUNT_BUFF_PM).min(HUNT_BUFF_MAX_PM);
        let appel = self.s.lab[LAB_APPEL] as u64 * LAB_APPEL_PM;
        (LEGEND_BASE_PM + pression + appel).min(LEGEND_MAX_PM)
    }
    /* la chance d'une approche à mains nues, appâts non compris */
    fn legend_take_chance(&self) -> f64 {
        0.25 + (self.global_luck() * 0.05).min(0.15) + self.s.lab[LAB_APPROCHE] as f64 * 0.05
    }
    /* la silhouette encore présente, s'il y en a une : on remonte les tirages
       de la vingtaine de minutes écoulée et on retient le plus ancien encore
       vivant, celui qu'il reste le moins de temps pour rejoindre. */
    fn legend_now(&self) -> Option<(u64, usize, (usize, usize))> {
        let now = now_ms();
        let w = (now / LEGEND_WINDOW_MS) as u64;
        let seuil = self.legend_chance(now);
        for k in (0..LEGEND_LINGER).rev() {
            let Some(cand) = w.checked_sub(k) else { continue };
            if self.s.legends_tried.contains(&cand) {
                continue;
            }
            /* un tirage déjà sorti le reste : la silhouette ne s'évanouit pas
               parce qu'une battue vient d'expirer */
            let tirage = splitmix(cand ^ 0x1E9E17D) % 1000;
            if tirage >= seuil && !self.s.legends_open.contains(&cand) {
                continue;
            }
            /* les légendes ne connaissent pas les frontières : elles paraissent
               aussi sur les terres qu'on n'a pas encore ouvertes */
            let b = (splitmix(cand ^ 0xB10) % WILDB as u64) as usize;
            let (lx, ly) = LEGEND_SPOTS[b];
            return Some((cand, b, self.spot_foulable(lx, ly)));
        }
        None
    }
    /* minutes restantes avant que la silhouette du tirage w s'en aille */
    fn legend_left_min(&self, w: u64, now: f64) -> u64 {
        ((((w + LEGEND_LINGER) as f64 * LEGEND_WINDOW_MS) - now) / 60_000.0).ceil().max(0.0) as u64
    }

    fn interact(&mut self) {
        if let Some((w, b, (lx, ly))) = self.legend_now() {
            if (lx as i32 - self.px).abs() <= 1 && (ly as i32 - self.py).abs() <= 1 {
                self.panels.push(Panel::new(PanelKind::Legend(b, w)));
                return;
            }
        }
        for (key, b, (tx, ty)) in self.traces_now() {
            if (tx as i32 - self.px).abs() <= 1 && (ty as i32 - self.py).abs() <= 1 {
                self.apply(Action::TraceFollow(key, b));
                return;
            }
        }
        {
            /* la fontaine occupe x 53-55, y 38-39 : on l'aborde par la place */
            let (fx, fy) = (54_i32, 38_i32);
            if (fx - self.px).abs() <= 2 && (fy - self.py).abs() <= 2 {
                if puits_assoiffe(now_ms()) {
                    self.panels.push(Panel::new(PanelKind::Puits));
                } else {
                    self.log(vec![("l'eau du puits est noire et immobile. rien ne s'y reflète.".into(), C::Dimmer)]);
                    self.toast("le puits dort");
                }
                return;
            }
        }
        {
            let (mx, my) = Self::MERCHANT_POS;
            if (mx as i32 - self.px).abs() <= 6 && (my as i32 - self.py).abs() <= 1 {
                self.panels.push(Panel::new(PanelKind::Merchant));
                return;
            }
        }
        let Some(z) = self.world.zone_at(self.px as usize, self.py as usize) else { return };
        let kind = match z {
            Zone::Biome(b) => {
                if self.s.biomes[b].is_none() {
                    PanelKind::Unlock(b)
                } else {
                    PanelKind::Biome(b)
                }
            }
            Zone::Boutique => PanelKind::Shop,
            Zone::Troc => PanelKind::Trade,
            Zone::Labo => PanelKind::Lab,
            Zone::Bestiaire => PanelKind::Dex,
            Zone::Succes => PanelKind::Achs,
            Zone::Musee => PanelKind::Museum,
            Zone::Enclos => PanelKind::Pens,
        };
        self.panels.push(Panel::new(kind));
    }

    fn zone_hint(&self) -> (String, C) {
        if let Some((_, _, (lx, ly))) = self.legend_now() {
            if (lx as i32 - self.px).abs() <= 1 && (ly as i32 - self.py).abs() <= 1 {
                return ("une silhouette étrange rôde — Entrée : l'approcher".into(), C::Gold);
            }
        }
        for (_, _, (tx, ty)) in self.traces_now() {
            if (tx as i32 - self.px).abs() <= 1 && (ty as i32 - self.py).abs() <= 1 {
                return ("des traces fraîches — Entrée : les suivre".into(), C::Gold);
            }
        }
        {
            let (mx, my) = Self::MERCHANT_POS;
            if (mx as i32 - self.px).abs() <= 6 && (my as i32 - self.py).abs() <= 1 {
                return if self.merchant_now().is_some() {
                    ("marchand ambulant — Entrée : voir sa malle".into(), C::Gold)
                } else {
                    ("étal du marchand, désert — Entrée : savoir quand il repasse".into(), C::Dimmer)
                };
            }
        }
        match self.world.zone_at(self.px as usize, self.py as usize) {
            None => ("promenez-vous · Entrée pour interagir près d'un lieu".into(), C::Dimmer),
            Some(Zone::Biome(b)) => {
                if self.s.biomes[b].is_none() {
                    (format!("{} · verrouillé — Entrée : débloquer pour {} écus", BIOMES[b].name, fmt(BIOMES[b].cost)), C::Red)
                } else {
                    let bs = self.s.biomes[b].as_ref().unwrap();
                    let placed = bs.pl.iter().flatten().count();
                    (format!("{} · emplacements {}/{} — Entrée : gérer", BIOMES[b].name, placed, bs.slots), C::Green)
                }
            }
            Some(Zone::Boutique) => ("boutique — Entrée : acheter et vendre".into(), C::Gold),
            Some(Zone::Labo) => ("labo — Entrée : recherches et migration".into(), C::Gold),
            Some(Zone::Bestiaire) => ("bestiaire — Entrée : consulter".into(), C::Gold),
            Some(Zone::Succes) => ("trophées — Entrée : consulter".into(), C::Gold),
            Some(Zone::Musee) => ("musée — Entrée : exposer vos plus belles prises".into(), C::Gold),
            Some(Zone::Enclos) => ("enclos — Entrée : faire reproduire vos créatures".into(), C::Gold),
            Some(Zone::Troc) => ("comptoir de troc — Entrée : échanger des doublons contre des curiosités".into(), C::Gold),
        }
    }

    /* ---------------------------------------------- construction panneaux */

    fn build_rows(&self, kind: &PanelKind) -> (String, Vec<Row>) {
        let (title, rows) = self.build_rows_raw(kind);
        // filet de sécurité global : aucune ligne ne peut déborder du panneau.
        // les lignes trop larges sont repliées (coupure aux espaces), les boutons
        // passent à la ligne sans être coupés, les filets d'en-tête sont raccourcis.
        (title, reflow_rows(rows, self.panel_w))
    }
    fn build_rows_raw(&self, kind: &PanelKind) -> (String, Vec<Row>) {
        match kind {
            PanelKind::Dashboard => self.rows_dashboard(),
            PanelKind::Inventory => self.rows_inventory(),
            PanelKind::Contracts => self.rows_contracts(),
            PanelKind::Museum => self.rows_museum(),
            PanelKind::MuseumPick(slot) => self.rows_museum_pick(*slot),
            PanelKind::Pens => self.rows_pens(),
            PanelKind::PenPick(slot) => self.rows_pen_pick(*slot),
            PanelKind::Legend(b, w) => self.rows_legend(*b, *w),
            PanelKind::Biome(b) => self.rows_biome(*b),
            PanelKind::TrapPick(b, i) => self.rows_trap_pick(*b, *i),
            PanelKind::BaitPick(b, i) => self.rows_bait_pick(*b, *i),
            PanelKind::Unlock(b) => self.rows_unlock(*b),
            PanelKind::Shop => self.rows_shop(),
            PanelKind::Lab => self.rows_lab(),
            PanelKind::MigrConfirm => self.rows_migr_confirm(),
            PanelKind::Dex => self.rows_dex(),
            PanelKind::Creature(ci) => self.rows_creature(*ci),
            PanelKind::Achs => self.rows_achs(),
            PanelKind::Help => self.rows_help(),
            PanelKind::Journal => self.rows_journal(),
            PanelKind::Board => self.rows_board(),
            PanelKind::News => self.rows_news(),
            PanelKind::Puits => self.rows_puits(),
            PanelKind::Trade => self.rows_trade(),
            PanelKind::Merchant => self.rows_merchant(),
            PanelKind::Offline(sum) => self.rows_offline(sum),
            PanelKind::ResetConfirm => self.rows_reset(),
        }
    }

    fn rows_inventory(&self) -> (String, Vec<Row>) {
        let mut rows = vec![Row::header("pièges")];
        let mut any = false;
        for t in 0..6 {
            let owned = self.s.traps[t];
            if owned == 0 {
                continue;
            }
            any = true;
            let placed = self.placed_count(t);
            rows.push(Row::text(
                format!("├─ {} ×{} — {} posé{}, {} en réserve", pad(TRAPS[t].n, 18), owned, placed, if placed > 1 { "s" } else { "" }, owned - placed),
                C::Text,
            ));
        }
        if !any {
            rows.push(Row::text("aucun piège.", C::Dimmer));
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("appâts"));
        any = false;
        for bt in 0..5 {
            if self.s.baits[bt] > 0 {
                any = true;
                rows.push(Row::text(format!("├─ {} ×{}", pad(BAITS[bt].n, 18), fmt(self.s.baits[bt] as f64)), C::Text));
            }
        }
        if !any {
            rows.push(Row::text("aucun appât.", C::Dimmer));
        }
        for b in 0..BIOMES.len() {
            let list: Vec<usize> = biome_creatures(b).filter(|&ci| self.s.inv2[ci].tn() + self.s.inv2[ci].ts() > 0).collect();
            if list.is_empty() {
                continue;
            }
            rows.push(Row::text("", C::Dim));
            rows.push(Row::header(&format!("créatures — {}", BIOMES[b].name)));
            for ci in list {
                let c = &CREATURES[ci];
                let iv = &self.s.inv2[ci];
                let mut per_rank = String::new();
                for r in (0..4).rev() {
                    if iv.nr(r) > 0 {
                        per_rank += &format!("{}:{} ", RANK_NAMES[r], iv.nr(r));
                    }
                }
                for r in (0..4).rev() {
                    if iv.sr(r) > 0 {
                        per_rank += &format!("⋆{}:{} ", RANK_NAMES[r], iv.sr(r));
                    }
                }
                rows.push(Row {
                    segs: vec![
                        ("├─ ".into(), C::Dimmer),
                        (pad(&format!("{} {}", c.g, c.n), 26), rarity_color(c.r)),
                        (pad(&format!("×{}", iv.tn() + iv.ts()), 5), C::Dim),
                        (pad(&format!("♂{} ♀{}", iv.tm(), iv.tf()), 9), C::Blue),
                        (per_rank, C::GoldDark),
                    ],
                    btns: vec![],
                    act: Some(Action::Open(PanelKind::Creature(ci))),
                    indent: 0,
                });
            }
        }
        ("inventaire".into(), rows)
    }

    /* valeur, à la boutique, des spécimens qu'une livraison consommerait :
       ce sont les plus bas rangs disponibles, dans l'ordre où take_lowest les
       prendrait. sert à dire au joueur si la prime vaut le coup. */
    fn valeur_a_la_vente(&self, ci: usize, qty: u64) -> f64 {
        let iv = &self.s.inv2[ci];
        let (mut reste, mut total) = (qty, 0.0);
        for r in 0..4 {
            if reste == 0 {
                break;
            }
            let dispo = iv.nr(r).min(reste);
            total += dispo as f64 * self.creature_value_r(ci, false, r);
            reste -= dispo;
        }
        total + reste as f64 * self.creature_value_r(ci, false, 0)
    }

    fn rows_contracts(&self) -> (String, Vec<Row>) {
        let (w, contracts) = self.contracts_now();
        let left_ms = ((w + 1) as f64 * 7_200_000.0) - now_ms();
        let mut rows = vec![
            Row::text("le comptoir affiche trois commandes ; livrez depuis votre réserve.", C::Dim),
            Row::text("jamais les shinies, jamais votre meilleur couple ♂♀ (« livrables » = stock hors couple).", C::Dimmer),
            Row::text("le ×N compare la prime à ce que rapporteraient les mêmes bêtes à la boutique. la livraison prend vos plus bas rangs : si vous n'avez que du beau, le rapport tombe sous 1.", C::Dimmer),
            Row::text(format!("renouvellement dans {} min", (left_ms / 60_000.0).ceil() as u64), C::Dimmer),
            Row::text("", C::Dim),
        ];
        for (i, c) in contracts.iter().enumerate() {
            let done = self.s.contracts_done.get(i).copied().unwrap_or(false);
            let have = self.deliverable(c.ci);
            let cr = &CREATURES[c.ci];
            if done {
                rows.push(Row {
                    segs: vec![
                        ("■ ".into(), C::Green),
                        (pad(&format!("{}× {}", c.qty, cr.n), 28), C::Dimmer),
                        ("livré".into(), C::Green),
                    ],
                    btns: vec![],
                    act: None,
                    indent: 0,
                });
            } else {
                let ok = have >= c.qty;
                rows.push(Row {
                    segs: vec![
                        ("□ ".into(), C::Dim),
                        (pad(&format!("{}× {}", c.qty, cr.n), 28), rarity_color(cr.r)),
                        (pad(&format!("(livrables : {})", have), 18), if ok { C::Green } else { C::Red }),
                        (pad(&format!("+{} écus", fmt(c.reward)), 16), C::GoldDark),
                        {
                            let gain = c.reward / self.valeur_a_la_vente(c.ci, c.qty).max(1.0);
                            (format!("×{}", fmt2(gain)), if gain >= 1.0 { C::Green } else { C::Red })
                        },
                    ],
                    btns: vec![("livrer".into(), if ok { C::Green } else { C::Dimmer }, Action::Deliver(i))],
                    act: None,
                    indent: 0,
                });
            }
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::text(format!("contrats livrés au total : {}", self.s.contracts_delivered), C::Dimmer));
        ("contrats".into(), rows)
    }

    /* le puits : une offrande par jour réel, et ce qu'il rend tient à la rareté de
       ce qu'on lui donne. le bestiaire, lui, garde la découverte. */
    fn well_used_tonight(&self) -> bool {
        self.s.well_night == jour_reel(now_ms())
    }
    /* la margelle ne bat que s'il reste quelque chose à donner */
    fn puits_luit(&self, ms: f64) -> bool {
        puits_assoiffe(ms) && self.s.well_night != jour_reel(ms)
    }
    fn rows_puits(&self) -> (String, Vec<Row>) {
        let mut rows = wrap_rows(
            "le puits luit d'une lueur rouge. l'eau a reculé, la margelle est tiède. il semble avoir soif — soif de sang.",
            self.panel_w,
            C::Red,
        );
        rows.push(Row::text("", C::Dim));
        if self.well_used_tonight() {
            rows.push(Row::text("la lueur faiblit : il a eu son compte pour aujourd'hui.", C::Dimmer));
            rows.push(Row::text("", C::Dim));
            rows.push(Row {
                segs: vec![],
                btns: vec![("s'éloigner".into(), C::Dim, Action::Close)],
                act: None,
                indent: 0,
            });
            return ("le puits".into(), rows);
        }
        rows.push(Row::text("une offrande par jour. le plus bas rang de l'espèce part, jamais un shiny.", C::Dimmer));
        rows.push(Row::text("plus la bête est rare, plus ce qui remonte a de la valeur.", C::Dimmer));
        rows.push(Row::text("", C::Dim));
        let mut any = false;
        for ci in 0..CREATURES.len() {
            if self.s.inv2[ci].tn() == 0 {
                continue;
            }
            any = true;
            let c = &CREATURES[ci];
            let rang = (0..4).find(|&r| self.s.inv2[ci].m[r] + self.s.inv2[ci].f[r] > 0).unwrap_or(0);
            rows.push(Row {
                segs: vec![
                    (pad(&format!("{} {}", c.g, c.n), 26), rarity_color(c.r)),
                    (pad(&format!("[{}] ×{}", RANK_NAMES[rang], fmt(self.s.inv2[ci].tn() as f64)), 12), C::Dimmer),
                    (pad(RAR_LABEL[c.r], 12), C::Dimmer),
                ],
                btns: vec![("sacrifier".into(), C::Red, Action::Sacrifice(ci))],
                act: None,
                indent: 0,
            });
        }
        if !any {
            rows.push(Row::text("votre réserve est vide. le puits attendra.", C::Dim));
        }
        ("le puits".into(), rows)
    }

    /* ce que le puits rend. l'échelle suit la rareté de l'offrande, et le rang
       pousse un peu le tirage : une bête commune paie en appâts, une légendaire
       en breloque. une seule offrande par nuit, donc la main peut être large
       sans devenir une source d'écus. */
    fn recompense_du_puits(&mut self, ci: usize, rang: usize) {
        let r = CREATURES[ci].r;
        /* ce qui dort au fond remonte rarement, et d'autant plus volontiers
           que l'offrande était belle. il prend alors toute la place : c'est la
           seule façon de l'obtenir. */
        let chance_puisard = match r {
            4 => 0.08,
            3 => 0.03,
            2 => 0.01,
            _ => 0.003,
        };
        if rand::thread_rng().gen::<f64>() < chance_puisard {
            let ci_puits = (0..CREATURES.len()).find(|&i| CREATURES[i].b == PUITS_B).unwrap();
            let rank = self.roll_rank(self.global_luck() + 0.5).max(2);
            let shiny = rand::thread_rng().gen::<f64>() < (SHINY_BASE * 8.0).min(0.05);
            self.add_specimen(ci_puits, shiny, rank);
            self.log(vec![
                ("la corde se tend toute seule. ce qui remonte n'est pas ce que vous avez donné : ".into(), C::Red),
                (format!("{}{} [{}]", CREATURES[ci_puits].n, if shiny { " ⋆" } else { "" }, RANK_NAMES[rank]),
                 if shiny { C::Shiny } else { C::Gold }),
            ]);
            self.toast("quelque chose est remonté");
            return;
        }
        let mut tir = rand::thread_rng().gen::<f64>() + rang as f64 * 0.08;
        tir = tir.min(0.999);
        let valeur = self.creature_value_r(ci, false, rang);
        match r {
            0 | 1 => {
                if tir < 0.35 {
                    self.log(vec![("rien ne remonte. l'eau reste noire.".into(), C::Dimmer)]);
                    self.toast("le puits garde tout");
                } else if tir < 0.85 {
                    let bt = if r == 0 { 0 } else { BAIT_VIANDE };
                    let n = 2 + (tir * 4.0) as u64;
                    self.s.baits[bt] += n;
                    self.log(vec![(format!("{} {} remontent, encore humides.", n, BAITS[bt].n), C::Green)]);
                    self.toast("le puits rend des appâts");
                } else {
                    let g = (valeur * 12.0).floor().max(50.0);
                    self.gain(g);
                    self.log(vec![("de la monnaie ancienne flotte à la surface : ".into(), C::Gold), (format!("+{} écus", fmt(g)), C::GoldDark)]);
                    self.toast("le puits rend de la monnaie");
                }
            }
            2 => {
                if tir < 0.55 {
                    let g = (valeur * 18.0).floor().max(500.0);
                    self.gain(g);
                    self.log(vec![("une bourse noircie remonte : ".into(), C::Gold), (format!("+{} écus", fmt(g)), C::GoldDark)]);
                    self.toast("le puits paie");
                } else {
                    let bt = BAIT_TRUFFE;
                    self.s.baits[bt] += 3;
                    self.log(vec![(format!("3 {} reposent sur la margelle, sans explication.", BAITS[bt].n), C::Green)]);
                    self.toast("le puits rend des appâts rares");
                }
            }
            3 => {
                if tir < 0.30 {
                    self.s.charms += 1;
                    self.log(vec![("une breloque tiède remonte au bout de la corde. elle bat, faiblement.".into(), C::Gold)]);
                    self.toast("breloque de chance");
                } else if tir < 0.75 {
                    let g = (valeur * 20.0).floor();
                    self.gain(g);
                    self.log(vec![("le puits rend bien plus qu'il n'a pris : ".into(), C::Gold), (format!("+{} écus", fmt(g)), C::GoldDark)]);
                    self.toast("le puits paie gros");
                } else {
                    self.s.baits[BAIT_ESSENCE] += 2;
                    self.log(vec![(format!("2 {} flottent, intactes.", BAITS[BAIT_ESSENCE].n), C::Blue)]);
                    self.toast("le puits rend de l'essence");
                }
            }
            _ => {
                if tir < 0.22 {
                    self.s.licences += 1;
                    self.log(vec![("un parchemin sec remonte, scellé d'un cachet que personne ne reconnaît : une licence de piégeage.".into(), C::Gold)]);
                    self.toast("licence de piégeage");
                } else {
                    self.s.charms += 1;
                    self.log(vec![("une breloque remonte, lourde comme une dent. le puits a apprécié.".into(), C::Gold)]);
                    self.toast("breloque de chance");
                }
            }
        }
    }

    fn rows_museum(&self) -> (String, Vec<Row>) {
        let rate_min = self.museum_rate() * 60_000.0;
        let pool = self.s.museum_pool + self.museum_rate() * (now_ms() - self.s.museum_at).max(0.0);
        let mut rows = vec![
            Row::text("exposez vos plus beaux spécimens : chacun génère des écus en continu.", C::Dim),
            Row::text("le spécimen exposé quitte la réserve (récupérable à tout moment).", C::Dimmer),
            Row::text("", C::Dim),
            Row {
                segs: vec![
                    (format!("revenu : {} écus/min · cagnotte : ", fmt2(rate_min)), C::Text),
                    (format!("{} écus", fmt(pool)), C::Gold),
                    (format!("  (plafond {} h)", self.museum_cap_h() as u64), C::Dimmer),
                ],
                btns: vec![("encaisser".into(), if pool >= 1.0 { C::Gold } else { C::Dimmer }, Action::MuseumCollect)],
                act: None,
                indent: 0,
            },
            Row {
                segs: vec![(
                    if self.s.museum_auto {
                        "■ garnissage automatique : les salles suivent vos plus belles prises".into()
                    } else {
                        "□ garnissage automatique".to_string()
                    },
                    if self.s.museum_auto { C::Green } else { C::Dim },
                )],
                btns: vec![
                    (
                        if self.s.museum_auto { "désactiver".into() } else { "activer".to_string() },
                        if self.s.museum_auto { C::Red } else { C::Green },
                        Action::ToggleMuseumAuto,
                    ),
                    ("garnir maintenant".into(), C::Gold, Action::MuseumAuto),
                ],
                act: None,
                indent: 0,
            },
            Row::text("", C::Dim),
        ];
        for slot in 0..self.museum_slots() {
            match &self.s.museum[slot] {
                None => rows.push(Row {
                    segs: vec![(format!("├─ salle {} : ", slot + 1), C::Dimmer), ("vide".into(), C::Dim)],
                    btns: if self.s.museum_auto {
                        vec![]
                    } else {
                        vec![("exposer une créature".into(), C::Green, Action::Open(PanelKind::MuseumPick(slot)))]
                    },
                    act: None,
                    indent: 0,
                }),
                Some(m) => {
                    let c = &CREATURES[m.ci];
                    rows.push(Row {
                        segs: vec![
                            (format!("├─ salle {} : ", slot + 1), C::Dimmer),
                            (format!("{} {}{} [{}]", c.g, c.n, if m.shiny { " ⋆" } else { "" }, RANK_NAMES[m.rank]),
                             if m.shiny { C::Shiny } else { rarity_color(c.r) }),
                            (format!("  {} écus/min", fmt2(self.creature_value_r(m.ci, m.shiny, m.rank) * 0.001)), C::GoldDark),
                        ],
                        btns: if self.s.museum_auto {
                            vec![]
                        } else {
                            vec![("retirer".into(), C::Red, Action::MuseumRemove(slot))]
                        },
                        act: None,
                        indent: 0,
                    });
                }
            }
        }
        ("musée".into(), rows)
    }

    fn rows_museum_pick(&self, slot: usize) -> (String, Vec<Row>) {
        let mut rows = vec![Row::text("le meilleur spécimen disponible de l'espèce sera exposé.", C::Dimmer), Row::text("", C::Dim)];
        let mut any = false;
        for ci in 0..CREATURES.len() {
            let iv = &self.s.inv2[ci];
            if iv.tn() == 0 && iv.ts() == 0 {
                continue;
            }
            any = true;
            let c = &CREATURES[ci];
            let best_n = (0..4).rev().find(|&r| iv.nr(r) > 0);
            let best_s = (0..4).rev().find(|&r| iv.sr(r) > 0);
            let mut btns = vec![];
            if let Some(r) = best_n {
                btns.push((format!("exposer [{}]", RANK_NAMES[r]), C::Green, Action::MuseumAdd(slot, ci, false)));
            }
            if let Some(r) = best_s {
                btns.push((format!("exposer ⋆ [{}]", RANK_NAMES[r]), C::Blue, Action::MuseumAdd(slot, ci, true)));
            }
            rows.push(Row {
                segs: vec![(pad(&format!("{} {}", c.g, c.n), 28), rarity_color(c.r))],
                btns,
                act: None,
                indent: 0,
            });
        }
        if !any {
            rows.push(Row::text("réserve vide.", C::Dim));
        }
        (format!("musée · salle {}", slot + 1), rows)
    }

    fn rows_pens(&self) -> (String, Vec<Row>) {
        let now = now_ms();
        let mut rows = vec![
            Row::text("un couple (♂ + ♀) d'une même espèce donne une naissance après un temps", C::Dim),
            Row::text("de couvaison. le petit peut monter en rang, et shiny ×3.", C::Dim),
            Row::text("les parents sont consommés : par défaut les plus bas rangs, ou le rang de votre choix.", C::Dimmer),
            Row::text("", C::Dim),
        ];
        for slot in 0..self.pen_slots() {
            match &self.s.pens[slot] {
                None => rows.push(Row {
                    segs: vec![(format!("├─ enclos {} : ", slot + 1), C::Dimmer), ("libre".into(), C::Dim)],
                    btns: vec![("installer un couple".into(), C::Green, Action::Open(PanelKind::PenPick(slot)))],
                    act: None,
                    indent: 0,
                }),
                Some(p) => {
                    let c = &CREATURES[p.ci];
                    if p.ready_at <= now {
                        rows.push(Row {
                            segs: vec![
                                (format!("├─ enclos {} : ", slot + 1), C::Dimmer),
                                (format!("{} {} — ", c.g, c.n), rarity_color(c.r)),
                                ("une naissance vous attend !".into(), C::Gold),
                            ],
                            btns: vec![("récupérer".into(), C::Gold, Action::PenCollect(slot))],
                            act: None,
                            indent: 0,
                        });
                    } else {
                        let left = ((p.ready_at - now) / 60_000.0).ceil() as u64;
                        rows.push(Row {
                            segs: vec![
                                (format!("├─ enclos {} : ", slot + 1), C::Dimmer),
                                (format!("{} {} ♂[{}]+♀[{}]", c.g, c.n, RANK_NAMES[p.r1], RANK_NAMES[p.r2]), rarity_color(c.r)),
                                (format!(" — naissance dans {} min", left), C::Dim),
                            ],
                            btns: vec![],
                            act: None,
                            indent: 0,
                        });
                    }
                }
            }
        }
        ("enclos".into(), rows)
    }

    /* ce qu'un couple donnerait : le rang du petit, et s'il ferait progresser
       le registre. c'est ce qui sert à trier le sélecteur. */
    fn interet_couple(&self, ci: usize, rang: usize) -> (bool, usize) {
        let vise = (rang + 1).min(3); // avec la montée de rang
        let record = self.s.dex2[ci].best as usize; // 0 = jamais vue, sinon rang+1
        (record == 0 || vise + 1 > record, rang)
    }
    /* le meilleur couple disponible : d'abord ce qui ferait progresser le
       registre, puis le rang, puis la rareté. */
    fn meilleur_couple(&self) -> Option<(usize, usize)> {
        let mut best: Option<(bool, usize, usize, usize, usize)> = None;
        for ci in 0..CREATURES.len() {
            let iv = &self.s.inv2[ci];
            for r in (0..4).rev() {
                if iv.m[r] >= 1 && iv.f[r] >= 1 {
                    let (progres, rang) = self.interet_couple(ci, r);
                    let cle = (progres, rang, CREATURES[ci].r, usize::MAX - ci, ci);
                    if best.map(|b| cle > (b.0, b.1, b.2, b.3, b.4)).unwrap_or(true) {
                        best = Some(cle);
                    }
                    break; // le plus haut rang de l'espèce suffit
                }
            }
        }
        best.map(|b| (b.4, b.1))
    }

    fn rows_pen_pick(&self, slot: usize) -> (String, Vec<Row>) {
        let mut rows = vec![
            Row::text("il faut un couple : au moins un mâle et une femelle de l'espèce.", C::Dimmer),
            Row::text("les shinies ne se reproduisent pas (trop précieux, trop susceptibles).", C::Dimmer),
            Row::text("le petit naît au meilleur rang de ses parents, qui sont consommés. « ↑ » marque un couple qui ferait progresser le registre.", C::Dimmer),
            Row::text("", C::Dim),
        ];
        let montee = (self.pen_rankup() * 100.0).round() as u64;
        /* le conseil : une ligne, un bouton, le meilleur couple du moment */
        if let Some((ci, rang)) = self.meilleur_couple() {
            let c = &CREATURES[ci];
            let vise = (rang + 1).min(3);
            rows.push(Row {
                segs: vec![
                    ("conseil : ".into(), C::Dimmer),
                    (format!("{} {} ", c.g, c.n), rarity_color(c.r)),
                    (
                        format!(
                            "♂[{}] ♀[{}] → petit [{}]{}",
                            RANK_NAMES[rang],
                            RANK_NAMES[rang],
                            RANK_NAMES[rang],
                            if rang < 3 { format!(", [{}] dans {}% des cas", RANK_NAMES[vise], montee) } else { String::new() }
                        ),
                        C::Text,
                    ),
                ],
                btns: vec![("meilleur couple".into(), C::Gold, Action::PenStart(slot, ci, Some(rang)))],
                act: None,
                indent: 0,
            });
            rows.push(Row::text("", C::Dim));
        }

        /* le sélecteur trié par intérêt : ce qui fait progresser le registre
           d'abord, puis le rang du couple, puis la rareté */
        let mut liste: Vec<(bool, usize, usize, usize)> = vec![];
        for ci in 0..CREATURES.len() {
            let iv = &self.s.inv2[ci];
            if iv.tn() == 0 {
                continue;
            }
            let ok = iv.tm() >= 1 && iv.tf() >= 1;
            if iv.tn() < 2 && !ok {
                continue;
            }
            let rang = (0..4).rev().find(|&r| iv.m[r] >= 1 && iv.f[r] >= 1);
            let (progres, r) = match rang {
                Some(r) => self.interet_couple(ci, r),
                None => (false, 0),
            };
            liste.push((progres && ok, r, CREATURES[ci].r, ci));
        }
        liste.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)).then(a.3.cmp(&b.3)));

        let any = !liste.is_empty();
        for (progres, rang_couple, _, ci) in liste {
            let iv = &self.s.inv2[ci];
            let ok = iv.tm() >= 1 && iv.tf() >= 1;
            let c = &CREATURES[ci];
            let record = self.s.dex2[ci].best as usize;
            let petit = if ok {
                let base = (0..4).find(|&r| iv.m[r] > 0).unwrap_or(0).max((0..4).find(|&r| iv.f[r] > 0).unwrap_or(0));
                format!("petit [{}]", RANK_NAMES[base.max(rang_couple)])
            } else {
                format!("il manque un{}", if iv.tm() == 0 { " ♂" } else { "e ♀" })
            };
            rows.push(Row {
                segs: vec![
                    (if progres { "↑ ".to_string() } else { "  ".to_string() }, C::Gold),
                    (pad(&format!("{} {}", c.g, c.n), 24), rarity_color(c.r)),
                    (pad(&format!("♂{} ♀{}", iv.tm(), iv.tf()), 8), if ok { C::Green } else { C::Red }),
                    (pad(&format!("registre [{}]", if record == 0 { "—".to_string() } else { RANK_NAMES[record - 1].to_string() }), 14), C::Dimmer),
                    (pad(&petit, 14), if progres { C::Gold } else { C::Dimmer }),
                    (format!("{} min", PEN_MIN[c.r] as u64), C::Dimmer),
                ],
                btns: {
                    let mut b = vec![(
                        if ok { "plus bas rangs".into() } else { "installer".to_string() },
                        if ok { C::Green } else { C::Dimmer },
                        if ok { Action::PenStart(slot, ci, None) } else { Action::Nothing },
                    )];
                    for r in 0..4 {
                        if iv.m[r] >= 1 && iv.f[r] >= 1 {
                            b.push((format!("[{}]", RANK_NAMES[r]), C::Blue, Action::PenStart(slot, ci, Some(r))));
                        }
                    }
                    b
                },
                act: None,
                indent: 0,
            });
        }
        if !any {
            rows.push(Row::text("aucune espèce avec plusieurs spécimens. les pièges y travaillent.", C::Dim));
        }
        (format!("enclos {} · choisir un couple", slot + 1), rows)
    }

    fn rows_legend(&self, b: usize, w: u64) -> (String, Vec<Row>) {
        let mut rows = wrap_rows(
            "une silhouette immense se découpe dans la brume. c'est votre unique chance : elle n'attendra pas une seconde tentative.",
            self.panel_w,
            C::Dim,
        );
        rows.push(Row::text(
            format!("légende errante — aperçue en {}{} · rang A minimum", BIOMES[b].name,
                if self.s.biomes[b].is_none() { " (terre non ouverte)" } else { "" }),
            C::Gold,
        ));
        rows.push(Row::text(
            format!("chances d'approche : {}%", (self.legend_take_chance() * 100.0).round() as u64),
            C::Dimmer,
        ));
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![("approche à mains nues".into(), C::Text)],
            btns: vec![("tenter".into(), C::Green, Action::LegendTry(b, w, None))],
            act: None,
            indent: 0,
        });
        for bt in 0..5 {
            if self.s.baits[bt] > 0 {
                rows.push(Row {
                    segs: vec![
                        (pad(&format!("avec {} (×{})", BAITS[bt].n, fmt(self.s.baits[bt] as f64)), 36), C::Text),
                        (if bt == BAIT_ESSENCE { "chance de shiny ×16".into() } else { "meilleures chances".to_string() }, C::Dimmer),
                    ],
                    btns: vec![("tenter".into(), C::Gold, Action::LegendTry(b, w, Some(bt)))],
                    act: None,
                    indent: 0,
                });
            }
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![],
            btns: vec![("s'éloigner sans bruit".into(), C::Dim, Action::Close)],
            act: None,
            indent: 0,
        });
        ("rencontre étrange".into(), rows)
    }

    fn rows_dashboard(&self) -> (String, Vec<Row>) {
        let now = now_ms();
        let (w, sea) = (weather_at(now), season_at(now));
        let mut rows = vec![Row::header("conditions")];
        let next_w = (((now / 1_200_000.0) as u64 + 1) as f64 * 1_200_000.0 - now) / 60_000.0;
        rows.push(Row {
            segs: vec![
                (format!("{} · ", SAISONS[sea]), C::Green),
                (format!("{} · ", METEOS[w]), C::Ice),
                (
                    if is_night_at(now) {
                        format!("{} h au village ◦ (espèces nocturnes de sortie)", heure_jeu(now) as u32)
                    } else {
                        format!("{} h au village", heure_jeu(now) as u32)
                    },
                    C::Text,
                ),
            ],
            btns: vec![],
            act: None,
            indent: 0,
        });
        rows.push(Row::text(format!("météo : {} — change dans {} min", weather_desc(w), next_w.ceil() as u64), C::Dim));
        rows.push(Row::text(format!("saison : {}", season_desc(sea)), C::Dim));
        if self.s.streak > 0 {
            rows.push(Row::text(
                format!("série : jour {} d'affilée · élan +{} de chance aujourd'hui", self.s.streak, fmt2(self.streak_bonus())),
                C::Gold,
            ));
        }
        if let Some((wid, b, _)) = self.legend_now() {
            let left = self.legend_left_min(wid, now);
            rows.push(Row::text(
                format!("✧ une légende errante rôde en {} — encore {} min pour la trouver !", BIOMES[b].name, left),
                C::Gold,
            ));
        }
        /* la pression des battues : ce qu'elles rapportent vraiment, au-delà
           des prises immédiates */
        let actives = self.hunts_actives(now);
        let chance = fmt2(self.legend_chance(now) as f64 / 10.0);
        if actives > 0 {
            let reste = self
                .s
                .hunts_at
                .iter()
                .filter(|&&t| now - t < HUNT_BUFF_MS)
                .map(|&t| ((t + HUNT_BUFF_MS - now) / 60_000.0).ceil() as u64)
                .max()
                .unwrap_or(0);
            rows.push(Row::text(
                format!(
                    "battues actives : {} — légendes à {}% par tirage, toutes les 10 min (la plus ancienne retombe dans {} min)",
                    actives, chance, reste
                ),
                C::Gold,
            ));
        } else {
            rows.push(Row::text(
                format!(
                    "légendes : {}% par tirage, toutes les 10 min — chaque battue ajoute {}% pendant une heure",
                    chance,
                    fmt2(HUNT_BUFF_PM as f64 / 10.0)
                ),
                C::Dimmer,
            ));
        }
        rows.push(Row::text("", C::Dim));

        // ── à faire maintenant : le conseiller de session
        let mut todo: Vec<Row> = vec![];
        let hunts_ready: Vec<&str> = (0..BIOMES.len())
            .filter(|&b| self.s.biomes[b].as_ref().map(|bs| bs.hunt_at <= now).unwrap_or(false))
            .map(|b| BIOMES[b].name)
            .collect();
        if !hunts_ready.is_empty() {
            todo.push(Row::text(format!("· battue prête : {}", hunts_ready.join(", ")), C::Gold));
        }
        let (_, contracts) = self.contracts_now();
        for (i, c) in contracts.iter().enumerate() {
            let done = self.s.contracts_done.get(i).copied().unwrap_or(false);
            if !done && self.deliverable(c.ci) >= c.qty {
                todo.push(Row {
                    segs: vec![(format!("· contrat livrable : {}× {} (+{} écus)", c.qty, CREATURES[c.ci].n, fmt(c.reward)), C::Gold)],
                    btns: vec![],
                    act: Some(Action::Open(PanelKind::Contracts)),
                    indent: 0,
                });
            }
        }
        let pool2 = self.s.museum_pool + self.museum_rate() * (now - self.s.museum_at).max(0.0);
        let cap2 = self.museum_rate() * self.museum_cap_h() * 3_600_000.0;
        if cap2 > 0.0 && pool2 >= cap2 * 0.9 {
            todo.push(Row {
                segs: vec![(format!("· la cagnotte du musée déborde ({} écus) — encaissez", fmt(pool2)), C::Gold)],
                btns: vec![],
                act: Some(Action::Open(PanelKind::Museum)),
                indent: 0,
            });
        }
        let free_pen = (0..self.pen_slots()).any(|i| self.s.pens[i].is_none());
        let couple_ready = (0..CREATURES.len()).any(|ci| self.s.inv2[ci].tm() >= 1 && self.s.inv2[ci].tf() >= 1 && self.s.inv2[ci].tn() > 2);
        if free_pen && couple_ready {
            todo.push(Row {
                segs: vec![("· un enclos est libre et des couples attendent".into(), C::Gold)],
                btns: vec![],
                act: Some(Action::Open(PanelKind::Pens)),
                indent: 0,
            });
        }
        for (_, b, _) in self.traces_now() {
            todo.push(Row::text(format!("· des traces fraîches en {} (∵ sur la carte)", BIOMES[b].name), C::Gold));
        }
        let boosted_empty: Vec<&str> = (0..BIOMES.len())
            .filter(|&b| self.s.biomes[b].as_ref().map(|bs| bs.pl.iter().flatten().count() == 0).unwrap_or(false))
            .filter(|&b| weather_luck(w, b) + season_luck(sea, b) > 0.0)
            .map(|b| BIOMES[b].name)
            .collect();
        if !boosted_empty.is_empty() && self.placed_total() > 0 {
            todo.push(Row::text(
                format!(
                    "· le temps avantage {} en ce moment, mais vous n'y avez aucun piège posé — pensez à en déplacer un",
                    boosted_empty.join(", ")
                ),
                C::Ice,
            ));
        }
        if !todo.is_empty() {
            rows.push(Row::header("à faire maintenant"));
            rows.extend(todo);
            rows.push(Row::text("", C::Dim));
        }

        rows.push(Row::header("expédition"));
        rows.push(Row::text(
            format!("écus {} · gagnés cette expédition {} · au total {}", fmt(self.s.ecus), fmt(self.s.run_earned), fmt(self.s.total_earned)),
            C::Text,
        ));
        rows.push(Row::text(
            format!("trophées {} · migrations {} · chance +{} · vente ×{} · vitesse ×{}",
                self.s.trophies, self.s.migrations, fmt2(self.global_luck()), fmt2(self.sell_mult()), fmt2(self.speed_mult())),
            C::Dim,
        ));
        let mut cpm = 0.0;
        for b in 0..BIOMES.len() {
            if let Some(bs) = &self.s.biomes[b] {
                for pl in bs.pl.iter().flatten() {
                    let bait_ok = pl.bait.filter(|&bt| self.s.baits[bt] > 0);
                    cpm += (TRAPS[pl.trap].succ + weather_succ_mod(w, b)).clamp(0.05, 0.99) * 60000.0 / self.trap_interval(pl.trap, bait_ok, b, now);
                }
            }
        }
        rows.push(Row::text(format!("rendement estimé : {} captures/min", fmt2(cpm)), C::Green));
        let (pt, tc) = (self.placed_total(), self.trap_cap());
        rows.push(Row::text(
            format!("licence de piégeage : {}/{} pièges posés{}", pt, tc, if pt >= tc { " — plafond atteint" } else { "" }),
            if pt >= tc { C::Gold } else { C::Dim },
        ));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("biomes — Entrée sur une ligne pour y aller"));
        for b in biomes_par_prix() {
            match &self.s.biomes[b] {
                None => rows.push(Row {
                    segs: vec![
                        ("├─ ".into(), C::Dimmer),
                        (pad(BIOMES[b].name, 10), C::Dimmer),
                        (format!("verrouillé — {} écus", fmt(BIOMES[b].cost)),
                         if self.s.ecus >= BIOMES[b].cost { C::Gold } else { C::Dimmer }),
                    ],
                    btns: vec![],
                    act: Some(Action::Open(PanelKind::Unlock(b))),
                    indent: 0,
                }),
                Some(bs) => {
                    let placed = bs.pl.iter().flatten().count();
                    let found = biome_creatures(b).filter(|&i| self.s.dex2[i].n > 0).count();
                    let bait_dead = bs.pl.iter().flatten().any(|pl| pl.bait.map(|bt| self.s.baits[bt] == 0).unwrap_or(false));
                    let next = bs.pl.iter().flatten().map(|pl| ((pl.next_at - now).max(0.0) / 1000.0) as u64).min();
                    let boosted = weather_luck(w, b) + season_luck(sea, b) > 0.0;
                    let mut segs = vec![
                        ("├─ ".into(), C::Dimmer),
                        (pad(BIOMES[b].name, 10), if boosted { C::Ice } else { C::Text }),
                        (pad(&format!("empl. {}/{}", placed, bs.slots), 12), if placed == 0 { C::Red } else { C::Dim }),
                        (pad(&format!("bestiaire {}/{}{}", found, biome_creatures(b).count(), if found == biome_creatures(b).count() { " ✓" } else { "" }), 19), C::Blue),
                    ];
                    match next {
                        Some(sec) => segs.push((format!("tentative dans {} s", sec), C::Green)),
                        None => segs.push(("aucun piège posé".into(), C::Red)),
                    }
                    if bait_dead {
                        segs.push(("  appât épuisé !".into(), C::Red));
                    }
                    if boosted {
                        segs.push(("  ↑ conditions favorables".into(), C::Ice));
                    }
                    rows.push(Row { segs, btns: vec![], act: Some(Action::Open(PanelKind::Biome(b))), indent: 0 });
                }
            }
        }

        // battues disponibles
        let hunts: Vec<&str> = (0..BIOMES.len())
            .filter(|&b| self.s.biomes[b].as_ref().map(|bs| bs.hunt_at <= now && bs.pl.iter().flatten().count() > 0).unwrap_or(false))
            .map(|b| BIOMES[b].name)
            .collect();
        if !hunts.is_empty() {
            rows.push(Row::text(format!("→ battues disponibles : {}", hunts.join(", ")), C::Gold));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("contrats · musée · enclos"));
        let (_, contracts) = self.contracts_now();
        let open_c = contracts.iter().enumerate().filter(|(i, _)| !self.s.contracts_done.get(*i).copied().unwrap_or(false)).count();
        rows.push(Row {
            segs: vec![(format!("├─ contrats : {} à livrer", open_c), if open_c > 0 { C::Blue } else { C::Dimmer })],
            btns: vec![],
            act: Some(Action::Open(PanelKind::Contracts)),
            indent: 0,
        });
        let pool = self.s.museum_pool + self.museum_rate() * (now - self.s.museum_at).max(0.0);
        let occ = self.s.museum.iter().flatten().count();
        rows.push(Row {
            segs: vec![(format!("├─ musée : {}/{} salles · cagnotte {} écus", occ, self.museum_slots(), fmt(pool)), if pool >= 1.0 { C::Gold } else { C::Dimmer })],
            btns: vec![],
            act: Some(Action::Open(PanelKind::Museum)),
            indent: 0,
        });
        let ready_pens = self.s.pens.iter().flatten().filter(|p| p.ready_at <= now).count();
        let busy_pens = self.s.pens.iter().flatten().count();
        rows.push(Row {
            segs: vec![(
                if ready_pens > 0 { format!("├─ enclos : {} naissance{} à récupérer !", ready_pens, if ready_pens > 1 { "s" } else { "" }) }
                else { format!("├─ enclos : {}/{} occupés", busy_pens, self.pen_slots()) },
                if ready_pens > 0 { C::Gold } else { C::Dimmer },
            )],
            btns: vec![],
            act: Some(Action::Open(PanelKind::Pens)),
            indent: 0,
        });

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("réserve et bestiaire"));
        let (mut dupes, mut dval, mut total_inv, mut shiny_inv) = (0u64, 0.0, 0u64, 0u64);
        for ci in 0..CREATURES.len() {
            let iv = &self.s.inv2[ci];
            total_inv += iv.tn() + iv.ts();
            shiny_inv += iv.ts();
            let q = iv.tn().saturating_sub(2);
            if iv.tn() > 2 {
                dupes += q;
                let mut left = q;
                for r in 0..4 {
                    let t = iv.nr(r).min(left);
                    dval += t as f64 * self.creature_value_r(ci, false, r);
                    left -= t;
                    if left == 0 { break; }
                }
            }
        }
        rows.push(Row::text(
            format!("créatures en réserve : {}{} · doublons vendables : {} (≈ {} écus)",
                fmt(total_inv as f64),
                if shiny_inv > 0 { format!(" dont {} ⋆", shiny_inv) } else { String::new() },
                fmt(dupes as f64), fmt(dval)),
            if dval > 0.0 { C::Gold } else { C::Dim },
        ));
        let found_total = wild_species().filter(|&i| self.s.dex2[i].n > 0).count();
        let shiny_total = wild_species().filter(|&i| self.s.dex2[i].s > 0).count();
        let s_total = wild_species().filter(|&i| self.s.dex2[i].best >= 4).count();
        rows.push(Row::text(
            format!("bestiaire : {}/{nc} espèces · {}/{nc} shinies · {}/{nc} en rang S · {} biome(s) complet(s)",
                found_total, shiny_total, s_total, self.completed_biomes(), nc = wild_total()),
            C::Blue,
        ));
        ("tableau de bord".into(), rows)
    }    fn rows_biome(&self, b: usize) -> (String, Vec<Row>) {
        let bio = &BIOMES[b];
        let bs = self.s.biomes[b].as_ref().unwrap();
        let now = now_ms();
        let found = biome_creatures(b).filter(|&i| self.s.dex2[i].n > 0).count();
        let mut rows = wrap_rows(bio.desc, self.panel_w, C::Dim);
        rows.push(Row::text(
            format!("bestiaire {}/{}{} · valeur des prises ×{}", found, biome_creatures(b).count(), if found == biome_creatures(b).count() { " ✓" } else { "" }, bio.mult),
            C::Dimmer,
        ));
        // conditions actives sur ce biome
        let (w, sea) = (weather_at(now), season_at(now));
        let mut fx = vec![];
        let wl = weather_luck(w, b);
        if wl > 0.0 { fx.push(format!("{} : chance +{}", METEOS[w], fmt2(wl))); }
        let im = weather_itv_mult(w, b);
        if im < 1.0 { fx.push(format!("{} : vitesse +{}%", METEOS[w], ((1.0 / im - 1.0) * 100.0).round() as u64)); }
        if im > 1.0 { fx.push(format!("{} : vitesse −{}%", METEOS[w], ((1.0 - 1.0 / im) * 100.0).round() as u64)); }
        let sm = weather_succ_mod(w, b);
        if sm < 0.0 { fx.push(format!("{} : réussite {}%", METEOS[w], (sm * 100.0).round() as i64)); }
        let sl = season_luck(sea, b);
        if sl > 0.0 { fx.push(format!("{} : chance +{}", SAISONS[sea], fmt2(sl))); }
        if !fx.is_empty() {
            rows.push(Row::text(format!("conditions : {}", fx.join(" · ")), C::Ice));
        }
        rows.push(Row::text("", C::Dim));
        /* la battue passe avant les emplacements : on la déclenche à chaque
           passage, alors qu'on ne repose un piège que de loin en loin.
           elle déclenche immédiatement chaque piège posé avec +0,2 chance */
        let placed = bs.pl.iter().flatten().count();
        let ready = bs.hunt_at <= now;
        let left = ((bs.hunt_at - now).max(0.0) / 1000.0).ceil() as u64;
        rows.push(Row {
            segs: vec![(
                if !ready { format!("prochaine battue possible dans {} s", left) }
                else if placed > 0 { "battre les fourrés vous-même (chance +0,2) :".into() }
                else { "battre les fourrés à mains nues (une seule tentative) :".into() },
                if ready { C::Text } else { C::Dimmer },
            )],
            btns: vec![(
                "battue !".into(),
                if ready { C::Gold } else { C::Dimmer },
                if ready { Action::Hunt(b) } else { Action::Nothing },
            )],
            act: None,
            indent: 0,
        });
        rows.push(Row::text("", C::Dim));
        for (i, pl) in bs.pl.iter().enumerate() {
            match pl {
                None => rows.push(Row {
                    segs: vec![("├─ ".into(), C::Dimmer), (format!("emplacement {} : vide", i + 1), C::Dim)],
                    btns: vec![("poser un piège".into(), C::Green, Action::Open(PanelKind::TrapPick(b, i)))],
                    act: None,
                    indent: 0,
                }),
                Some(pl) => {
                    let bait_ok = pl.bait.filter(|&bt| self.s.baits[bt] > 0);
                    let itv = self.trap_interval(pl.trap, bait_ok, b, now);
                    let frac = 1.0 - ((pl.next_at - now).max(0.0) / itv).min(1.0);
                    let sec = ((pl.next_at - now).max(0.0) / 1000.0).ceil() as u64;
                    let bait_txt = match pl.bait {
                        Some(bt) if self.s.baits[bt] > 0 => format!("{} ×{}", BAITS[bt].n, fmt(self.s.baits[bt] as f64)),
                        Some(bt) => format!("{} épuisé !", BAITS[bt].n),
                        None => "sans appât".into(),
                    };
                    let bait_c = if pl.bait.is_some() && bait_ok.is_none() { C::Red } else { C::Dim };
                    rows.push(Row {
                        segs: vec![
                            ("├─ ".into(), C::Dimmer),
                            (format!("{} ", TRAPS[pl.trap].n), C::Text),
                            (format!("{} {}s ", ascii_bar(frac, 10), sec), C::Green),
                            (format!("· {}", bait_txt), bait_c),
                        ],
                        btns: vec![
                            ("appât".into(), C::Blue, Action::Open(PanelKind::BaitPick(b, i))),
                            ("retirer".into(), C::Red, Action::Remove(b, i)),
                        ],
                        act: None,
                        indent: 0,
                    });
                }
            }
        }
        if bs.slots < 4 {
            let cost = self.slot_cost(b);
            let ok = self.s.ecus >= cost;
            rows.push(Row {
                segs: vec![("└─ ".into(), C::Dimmer)],
                btns: vec![(format!("+ emplacement — {} écus", fmt(cost)), if ok { C::Gold } else { C::Dimmer }, Action::BuySlot(b))],
                act: None,
                indent: 0,
            });
        }
        (bio.name.to_string(), rows)
    }    fn rows_trap_pick(&self, b: usize, i: usize) -> (String, Vec<Row>) {
        let at_cap = self.placed_total() >= self.trap_cap();
        let mut rows = wrap_rows(
            &format!("licence de piégeage : {} piège{} posé{} sur {} autorisés, tous biomes confondus.{}",
                self.placed_total(), if self.placed_total() > 1 { "s" } else { "" }, if self.placed_total() > 1 { "s" } else { "" },
                self.trap_cap(),
                if at_cap { " plafond atteint — retirez un piège ailleurs, ou améliorez « licence de piégeage » au labo." } else { "" }),
            self.panel_w,
            if at_cap { C::Red } else { C::Dim },
        );
        rows.push(Row::text("", C::Dim));
        let avail: Vec<usize> = (0..6).filter(|&t| self.s.traps[t] > self.placed_count(t)).collect();
        if avail.is_empty() {
            rows.push(Row::text("aucun piège en réserve.", C::Dim));
            rows.push(Row::text("la boutique du village en vend — au centre de la carte.", C::Dimmer));
        }
        for t in avail {
            let free = self.s.traps[t] - self.placed_count(t);
            rows.push(Row {
                segs: vec![
                    (format!("{} ", TRAPS[t].n), C::Text),
                    (format!("×{} · {}s · {}% · chance +{}", free, TRAPS[t].itv, (TRAPS[t].succ * 100.0) as u32, fmt_luck(TRAPS[t].luck)), C::Dimmer),
                ],
                btns: if at_cap { vec![] } else { vec![("poser".into(), C::Green, Action::Place(b, i, t))] },
                act: None,
                indent: 0,
            });
        }
        (format!("poser un piège · {}", BIOMES[b].name), rows)
    }

    fn rows_bait_pick(&self, b: usize, i: usize) -> (String, Vec<Row>) {
        let mut rows = vec![
            Row::text("un appât est consommé à chaque tentative du piège.", C::Dimmer),
            Row::text("", C::Dim),
            Row {
                segs: vec![("aucun appât".into(), C::Dim)],
                btns: vec![("choisir".into(), C::Blue, Action::SetBait(b, i, None))],
                act: None,
                indent: 0,
            },
        ];
        let mut any = false;
        for bt in 0..5 {
            if self.s.baits[bt] > 0 {
                any = true;
                rows.push(Row {
                    segs: vec![
                        (format!("{} ", BAITS[bt].n), C::Text),
                        (format!("×{} · {}", fmt(self.s.baits[bt] as f64), BAITS[bt].desc), C::Dimmer),
                    ],
                    btns: vec![("choisir".into(), C::Green, Action::SetBait(b, i, Some(bt)))],
                    act: None,
                    indent: 0,
                });
            }
        }
        if !any {
            rows.push(Row::text("réserve vide — les appâts s'achètent à la boutique.", C::Dim));
        }
        (format!("appât · {}", BIOMES[b].name), rows)
    }

    fn rows_unlock(&self, b: usize) -> (String, Vec<Row>) {
        let bio = &BIOMES[b];
        let ok = self.s.ecus >= bio.cost;
        let mut rows = wrap_rows(bio.desc, self.panel_w, C::Dim);
        rows.push(Row::text(format!("valeur des prises ×{} · {} espèces à découvrir", bio.mult, biome_creatures(b).count()), C::Dimmer));
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![
                ("droit d'accès : ".into(), C::Text),
                (format!("{} écus", fmt(bio.cost)), if ok { C::Gold } else { C::Red }),
                (format!(" (vous avez {})", fmt(self.s.ecus)), C::Dimmer),
            ],
            btns: vec![("débloquer".into(), if ok { C::Green } else { C::Dimmer }, Action::Unlock(b))],
            act: None,
            indent: 0,
        });
        (format!("{} · verrouillé", bio.name), rows)
    }

    fn rows_shop(&self) -> (String, Vec<Row>) {
        // le comptoir affiche aussi les commandes en cours (raccourci : [c] partout)
        let (w, contracts) = self.contracts_now();
        let left_ms = ((w + 1) as f64 * 7_200_000.0) - now_ms();
        let mut rows = vec![];
        /* la vente des doublons passe avant tout le reste : on y revient à
           chaque retour au village, alors que les commandes et les achats se
           consultent de loin en loin */
        if (0..CREATURES.len()).any(|ci| self.s.inv2[ci].tn() + self.s.inv2[ci].ts() > 0) {
            rows.push(Row {
                segs: vec![("réserve à écouler :".into(), C::Dim)],
                btns: vec![("vendre tous les doublons".into(), C::Green, Action::SellDupes)],
                act: None,
                indent: 0,
            });
            rows.push(Row::text("", C::Dim));
        }
        rows.push(Row::header(&format!("commandes du comptoir — renouvelées dans {} min", (left_ms / 60_000.0).ceil() as u64)));
        for (i, c) in contracts.iter().enumerate() {
            let done = self.s.contracts_done.get(i).copied().unwrap_or(false);
            let cr = &CREATURES[c.ci];
            if done {
                rows.push(Row {
                    segs: vec![("■ ".into(), C::Green), (pad(&format!("{}× {}", c.qty, cr.n), 26), C::Dimmer), ("livré".into(), C::Green)],
                    btns: vec![],
                    act: None,
                    indent: 0,
                });
            } else {
                let have = self.deliverable(c.ci);
                let ok = have >= c.qty;
                rows.push(Row {
                    segs: vec![
                        ("□ ".into(), C::Dim),
                        (pad(&format!("{}× {}", c.qty, cr.n), 26), rarity_color(cr.r)),
                        (pad(&format!("(livrables : {})", have), 18), if ok { C::Green } else { C::Red }),
                        (pad(&format!("+{} écus", fmt(c.reward)), 16), C::GoldDark),
                        {
                            let gain = c.reward / self.valeur_a_la_vente(c.ci, c.qty).max(1.0);
                            (format!("×{}", fmt2(gain)), if gain >= 1.0 { C::Green } else { C::Red })
                        },
                    ],
                    btns: vec![("livrer".into(), if ok { C::Green } else { C::Dimmer }, Action::Deliver(i))],
                    act: None,
                    indent: 0,
                });
            }
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("pièges"));
        for t in 0..6 {
            let owned = self.s.traps[t];
            let free = owned - self.placed_count(t);
            let ok = self.s.ecus >= TRAPS[t].cost;
            rows.push(Row {
                segs: vec![
                    (pad(TRAPS[t].n, 18), C::Text),
                    (pad(&format!("{}s · {}% · chance +{}", TRAPS[t].itv, (TRAPS[t].succ * 100.0) as u32, fmt_luck(TRAPS[t].luck)), 26), C::Dimmer),
                    (pad(&format!("×{}{}", owned, if owned > 0 { format!(" ({} libre{})", free, if free > 1 { "s" } else { "" }) } else { String::new() }), 14), C::Dim),
                ],
                btns: {
                    let mut b = vec![(format!("{} écus", fmt(TRAPS[t].cost)), if ok { C::Gold } else { C::Dimmer }, Action::BuyTrap(t))];
                    // revente possible tant qu'il reste un piège libre, et
                    // jamais le dernier de la réserve
                    if free > 0 && self.s.traps.iter().sum::<u32>() > 1 {
                        b.push((format!("vendre {} écus", fmt((TRAPS[t].cost * TRAP_RESALE).floor())), C::Blue, Action::SellTrap(t)));
                    }
                    b
                },
                act: None,
                indent: 0,
            });
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("appâts — consommés à chaque tentative"));
        for bt in 0..5 {
            rows.push(Row {
                segs: vec![
                    (pad(BAITS[bt].n, 18), C::Text),
                    (pad(&format!("{} écus/u", fmt(BAITS[bt].cost)), 14), C::GoldDark),
                    (pad(&format!("×{}", fmt(self.s.baits[bt] as f64)), 8), C::Dim),
                ],
                btns: vec![
                    ("×1".into(), if self.s.ecus >= BAITS[bt].cost { C::Gold } else { C::Dimmer }, Action::BuyBait(bt, 1)),
                    ("×10".into(), if self.s.ecus >= BAITS[bt].cost * 10.0 { C::Gold } else { C::Dimmer }, Action::BuyBait(bt, 10)),
                    ("×100".into(), if self.s.ecus >= BAITS[bt].cost * 100.0 { C::Gold } else { C::Dimmer }, Action::BuyBait(bt, 100)),
                ],
                act: None,
                indent: 0,
            });
            rows.push(Row { segs: vec![(BAITS[bt].desc.into(), C::Dimmer)], btns: vec![], act: None, indent: 2 });
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header(&format!("comptoir de vente — prix ×{}", fmt2(self.sell_mult()))));
        let any = (0..CREATURES.len()).any(|ci| self.s.inv2[ci].tn() + self.s.inv2[ci].ts() > 0);
        if !any {
            rows.push(Row::text("réserve vide. les pièges y remédieront.", C::Dim));
        } else {
            for r in wrap_rows(
                "« vendre tous les doublons » garde vos meilleurs couples ♂♀ de chaque espèce (un par défaut, jusqu'à quatre au réglage ci-dessous), met de côté ce qu'exigent les commandes du comptoir et les demandes du troc, jamais les shinies. la vente écoule d'abord les rangs les plus bas.",
                self.panel_w, C::Dimmer,
            ) {
                rows.push(r);
            }
            /* les espèces hors biome se vendent comme les autres : un doublon
               de curiosité ou de légende n'a aucune raison de rester coincé en
               réserve, la découverte étant déjà acquise au bestiaire */
            for b in biomes_par_prix().into_iter().chain([CURIO_B, LEGEND_B, PUITS_B]) {
                let mut list: Vec<usize> = biome_creatures(b).filter(|&ci| self.s.inv2[ci].tn() + self.s.inv2[ci].ts() > 0).collect();
                if list.is_empty() {
                    continue;
                }
                list.sort_by_key(|&ci| CREATURES[ci].r);
                rows.push(Row::text(format!("· {}", BIOMES[b].name), C::Blue));
                for ci in list {
                    let c = &CREATURES[ci];
                    let iv = &self.s.inv2[ci];
                    let (n, s) = (iv.tn(), iv.ts());
                    if n > 0 {
                        let low = (0..4).find(|&r| iv.nr(r) > 0).unwrap_or(0);
                        let best = (0..4).rev().find(|&r| iv.nr(r) > 0).unwrap_or(0);
                        rows.push(Row {
                            segs: vec![
                                (pad(&format!("{} {}", c.g, c.n), 24), rarity_color(c.r)),
                                (pad(&format!("×{}", fmt(n as f64)), 6), C::Dim),
                                (pad(&(if low == best { format!("[{}]", RANK_NAMES[low]) } else { format!("[{}-{}]", RANK_NAMES[low], RANK_NAMES[best]) }), 6), if best >= 3 { C::Gold } else { C::Dim }),
                                (pad(&format!("dès {}/u", fmt(self.creature_value_r(ci, false, low))), 11), C::GoldDark),
                            ],
                            btns: vec![
                                ("1".into(), C::Blue, Action::Sell(ci, false, SellQty::One)),
                                ("sauf couple".into(), C::Blue, Action::Sell(ci, false, SellQty::Keep1)),
                                ("tout".into(), C::Red, Action::Sell(ci, false, SellQty::All)),
                            ],
                            act: None,
                            indent: 1,
                        });
                    }
                    if s > 0 {
                        let low = (0..4).find(|&r| iv.sr(r) > 0).unwrap_or(0);
                        rows.push(Row {
                            segs: vec![
                                (pad(&format!("{} {} ⋆", c.g, c.n), 24), C::Shiny),
                                (pad(&format!("×{}", fmt(s as f64)), 6), C::Dim),
                                (pad(&format!("[{}]", RANK_NAMES[low]), 6), C::Dim),
                                (pad(&format!("{}/u", fmt(self.creature_value_r(ci, true, low))), 11), C::GoldDark),
                            ],
                            btns: vec![("vendre 1 shiny".into(), C::Red, Action::Sell(ci, true, SellQty::One))],
                            act: None,
                            indent: 1,
                        });
                    }
                }
            }
        }
        if self.s.lab[LAB_AUTOVENTE] >= 1 {
            rows.push(Row::text("", C::Dim));
            rows.push(Row::header("auto-vente — garde vos couples, les commandes du comptoir et les demandes du troc"));
            rows.push(Row {
                segs: vec![],
                btns: (0..5)
                    .map(|r| {
                        (
                            format!("{} {}", if self.s.autosell[r] { "■" } else { "□" }, RAR_LABEL[r]),
                            if self.s.autosell[r] { C::Green } else { C::Dim },
                            Action::ToggleAutosell(r),
                        )
                    })
                    .collect(),
                act: None,
                indent: 0,
            });
            /* l'enclos consomme les parents : un seul couple épargné ne permet
               qu'une reproduction, après quoi l'espèce repart de zéro */
            rows.push(Row {
                segs: vec![(
                    format!(
                        "couples épargnés par espèce : {} (soit {} bêtes gardées, de quoi tenir {} reproduction{})",
                        self.s.autokeep,
                        self.s.autokeep * 2,
                        self.s.autokeep,
                        if self.s.autokeep > 1 { "s" } else { "" }
                    ),
                    C::Text,
                )],
                btns: (1..=4)
                    .map(|n| {
                        (
                            format!("{} {}", if self.s.autokeep == n { "■" } else { "□" }, n),
                            if self.s.autokeep == n { C::Green } else { C::Dim },
                            Action::SetAutokeep(n),
                        )
                    })
                    .collect(),
                act: None,
                indent: 0,
            });
        }
        (format!("boutique — {} écus", fmt(self.s.ecus)), rows)
    }

    fn rows_lab(&self) -> (String, Vec<Row>) {
        let mut rows = vec![Row::header("recherches")];
        for k in 0..LABS.len() {
            let lv = self.s.lab[k];
            let maxed = lv >= LABS[k].max;
            let cost = self.lab_cost(k);
            let ok = self.s.ecus >= cost;
            let fx = match k {
                LAB_AFFUTAGE => format!("vitesse des pièges +{}%", lv * 6),
                LAB_FLAIR => format!("chance +{}", fmt2(lv as f64 * 0.08)),
                LAB_NEGOCE => format!("prix de vente +{}%", lv * 5),
                LAB_HORLOGE => format!("hors-ligne : {} h max", 2 + lv * 2),
                LAB_ECLAT => format!("chance de shiny +{}%", lv * 10),
                LAB_AUTOVENTE => if lv > 0 { "filtres débloqués".into() } else { "non débloquée".into() },
                LAB_CONSERVATION => format!("cagnotte du musée : {} h max", 4 + lv * 2),
                LAB_AILES => format!("{} salles au musée", 6 + lv),
                LAB_ENCLOS => format!("{} enclos", 3 + lv),
                LAB_LIGNEES => format!("montée de rang : {}%", 25 + lv * 4),
                LAB_TRAQUEUR => format!("battue toutes les {} s", 300 - lv * 30),
                LAB_COURTAGE => format!("primes de contrats +{}%", lv * 15),
                LAB_LICENCE => format!("{} pièges posés autorisés", 2 + lv + self.s.licences),
                LAB_APPEL => format!("légendes : {}% par tirage", fmt2((LEGEND_BASE_PM + lv as u64 * LAB_APPEL_PM) as f64 / 10.0)),
                _ => format!("approche des légendes +{}%", lv * 5),
            };
            rows.push(Row {
                segs: vec![
                    (pad(LABS[k].n, 20), C::Text),
                    (pad(&format!("niv {}/{}", lv, LABS[k].max), 10), C::Dimmer),
                    (pad(&fx, 24), C::Green),
                ],
                btns: if maxed {
                    vec![("max".into(), C::Dimmer, Action::Nothing)]
                } else {
                    vec![(format!("{} écus", fmt(cost)), if ok { C::Gold } else { C::Dimmer }, Action::BuyLab(k))]
                },
                act: None,
                indent: 0,
            });
            for l in wrap_lines(LABS[k].desc, self.panel_w.saturating_sub(2)) {
                rows.push(Row { segs: vec![(l, C::Dimmer)], btns: vec![], act: None, indent: 2 });
            }
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header(&format!("migration — {} effectuée{}", self.s.migrations, if self.s.migrations > 1 { "s" } else { "" })));
        for r in wrap_rows("repartir de zéro vers des terres plus giboyeuses, contre des frais de voyage qui doublent à chaque départ. le bestiaire et les succès sont conservés ; écus, pièges, labo et réserve sont perdus. chaque trophée offre définitivement +0,008 de chance et +1% aux prix de vente.", self.panel_w, C::Dim) {
            rows.push(r);
        }
        let g = self.trophy_gain();
        let mcost = self.migration_cost();
        let can = g >= 1 && self.s.ecus >= mcost;
        rows.push(Row {
            segs: vec![
                (format!("écus gagnés : {} · ", fmt(self.s.run_earned)), C::Dimmer),
                (format!("frais de voyage : {} écus", fmt(mcost)), if self.s.ecus >= mcost { C::GoldDark } else { C::Red }),
            ],
            btns: vec![],
            act: None,
            indent: 0,
        });
        rows.push(Row {
            segs: vec![
                ("trophées à la migration : ".into(), C::Text),
                (format!("+{}", g), if g > 0 { C::Gold } else { C::Dimmer }),
                (if g < 1 { "  (1 M d'écus gagnés = 1er trophée)".into() } else { String::new() }, C::Dimmer),
            ],
            btns: vec![("migrer".into(), if can { C::Red } else { C::Dimmer }, if can { Action::Open(PanelKind::MigrConfirm) } else { Action::Nothing })],
            act: None,
            indent: 0,
        });
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("bonus actifs"));
        rows.push(Row::text(
            format!("chance globale +{} · vente ×{} · vitesse ×{}", fmt2(self.global_luck()), fmt2(self.sell_mult()), fmt2(self.speed_mult())),
            C::Dim,
        ));
        for r in wrap_rows(
            &format!("détail chance : flair +{} · trophées +{} · biomes complets +{} · élan (série j{}) +{}",
                fmt2(self.s.lab[LAB_FLAIR] as f64 * 0.08),
                fmt2(self.s.trophies as f64 * 0.008),
                fmt2(self.completed_biomes() as f64 * 0.04),
                self.s.streak,
                fmt2(self.streak_bonus())),
            self.panel_w.saturating_sub(2),
            C::Dimmer,
        ) {
            rows.push(Row { segs: r.segs, btns: vec![], act: None, indent: 2 });
        }
        rows.push(Row::text(
            format!("shiny 1/{} · hors-ligne {} h max", (1.0 / self.shiny_chance(None, now_ms())).round() as u64, (self.offline_cap_ms() / 3600000.0) as u64),
            C::Dim,
        ));
        (format!("labo — {} écus", fmt(self.s.ecus)), rows)
    }

    fn rows_migr_confirm(&self) -> (String, Vec<Row>) {
        let g = self.trophy_gain();
        let mut rows = wrap_rows(
            &format!("les frais de voyage ({} écus) sont réglés, puis vos écus, pièges, appâts, améliorations et créatures en réserve disparaissent. le bestiaire et les succès restent. vous gagnez {} trophée{} permanents.", fmt(self.migration_cost()), g, if g > 1 { "s" } else { "" }),
            self.panel_w,
            C::Dim,
        );
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![],
            btns: vec![
                ("migrer maintenant".into(), C::Red, Action::Migrate),
                ("rester ici".into(), C::Green, Action::Close),
            ],
            act: None,
            indent: 0,
        });
        ("migration — confirmer".into(), rows)
    }

    fn rows_dex(&self) -> (String, Vec<Row>) {
        let total = wild_species().filter(|&i| self.s.dex2[i].n > 0).count();
        let shiny_total = wild_species().filter(|&i| self.s.dex2[i].s > 0).count();
        let s_total = wild_species().filter(|&i| self.s.dex2[i].best >= 4).count();
        let mut rows = vec![
            Row::text(
                format!("espèces {}/{} {}  shinies {}/{} {}  rang S {}/{}", total, wild_total(), ascii_bar(total as f64 / wild_total() as f64, 10), shiny_total, wild_total(), ascii_bar(shiny_total as f64 / wild_total() as f64, 10), s_total, wild_total()),
                C::Text,
            ),
            Row::text("biome complet : +0,04 chance · biome 100% shiny : +5% vente — pour toujours", C::Dimmer),
            Row::text("colonnes : rareté · ×captures · ⋆shinies · [rang] · sexes vus · réserve", C::Dimmer),
        ];
        for b in biomes_par_prix().into_iter().chain([CURIO_B, LEGEND_B, PUITS_B]) {
            let found = biome_creatures(b).filter(|&i| self.s.dex2[i].n > 0).count();
            rows.push(Row::text("", C::Dim));
            rows.push(Row::header(&format!("{} — {}/{}{}", BIOMES[b].name, found, biome_creatures(b).count(), if found == biome_creatures(b).count() { " ✓" } else { "" })));
            let mut species: Vec<usize> = biome_creatures(b).collect();
            species.sort_by_key(|&ci| (CREATURES[ci].r, ci));
            for ci in species {
                let c = &CREATURES[ci];
                let d = &self.s.dex2[ci];
                if d.n == 0 {
                    rows.push(Row {
                        segs: vec![
                            ("├─ ".into(), C::Dimmer),
                            (pad("???", DEX_W_GLYPH), C::Dimmer),
                            (pad("— inconnu —", DEX_W_NAME), C::Dimmer),
                            (pad(if NOCTURNES.contains(&ci) { "◦" } else { "" }, DEX_W_MOON), C::Abyss),
                            (pad(RAR_LABEL[c.r], DEX_W_RAR), C::Dimmer),
                            (if NOCTURNES.contains(&ci) { "nocturne".into() } else { String::new() }, C::Abyss),
                        ],
                        btns: vec![],
                        act: None,
                        indent: 0,
                    });
                } else {
                    let iv = &self.s.inv2[ci];
                    let best = if d.best > 0 { RANK_NAMES[(d.best - 1) as usize] } else { "-" };
                    rows.push(Row {
                        segs: vec![
                            ("├─ ".into(), C::Dimmer),
                            (pad(c.g, DEX_W_GLYPH), if d.s > 0 { C::Shiny } else { rarity_color(c.r) }),
                            (pad(c.n, DEX_W_NAME), rarity_color(c.r)),
                            (pad(if NOCTURNES.contains(&ci) { "◦" } else { "" }, DEX_W_MOON), C::Abyss),
                            (pad(&format!("{} ×{}{}", RAR_LABEL[c.r], fmt(d.n as f64), if d.s > 0 { format!(" ⋆{}", d.s) } else { String::new() }), DEX_W_RAR), C::Dim),
                            (pad(&format!("[{}]", best), DEX_W_RANK), if d.best >= 4 { C::Gold } else { C::Dim }),
                            (pad(&format!("{}{}", if d.mf & 1 != 0 { "♂" } else { "·" }, if d.mf & 2 != 0 { "♀" } else { "·" }), DEX_W_SEX),
                             if d.mf == 3 { C::Green } else { C::Dim }),
                            (format!("stock {}{}", iv.tn(), if iv.ts() > 0 { format!("+{}⋆", iv.ts()) } else { String::new() }),
                             if iv.tn() + iv.ts() > 0 { C::GoldDark } else { C::Dimmer }),
                        ],
                        btns: vec![],
                        act: Some(Action::Open(PanelKind::Creature(ci))),
                        indent: 0,
                    });
                }
            }
        }
        ("bestiaire".into(), rows)
    }    fn rows_creature(&self, ci: usize) -> (String, Vec<Row>) {
        let c = &CREATURES[ci];
        let d = &self.s.dex2[ci];
        let iv = &self.s.inv2[ci];
        let mut rows = vec![Row {
            segs: vec![
                (format!("{}  ", c.g), if d.s > 0 { C::Shiny } else { rarity_color(c.r) }),
                (format!("{} · {}", BIOMES[c.b].name, RAR_LABEL[c.r]), C::Dimmer),
                (if NOCTURNES.contains(&ci) { "  ◦ nocturne (21 h – 7 h)".into() } else { String::new() }, C::Abyss),
            ],
            btns: vec![],
            act: None,
            indent: 0,
        }];
        rows.push(Row::text("", C::Dim));
        for r in wrap_rows(c.lore, self.panel_w, C::Dim) {
            rows.push(r);
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::text(
            format!("capturés (total) : {}{}", fmt(d.n as f64), if d.s > 0 { format!(" · shinies : {} ⋆", fmt(d.s as f64)) } else { String::new() }),
            C::Text,
        ));
        rows.push(Row::text(
            format!("meilleur rang : {}{}",
                if d.best > 0 { RANK_NAMES[(d.best - 1) as usize] } else { "aucun" },
                if d.bests > 0 { format!(" · shiny : {}", RANK_NAMES[(d.bests - 1) as usize]) } else { String::new() }),
            if d.best >= 4 { C::Gold } else { C::Dim },
        ));
        rows.push(Row::text(
            format!("sexes observés : ♂ {} · ♀ {}", if d.mf & 1 != 0 { "oui" } else { "jamais" }, if d.mf & 2 != 0 { "oui" } else { "jamais" }),
            if d.mf == 3 { C::Green } else { C::Dim },
        ));
        let mut per_rank = String::new();
        for r in (0..4).rev() {
            if iv.nr(r) > 0 {
                per_rank += &format!("{}:♂{}♀{} ", RANK_NAMES[r], iv.m[r], iv.f[r]);
            }
        }
        for r in (0..4).rev() {
            if iv.sr(r) > 0 {
                per_rank += &format!("⋆{}:{} ", RANK_NAMES[r], iv.sr(r));
            }
        }
        rows.push(Row::text(
            format!("en réserve : {}{}{}", iv.tn(), if iv.ts() > 0 { format!(" + {} ⋆", iv.ts()) } else { String::new() },
                if per_rank.is_empty() { String::new() } else { format!("  ({})", per_rank.trim_end()) }),
            C::Dim,
        ));
        rows.push(Row::text(
            format!("valeur (rang C) : {} écus · rang S : {} · shiny S : {}",
                fmt(self.creature_value(ci, false)), fmt(self.creature_value_r(ci, false, 3)), fmt(self.creature_value_r(ci, true, 3))),
            C::GoldDark,
        ));
        (c.n.to_string(), rows)
    }    fn rows_achs(&self) -> (String, Vec<Row>) {
        let done = (0..ACHS.len()).filter(|&i| self.s.ach[i]).count();
        let mut rows = vec![Row::text(format!("{}/{} débloqués", done, ACHS.len()), C::Dim), Row::text("", C::Dim)];
        for i in 0..ACHS.len() {
            let ok = self.s.ach[i];
            let scelle = !ok && ACH_SCELLES.contains(&i);
            rows.push(Row {
                segs: vec![
                    (
                        format!("{} {}", if ok { "■" } else { "□" }, pad(if scelle { "? ? ?" } else { ACHS[i].n }, 26)),
                        if ok { C::Green } else { C::Dimmer },
                    ),
                    (
                        if scelle {
                            "personne n'en parle.".to_string()
                        } else {
                            format!("{}{}", ACHS[i].d, if ACHS[i].r > 0.0 { format!(" (+{} écus)", fmt(ACHS[i].r)) } else { String::new() })
                        },
                        C::Dimmer,
                    ),
                ],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        ("trophées".into(), rows)
    }

    fn rows_help(&self) -> (String, Vec<Row>) {
        let w = self.panel_w;
        let mut rows = vec![Row::header("démarrage rapide — poser son premier piège")];
        for (i, t) in [
            "un piège en bois vous attend déjà en réserve.",
            "déplacez-vous avec les flèches (ou zqsd) : la forêt est à l'ouest du village.",
            "une fois dans la forêt, la ligne sous la carte l'indique — appuyez sur Entrée.",
            "choisissez [poser un piège] avec ↑↓, validez avec Entrée. c'est posé.",
            "le piège tente une capture toutes les 30 s, même le jeu fermé. patience.",
            "les prises s'accumulent en réserve : revendez les doublons à la boutique (porte ╡ o ╞).",
        ]
        .iter()
        .enumerate()
        {
            rows.extend(bullet_rows(&format!("{}. ", i + 1), t, w, C::Text));
        }
        rows.push(Row {
            segs: vec![],
            btns: vec![("compris, fermer ce guide".into(), C::Green, Action::Close)],
            act: None,
            indent: 0,
        });

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("la boucle"));
        for t in [
            "posez des pièges dans les biomes ; ils capturent seuls, à intervalle régulier.",
            "revendez les doublons pour financer de meilleurs pièges, des appâts, de nouveaux biomes et le labo.",
            "l'objectif de fond : compléter le bestiaire — 114 espèces, leurs shinies ⋆, et un rang S partout.",
            "compléter un biome donne +0,04 de chance pour toujours ; le compléter en shiny, +5% à la vente.",
            "les pièges ne s'usent jamais : posés une fois, ils travaillent indéfiniment. l'horlogerie (labo) ne limite que la progression simulée hors-ligne (2 h de base).",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("le village : troc, marchand, foire"));
        for t in [
            "comptoir de troc (touche r) : des collectionneurs échangent des curiosités — six espèces qu'aucun piège n'attrape — contre vos doublons. leurs demandes changent chaque jour.",
            "les curiosités ne se revendent pas et ne comptent pas dans le pourcentage du bestiaire : ce sont des pièces de collection.",
            "marchand ambulant : il s'installe sur la place plusieurs fois par jour et repart vite. sa malle change à chaque passage — breloques de chance, licences de piégeage, lots d'appâts, œufs de curiosité.",
            "jour de foire (un jour sur quatre) : le marchand reste toute la journée, le troc double ses demandes, sa malle est à −25 % et la chance monte de 0,15.",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("les prises — raretés, rangs, shinies"));
        for r in 0..5 {
            rows.push(Row {
                segs: vec![
                    ("· ".into(), C::Dimmer),
                    (pad(RAR_LABEL[r], 13), rarity_color(r)),
                    (format!("valeur de base {} écus", RAR_VAL[r] as u64), C::Dimmer),
                ],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        rows.extend(bullet_rows("· ", "chaque prise reçoit un rang, tiré selon votre chance :", w, C::Dim));
        for r in 0..4 {
            rows.push(Row {
                segs: vec![
                    ("    ".into(), C::Dim),
                    (pad(&format!("rang {}", RANK_NAMES[r]), 9), if r == 3 { C::Gold } else if r == 2 { C::Blue } else { C::Text }),
                    (format!("valeur ×{}", fmt_luck(RANK_MULT[r])), C::Dimmer),
                ],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        rows.extend(bullet_rows("· ", "chaque spécimen est ♂ ou ♀ (50/50). le bestiaire trace les sexes observés, et l'enclos exige un couple — le vrai défi : obtenir un beau ♂ ET une belle ♀.", w, C::Dim));
        rows.extend(bullet_rows("· ", "la vente « sauf couple » et l'auto-vente protègent le meilleur ♂ et la meilleure ♀ ; l'auto-vente et la vente groupée réservent aussi ce qu'attendent les commandes du comptoir et les demandes du troc.", w, C::Dim));
        rows.extend(bullet_rows("· ", "le bestiaire retient le meilleur rang obtenu par espèce, à vie.", w, C::Dim));
        rows.extend(bullet_rows("· ", "la vente écoule toujours les rangs les plus bas d'abord : vos beaux spécimens restent.", w, C::Dim));
        rows.push(Row {
            segs: vec![
                ("· ".into(), C::Dimmer),
                ("shiny ⋆".into(), C::Shiny),
                (format!("  1/{} de base · valeur ×15 · cumulable avec le rang", (1.0 / SHINY_BASE) as u64), C::Dimmer),
            ],
            btns: vec![],
            act: None,
            indent: 0,
        });

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("le temps — météo, saisons, jour et nuit"));
        rows.extend(bullet_rows("· ", "la météo change toutes les 20 minutes et s'applique aussi hors-ligne :", w, C::Dim));
        for m in 1..6 {
            rows.push(Row {
                segs: vec![("    ".into(), C::Dim), (pad(METEOS[m], 14), C::Ice), (weather_desc(m).into(), C::Dimmer)],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        rows.extend(bullet_rows("· ", "chaque jour réel est une saison (cycle de 4) :", w, C::Dim));
        for s in 0..4 {
            rows.push(Row {
                segs: vec![("    ".into(), C::Dim), (pad(SAISONS[s], 14), C::Green), (season_desc(s).into(), C::Dimmer)],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        let mut noct_names: Vec<String> = vec![];
        for &ci in NOCTURNES.iter() {
            noct_names.push(if self.s.dex2[ci].n > 0 { CREATURES[ci].n.to_string() } else { "???".into() });
        }
        rows.extend(bullet_rows("· ", &format!(
            "une journée au village dure deux heures réelles : le soleil y tourne douze fois par jour, saisons comprises. la nuit (21 h – 7 h à l'horloge du village, soit cinquante minutes réelles) fait sortir vingt espèces nocturnes ◦, introuvables le jour : {}.",
            noct_names.join(", ")), w, C::Dim));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("sur le terrain"));
        for t in [
            "battue : dans un biome, déclenchez vous-même tous vos pièges avec +0,2 chance — ou tentez votre chance à mains nues s'il n'y en a aucun (repos 5 min, réductible au labo). chaque battue remue le terrain : pendant une heure, chaque tirage de légende gagne +2 points (20% de base), et les battues se cumulent jusqu'à +16.",
            "appâts : consommés à chaque tentative du piège équipé ; effets décrits à la boutique.",
            "légende errante : une silhouette ✧ paraît parfois sur la carte — un tirage toutes les 10 min, 20 chances sur 100, et elle reste vingt minutes — y compris sur les terres que vous n'avez pas encore ouvertes. approchez-la et tentez votre chance, une seule fois. la prise est toujours une des 12 légendes errantes, un bestiaire qu'aucun piège n'attrape, rang A minimum. le labo améliore l'appel (leur fréquence) et l'approche (votre réussite).",
            "contrats [c] : trois commandes toutes les 2 h, payées bien au-dessus du marché. la livraison ne prend jamais les shinies ni votre meilleur couple ♂♀.",
            "des traces fraîches ∵ apparaissent sur la carte : approchez-vous et faites Entrée pour les suivre. tout se joue immédiatement — trois fois sur cinq une prise offerte du biome, une fois sur sept une cache d'appâts, sinon la piste se perd. une trace ne se suit qu'une fois.",
            "chaque jour de chasse consécutif augmente votre élan (+0,015 de chance par jour, jusqu'à +0,15) et offre quelques baies. la série retombe si vous sautez un jour.",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("les bâtiments du village"));
        for t in [
            "boutique ╡ o ╞ : acheter pièges et appâts, vendre vos prises, régler l'auto-vente.",
            "labo ╡ l ╞ : améliorations permanentes (vitesse, chance, prix, hors-ligne, shiny) et la migration.",
            "bestiaire ╡ b ╞ : le registre — découvertes, shinies, meilleurs rangs. jamais décrémenté par les ventes.",
            "trophées ╡ t ╞ : la liste des succès et leurs récompenses.",
            "musée ╡ m ╞ : exposez vos plus beaux spécimens ; chacun génère des écus en continu (cagnotte plafonnée à 4 h de base, extensible au labo). le spécimen exposé quitte la réserve, récupérable à tout moment. le garnissage automatique choisit seul les pièces les mieux payées et suit vos nouvelles prises.",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }
        rows.extend(bullet_rows("· ", &format!(
            "enclos ╡ e ╞ : un couple ♂+♀ d'une même espèce donne une naissance ({}). {}% de chance de monter d'un rang (25% de base, +4 par niveau de lignées), shiny ×3. le petit naît au meilleur rang de ses parents, qui sont consommés : par défaut les plus bas rangs de chaque sexe, ou un rang que vous choisissez pour viser plus haut.",
            (0..5).map(|r| format!("{} {} min", RAR_LABEL[r], PEN_MIN[r] as u64)).collect::<Vec<_>>().join(" · "),
            (self.pen_rankup() * 100.0).round() as u64), w, C::Dim));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("la migration"));
        for t in [
            "au labo, quand une expédition a bien rapporté (et contre des frais de voyage qui doublent à chaque départ) : repartez de zéro avec des trophées permanents (+0,008 chance et +1% vente chacun).",
            "conservés : bestiaire, succès, trophées. perdus : écus, pièges, appâts, labo, réserve, musée, enclos.",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("raccourcis"));
        for t in [
            "carte : flèches/zqsd déplacer · Entrée interagir · Échap fermer les panneaux",
            "[v] tableau de bord · [i] inventaire · [b] bestiaire · [o] boutique · [c] contrats",
            "[l] labo · [m] musée · [e] enclos · [t] trophées · [j] journal · [?] cette aide",
            "panneaux : ↑↓/jk naviguer · ←→ changer de bouton · Entrée valider · PgUp/PgDn défiler",
            if cfg!(target_arch = "wasm32") { "fermez l'onglet quand vous voulez — la partie est sauvegardée toutes les 10 s. les touches + et - ajustent la taille du texte." } else { "quitter : ctrl+c — la partie est sauvegardée à la sortie (et toutes les 10 s de toute façon)." },
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dimmer));
        }

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("statistiques"));
        rows.push(Row::text(
            format!("tentatives {} · captures {} · shinies {} · battues {}", fmt(self.s.attempts as f64), fmt(self.s.captures as f64), fmt(self.s.shinies as f64), fmt(self.s.hunts_done as f64)),
            C::Dim,
        ));
        rows.push(Row::text(
            format!("contrats livrés {} · naissances {} · légendes {} · migrations {}", self.s.contracts_delivered, self.s.pen_born, self.s.legends_caught, self.s.migrations),
            C::Dim,
        ));
        rows.push(Row::text(
            format!("écus gagnés (total) {}", fmt(self.s.total_earned)),
            C::Dim,
        ));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("sauvegarde"));
        #[cfg(not(target_arch = "wasm32"))]
        rows.push(Row::text(format!("automatique dans {} — copiez ce fichier pour changer de machine.", save_path().display()), C::Dimmer));
        #[cfg(target_arch = "wasm32")]
        rows.push(Row::text("automatique dans ce navigateur (localStorage).", C::Dimmer));
        rows.push(Row {
            segs: vec![],
            btns: vec![("tout effacer".into(), C::Red, Action::Open(PanelKind::ResetConfirm))],
            act: None,
            indent: 0,
        });
        ("aide & manuel".into(), rows)
    }    fn rows_journal(&self) -> (String, Vec<Row>) {
        if self.logs.is_empty() {
            return ("journal".into(), vec![Row::text("rien à signaler pour l'instant.", C::Dimmer)]);
        }
        let rows = self
            .logs
            .iter()
            .map(|l| {
                let mut segs = vec![(format!("[{}] ", l.t), C::Dimmer)];
                segs.extend(l.segs.clone());
                Row { segs, btns: vec![], act: None, indent: 0 }
            })
            .collect();
        ("journal".into(), rows)
    }

    fn merch_label(&self, item: usize) -> (String, String) {
        match item {
            0 => (
                "œuf de curiosité".into(),
                "éclot en une espèce qu'aucun piège n'attrape. laquelle ? il ne le sait pas non plus.".into(),
            ),
            1 => (
                format!("breloque de chance (vous en avez {})", self.s.charms),
                "chance +0,10, pour toujours. le prix monte à chaque breloque.".into(),
            ),
            2 => (
                format!("dix {}", BAITS[self.best_bait()].n),
                format!("son lot du jour, à prix d'ami. {}", BAITS[self.best_bait()].desc),
            ),
            3 => (
                format!("licence de piégeage (vous en avez {})", self.s.licences),
                if self.s.lab[LAB_LICENCE] < LABS[LAB_LICENCE].max {
                    format!(
                        "un emplacement de piège de plus, partout — 40 % de moins que le prochain palier du labo ({} écus).",
                        fmt(self.lab_cost(LAB_LICENCE))
                    )
                } else {
                    "un emplacement de plus, au-delà de ce que le labo autorise. les autorités ferment les yeux.".into()
                },
            ),
            _ => {
                let t = self.next_trap();
                (format!("plan de {}", TRAPS[t].n), "un piège monté la veille, vendu moitié prix.".into())
            }
        }
    }
    fn next_trap(&self) -> usize {
        let best = (0..TRAPS.len()).rev().find(|&t| self.s.traps[t] > 0).unwrap_or(0);
        (best + 1).min(TRAPS.len() - 1)
    }
    fn rows_merchant(&self) -> (String, Vec<Row>) {
        let title = "marchand ambulant".to_string();
        let Some((w, fin)) = self.merchant_now() else {
            let quand = match self.merchant_next() {
                Some(a) => {
                    let dans = (a - now_ms()) / 60_000.0;
                    if dans >= 90.0 {
                        format!("il revient dans {} h environ.", (dans / 60.0).round().max(1.0) as u64)
                    } else {
                        format!("il revient dans {} min environ.", dans.ceil().max(1.0) as u64)
                    }
                }
                None => "il repassera, il repasse toujours.".to_string(),
            };
            return (
                title,
                vec![
                    Row::text("l'étal est vide : le marchand est sur les routes.", C::Dimmer),
                    Row::text(quand, C::Blue),
                    Row::text("", C::Dim),
                    Row::text("il s'installe plusieurs fois par jour, ne reste que trois heures, et tient boutique toute la journée les jours de foire.", C::Dimmer),
                ],
            );
        };
        let mut rows = Vec::new();
        let reste = fin - now_ms();
        for r in wrap_rows(
            "il déballe sa malle sur la place, ne reste jamais longtemps, et ne vend chaque chose qu'une fois par passage.",
            self.panel_w,
            C::Dimmer,
        ) {
            rows.push(r);
        }
        rows.push(Row::header(&format!(
            "sa malle — il repart dans {} min{}",
            (reste / 60_000.0).ceil() as u64,
            if fair_day() { " · jour de foire : −25 %" } else { "" }
        )));
        for item in self.merchant_stock(w) {
            let key = w * 8 + item as u64;
            let vendu = self.s.merchant_done.contains(&key);
            let prix = self.merchant_price(item);
            let (nom, desc) = self.merch_label(item);
            rows.push(Row {
                segs: vec![
                    (pad(&nom, 40), C::Text),
                    (pad(&format!("{} écus", fmt(prix)), 16), if self.s.ecus >= prix { C::GoldDark } else { C::Red }),
                ],
                btns: if vendu || self.s.ecus < prix {
                    vec![]
                } else {
                    vec![("acheter".into(), C::Gold, Action::MerchBuy(item))]
                },
                act: None,
                indent: 0,
            });
            for r in bullet_rows("  ", &if vendu { format!("{} — déjà vendu", desc) } else { desc }, self.panel_w, C::Dimmer) {
                rows.push(r);
            }
        }
        (title, rows)
    }

    fn rows_trade(&self) -> (String, Vec<Row>) {
        let title = "comptoir de troc".to_string();
        let mut rows = Vec::new();
        for r in wrap_rows(
            "des collectionneurs de passage échangent des espèces qu'aucun piège n'attrape contre vos doublons. leurs demandes changent chaque jour.",
            self.panel_w,
            C::Dimmer,
        ) {
            rows.push(r);
        }
        if self.s.trades.is_empty() {
            rows.push(Row::text("", C::Dim));
            rows.push(Row::text(
                "personne aujourd'hui. revenez avec quelques espèces au bestiaire : on ne troque pas contre du vide.",
                C::Dimmer,
            ));
            return (title, rows);
        }
        let reste = 86_400_000.0 - (now_ms() % 86_400_000.0);
        rows.push(Row::header(&format!(
            "demandes du jour — renouvelées dans {} h{}",
            (reste / 3_600_000.0).ceil() as u64,
            if fair_day() { " · jour de foire : deux fois plus d'offres" } else { "" }
        )));
        for (i, &(want, qty, give)) in self.s.trades.iter().enumerate() {
            let done = self.s.trades_done.get(i).copied().unwrap_or(false);
            let have = self.s.inv2[want].tn();
            let (wc, gc) = (&CREATURES[want], &CREATURES[give]);
            let mut segs = vec![
                (pad(&format!("{} {} ×{}", wc.g, wc.n, qty), 30), rarity_color(wc.r)),
                ("→ ".into(), C::Dimmer),
                (pad(&format!("{} {}", gc.g, gc.n), 22), rarity_color(gc.r)),
            ];
            if done {
                segs.push(("déjà échangé".into(), C::Dimmer));
            } else {
                segs.push((format!("vous en avez {}", have), if have >= qty { C::Green } else { C::Dimmer }));
            }
            rows.push(Row {
                segs,
                btns: if !done && have >= qty {
                    vec![("échanger".into(), C::Gold, Action::Trade(i))]
                } else {
                    vec![]
                },
                act: None,
                indent: 0,
            });
        }
        rows.push(Row::text("", C::Dim));
        rows.push(Row::text(
            "les curiosités ne se revendent pas et ne comptent pas dans le pourcentage du bestiaire : ce sont des pièces de collection.",
            C::Dimmer,
        ));
        (title, rows)
    }

    fn rows_news(&self) -> (String, Vec<Row>) {
        let mut rows = Vec::new();
        for (i, (v, date, lignes)) in NEWS.iter().enumerate() {
            if i > 0 {
                rows.push(Row::text("", C::Dim));
            }
            rows.push(Row::header(&format!("version {} — {}", v, date)));
            for l in lignes.iter() {
                for r in bullet_rows("· ", l, self.panel_w, C::Text) {
                    rows.push(r);
                }
            }
        }
        ("quoi de neuf".into(), rows)
    }

    fn rows_board(&self) -> (String, Vec<Row>) {
        let title = "palmarès du comptoir".to_string();
        if cfg!(not(target_arch = "wasm32")) {
            return (
                title,
                vec![Row::text("le classement n'existe que dans la version navigateur.", C::Dimmer)],
            );
        }
        let mut rows = Vec::new();
        if self.board.is_empty() {
            rows.push(Row::text(
                match self.board_state {
                    0 => "chargement du classement…",
                    2 => "classement injoignable (hors-ligne ?).",
                    _ => "personne n'est encore inscrit.",
                },
                C::Dimmer,
            ));
        } else {
            rows.push(Row {
                segs: vec![(
                    format!(
                        "{}{}{}{}{}{}",
                        pad("#", 3),
                        pad("piégeur", 17),
                        format!("{:>7}", "score"),
                        format!("{:>10}", "espèces"),
                        format!("{:>9}", "shinies"),
                        format!("{:>11}", "captures")
                    ),
                    C::Dimmer,
                )],
                btns: vec![],
                act: None,
                indent: 0,
            });
            for (i, e) in self.board.iter().enumerate() {
                let me = !self.board_me.is_empty() && e.pseudo == self.board_me;
                let pos_c = match i {
                    0 => C::Gold,
                    1 => C::Text,
                    2 => C::GoldDark,
                    _ => C::Dimmer,
                };
                rows.push(Row {
                    segs: vec![
                        (pad(&format!("{}", i + 1), 3), pos_c),
                        (pad(&e.pseudo, 17), if me { C::Gold } else { C::Blue }),
                        (format!("{:>7}", fmt(e.score)), C::GoldDark),
                        (format!("{:>7}/{}", fmt(e.especes), wild_total()), C::Text),
                        (format!("{:>9}", fmt(e.shinies)), if e.shinies > 0.0 { C::Shiny } else { C::Dimmer }),
                        (format!("{:>11}", fmt(e.captures)), C::Dim),
                    ],
                    btns: vec![],
                    act: None,
                    indent: 0,
                });
            }
        }
        rows.push(Row::text("", C::Dim));
        if self.board_me.is_empty() {
            rows.push(Row::text(
                "vous n'êtes pas inscrit : choisissez un pseudo dans « ⇄ session partagée », en bas à droite de la page.",
                C::Dimmer,
            ));
        } else {
            rows.push(Row::text(
                format!("vous jouez sous « {} ». vos statistiques repartent toutes les 5 minutes.", self.board_me),
                C::Dimmer,
            ));
        }
        rows.extend(bullet_rows(
            "",
            "score = espèces×100 + shinies×300 + cote×40 + curiosités×500 + légendes×1000 + trophées×1000 + écus gagnés÷1000.",
            self.panel_w,
            C::Dimmer,
        ));
        if let Some(moi) = self.board.iter().find(|e| !self.board_me.is_empty() && e.pseudo == self.board_me) {
            if moi.curiosites > 0.0 || moi.legendes > 0.0 {
                rows.push(Row::text(
                    format!(
                        "vos {} curiosité{} et {} légende{} y pèsent {} points.",
                        moi.curiosites as u64,
                        if moi.curiosites > 1.0 { "s" } else { "" },
                        moi.legendes as u64,
                        if moi.legendes > 1.0 { "s" } else { "" },
                        fmt(moi.curiosites * PTS_CURIOSITE + moi.legendes * PTS_LEGENDE)
                    ),
                    C::Gold,
                ));
            }
        }
        (title, rows)
    }

    fn rows_offline(&self, sum: &OfflineSummary) -> (String, Vec<Row>) {
        let mut rows = vec![Row::text(
            format!(
                "vos pièges ont travaillé {}{}",
                if sum.h > 0 { format!("{} h {} min", sum.h, sum.m) } else { format!("{} min", sum.m) },
                if sum.hit_cap { " (plafond atteint — voir horlogerie au labo)" } else { "" }
            ),
            C::Dim,
        )];
        rows.push(Row::text("", C::Dim));
        rows.push(Row::text(format!("├─ captures : {}", fmt(sum.caught as f64)), C::Green));
        if sum.shinies > 0 {
            rows.push(Row::text(format!("├─ shinies : {} ⋆", sum.shinies), C::Blue));
        }
        if sum.earned > 0.0 {
            rows.push(Row::text(format!("├─ écus gagnés (auto-vente, succès) : +{}", fmt(sum.earned)), C::GoldDark));
        }
        let mut segs = vec![("└─ nouvelles espèces : ".to_string(), C::Text)];
        if sum.discoveries.is_empty() {
            segs.push(("aucune".into(), C::Dimmer));
        } else {
            for (k, &ci) in sum.discoveries.iter().take(6).enumerate() {
                segs.push((CREATURES[ci].n.to_string(), rarity_color(CREATURES[ci].r)));
                if k + 1 < sum.discoveries.len().min(6) {
                    segs.push((", ".into(), C::Dim));
                }
            }
            if sum.discoveries.len() > 6 {
                segs.push(("…".into(), C::Dim));
            }
        }
        rows.push(Row { segs, btns: vec![], act: None, indent: 0 });
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![],
            btns: vec![("reprendre la traque".into(), C::Green, Action::Close)],
            act: None,
            indent: 0,
        });
        ("pendant votre absence".into(), rows)
    }

    fn rows_reset(&self) -> (String, Vec<Row>) {
        let mut rows = wrap_rows("bestiaire, succès, trophées : tout disparaît. définitivement.", self.panel_w, C::Red);
        rows.push(Row::text("", C::Dim));
        rows.push(Row {
            segs: vec![],
            btns: vec![
                ("effacer ma partie".into(), C::Red, Action::DoReset),
                ("annuler".into(), C::Green, Action::Close),
            ],
            act: None,
            indent: 0,
        });
        ("tout effacer".into(), rows)
    }
}

/* ================================================================== format */

/* on tronque au lieu d'arrondir : avec 2 999 500 écus, « 3,00 M » laissait
   croire qu'un biome à 3 M était payable alors que le déblocage refusait. */
fn tronque(x: f64, dec: i32) -> f64 {
    let p = 10f64.powi(dec);
    (x * p).floor() / p
}

fn fmt(n: f64) -> String {
    let n = n.floor();
    if n >= 1e9 {
        return format!("{:.2} Md", tronque(n / 1e9, 2)).replace('.', ",");
    }
    if n >= 1e6 {
        return format!("{:.2} M", tronque(n / 1e6, 2)).replace('.', ",");
    }
    if n >= 10000.0 {
        return format!("{:.1} k", tronque(n / 1e3, 1)).replace('.', ",").replace(",0 k", " k");
    }
    let s = format!("{}", n as i64);
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(*ch);
    }
    out
}
fn fmt2(n: f64) -> String {
    format!("{:.2}", n).replace('.', ",")
}
fn fmt_luck(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{}", n).replace('.', ",") }
}
fn ascii_bar(frac: f64, width: usize) -> String {
    let n = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("[{}{}]", "█".repeat(n), "·".repeat(width - n))
}

/* ==================================================================== rendu */

fn draw_str(buf: &mut Buffer, area: Rect, x: i32, y: i32, s: &str, st: Style) {
    if y < 0 || y >= area.height as i32 || x >= area.width as i32 {
        return;
    }
    let (skip, x0) = if x < 0 { ((-x) as usize, 0u16) } else { (0, x as u16) };
    let vis: String = s.chars().skip(skip).collect();
    if vis.is_empty() {
        return;
    }
    let maxw = (area.width - x0) as usize;
    buf.set_stringn(area.x + x0, area.y + y as u16, &vis, maxw, st);
}

fn selectables(rows: &[Row]) -> Vec<(usize, isize)> {
    let mut out = vec![];
    for (i, r) in rows.iter().enumerate() {
        if r.act.is_some() {
            out.push((i, -1));
        }
        for j in 0..r.btns.len() {
            out.push((i, j as isize));
        }
    }
    out
}

fn render(game: &mut Game, theme: &Theme, buf: &mut Buffer, area: Rect) {
    let cols = area.width as i32;
    let rows_n = area.height as i32;
    if cols < 70 || rows_n < 22 {
        draw_str(buf, area, 2, 1, "terminal trop petit — 70×22 minimum", theme.style(C::Red, false));
        return;
    }

    // ---- monde ----
    let vy0 = 1i32;
    let vy1 = rows_n - 6;
    let vh = vy1 - vy0 + 1;
    let vw = cols - 2;
    let mut cam_x = game.px - vw / 2;
    let mut cam_y = game.py - vh / 2;
    cam_x = cam_x.clamp(0, (MAPW as i32 - vw).max(0));
    cam_y = cam_y.clamp(0, (MAPH as i32 - vh).max(0));
    let off_x = if vw > MAPW as i32 { (vw - MAPW as i32) / 2 } else { 0 };
    let off_y = if vh > MAPH as i32 { (vh - MAPH as i32) / 2 } else { 0 };

    for sy in 0..vh {
        let wy = cam_y + sy - off_y;
        if wy < 0 || wy >= MAPH as i32 {
            continue;
        }
        for sx in 0..vw {
            let wx = cam_x + sx - off_x;
            if wx < 0 || wx >= MAPW as i32 {
                continue;
            }
            let cell = game.world.cells[wy as usize][wx as usize];
            if cell.ch != ' ' {
                draw_str(buf, area, 1 + sx, vy0 + sy, &cell.ch.to_string(), theme.style(cell.c, false));
            }
        }
    }
    /* la soif du puits se voit de loin : la margelle bat d'une lueur rouge,
       c'est le seul indice donné au joueur */
    let maintenant = now_ms();
    /* rassasié, il ne luit plus : la lueur annonce une offrande possible, pas
       seulement une heure de la nuit */
    if game.puits_luit(maintenant) {
        let pulse = (maintenant / 600.0) as u64 % 2 == 0;
        for (i, line) in ["╭─╮", "╰─╯"].iter().enumerate() {
            for (j, ch) in line.chars().enumerate() {
                let sx = 1 + 53 + j as i32 - cam_x + off_x;
                let sy = vy0 + 38 + i as i32 - cam_y + off_y;
                if sy < vy0 || sy > vy1 {
                    continue;
                }
                draw_str(buf, area, sx, sy, &ch.to_string(), theme.style(if pulse { C::Red } else { C::GoldDark }, false));
            }
        }
    }
    // le cercle des 666 prises : purement décoratif, il s'efface tout seul
    if now_ms() < game.pentacle_until {
        const PENTACLE: [&str; 5] = [
            "  ·─────────·  ",
            " ╱  ⋆     ⋆  ╲ ",
            "│      ✧      │",
            " ╲  ⋆     ⋆  ╱ ",
            "  ·─────────·  ",
        ];
        let pulse = (now_ms() / 350.0) as u64 % 2 == 0;
        for (i, line) in PENTACLE.iter().enumerate() {
            for (j, ch) in line.chars().enumerate() {
                // les espaces sont dessinés eux aussi : le cercle doit effacer
                // le pavage sous lui, sinon il se noie dans le décor
                let wx = 40 + j as i32; // à gauche de la fontaine, sur la place
                let wy = 36 + i as i32;
                let sx = 1 + wx - cam_x + off_x;
                let sy = vy0 + wy - cam_y + off_y;
                if sy < vy0 || sy > vy1 {
                    continue;
                }
                let c = match ch {
                    '✧' => C::Gold,
                    '⋆' => {
                        if pulse {
                            C::Purple
                        } else {
                            C::Red
                        }
                    }
                    _ => {
                        if pulse {
                            C::Red
                        } else {
                            C::Purple
                        }
                    }
                };
                draw_str(buf, area, sx, sy, &ch.to_string(), theme.style(c, false));
            }
        }
    }

    // le marchand ambulant, quand il est de passage
    {
        // l'étal reste visible même quand il n'y a personne : sinon rien
        // n'indique au joueur que ce marchand existe
        let ici = game.merchant_now().is_some();
        let (mx, my) = Game::MERCHANT_POS;
        let sx = 1 + mx as i32 - cam_x + off_x;
        let sy = vy0 + my as i32 - cam_y + off_y;
        if sy >= vy0 && sy <= vy1 {
            let (txt, c) = if ici { ("╡ marchand ╞", C::Gold) } else { ("╡ étal vide ╞", C::Dimmer) };
            draw_str(buf, area, sx, sy, txt, theme.style(c, false));
        }
    }

    // étiquettes de biomes
    for b in 0..WILDB {
        let (lx, ly) = LABEL_POS[b];
        let sx = 1 + lx as i32 - cam_x + off_x;
        let sy = vy0 + ly as i32 - cam_y + off_y;
        if sy < vy0 || sy > vy1 {
            continue;
        }
        let owned = game.s.biomes[b].is_some();
        let legend_here = matches!(game.legend_now(), Some((_, lb, _)) if lb == b);
        let lbl = if owned {
            if legend_here { format!("╡ {} ✧ ╞", BIOMES[b].name) } else { format!("╡ {} ╞", BIOMES[b].name) }
        } else {
            format!("╡ {} × ╞", BIOMES[b].name)
        };
        draw_str(buf, area, sx, sy, &lbl, theme.style(if legend_here { C::Gold } else if owned { C::White } else { C::Red }, false));
        if owned {
            let placed = game.s.biomes[b].as_ref().unwrap().pl.iter().flatten().count();
            if placed > 0 && sy + 1 <= vy1 {
                draw_str(buf, area, sx + 1, sy + 1, &format!("{} piège{}", placed, if placed > 1 { "s" } else { "" }), theme.style(C::GoldDark, false));
            }
        }
    }
    // traces fraîches
    for (_, _, (tx, ty)) in game.traces_now() {
        let sx = 1 + tx as i32 - cam_x + off_x;
        let sy = vy0 + ty as i32 - cam_y + off_y;
        if sy >= vy0 && sy <= vy1 {
            draw_str(buf, area, sx, sy, "∵", theme.style(C::Gold, false));
        }
    }
    // légende errante
    if let Some((_, _, (lx, ly))) = game.legend_now() {
        let sx = 1 + lx as i32 - cam_x + off_x;
        let sy = vy0 + ly as i32 - cam_y + off_y;
        if sy >= vy0 && sy <= vy1 {
            draw_str(buf, area, sx - 1, sy, ">", theme.style(C::Gold, false));
            draw_str(buf, area, sx, sy, "✧", theme.style(C::Shiny, false));
            draw_str(buf, area, sx + 1, sy, "<", theme.style(C::Gold, false));
        }
    }
    // joueur
    let psx = 1 + game.px - cam_x + off_x;
    let psy = vy0 + game.py - cam_y + off_y;
    draw_str(buf, area, psx - 1, psy, "(_)", theme.style(C::White, false));
    draw_str(buf, area, psx, psy - 1, "o", theme.style(C::White, false));

    // ---- séparateur + contexte ----
    let sep_y = rows_n - 5;
    draw_str(buf, area, 0, sep_y, &format!("├{}┤", "─".repeat((cols - 2) as usize)), theme.style(C::Dimmer, false));
    let (hint, hint_c) = game.zone_hint();
    draw_str(buf, area, 3, sep_y, &format!(" {} ", hint), theme.style(hint_c, false));
    let nowc = now_ms();
    let cond = format!(
        " {} · {} · {}{} ",
        SAISONS[season_at(nowc)],
        METEOS[weather_at(nowc)],
        &if is_night_at(nowc) { format!("{} h ◦", heure_jeu(nowc) as u32) } else { format!("{} h", heure_jeu(nowc) as u32) },
        if fair_day() { " · foire ✧" } else { "" }
    );
    draw_str(buf, area, cols - cond.chars().count() as i32 - 3, sep_y, &cond, theme.style(C::Ice, false));
    if let Some((wid, lb, _)) = game.legend_now() {
        let left = game.legend_left_min(wid, nowc);
        let lg = format!(" ✧ légende en {} ({} min) ", BIOMES[lb].name, left);
        let lx = cols - cond.chars().count() as i32 - lg.chars().count() as i32 - 4;
        draw_str(buf, area, lx, sep_y, &lg, theme.style(C::Gold, false));
    }

    /* ---- journal ----
       deux colonnes : les prises à gauche, tout le reste à droite. une éclosion
       ou une légende ne doit pas être chassée par le flot des captures. */
    {
        let mi = cols / 2; // la cloison
        let colonnes: [(i32, i32, bool); 2] = [(2, mi - 1, true), (mi + 2, cols - 2, false)];
        for (x0, x1, prises) in colonnes {
            let mut i = 0usize;
            for l in game.logs.iter().filter(|l| l.prise == prises).take(3) {
                let y = rows_n - 4 + i as i32;
                i += 1;
                let mut x = x0;
                draw_str(buf, area, x, y, &format!("[{}] ", l.t), theme.style(C::Dimmer, false));
                x += 11;
                for (txt, c) in &l.segs {
                    if x >= x1 {
                        break;
                    }
                    let t: String = txt.chars().take((x1 - x) as usize).collect();
                    draw_str(buf, area, x, y, &t, theme.style(*c, false));
                    x += t.chars().count() as i32;
                }
            }
            if i == 0 {
                draw_str(
                    buf,
                    area,
                    x0,
                    rows_n - 4,
                    if prises { "aucune prise pour l'instant." } else { "rien à signaler." },
                    theme.style(C::Dimmer, false),
                );
            }
        }
        for y in rows_n - 4..rows_n - 1 {
            draw_str(buf, area, mi, y, "│", theme.style(C::Dim, false));
        }
        draw_str(buf, area, mi, sep_y, "┬", theme.style(C::Dim, false));
    }

    // ---- cadre ----
    draw_str(buf, area, 0, 0, &format!("┌{}┐", "─".repeat((cols - 2) as usize)), theme.style(C::Dim, false));
    draw_str(buf, area, 0, rows_n - 1, &format!("└{}┘", "─".repeat((cols - 2) as usize)), theme.style(C::Dim, false));
    for y in 1..rows_n - 1 {
        if y != sep_y {
            draw_str(buf, area, 0, y, "│", theme.style(C::Dim, false));
            draw_str(buf, area, cols - 1, y, "│", theme.style(C::Dim, false));
        }
    }
    // titre + stats
    let mut tx = 2;
    draw_str(buf, area, tx, 0, " affut.sh ", theme.style(C::Green, false));
    tx += 10;
    if (now_ms() as u64 / 1000) % 2 == 0 {
        draw_str(buf, area, tx, 0, "▌", theme.style(C::Green, false));
    }
    tx += 1;
    let dex = wild_species().filter(|&i| game.s.dex2[i].n > 0).count();
    let stats: Vec<(String, C)> = vec![
        (format!(" {} écus ", fmt(game.s.ecus)), C::Gold),
        (format!("· {} captures ", fmt(game.s.captures as f64)), C::Dim),
        (format!("· bestiaire {}% ", (dex as f64 / CREATURES.len() as f64 * 100.0).round() as u64), C::Blue),
        (if game.s.trophies > 0 { format!("· {} trophées ", game.s.trophies) } else { String::new() }, C::GoldDark),
    ];
    for (txt, c) in stats {
        draw_str(buf, area, tx, 0, &txt, theme.style(c, false));
        tx += txt.chars().count() as i32;
    }
    // raccourcis
    // la barre s'adapte à la largeur : plutôt trois versions complètes qu'une
    // seule tronquée en plein milieu d'un raccourci
    let pastille = if game.s.news_seen != VERSION { " ●" } else { "" };
    let web = cfg!(target_arch = "wasm32");
    let fin = if web { "+/- taille" } else { "ctrl+c quitter" };
    let large = format!(
        " zqsd/←↑↓→ · Entrée · [v]ue [i]nvent. [b]estiaire b[o]utique [c]ontrats [l]abo [m]usée [e]nclos [t]rophées [j]ournal t[r]oc [p]almarès [n]ouveautés{} [?] aide · {} ",
        pastille, fin
    );
    let moyen = format!(
        " zqsd · Entrée · [v]ue [i]nv [b]est b[o]utique [c]ontrats [l]abo [m]usée [e]nclos [t]roph [j]ourn t[r]oc [p]alm [n]euf{} [?] aide ",
        pastille
    );
    let court = format!(" zqsd · Entrée · [?] aide · [n]ouveautés{} ", pastille);
    let dispo = (cols - 4).max(1) as usize;
    let kb = [large, moyen, court]
        .into_iter()
        .find(|s| s.chars().count() <= dispo)
        .unwrap_or_else(|| " zqsd · Entrée · [?] aide ".to_string());
    let kbt: String = kb.chars().take(dispo).collect();
    draw_str(buf, area, 2, rows_n - 1, &kbt, theme.style(C::Dimmer, false));

    // ---- toasts ----
    for (i, (msg, _)) in game.toasts.iter().enumerate() {
        let wdt = (msg.chars().count() + 4).min((cols - 4) as usize);
        let x = cols - wdt as i32 - 2;
        let y = 2 + i as i32 * 3;
        let inner: String = msg.chars().take(wdt - 4).collect();
        draw_str(buf, area, x, y, &format!("┌{}┐", "─".repeat(wdt - 2)), theme.style(C::Green, false));
        draw_str(buf, area, x, y + 1, &format!("│ {} │", pad(&inner, wdt - 4)), theme.style(C::Green, false));
        draw_str(buf, area, x, y + 2, &format!("└{}┘", "─".repeat(wdt - 2)), theme.style(C::Green, false));
    }

    // ---- panneau ----
    if !game.panels.is_empty() {
        draw_panel(game, theme, buf, area);
    }
}

fn draw_panel(game: &mut Game, theme: &Theme, buf: &mut Buffer, area: Rect) {
    let cols = area.width as i32;
    let rows_n = area.height as i32;
    let (kind, psel, pscroll) = {
        let p = game.panels.last().unwrap();
        (p.kind.clone(), p.sel, p.scroll)
    };
    let pw = 80.min(cols - 6);
    game.panel_w = (pw as usize).saturating_sub(5);
    let (title, rows) = game.build_rows(&kind);
    let ph = (rows_n - 6).min(rows.len() as i32 + 4);
    let px = (cols - pw) / 2;
    let py = (rows_n - ph) / 2;
    let inner = (ph - 4) as usize;

    let sels = selectables(&rows);
    let sel = psel.min(sels.len().saturating_sub(1));
    let cur = sels.get(sel).copied();

    let scroll = pscroll.min(rows.len().saturating_sub(inner));
    {
        let p = game.panels.last_mut().unwrap();
        p.inner = inner;
        p.scroll = scroll;
    }

    // fond + cadre
    for y in py..py + ph {
        draw_str(buf, area, px, y, &" ".repeat(pw as usize), theme.style(C::Text, true));
    }
    draw_str(buf, area, px, py, &format!("┌{}┐", "─".repeat((pw - 2) as usize)), theme.style(C::Dim, true));
    draw_str(buf, area, px, py + ph - 1, &format!("└{}┘", "─".repeat((pw - 2) as usize)), theme.style(C::Dim, true));
    for y in py + 1..py + ph - 1 {
        draw_str(buf, area, px, y, "│", theme.style(C::Dim, true));
        draw_str(buf, area, px + pw - 1, y, "│", theme.style(C::Dim, true));
    }
    draw_str(buf, area, px + 2, py, &format!(" {} ", title), theme.style(C::Gold, true));
    draw_str(buf, area, px + 2, py + ph - 1, " ↑↓ naviguer · Entrée valider · Échap fermer ", theme.style(C::Dimmer, true));
    let info = if rows.len() > inner {
        format!(" {}-{}/{} ", scroll + 1, (scroll + inner).min(rows.len()), rows.len())
    } else {
        " tout est affiché ".to_string()
    };
    draw_str(buf, area, px + pw - info.chars().count() as i32 - 2, py + ph - 1, &info, theme.style(C::Dimmer, true));

    for i in 0..inner {
        let ri = scroll + i;
        if ri >= rows.len() {
            break;
        }
        let r = &rows[ri];
        let mut x = px + 2 + r.indent as i32;
        let y = py + 2 + i as i32;
        let row_sel = cur == Some((ri, -1));
        for (txt, c) in &r.segs {
            let st = if row_sel { theme.style(C::Sel, false) } else { theme.style(*c, true) };
            let avail = (px + pw - 1 - x).max(0) as usize;
            let t: String = txt.chars().take(avail).collect();
            draw_str(buf, area, x, y, &t, st);
            x += t.chars().count() as i32;
        }
        for (j, (label, c, _)) in r.btns.iter().enumerate() {
            x += 1;
            let is_sel = cur == Some((ri, j as isize));
            let lbl = format!("[{}]", label);
            let st = if is_sel { theme.style(C::Sel, false) } else { theme.style(*c, true) };
            let avail = (px + pw - 1 - x).max(0) as usize;
            let t: String = lbl.chars().take(avail).collect();
            draw_str(buf, area, x, y, &t, st);
            x += t.chars().count() as i32;
        }
    }
}

/* =================================================================== main */

/* un cran de molette fait trois lignes : le pas d'un seul cran paraît collant,
   celui d'une page entière fait perdre le fil. le web n'en a pas besoin : il
   convertit les pixels que lui donne le navigateur avec l'interligne réel. */
#[cfg(not(target_arch = "wasm32"))]
const WHEEL_LINES: i32 = 3;

/* molette et trackpad : on déplace la fenêtre du panneau sans toucher à la
   sélection — le geste sert à lire, pas à choisir. renvoie true si un panneau
   était ouvert, pour que le web sache s'il doit retenir le défilement de page. */
fn panel_scroll(game: &mut Game, lines: i32) -> bool {
    let Some(panel) = game.panels.last() else { return false };
    let (_, rows) = game.build_rows(&panel.kind);
    let inner = panel.inner.max(1);
    let max_scroll = rows.len().saturating_sub(inner);
    let p = game.panels.last_mut().unwrap();
    p.scroll = if lines < 0 {
        p.scroll.saturating_sub(lines.unsigned_abs() as usize)
    } else {
        (p.scroll + lines as usize).min(max_scroll)
    };
    true
}

fn panel_key(game: &mut Game, code: GKey) {
    let Some(panel) = game.panels.last() else { return };
    let (_, rows) = game.build_rows(&panel.kind);
    let sels = selectables(&rows);
    let sel = panel.sel.min(sels.len().saturating_sub(1));
    let inner = panel.inner.max(1);
    let max_scroll = rows.len().saturating_sub(inner);
    let snap = |p: &mut Panel, r: usize| {
        if r < p.scroll {
            p.scroll = r;
        } else if r >= p.scroll + p.inner.max(1) {
            p.scroll = r + 1 - p.inner.max(1);
        }
    };
    match code {
        GKey::Esc => {
            game.panels.pop();
        }
        GKey::PageDown => {
            let p = game.panels.last_mut().unwrap();
            p.scroll = (p.scroll + inner).min(max_scroll);
        }
        GKey::PageUp => {
            let p = game.panels.last_mut().unwrap();
            p.scroll = p.scroll.saturating_sub(inner);
        }
        GKey::Down | GKey::Char('j') | GKey::Char('s') => {
            let p_scroll = game.panels.last().unwrap().scroll;
            let next = sels.get(sel).and_then(|&(r0, _)| (sel + 1..sels.len()).find(|&i| sels[i].0 > r0));
            match next {
                // la cible est visible (ou à une ligne) : on la sélectionne
                Some(n) if sels[n].0 < p_scroll + inner => {
                    let p = game.panels.last_mut().unwrap();
                    p.sel = n;
                    snap(p, sels[n].0);
                }
                /* dernière entrée et rien de plus à lire : la liste boucle.
                   les menus d'espèces sont longs, revenir au début ne doit pas
                   demander vingt appuis. */
                None if p_scroll >= max_scroll && !sels.is_empty() => {
                    let p = game.panels.last_mut().unwrap();
                    p.sel = 0;
                    p.scroll = 0;
                }
                // sinon : simple défilement d'une ligne, la lecture d'abord
                _ => {
                    let p = game.panels.last_mut().unwrap();
                    p.scroll = (p.scroll + 1).min(max_scroll);
                }
            }
        }
        GKey::Up | GKey::Char('k') | GKey::Char('z') => {
            let p_scroll = game.panels.last().unwrap().scroll;
            let prev = sels.get(sel).and_then(|&(r0, _)| (0..sel).rev().find(|&i| sels[i].0 < r0));
            match prev {
                Some(n) if sels[n].0 >= p_scroll => {
                    let rt = sels[n].0;
                    let first = (0..=n).rev().take_while(|&i| sels[i].0 == rt).last().unwrap();
                    let p = game.panels.last_mut().unwrap();
                    p.sel = first;
                    snap(p, rt);
                }
                /* première entrée, panneau déjà en haut : on saute à la fin */
                None if p_scroll == 0 && !sels.is_empty() => {
                    let rt = sels[sels.len() - 1].0;
                    let first = (0..sels.len()).find(|&i| sels[i].0 == rt).unwrap();
                    let p = game.panels.last_mut().unwrap();
                    p.sel = first;
                    p.scroll = max_scroll;
                    snap(p, rt);
                }
                _ => {
                    let p = game.panels.last_mut().unwrap();
                    p.scroll = p.scroll.saturating_sub(1);
                }
            }
        }
        GKey::Right | GKey::Char('l') | GKey::Char('d') => {
            if sel + 1 < sels.len() && sels[sel + 1].0 == sels[sel].0 {
                game.panels.last_mut().unwrap().sel = sel + 1;
            }
        }
        GKey::Left | GKey::Char('h') | GKey::Char('q') | GKey::Char('a') => {
            if sel > 0 && sels[sel - 1].0 == sels[sel].0 {
                game.panels.last_mut().unwrap().sel = sel - 1;
            }
        }
        GKey::Enter | GKey::Char(' ') => {
            if let Some(&(r, b)) = sels.get(sel) {
                let action = if b < 0 {
                    rows[r].act.clone()
                } else {
                    rows[r].btns.get(b as usize).map(|x| x.2.clone())
                };
                if let Some(a) = action {
                    game.apply(a);
                }
            }
        }
        _ => {}
    }
}

fn world_key(game: &mut Game, code: GKey) {
    let (mut dx, mut dy) = (0i32, 0i32);
    match code {
        GKey::Up | GKey::Char('z') | GKey::Char('w') => dy = -1,
        GKey::Down | GKey::Char('s') => dy = 1,
        GKey::Left | GKey::Char('q') | GKey::Char('a') => dx = -1,
        GKey::Right | GKey::Char('d') => dx = 1,
        GKey::Enter | GKey::Char(' ') => {
            game.interact();
            return;
        }
        GKey::Char('v') => {
            game.panels.push(Panel::new(PanelKind::Dashboard));
            return;
        }
        GKey::Char('i') => {
            game.panels.push(Panel::new(PanelKind::Inventory));
            return;
        }
        GKey::Char('c') => {
            game.panels.push(Panel::new(PanelKind::Contracts));
            return;
        }
        GKey::Char('m') => {
            game.panels.push(Panel::new(PanelKind::Museum));
            return;
        }
        GKey::Char('e') => {
            game.panels.push(Panel::new(PanelKind::Pens));
            return;
        }
        GKey::Char('b') => {
            game.panels.push(Panel::new(PanelKind::Dex));
            return;
        }
        GKey::Char('o') => {
            game.panels.push(Panel::new(PanelKind::Shop));
            return;
        }
        GKey::Char('l') => {
            game.panels.push(Panel::new(PanelKind::Lab));
            return;
        }
        GKey::Char('t') => {
            game.panels.push(Panel::new(PanelKind::Achs));
            return;
        }
        GKey::Char('j') => {
            game.panels.push(Panel::new(PanelKind::Journal));
            return;
        }
        GKey::Char('p') => {
            game.panels.push(Panel::new(PanelKind::Board));
            return;
        }
        GKey::Char('r') => {
            game.panels.push(Panel::new(PanelKind::Trade));
            return;
        }
        GKey::Char('n') => {
            game.s.news_seen = VERSION.to_string();
            game.panels.push(Panel::new(PanelKind::News));
            return;
        }
        GKey::Char('?') | GKey::Char('/') => {
            game.panels.push(Panel::new(PanelKind::Help));
            return;
        }
        GKey::Esc => {
            game.panels.clear();
            return;
        }
        _ => return,
    }
    if !game.world.solid(game.px + dx, game.py + dy) {
        game.px += dx;
        game.py += dy;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn map_key(code: crossterm::event::KeyCode) -> Option<GKey> {
    use crossterm::event::KeyCode as K;
    Some(match code {
        K::Up => GKey::Up,
        K::Down => GKey::Down,
        K::Left => GKey::Left,
        K::Right => GKey::Right,
        K::Enter => GKey::Enter,
        K::Esc => GKey::Esc,
        K::PageUp => GKey::PageUp,
        K::PageDown => GKey::PageDown,
        K::Char(c) => GKey::Char(c),
        _ => return None,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> std::io::Result<()> {
    use crossterm::event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    };
    use std::time::{Duration, Instant};
    let (mut game, fresh) = Game::new();
    game.run_offline();
    game.welcome(fresh);
    if fresh {
        // première partie : ouvrir le guide directement
        game.s.news_seen = VERSION.to_string();
        game.panels.push(Panel::new(PanelKind::Help));
    } else if game.s.news_seen != VERSION {
        // le jeu a changé depuis la dernière session : on montre ce qui est neuf
        game.s.news_seen = VERSION.to_string();
        game.panels.push(Panel::new(PanelKind::News));
    }

    let theme = Theme::detect();
    let mut terminal = ratatui::init();
    let mut last_tick = Instant::now();
    let mut last_save = Instant::now();
    /* la capture souris coûte la sélection de texte du terminal : on ne la
       prend que pendant qu'un panneau est ouvert, seul endroit qui défile.
       (shift + glisser permet de sélectionner malgré tout sur la plupart
       des terminaux, si le besoin s'en fait sentir sur un panneau.) */
    let mut mouse_on = false;

    while !game.quit {
        if last_tick.elapsed() >= Duration::from_millis(500) {
            game.tick();
            last_tick = Instant::now();
        }
        if last_save.elapsed() >= Duration::from_secs(10) {
            game.save();
            last_save = Instant::now();
        }


        let want_mouse = !game.panels.is_empty();
        if want_mouse != mouse_on {
            let r = if want_mouse {
                crossterm::execute!(std::io::stdout(), EnableMouseCapture)
            } else {
                crossterm::execute!(std::io::stdout(), DisableMouseCapture)
            };
            // un terminal sans support souris ne doit pas faire tomber la partie
            if r.is_ok() {
                mouse_on = want_mouse;
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();
            render(&mut game, &theme, frame.buffer_mut(), area);
        })?;

        let mut wait = Duration::from_millis(100);
        while event::poll(wait)? {
            wait = Duration::ZERO; // draine toutes les touches en attente
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('q'))
                    {
                        game.quit = true;
                    } else if let Some(gk) = map_key(k.code) {
                        if game.panels.is_empty() {
                            world_key(&mut game, gk);
                        } else {
                            panel_key(&mut game, gk);
                        }
                    }
                }
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => {
                        panel_scroll(&mut game, -WHEEL_LINES);
                    }
                    MouseEventKind::ScrollDown => {
                        panel_scroll(&mut game, WHEEL_LINES);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    game.save();
    if mouse_on {
        crossterm::execute!(std::io::stdout(), DisableMouseCapture).ok();
    }
    ratatui::restore();
    println!("partie sauvegardée dans {}. à bientôt.", save_path().display());
    Ok(())
}

/* ============================================================= version web */

#[cfg(target_arch = "wasm32")]
mod webapp {
    use super::*;
    use wasm_bindgen::prelude::*;

    fn color_hex(c: Color) -> Option<String> {
        match c {
            Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
            _ => None,
        }
    }
    fn esc_html(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    #[wasm_bindgen]
    pub struct Web {
        game: Game,
        theme: Theme,
    }

    #[wasm_bindgen]
    impl Web {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Web {
            let (mut game, fresh) = Game::new();
            game.run_offline();
            game.welcome(fresh);
            /* première partie : le guide. sinon, si le jeu a changé depuis la
               dernière session, on ouvre le journal des nouveautés — comme un
               vrai jeu au premier lancement après une mise à jour. */
            if fresh {
                game.s.news_seen = VERSION.to_string();
                game.panels.push(Panel::new(PanelKind::Help));
            } else if game.s.news_seen != VERSION {
                game.s.news_seen = VERSION.to_string();
                game.panels.push(Panel::new(PanelKind::News));
            }
            Web { game, theme: Theme::detect() }
        }

        pub fn tick(&mut self) {
            self.game.tick();
        }

        /* les statistiques du classement, calculées ici et pas dans le
           navigateur : le serveur vérifie un bestiaire d'espèces sauvages
           (celles qu'on peut capturer dans un biome payé). compter en plus
           les curiosités du troc, qui ne vivent dans aucun biome, faisait
           passer un joueur honnête pour un tricheur. */
        /* ce qui mérite d'attirer l'œil hors de l'onglet : le navigateur en
           fait clignoter la favicon. vide quand il n'y a rien à courir. */
        pub fn alerte(&self) -> String {
            let now = now_ms();
            if self.game.legend_now().is_some() {
                return "legende".into();
            }
            if self.game.puits_luit(now) {
                return "puits".into();
            }
            String::new()
        }

        pub fn lb_stats(&self) -> String {
            let s = &self.game.s;
            let especes = wild_species().filter(|&i| s.dex2[i].n > 0).count();
            let curiosites = (0..CREATURES.len()).filter(|&i| CREATURES[i].b == CURIO_B && s.dex2[i].n > 0).count();
            let legendes = (0..CREATURES.len()).filter(|&i| CREATURES[i].b >= LEGEND_B && s.dex2[i].n > 0).count();
            let rangs: u64 = wild_species().filter(|&i| s.dex2[i].n > 0).map(|i| s.dex2[i].best as u64).sum();
            serde_json::json!({
                "captures": s.captures,
                "especes": especes,
                "curiosites": curiosites,
                "legendes": legendes,
                "rangs": rangs,
                "shinies": s.shinies,
                "ecus": s.total_earned.floor(),
                "trophees": s.trophies,
                "migrations": s.migrations,
            })
            .to_string()
        }

        /* le navigateur pousse ici le classement récupéré sur /lb :
           {"me":"pseudo","rows":[…]} — trié par score décroissant */
        pub fn set_board(&mut self, json: &str) {
            #[derive(Deserialize, Default)]
            struct Feed {
                #[serde(default)]
                me: String,
                #[serde(default)]
                rows: Vec<BoardRow>,
            }
            match serde_json::from_str::<Feed>(json) {
                Ok(f) => {
                    let mut rows = f.rows;
                    rows.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                    self.game.board = rows;
                    self.game.board_me = f.me;
                    self.game.board_state = 1;
                }
                Err(_) => self.game.board_state = 2,
            }
        }

        pub fn save(&mut self) {
            self.game.save();
        }

        /* renvoie true si la touche a été consommée par le jeu */
        pub fn key(&mut self, k: &str, ctrl: bool) -> bool {
            if ctrl {
                if k == "s" {
                    self.game.save();
                    return true;
                }
                return false; // laisser les raccourcis navigateur tranquilles
            }
            let gk = match k {
                "ArrowUp" => GKey::Up,
                "ArrowDown" => GKey::Down,
                "ArrowLeft" => GKey::Left,
                "ArrowRight" => GKey::Right,
                "Enter" => GKey::Enter,
                "Escape" => GKey::Esc,
                "PageUp" => GKey::PageUp,
                "PageDown" => GKey::PageDown,
                _ => {
                    let mut it = k.chars();
                    match (it.next(), it.next()) {
                        (Some(c), None) => GKey::Char(c),
                        _ => return false,
                    }
                }
            };
            if self.game.panels.is_empty() {
                world_key(&mut self.game, gk);
            } else {
                panel_key(&mut self.game, gk);
            }
            true
        }

        /* molette et trackpad. le navigateur compte en pixels : c'est lui qui
           convertit en lignes, il est le seul à connaître l'interligne réel.
           renvoie true si un panneau était ouvert — sinon la page doit garder
           son défilement normal. */
        pub fn scroll(&mut self, lines: i32) -> bool {
            panel_scroll(&mut self.game, lines)
        }

        pub fn render(&mut self, cols: u16, rows: u16) -> String {
            let area = Rect::new(0, 0, cols, rows);
            let mut buf = Buffer::empty(area);
            render(&mut self.game, &self.theme, &mut buf, area);
            let mut html = String::with_capacity(cols as usize * rows as usize * 4);
            for y in 0..rows {
                html.push_str("<div>");
                let mut run = String::new();
                let mut cur: Option<(String, String)> = None;
                let flush = |html: &mut String, cur: &Option<(String, String)>, run: &str| {
                    if run.is_empty() {
                        return;
                    }
                    if let Some((fg, bg)) = cur {
                        let bg_css = if bg.is_empty() { String::new() } else { format!(";background:{}", bg) };
                        html.push_str(&format!("<span style=\"color:{}{}\">{}</span>", fg, bg_css, esc_html(run)));
                    }
                };
                for x in 0..cols {
                    let cell = &buf[(x, y)];
                    let fg = color_hex(cell.fg).unwrap_or_else(|| "#c2cdda".into());
                    let bg = color_hex(cell.bg).unwrap_or_default();
                    let key = (fg, bg);
                    if cur.as_ref() != Some(&key) {
                        flush(&mut html, &cur, &run);
                        run.clear();
                        cur = Some(key);
                    }
                    run.push_str(cell.symbol());
                }
                flush(&mut html, &cur, &run);
                html.push_str("</div>");
            }
            html
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /* filet de sécurité de la carte : depuis la place, on doit pouvoir
       atteindre à pied une case de CHAQUE biome, rivière comprise. */
    #[test]
    fn tous_les_biomes_sont_accessibles_a_pied() {
        let w = WorldMap::build();
        let mut vu = vec![vec![usize::MAX; MAPW]; MAPH];
        let mut q = VecDeque::new();
        q.push_back((60usize, 37usize));
        vu[37][60] = 0;
        let mut atteint: Vec<(Option<Zone>, usize)> = vec![];
        while let Some((x, y)) = q.pop_front() {
            atteint.push((w.zone_at(x, y), vu[y][x]));
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAPW as i32 || ny >= MAPH as i32 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                if vu[ny][nx] != usize::MAX || w.cells[ny][nx].solid {
                    continue;
                }
                vu[ny][nx] = vu[y][x] + 1;
                q.push_back((nx, ny));
            }
        }
        for b in 0..WILDB {
            let d = atteint
                .iter()
                .filter(|(z, _)| matches!(z, Some(Zone::Biome(bb)) if *bb == b))
                .map(|(_, d)| *d)
                .min();
            let Some(d) = d else {
                panic!("biome {} ({}) inatteignable depuis la place", b, BIOMES[b].name);
            };
            // sans route directe, on finit par y arriver mais en faisant le tour :
            // ce plafond fait échouer la construction si un chemin a disparu
            assert!(d <= 90, "biome {} atteint en {} pas : une route manque", BIOMES[b].name, d);
        }
        for z in [Zone::Boutique, Zone::Labo, Zone::Bestiaire, Zone::Succes, Zone::Musee, Zone::Enclos, Zone::Troc] {
            assert!(atteint.iter().any(|(a, _)| *a == Some(z)), "lieu du village inatteignable");
        }
    }

    /* on ne doit pas avoir à faire le tour de la place pour aller d'un
       bâtiment à l'autre : la fontaine ne bouche pas l'axe principal */
    #[test]
    fn la_traversee_du_village_reste_courte() {
        let w = WorldMap::build();
        let depart = (44usize, 37usize); // devant la boutique
        let arrivee = (66usize, 37usize); // devant le labo
        let mut dist = vec![vec![usize::MAX; MAPW]; MAPH];
        let mut q = VecDeque::new();
        dist[depart.1][depart.0] = 0;
        q.push_back(depart);
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAPW as i32 || ny >= MAPH as i32 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                if w.cells[ny][nx].solid || dist[ny][nx] != usize::MAX {
                    continue;
                }
                dist[ny][nx] = dist[y][x] + 1;
                q.push_back((nx, ny));
            }
        }
        let pas = dist[arrivee.1][arrivee.0];
        eprintln!("trajet boutique -> labo : {} pas", pas);
        assert!(pas != usize::MAX, "le labo est injoignable depuis la boutique");
        // 22 cases séparent les deux portes : au-delà de 24 pas, c'est un détour
        assert!(pas <= 24, "trajet boutique -> labo trop long : {} pas", pas);
    }

    /* chaque bâtiment doit rester accessible depuis la place, sans détour :
       un banc ou un lampadaire mal posé suffirait à condamner une porte */
    #[test]
    fn chaque_batiment_est_accessible_depuis_la_place() {
        let w = WorldMap::build();
        let mut dist = vec![vec![usize::MAX; MAPW]; MAPH];
        let mut q = VecDeque::new();
        dist[37][60] = 0;
        q.push_back((60usize, 37usize));
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAPW as i32 || ny >= MAPH as i32 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                if w.cells[ny][nx].solid || dist[ny][nx] != usize::MAX {
                    continue;
                }
                dist[ny][nx] = dist[y][x] + 1;
                q.push_back((nx, ny));
            }
        }
        for &(dx, dy, z) in w.doors.iter() {
            let d = dist[dy][dx];
            assert!(d != usize::MAX, "porte inatteignable en ({}, {})", dx, dy);
            assert!(d <= 40, "porte en ({}, {}) atteinte en {} pas : un passage est bouché", dx, dy, d);
            let _ = z;
        }
    }

    /* le marchand doit passer souvent, sans jamais être là en permanence */
    #[test]
    fn le_marchand_passe_souvent_mais_pas_toujours() {
        let (mut couvert, mut total) = (0u32, 0u32);
        let mut passages = 0usize;
        for j in 0..40u64 {
            let v = Game::merchant_visits(j);
            assert!((3..=5).contains(&v.len()), "{} passages dans la journée", v.len());
            passages += v.len();
            let veille = Game::merchant_visits(j.saturating_sub(1));
            // échantillonnage toutes les 10 minutes
            for k in 0..144 {
                let t = j as f64 * 86_400_000.0 + k as f64 * 600_000.0;
                total += 1;
                if v.iter().chain(veille.iter()).any(|&(a, b)| t >= a && t < b) {
                    couvert += 1;
                }
            }
        }
        let part = couvert as f64 / total as f64;
        eprintln!("marchand présent {:.0} % du temps, {:.1} passages par jour", part * 100.0, passages as f64 / 40.0);
        assert!(part > 0.45, "trop rare : {:.0} %", part * 100.0);
        assert!(part < 0.80, "trop souvent là : {:.0} %", part * 100.0);
    }

    /* aucun point d'apparition ne doit tomber dans l'eau : une trace ou une
       légende inaccessible serait perdue pour toujours */
    #[test]
    fn les_apparitions_tombent_sur_du_solide_foulable() {
        let g = jeu_neuf();
        for (b, &(x, y)) in LEGEND_SPOTS.iter().enumerate() {
            let (fx, fy) = g.spot_foulable(x, y);
            assert!(!g.world.solid(fx as i32, fy as i32), "légende du biome {} injoignable", b);
            // les trois décalages possibles d'une trace autour de ce point
            for (dx, dy) in [(-6i32, 3i32), (5, -2), (-2, 6)] {
                let tx = (x as i32 + dx).clamp(2, MAPW as i32 - 3) as usize;
                let ty = (y as i32 + dy).clamp(2, MAPH as i32 - 3) as usize;
                let (px, py) = g.spot_foulable(tx, ty);
                assert!(!g.world.solid(px as i32, py as i32), "trace du biome {} injoignable", b);
            }
        }
    }

    /* la marche doit rester agréable : peu d'obstacles réels dans les biomes */
    #[test]
    fn les_biomes_restent_praticables() {
        let w = WorldMap::build();
        for &(zx, zy, zw, zh, b) in ZONE_RECTS.iter() {
            if b == 10 {
                continue; // le lac est de l'eau : bloquant par nature
            }
            let (mut total, mut durs) = (0, 0);
            for y in zy..(zy + zh).min(MAPH) {
                for x in zx..(zx + zw).min(MAPW) {
                    total += 1;
                    if w.cells[y][x].solid {
                        durs += 1;
                    }
                }
            }
            let part = durs as f64 / total as f64;
            assert!(
                part < 0.08,
                "{} : {:.1} % de cases bloquantes, la marche y devient pénible",
                BIOMES[b].name,
                part * 100.0
            );
        }
    }

    /* la table des créatures et celle des biomes doivent rester d'accord */
    #[test]
    fn chaque_biome_a_ses_especes() {
        for b in 0..WILDB {
            assert!(biome_creatures(b).count() > 0, "biome {} sans espèce", BIOMES[b].name);
        }
        let curios = CREATURES.iter().filter(|c| c.b == CURIO_B).count();
        let legendes = CREATURES.iter().filter(|c| c.b == LEGEND_B).count();
        let puits = CREATURES.iter().filter(|c| c.b == PUITS_B).count();
        assert_eq!(curios, 6, "six curiosités au troc");
        assert_eq!(legendes, 12, "douze légendes errantes");
        assert_eq!(puits, 1, "une seule chose au fond du puits");
        assert_eq!(
            wild_total() + curios + legendes + puits,
            CREATURES.len(),
            "tout ce qui vit hors des biomes reste hors du bestiaire sauvage"
        );
    }

    /* une partie neuve, sans toucher au disque : un piège en bois, la forêt
       ouverte, deux emplacements. */
    fn jeu_neuf() -> Game {
        let mut s = State::default();
        s.normalize();
        Game {
            s,
            world: WorldMap::build(),
            px: 60,
            py: 38,
            panels: vec![],
            logs: VecDeque::new(),
            toasts: vec![],
            quit: false,
            panel_w: 75,
            legend_seen: 0,
            board: Vec::new(),
            board_me: String::new(),
            board_state: 0,
            merchant_seen: 0,
            pens_seen: vec![],
            pentacle_until: 0.0,
            museum_auto_at: 0.0,
        }
    }

    fn largeur_max(rows: &[Row]) -> (usize, String) {
        rows.iter()
            .map(|r| {
                let t: String = r.segs.iter().map(|(t, _)| t.as_str()).collect();
                (r.indent as usize + t.chars().count(), t)
            })
            .max_by_key(|(w, _)| *w)
            .unwrap_or((0, String::new()))
    }

    #[test]
    fn le_bestiaire_tient_en_80_colonnes() {
        // espèces inconnues : la ligne « ??? » et son marqueur nocturne
        let g = jeu_neuf();
        let (_, rows) = g.rows_dex();
        let (w, l) = largeur_max(&rows);
        assert!(w <= 75, "ligne « inconnu » trop large : {} colonnes pour 75 : {:?}", w, l);

        // espèces capturées, avec des compteurs à trois chiffres et des shinies
        let mut g = jeu_neuf();
        for ci in 0..CREATURES.len() {
            g.s.dex2[ci] = DexE { n: 999, s: 99, best: 4, bests: 4, mf: 3 };
            g.s.inv2[ci].m[0] = 99;
            g.s.inv2[ci].sf[0] = 9;
        }
        let (_, rows) = g.rows_dex();
        let (w, l) = largeur_max(&rows);
        assert!(w <= 75, "ligne d'espèce capturée trop large : {} colonnes pour 75 : {:?}", w, l);
    }

    /* molette : la fenêtre bouge, la sélection ne bouge pas, et on ne sort
       jamais des bornes du panneau */
    #[test]
    fn la_molette_fait_defiler_le_panneau() {
        let mut g = jeu_neuf();
        assert!(!panel_scroll(&mut g, 3), "sans panneau ouvert, rien à défiler");

        g.panels.push(Panel::new(PanelKind::Help));
        g.panels.last_mut().unwrap().inner = 10;
        let (_, rows) = g.build_rows(&PanelKind::Help);
        let max = rows.len().saturating_sub(10);
        assert!(max > 3, "le guide doit être plus long qu'une fenêtre pour ce test");

        assert!(panel_scroll(&mut g, 3), "un panneau est ouvert");
        assert_eq!(g.panels.last().unwrap().scroll, 3);
        assert_eq!(g.panels.last().unwrap().sel, 0, "la molette ne touche pas la sélection");

        panel_scroll(&mut g, -1);
        assert_eq!(g.panels.last().unwrap().scroll, 2);

        // vers le haut : on s'arrête en haut, sans déborder
        panel_scroll(&mut g, -99);
        assert_eq!(g.panels.last().unwrap().scroll, 0);

        // vers le bas : on s'arrête sur la dernière fenêtre complète
        panel_scroll(&mut g, 9_999);
        assert_eq!(g.panels.last().unwrap().scroll, max);
    }

    /* la police du jeu (Iosevka Affut) est à chasse 500, mais certains glyphes
       y sont dessinés sur 1000 unités : ils prennent deux cellules et décalent
       d'un cran tout ce qui suit sur la ligne. interdits dans les colonnes
       alignées — mesuré avec fontTools sur docs/fonts/iosevka-affut-300.woff2. */
    const GLYPHES_LARGES: [char; 3] = ['◗', '✦', '✓'];

    fn sans_glyphe_large(rows: &[Row], ou: &str) {
        for r in rows {
            // les en-têtes à filet se rétrécissent d'eux-mêmes : leur ✓ final
            // ne décale rien puisque les ─ absorbent la différence
            if r.segs.last().is_some_and(|(t, _)| t.chars().all(|c| c == '─')) {
                continue;
            }
            let ligne: String = r.segs.iter().map(|(t, _)| t.as_str()).collect();
            for c in GLYPHES_LARGES {
                assert!(
                    !ligne.contains(c),
                    "{:?} occupe deux cellules et décalera la ligne ({}) : {:?}",
                    c,
                    ou,
                    ligne
                );
            }
        }
    }

    #[test]
    fn aucun_glyphe_a_double_chasse_dans_le_bestiaire() {
        // espèces inconnues : la ligne « ??? » et son marqueur nocturne
        let g = jeu_neuf();
        let (_, rows) = g.rows_dex();
        sans_glyphe_large(&rows, "espèce inconnue");

        // espèces capturées, avec shinies : l'autre branche du bestiaire
        let mut g = jeu_neuf();
        for ci in 0..CREATURES.len() {
            g.s.dex2[ci] = DexE { n: 999, s: 99, best: 4, bests: 4, mf: 3 };
            g.s.inv2[ci].m[0] = 99;
            g.s.inv2[ci].sf[0] = 9;
        }
        let (_, rows) = g.rows_dex();
        sans_glyphe_large(&rows, "espèce capturée");
    }

    /* revente des pièges : deux garde-fous. un piège posé n'est pas en
       réserve, et se retrouver sans aucun piège avec zéro écu bloquerait la
       partie — un piège en bois neuf coûte plus que ce qu'en donne la reprise. */
    #[test]
    fn on_ne_revend_ni_le_dernier_piege_ni_un_piege_pose() {
        let reprise = (TRAPS[0].cost * TRAP_RESALE).floor();

        // partie neuve : un seul piège en réserve, la vente doit être refusée
        let mut g = jeu_neuf();
        assert_eq!(g.s.traps.iter().sum::<u32>(), 1);
        let ecus = g.s.ecus;
        g.apply(Action::SellTrap(0));
        assert_eq!(g.s.traps[0], 1, "le dernier piège doit rester");
        assert_eq!(g.s.ecus, ecus, "et ne rien rapporter");

        // deux pièges en réserve : la vente passe
        let mut g = jeu_neuf();
        g.s.traps[0] = 2;
        let ecus = g.s.ecus;
        g.apply(Action::SellTrap(0));
        assert_eq!(g.s.traps[0], 1);
        assert_eq!(g.s.ecus, ecus + reprise);

        // deux pièges dont un posé : il reste bien un piège libre, ça passe
        let mut g = jeu_neuf();
        g.s.traps[0] = 2;
        g.s.biomes[0].as_mut().unwrap().pl[0] = Some(Placement { trap: 0, bait: None, next_at: 0.0 });
        let ecus = g.s.ecus;
        g.apply(Action::SellTrap(0));
        assert_eq!(g.s.traps[0], 1);
        assert_eq!(g.s.ecus, ecus + reprise);

        // un seul piège, posé : plus rien de libre, la vente est refusée
        let mut g = jeu_neuf();
        g.s.traps[0] = 1;
        g.s.biomes[0].as_mut().unwrap().pl[0] = Some(Placement { trap: 0, bait: None, next_at: 0.0 });
        let ecus = g.s.ecus;
        g.apply(Action::SellTrap(0));
        assert_eq!(g.s.traps[0], 1, "un piège posé n'est pas en réserve");
        assert_eq!(g.s.ecus, ecus);
    }

    /* intention d'équilibrage : un palier de piège doit se sentir. la table
       a longtemps donné +40% pour un prix multiplié par 6 à 20, si bien que
       l'amélioration ne changeait rien de perceptible. */
    #[test]
    fn chaque_palier_de_piege_double_le_revenu() {
        // revenu par seconde d'un piège, à chance et biome égaux : la réussite
        // et la cadence, pondérées par la valeur moyenne d'une prise
        let revenu = |t: usize| {
            let d = &TRAPS[t];
            let lk = d.luck.min(LUCK_CAP);
            let w: Vec<f64> = (0..5).map(|r| RAR_W[r] * (1.0 + lk * RAR_LUCK_FACT[r])).collect();
            let tot: f64 = w.iter().sum();
            let val: f64 = (0..5).map(|r| w[r] / tot * RAR_VAL[r]).sum();
            d.succ * val / d.itv
        };
        for t in 1..TRAPS.len() {
            let x = revenu(t) / revenu(t - 1);
            assert!(
                x >= 1.7,
                "« {} » ne rapporte que ×{:.2} de plus que « {} » : le palier ne se sent pas",
                TRAPS[t].n,
                x,
                TRAPS[t - 1].n
            );
        }
    }

    /* un emplacement doit rester au prix d'un piège : adossé au prix du biome,
       le 4e emplacement des ruines coûtait deux fois le biome lui-même, soit
       1500 h d'économies pour un seul piège de plus. */
    #[test]
    fn un_emplacement_coute_le_prix_du_piege_quon_y_met() {
        let mut g = jeu_neuf();
        // partie neuve : un piège en bois, deux emplacements en forêt
        assert_eq!(g.slot_cost(0), TRAPS[0].cost);

        // le 4e emplacement est plus cher, mais reste du même ordre
        g.s.biomes[0].as_mut().unwrap().slots = 3;
        assert!(g.slot_cost(0) <= TRAPS[0].cost * 3.0);

        // avec un piège quantique en réserve, l'emplacement suit — et reste
        // très en dessous du prix du biome le plus cher
        g.s.traps[5] = 1;
        let plus_cher = BIOMES.iter().map(|b| b.cost).filter(|c| c.is_finite()).fold(0.0, f64::max);
        for b in 0..WILDB {
            assert!(
                g.slot_cost(b) < plus_cher,
                "un emplacement ({}) coûte plus cher que le biome le plus cher ({})",
                g.slot_cost(b),
                plus_cher
            );
        }
        assert_eq!(g.slot_cost(0), (TRAPS[5].cost * 2.5).floor());
    }

    /* un montant abrégé ne doit jamais paraître plus gros qu'il n'est : sinon
       le prix d'un biome semble payable alors que le déblocage refuse. */
    #[test]
    fn les_montants_abreges_ne_sur_annoncent_jamais() {
        for n in [2_999_500.0, 2_999_999.0, 3_000_000.0, 29_960.0, 999_999_999.0, 9_999.0] {
            let s = fmt(n);
            let brut: String = s.chars().filter(|c| c.is_ascii_digit() || *c == ',').collect();
            let mult = if s.ends_with("Md") { 1e9 } else if s.ends_with('M') { 1e6 } else if s.ends_with('k') { 1e3 } else { 1.0 };
            let v = brut.replace(',', ".").parse::<f64>().unwrap_or(0.0) * mult;
            assert!(v <= n, "{} affiché « {} », soit {} écus annoncés en trop", n, s, v - n);
        }
        assert_eq!(fmt(2_999_500.0), "2,99 M");
        assert_eq!(fmt(29_960.0), "29,9 k");
        assert_eq!(fmt(10_000.0), "10 k");
        assert_eq!(fmt(3_000_000.0), "3,00 M");
    }

    /* le garnissage automatique doit exposer exactement les pièces les mieux
       payées : rien en réserve ne doit rapporter plus qu'une pièce exposée. */
    #[test]
    fn le_musee_expose_les_specimens_les_mieux_payes() {
        let mut g = jeu_neuf();
        for ci in 0..20 {
            for r in 0..4 {
                g.add_specimen(ci, false, r);
            }
        }
        g.add_specimen(3, true, 3);
        // une pièce médiocre déjà exposée doit céder sa place
        g.s.museum[0] = g.take_best(0, false).map(|(rank, sex)| MusE { ci: 0, rank, shiny: false, sex });

        assert!(g.museum_optimize(), "le musée aurait dû changer");
        let n = g.museum_slots();
        let exposes: Vec<f64> = g
            .s
            .museum
            .iter()
            .take(n)
            .filter_map(|m| m.as_ref())
            .map(|m| g.creature_value_r(m.ci, m.shiny, m.rank))
            .collect();
        assert_eq!(exposes.len(), n, "toutes les salles devaient être garnies");
        let plancher = exposes.iter().cloned().fold(f64::INFINITY, f64::min);
        for ci in 0..CREATURES.len() {
            for shiny in [false, true] {
                for r in 0..4 {
                    let iv = &g.s.inv2[ci];
                    let reste = if shiny { iv.sr(r) } else { iv.nr(r) };
                    if reste > 0 {
                        let v = g.creature_value_r(ci, shiny, r);
                        assert!(v <= plancher, "{} [{}] vaut {} et dort en réserve alors qu'une salle expose {}", CREATURES[ci].n, RANK_NAMES[r], v, plancher);
                    }
                }
            }
        }
        // idempotent : relancer ne bouge plus rien
        assert!(!g.museum_optimize(), "un second passage ne devait rien changer");
    }

    /* le serveur du classement plafonne le bestiaire par ce que la partie peut
       financer, biome par biome. les curiosités du troc ne vivant dans aucun
       biome, elles doivent rester hors de ce décompte — sinon un joueur
       honnête qui troque passe pour un tricheur. */
    #[test]
    fn les_curiosites_restent_hors_du_bestiaire_finançable() {
        let par_prix: f64 = paliers_bestiaire().iter().map(|(_, n)| n).sum();
        assert_eq!(par_prix as usize, wild_total(), "la table de prix et le bestiaire sauvage divergent");
        assert!(CREATURES.iter().any(|c| c.b == CURIO_B), "plus aucune curiosité : le test n'a plus d'objet");
        assert!(wild_species().all(|i| CREATURES[i].b != CURIO_B));
    }

    /* les listes d'espèces sont longues : la sélection doit boucler, sinon
       atteindre la dernière entrée demande de traverser tout le panneau. */
    #[test]
    fn les_listes_bouclent_en_haut_et_en_bas() {
        let mut g = jeu_neuf();
        g.panels.push(Panel::new(PanelKind::Shop));
        let (_, rows) = g.build_rows(&PanelKind::Shop);
        let sels = selectables(&rows);
        assert!(sels.len() > 2, "la boutique doit proposer plusieurs entrées");
        // panneau assez haut pour tout montrer : la lecture ne prime pas
        g.panels.last_mut().unwrap().inner = rows.len();

        // depuis la première entrée, le haut mène à la dernière
        panel_key(&mut g, GKey::Up);
        let derniere_ligne = sels[sels.len() - 1].0;
        assert_eq!(sels[g.panels.last().unwrap().sel].0, derniere_ligne, "le haut devait boucler vers la fin");

        // et depuis la dernière, le bas ramène à la première
        panel_key(&mut g, GKey::Down);
        assert_eq!(g.panels.last().unwrap().sel, 0, "le bas devait boucler vers le début");
        assert_eq!(g.panels.last().unwrap().scroll, 0, "et remonter le panneau");

        // au milieu de la liste, rien ne boucle
        panel_key(&mut g, GKey::Down);
        let milieu = g.panels.last().unwrap().sel;
        assert!(milieu > 0 && milieu < sels.len() - 1);

        /* cas réel : un panneau plus court que sa liste. le haut boucle depuis
           le sommet, et la ligne visée reste visible. */
        g.panels.clear();
        g.panels.push(Panel::new(PanelKind::Shop));
        g.panels.last_mut().unwrap().inner = 6;
        panel_key(&mut g, GKey::Up);
        let p = g.panels.last().unwrap();
        let ligne = sels[p.sel].0;
        assert_eq!(ligne, derniere_ligne, "le haut devait boucler malgré le défilement");
        assert!(ligne >= p.scroll && ligne < p.scroll + p.inner, "la dernière entrée doit être à l'écran");
    }

    /* les battues doivent payer autrement qu'en prises immédiates : elles
       attirent les légendes une heure durant, et l'effet se cumule. */
    #[test]
    fn les_battues_attirent_les_legendes() {
        let mut g = jeu_neuf();
        let t = 1_788_000_000_000.0;
        assert_eq!(g.legend_chance(t), LEGEND_BASE_PM, "sans battue, la chance de base");

        for _ in 0..3 {
            g.s.hunts_at.push(t);
        }
        assert_eq!(g.legend_chance(t), LEGEND_BASE_PM + 3 * HUNT_BUFF_PM, "trois battues, trois fois le bonus");

        for _ in 0..30 {
            g.s.hunts_at.push(t);
        }
        assert_eq!(g.legend_chance(t), LEGEND_BASE_PM + HUNT_BUFF_MAX_PM, "le cumul reste plafonné");

        assert_eq!(g.legend_chance(t + HUNT_BUFF_MS + 1.0), LEGEND_BASE_PM, "une heure plus tard, tout est retombé");

        g.s.lab[LAB_APPEL] = 5;
        assert_eq!(
            g.legend_chance(t + HUNT_BUFF_MS + 1.0),
            LEGEND_BASE_PM + 5 * LAB_APPEL_PM,
            "l'appel du labo s'ajoute à la base"
        );
        assert!(g.legend_chance(t) <= LEGEND_MAX_PM, "la chance reste bornée");

        /* le rythme visé : une trentaine de silhouettes par jour sans rien
           faire, le double en jouant à fond */
        let par_jour = |pm: u64| 86_400_000.0 / LEGEND_WINDOW_MS * pm as f64 / 1000.0;
        assert!((par_jour(LEGEND_BASE_PM) - 28.8).abs() < 0.1);
        assert!(par_jour(LEGEND_MAX_PM) < 70.0);
    }

    /* les légendes errantes n'appartiennent à aucun biome : aucun piège ne
       doit pouvoir les sortir, et elles restent hors du bestiaire sauvage. */
    #[test]
    fn les_legendes_vivent_hors_des_biomes() {
        let pantheon: Vec<usize> = (0..CREATURES.len()).filter(|&i| CREATURES[i].b == LEGEND_B).collect();
        assert_eq!(pantheon.len(), 12);
        for &i in &pantheon {
            assert!(!wild_species().any(|w| w == i), "{} compte à tort dans le bestiaire sauvage", CREATURES[i].n);
            assert!(CREATURES[i].r >= 3, "une légende doit être au moins épique");
        }
        for b in 0..WILDB {
            assert!(biome_creatures(b).all(|i| CREATURES[i].b != LEGEND_B));
        }
        // et le tirage d'une approche ne sort jamais du panthéon
        for _ in 0..200 {
            assert_eq!(CREATURES[tirage_legende()].b, LEGEND_B);
        }
    }

    /* une silhouette peut paraître sur une terre non ouverte : c'est ce qui
       donne envie d'aller voir avant d'en avoir les moyens. */
    #[test]
    fn les_silhouettes_ignorent_les_frontieres() {
        let vus: std::collections::HashSet<usize> =
            (0..500u64).map(|w| (splitmix(w ^ 0xB10) % WILDB as u64) as usize).collect();
        assert_eq!(vus.len(), WILDB, "tous les biomes doivent pouvoir accueillir une légende");
    }

    /* la silhouette tient deux fenêtres : assez pour qu'une session la croise,
       pas assez pour qu'un passage quotidien suffise. */
    #[test]
    fn une_silhouette_tient_vingt_minutes() {
        let g = jeu_neuf();
        let w = 1_000_000u64;
        assert_eq!(g.legend_left_min(w, w as f64 * LEGEND_WINDOW_MS), 20, "vingt minutes en tout");
        assert_eq!(g.legend_left_min(w, (w + 1) as f64 * LEGEND_WINDOW_MS), 10, "dix à la seconde fenêtre");
        assert_eq!(g.legend_left_min(w, (w + LEGEND_LINGER) as f64 * LEGEND_WINDOW_MS), 0, "puis elle s'en va");
    }

    /* le rythme visé, mesuré sur trente jours de tirages : une douzaine de
       silhouettes par jour sans rien faire, le double en enchaînant les
       battues. la garde attrape un déréglage des constantes. */
    #[test]
    fn le_rythme_des_legendes_reste_dans_sa_fourchette() {
        let par_jour = |pm: u64| {
            let w0 = (now_ms() / LEGEND_WINDOW_MS) as u64;
            let (mut visibles, mut fin) = (0u32, 0u64);
            for k in 0..144u64 * 30 {
                let w = w0 + k;
                if splitmix(w ^ 0x1E9E17D) % 1000 < pm && w >= fin {
                    visibles += 1;
                    fin = w + LEGEND_LINGER;
                }
            }
            visibles as f64 / 30.0
        };
        let base = par_jour(LEGEND_BASE_PM);
        let battues = par_jour(LEGEND_BASE_PM + HUNT_BUFF_MAX_PM);
        let max = par_jour(LEGEND_MAX_PM);
        /* les silhouettes tenant deux fenêtres, deux tirages voisins se
           recouvrent : le compte distinct est un peu sous le nombre de tirages */
        assert!((21.0..29.0).contains(&base), "sans rien faire : {:.1} silhouettes par jour", base);
        assert!(battues > base * 1.4, "les battues doivent peser : {:.1} contre {:.1}", battues, base);
        assert!((40.0..55.0).contains(&max), "tout à fond : {:.1} silhouettes par jour", max);
        /* et une silhouette est visible bien plus longtemps qu'avant : c'est
           ce qui compte pour qui joue par sessions */
        let minutes_visibles = base * (LEGEND_LINGER as f64 * LEGEND_WINDOW_MS / 60_000.0);
        assert!(minutes_visibles > 400.0, "{:.0} minutes de présence par jour", minutes_visibles);
    }

    /* la colonne d'effet du labo doit sortir des mêmes formules que le jeu :
       elle annonçait +8% de négoce pour +5% réels, +15% d'éclat pour +10%,
       +0,06 de flair pour +0,08 et 35% de montée de rang pour 25%. */
    #[test]
    fn le_labo_annonce_ses_effets_reels() {
        let mut g = jeu_neuf();
        for (k, niveau) in [(LAB_NEGOCE, 4), (LAB_FLAIR, 4), (LAB_ECLAT, 3), (LAB_LIGNEES, 4), (LAB_AFFUTAGE, 3)] {
            g.s.lab[k] = niveau;
        }
        // ce que le jeu applique vraiment
        assert!((g.sell_mult() - 1.20).abs() < 1e-9, "négoce : +5% par niveau");
        /* la chance globale dépend aussi du jour de foire et de l'élan : on
           mesure l'écart dû au flair, pas sa valeur absolue */
        let luck_avec = g.global_luck();
        let flair = std::mem::replace(&mut g.s.lab[LAB_FLAIR], 0);
        let ecart = luck_avec - g.global_luck();
        g.s.lab[LAB_FLAIR] = flair;
        assert!((ecart - 0.32).abs() < 1e-9, "flair : +0,08 par niveau");
        assert!((g.pen_rankup() - 0.41).abs() < 1e-9, "lignées : 25% + 4% par niveau");
        assert!((g.speed_mult() - 1.18).abs() < 1e-9, "affûtage : +6% par niveau");

        // et ce que le panneau en dit
        let (_, rows) = g.build_rows(&PanelKind::Lab);
        let ligne = |nom: &str| -> String {
            rows.iter()
                .find(|r| r.segs.first().map(|(t, _)| t.trim_end().starts_with(nom)).unwrap_or(false))
                .map(|r| r.segs.iter().map(|(t, _)| t.clone()).collect::<String>())
                .unwrap_or_else(|| panic!("ligne « {} » introuvable au labo", nom))
        };
        assert!(ligne("négoce").contains("+20%"), "négoce affiché : {}", ligne("négoce"));
        assert!(ligne("flair").contains("+0,32"), "flair affiché : {}", ligne("flair"));
        assert!(ligne("chasse nocturne").contains("+30%"), "éclat affiché : {}", ligne("chasse nocturne"));
        assert!(ligne("lignées").contains("41%"), "lignées affiché : {}", ligne("lignées"));
        assert!(ligne("affûtage").contains("+18%"), "affûtage affiché : {}", ligne("affûtage"));
    }

    /* un doublon de curiosité ou de légende doit pouvoir se revendre : la
       découverte reste acquise au bestiaire, le spécimen n'a plus d'usage. */
    #[test]
    fn les_especes_hors_biome_se_vendent() {
        let mut g = jeu_neuf();
        let curio = (0..CREATURES.len()).find(|&i| CREATURES[i].b == CURIO_B && CREATURES[i].r == 4).unwrap();
        let legende = (0..CREATURES.len()).find(|&i| CREATURES[i].b == LEGEND_B).unwrap();
        g.add_specimen(curio, false, 0);
        g.add_specimen(legende, false, 0);

        let (_, rows) = g.build_rows(&PanelKind::Shop);
        let texte: String = rows
            .iter()
            .flat_map(|r| r.segs.iter().map(|(t, _)| t.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(texte.contains(CREATURES[curio].n), "la curiosité doit apparaître à la vente");
        assert!(texte.contains(CREATURES[legende].n), "la légende doit apparaître à la vente");

        /* et le prix ne doit pas être dérisoire : une curiosité légendaire vaut
           au moins ce que rapporte une épique du désert */
        let deserte = (0..CREATURES.len()).find(|&i| BIOMES[CREATURES[i].b].name == "désert" && CREATURES[i].r == 3).unwrap();
        assert!(
            g.creature_value(curio, false) > g.creature_value(deserte, false),
            "curiosité légendaire {} contre épique du désert {}",
            g.creature_value(curio, false),
            g.creature_value(deserte, false)
        );
        /* mais le troc ne doit pas devenir une planche à billets : la revente
           reste sous le prix des spécimens qu'il faut donner */
        assert!(g.creature_value(curio, false) < 2600.0 * 4.0, "une curiosité ne doit pas payer plus qu'elle ne coûte");
    }

    /* l'enclos consomme les plus bas rangs par défaut, mais le petit naît au
       meilleur rang de ses parents : viser un S doit rester possible. */
    #[test]
    fn l_enclos_laisse_choisir_le_rang_des_parents() {
        let mut g = jeu_neuf();
        let ci = 0;
        // un couple médiocre et un couple de rang S
        g.s.inv2[ci].m[0] = 1;
        g.s.inv2[ci].f[0] = 1;
        g.s.inv2[ci].m[3] = 1;
        g.s.inv2[ci].f[3] = 1;

        // le sélecteur propose le défaut et les rangs dont on a le couple
        let (_, rows) = g.build_rows(&PanelKind::PenPick(0));
        /* la ligne de l'espèce, pas celle du conseil : c'est celle qui porte
           le bouton du couple par défaut */
        /* la ligne de l'espèce, pas celle du conseil, qui n'a pas de bouton de
           rang. les boutons débordent la largeur du panneau et se poursuivent
           sur les lignes sans texte qui suivent : on les rassemble. */
        let debut = rows
            .iter()
            .position(|r| {
                r.segs.iter().any(|(t, _)| t.contains(CREATURES[ci].n))
                    && !r.segs.iter().any(|(t, _)| t.contains("conseil"))
            })
            .expect("l'espèce doit être listée");
        let libelles: Vec<String> = rows[debut..]
            .iter()
            .enumerate()
            .take_while(|(i, r)| *i == 0 || r.segs.is_empty())
            .flat_map(|(_, r)| r.btns.iter().map(|(t, _, _)| t.clone()))
            .collect();
        assert!(libelles.iter().any(|t| t == "plus bas rangs"), "boutons : {:?}", libelles);
        assert!(libelles.iter().any(|t| t == "[C]"), "boutons : {:?}", libelles);
        assert!(libelles.iter().any(|t| t == "[S]"), "boutons : {:?}", libelles);
        assert!(!libelles.iter().any(|t| t == "[B]"), "sans couple de rang B, pas de bouton");

        // le défaut prend bien les plus bas et laisse les S en réserve
        g.apply(Action::PenStart(0, ci, None));
        assert_eq!(g.s.inv2[ci].m[0], 0);
        assert_eq!(g.s.inv2[ci].m[3], 1, "le S ne doit pas partir tout seul");
        assert_eq!(g.s.pens[0].as_ref().unwrap().r1, 0);

        // et le rang choisi consomme ce rang-là
        g.s.pens[0] = None;
        g.apply(Action::PenStart(0, ci, Some(3)));
        assert_eq!(g.s.inv2[ci].m[3], 0, "le couple S choisi est consommé");
        let pen = g.s.pens[0].as_ref().unwrap();
        assert_eq!((pen.r1, pen.r2), (3, 3));
    }

    /* le puits : une offrande par jour, dans sa fenêtre, et jamais un shiny. */
    #[test]
    fn le_puits_ne_boit_qu_une_fois_par_jour() {
        let mut g = jeu_neuf();
        let ci = 0;
        g.s.inv2[ci].m[0] = 2;
        g.s.inv2[ci].sm[0] = 1; // un shiny, qui ne doit jamais partir

        // hors fenêtre, rien ne se passe
        g.s.well_night = -1.0;
        let avant = g.s.inv2[ci].tn();
        if !puits_assoiffe(now_ms()) {
            g.apply(Action::Sacrifice(ci));
            assert_eq!(g.s.inv2[ci].tn(), avant, "le puits endormi ne prend rien");
            assert_eq!(g.s.sacrifices, 0);
        }

        /* on force la nuit en cours pour tester la suite sans dépendre de
           l'heure qu'il est : take_lowest_one et le verrou de nuit */
        let (rang, _) = g.take_lowest_one(ci).expect("un spécimen doit partir");
        assert_eq!(rang, 0, "c'est le plus bas rang qui part");
        assert_eq!(g.s.inv2[ci].tn(), avant - 1);
        assert_eq!(g.s.inv2[ci].ts(), 1, "le shiny reste en réserve");

        // tant qu'il n'a pas bu, chaque espèce en réserve peut être offerte
        g.s.well_night = -1.0;
        let (_, rows) = g.build_rows(&PanelKind::Puits);
        assert!(
            rows.iter().any(|r| r.btns.iter().any(|(t, _, _)| t == "sacrifier")),
            "le puits doit proposer une offrande"
        );

        // une fois servi, il ne prend plus rien de la journée et cesse de luire
        g.s.well_night = jour_reel(now_ms());
        assert!(g.well_used_tonight(), "le jour doit être marqué");
        assert!(!g.puits_luit(now_ms()), "rassasié, le puits ne doit plus luire");
        /* une fenêtre de soif prise dans la journée réelle en cours : elles
           reviennent douze fois par jour, il y en a forcément une */
        let debut = jour_reel(now_ms()) * 86_400_000.0;
        let t_soif = (0..1440)
            .map(|k| debut + k as f64 * 60_000.0)
            .find(|&t| puits_assoiffe(t))
            .expect("le puits a soif douze fois par jour");
        assert!(!g.puits_luit(t_soif), "même assoiffé, il ne luit pas s'il a déjà bu");
        g.s.well_night = -1.0;
        assert!(g.puits_luit(t_soif), "à jeun et assoiffé, la margelle bat");
        g.s.well_night = jour_reel(now_ms());
        let (_, rows) = g.build_rows(&PanelKind::Puits);
        let texte: String = rows.iter().flat_map(|r| r.segs.iter().map(|(t, _)| t.clone())).collect();
        assert!(texte.contains("son compte"), "{}", texte);
        assert!(!rows.iter().any(|r| r.btns.iter().any(|(t, _, _)| t == "sacrifier")));
        let apres = g.s.inv2[ci].tn();
        g.apply(Action::Sacrifice(ci));
        assert_eq!(g.s.inv2[ci].tn(), apres, "une seule offrande par jour");
    }

    /* l'horloge du village : une journée en deux heures réelles, et la soif du
       puits qui suit ce cycle plutôt que la montre du joueur. */
    #[test]
    fn la_soif_du_puits_suit_l_horloge_du_village() {
        let base = 1_790_000_000_000.0;
        // le numéro de nuit ne change pas d'une heure de village à l'autre
        let n0 = nuit_index(base);
        assert_eq!(nuit_index(base + JOUR_JEU_MS / 24.0), n0, "une heure de village plus tard, même nuit");
        // une nuit sur huit
        let lunes = (0..80).filter(|k| pleine_lune(base + *k as f64 * JOUR_JEU_MS)).count();
        assert!((9..=11).contains(&lunes), "{} pleines lunes sur 80 nuits", lunes);

        /* la proportion de nuit est celle d'une vraie journée — 21 h à 7 h —
           mais répartie sur deux heures réelles, donc accessible à tous */
        let pas = 60_000.0;
        let n = (JOUR_JEU_MS / pas) as i64;
        let nuits = (0..n).filter(|k| is_night_at(base + *k as f64 * pas)).count() as f64 / n as f64;
        assert!((nuits - 10.0 / 24.0).abs() < 0.02, "part de nuit : {:.2}", nuits);

        /* le puits a soif deux heures de village par jour, plus la nuit de
           pleine lune : sur huit nuits, un huitième du temps environ */
        let m = (JOUR_JEU_MS * 8.0 / pas) as i64;
        let soif = (0..m).filter(|k| puits_assoiffe(base + *k as f64 * pas)).count() as f64 / m as f64;
        assert!((0.10..0.16).contains(&soif), "part de soif sur huit nuits : {:.3}", soif);
        // et les quatre trophées scellés restent muets tant qu'ils dorment
        let g = jeu_neuf();
        let (_, rows) = g.build_rows(&PanelKind::Achs);
        let texte: String = rows.iter().flat_map(|r| r.segs.iter().map(|(t, _)| t.clone())).collect();
        for &i in ACH_SCELLES.iter() {
            assert!(!texte.contains(ACHS[i].n), "« {} » ne doit pas s'afficher avant d'être gagné", ACHS[i].n);
        }
        assert!(texte.contains("? ? ?"), "les trophées scellés doivent apparaître masqués");
    }

    /* le journal tient deux files : les prises d'un côté, le reste de l'autre.
       une éclosion ne doit pas être chassée par trois captures. */
    #[test]
    fn le_journal_separe_les_prises_du_reste() {
        let mut g = jeu_neuf();
        g.log(vec![("une naissance à l'enclos".into(), C::Green)]);
        for _ in 0..10 {
            g.log_prise(vec![("forêt → mulotin".into(), C::Text)]);
        }
        let prises: Vec<&LogLine> = g.logs.iter().filter(|l| l.prise).collect();
        let reste: Vec<&LogLine> = g.logs.iter().filter(|l| !l.prise).collect();
        assert_eq!(prises.len(), 10);
        assert_eq!(reste.len(), 1, "la naissance doit survivre au flot des captures");
        assert!(reste[0].segs[0].0.contains("naissance"));

        /* et la colonne des prises garde bien les plus récentes en tête */
        g.log_prise(vec![("glacier → mirage".into(), C::Text)]);
        assert!(g.logs.iter().find(|l| l.prise).unwrap().segs[0].0.contains("glacier"));
    }

    /* la vente doit laisser de quoi reproduire : autant de couples que réglé,
       et les meilleurs, jamais les rebuts. */
    #[test]
    fn la_vente_epargne_les_couples_demandes() {
        let mut g = jeu_neuf();
        let ci = 0;
        g.s.lab[LAB_NEGOCE] = 0;
        // huit mâles et huit femelles, rangs mélangés
        for r in 0..4 {
            g.s.inv2[ci].m[r] = 2;
            g.s.inv2[ci].f[r] = 2;
        }
        g.s.autokeep = 3;
        let (vendus, _) = g.sell_surplus(ci);
        assert_eq!(vendus, 16 - 6, "trois couples restent en réserve");
        assert_eq!(g.s.inv2[ci].tn(), 6);
        // et ce sont les meilleurs rangs qui restent
        assert_eq!(g.s.inv2[ci].m[3], 2, "les deux mâles [S] restent");
        assert_eq!(g.s.inv2[ci].f[3], 2, "les deux femelles [S] restent");
        assert_eq!(g.s.inv2[ci].m[0] + g.s.inv2[ci].f[0], 0, "les [C] sont partis");

        // le seuil de surplus suit le réglage
        g.s.autokeep = 1;
        assert_eq!(g.seuil_garde(ci), 2);
        g.s.autokeep = 4;
        assert_eq!(g.seuil_garde(ci), 8);
    }

    /* le sélecteur de l'enclos doit conseiller le couple le plus utile :
       celui qui ferait progresser le registre, au meilleur rang disponible. */
    #[test]
    fn l_enclos_conseille_le_couple_le_plus_utile() {
        let mut g = jeu_neuf();
        // une espèce déjà au record, avec un beau couple
        g.s.inv2[1].m[3] = 1;
        g.s.inv2[1].f[3] = 1;
        g.s.dex2[1].best = 4; // déjà [S] au registre
        // une autre, moins bien lotie, mais qui ferait progresser le registre
        g.s.inv2[2].m[2] = 1;
        g.s.inv2[2].f[2] = 1;
        g.s.dex2[2].best = 1; // [C] au registre

        let (ci, rang) = g.meilleur_couple().expect("un couple doit être conseillé");
        assert_eq!(ci, 2, "le conseil doit viser ce qui fait progresser le registre");
        assert_eq!(rang, 2);

        let (_, rows) = g.build_rows(&PanelKind::PenPick(0));
        let texte: String = rows.iter().flat_map(|r| r.segs.iter().map(|(t, _)| t.clone())).collect();
        assert!(texte.contains("conseil : "), "le panneau doit porter le conseil");
        assert!(texte.contains("↑"), "les couples qui font progresser sont marqués");
        // la ligne conseillée passe devant celle qui n'apporte rien
        let pos = |ci: usize| texte.find(CREATURES[ci].n).unwrap_or(usize::MAX);
        assert!(pos(2) < pos(1), "l'espèce utile doit être listée avant l'autre");
    }
}
