// affut.sh — un comptoir de capture pour gens patients
// TUI ratatui/crossterm, même stack que late.sh

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

/* ================================================================ données */

const RAR_LABEL: [&str; 5] = ["commun", "peu commun", "rare", "épique", "légendaire"];
const RAR_W: [f64; 5] = [100.0, 38.0, 11.0, 2.6, 0.45];
const RAR_VAL: [f64; 5] = [4.0, 12.0, 45.0, 220.0, 1500.0];

struct BiomeDef {
    name: &'static str,
    cost: f64,
    mult: f64,
    desc: &'static str,
}
const BIOMES: [BiomeDef; 6] = [
    BiomeDef { name: "forêt",    cost: 0.0,       mult: 1.0,  desc: "des sous-bois humides où tout bruisse. le point de départ de toute traque." },
    BiomeDef { name: "marais",   cost: 1000.0,    mult: 2.0,  desc: "de la vase, des bulles, des choses qui clignent des yeux sous la surface." },
    BiomeDef { name: "montagne", cost: 8000.0,    mult: 4.0,  desc: "des cimes venteuses. les pièges y gèlent mais les prises valent le détour." },
    BiomeDef { name: "désert",   cost: 50000.0,   mult: 8.0,  desc: "des dunes à perte de vue. tout ce qui y survit vaut cher." },
    BiomeDef { name: "glacier",  cost: 300000.0,  mult: 16.0, desc: "un silence bleu et parfait. les espèces y sont rares et magnifiques." },
    BiomeDef { name: "abysses",  cost: 2000000.0, mult: 32.0, desc: "là où la lumière renonce. le fond du bestiaire, littéralement." },
];

struct CreatureDef {
    b: usize,
    r: usize,
    g: &'static str,
    n: &'static str,
    lore: &'static str,
}
const CREATURES: [CreatureDef; 60] = [
    CreatureDef { b: 0, r: 0, g: "(o.o)", n: "mulotin",          lore: "un rongeur curieux qui entasse des graines dans les pièges eux-mêmes." },
    CreatureDef { b: 0, r: 0, g: "~(°>",  n: "sourivole",        lore: "moitié souris, moitié feuille morte. plane mal, atterrit pire." },
    CreatureDef { b: 0, r: 0, g: ".ø.",   n: "champillon",       lore: "un champignon qui marche. lentement, mais il marche." },
    CreatureDef { b: 0, r: 0, g: "=ö=",   n: "bourdonel",        lore: "bourdonne en permanence, même endormi. surtout endormi." },
    CreatureDef { b: 0, r: 1, g: "\\|/",  n: "cerfeuil",         lore: "un petit cervidé dont les bois fleurissent au printemps." },
    CreatureDef { b: 0, r: 1, g: "/\\.",  n: "renardou cendré",  lore: "sa fourrure sent le feu de camp éteint. personne ne sait pourquoi." },
    CreatureDef { b: 0, r: 2, g: "{|}",   n: "sylvestre",        lore: "un esprit d'arbre qui déteste être dérangé mais adore les appâts." },
    CreatureDef { b: 0, r: 2, g: "*.*",   n: "lucioleau",        lore: "clignote en morse. les messages sont rarement polis." },
    CreatureDef { b: 0, r: 3, g: "(*)",   n: "dryadelle",        lore: "gardienne des clairières. se laisse capturer uniquement par curiosité." },
    CreatureDef { b: 0, r: 4, g: "\\VV/", n: "grand cornu",      lore: "le patron de la forêt. les autres espèces s'inclinent sur son passage." },
    CreatureDef { b: 1, r: 0, g: "~o~",   n: "vasouille",        lore: "une bulle de vase avec des yeux. pop." },
    CreatureDef { b: 1, r: 0, g: "(o)~",  n: "crapotin",         lore: "croasse faux. les autres crapauds l'évitent." },
    CreatureDef { b: 1, r: 0, g: "_@/",   n: "limacet",          lore: "laisse une trace luisante qui épelle son propre nom." },
    CreatureDef { b: 1, r: 0, g: "-=+",   n: "moustiflard",      lore: "trop gros pour voler discrètement, trop têtu pour arrêter." },
    CreatureDef { b: 1, r: 1, g: "(°°)",  n: "grenouillard",     lore: "un vieux sage amphibien. donne des conseils que personne ne demande." },
    CreatureDef { b: 1, r: 1, g: "~~~o",  n: "sangsurelle",      lore: "s'attache facilement. au sens propre, hélas." },
    CreatureDef { b: 1, r: 2, g: ".::.",  n: "brumelin",         lore: "un morceau de brouillard devenu autonome un soir d'octobre." },
    CreatureDef { b: 1, r: 2, g: "!*!",   n: "feufollet",        lore: "attire les voyageurs vers les pièges. techniquement un collègue." },
    CreatureDef { b: 1, r: 3, g: "}~{",   n: "hydrelle",         lore: "trois têtes, un seul avis : contre." },
    CreatureDef { b: 1, r: 4, g: "~S~",   n: "basilombre",       lore: "son regard fige la vase elle-même. ne le fixez pas trop longtemps." },
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
    CreatureDef { b: 3, r: 0, g: "(=)",   n: "scarabinet",       lore: "pousse une boule de sable partout. c'est son projet de vie." },
    CreatureDef { b: 3, r: 0, g: "~s",    n: "serpentile",       lore: "écrit des poèmes dans le sable en rampant. illisibles." },
    CreatureDef { b: 3, r: 0, g: "^..^",  n: "fennecot",         lore: "ses oreilles captent la radio. il préfère le jazz." },
    CreatureDef { b: 3, r: 0, g: "|#|",   n: "cactille",         lore: "un cactus timide. les épines, c'est de la gêne." },
    CreatureDef { b: 3, r: 1, g: "-E<",   n: "scorpiard",        lore: "brille sous la lune et le sait parfaitement." },
    CreatureDef { b: 3, r: 1, g: "\\_/",  n: "vautourin",        lore: "patiente au-dessus des pièges. il a compris le concept." },
    CreatureDef { b: 3, r: 2, g: ".?.",   n: "mirageon",         lore: "existe-t-il vraiment ? le piège dit oui. le doute demeure." },
    CreatureDef { b: 3, r: 2, g: "=A=",   n: "dunataure",        lore: "mi-homme mi-dune. entièrement insaisissable, ou presque." },
    CreatureDef { b: 3, r: 3, g: "[:]",   n: "sphinxel",         lore: "pose une énigme avant chaque capture. le piège ne répond jamais, ça l'agace." },
    CreatureDef { b: 3, r: 4, g: "OOO~",  n: "ver des sables",   lore: "le désert n'est pas vide. il digère." },
    CreatureDef { b: 4, r: 0, g: "(v)",   n: "pingolin",         lore: "glisse sur le ventre par efficacité, pas par jeu. enfin, un peu par jeu." },
    CreatureDef { b: 4, r: 0, g: "*o*",   n: "frimousse",        lore: "une boule de neige avec un visage. fond au printemps, revient vexée." },
    CreatureDef { b: 4, r: 0, g: "(\\_",  n: "lièvrelin",        lore: "blanc sur blanc. on ne capture souvent que ses empreintes." },
    CreatureDef { b: 4, r: 0, g: ":3=",   n: "morsille",         lore: "des défenses imposantes, un caractère de peluche." },
    CreatureDef { b: 4, r: 1, g: "vvv",   n: "stalactin",        lore: "tombe du plafond des grottes sur les pièges. par solidarité." },
    CreatureDef { b: 4, r: 1, g: "|-|",   n: "rennelune",        lore: "ne touche jamais vraiment le sol. vérifiez ses empreintes." },
    CreatureDef { b: 4, r: 2, g: "[Y]",   n: "yétillon",         lore: "un yéti junior. floute lui-même les photos, c'est de famille." },
    CreatureDef { b: 4, r: 2, g: "≈≈≈",   n: "aurorelle",        lore: "un ruban d'aurore boréale qui a pris goût au sol." },
    CreatureDef { b: 4, r: 3, g: "|>o",   n: "givrecorne",       lore: "sa corne givre l'air. les collectionneurs givrent d'envie." },
    CreatureDef { b: 4, r: 4, g: "~O~",   n: "léviathan blanc",  lore: "la banquise, c'est son dos. réfléchissez-y." },
    CreatureDef { b: 5, r: 0, g: "-o)",   n: "lanternet",        lore: "sa lampe frontale est en panne un jour sur deux. il fait avec." },
    CreatureDef { b: 5, r: 0, g: "(((",   n: "méduselle",        lore: "transparente et fière de l'être. difficile à compter." },
    CreatureDef { b: 5, r: 0, g: "}={",   n: "crabique",         lore: "marche de côté même dans ses rêves." },
    CreatureDef { b: 5, r: 0, g: "~~>",   n: "anguiliss",        lore: "un éclair au ralenti. électrise les conversations, littéralement." },
    CreatureDef { b: 5, r: 1, g: "(8)",   n: "poulpinet",        lore: "ouvre les pièges de l'intérieur. reste dedans par confort." },
    CreatureDef { b: 5, r: 1, g: ">:)",   n: "nocturnix",        lore: "sourit dans le noir. c'est précisément le problème." },
    CreatureDef { b: 5, r: 2, g: ".-.",   n: "spectrelle",       lore: "le fantôme d'un poisson qui refuse d'admettre quoi que ce soit." },
    CreatureDef { b: 5, r: 2, g: "[ ]",   n: "néantin",          lore: "un morceau de rien, soigneusement encadré." },
    CreatureDef { b: 5, r: 3, g: "{X}",   n: "krakenot",         lore: "un kraken de poche. les navires miniatures le redoutent." },
    CreatureDef { b: 5, r: 4, g: "(Ω)",   n: "ancien des profondeurs", lore: "il était là avant les biomes. il sera là après vous." },
];

fn biome_creatures(b: usize) -> impl Iterator<Item = usize> {
    (0..CREATURES.len()).filter(move |&i| CREATURES[i].b == b)
}

struct TrapDef { n: &'static str, cost: f64, itv: f64, luck: f64, succ: f64 }
const TRAPS: [TrapDef; 6] = [
    TrapDef { n: "piège en bois",   cost: 25.0,     itv: 30.0, luck: 0.0,  succ: 0.55 },
    TrapDef { n: "cage en fer",     cost: 400.0,    itv: 24.0, luck: 0.15, succ: 0.65 },
    TrapDef { n: "piège à ressort", cost: 2500.0,   itv: 18.0, luck: 0.35, succ: 0.75 },
    TrapDef { n: "piège chromé",    cost: 15000.0,  itv: 13.0, luck: 0.6,  succ: 0.85 },
    TrapDef { n: "piège à plasma",  cost: 90000.0,  itv: 9.0,  luck: 1.0,  succ: 0.92 },
    TrapDef { n: "piège quantique", cost: 600000.0, itv: 6.0,  luck: 1.6,  succ: 0.98 },
];

struct BaitDef { n: &'static str, cost: f64, desc: &'static str }
const BAITS: [BaitDef; 5] = [
    BaitDef { n: "baies sauvages",    cost: 8.0,    desc: "vitesse du piège +25%" },
    BaitDef { n: "viande fumée",      cost: 30.0,   desc: "chance +0,35" },
    BaitDef { n: "nectar doré",       cost: 120.0,  desc: "valeur des prises ×1,5 · chance +0,2" },
    BaitDef { n: "truffe des brumes", cost: 400.0,  desc: "poids des raretés rare+ ×2,5" },
    BaitDef { n: "essence lunaire",   cost: 1000.0, desc: "chance de shiny ×4 · chance +0,3" },
];
const BAIT_BAIES: usize = 0;
const BAIT_VIANDE: usize = 1;
const BAIT_NECTAR: usize = 2;
const BAIT_TRUFFE: usize = 3;
const BAIT_ESSENCE: usize = 4;

struct LabDef { n: &'static str, max: u32, base: f64, mult: f64, desc: &'static str }
const LABS: [LabDef; 12] = [
    LabDef { n: "affûtage",        max: 10, base: 200.0,  mult: 2.2, desc: "des mâchoires mieux huilées : +6% de vitesse par niveau." },
    LabDef { n: "flair",           max: 15, base: 300.0,  mult: 2.1, desc: "l'instinct du traqueur : +0,06 de chance par niveau." },
    LabDef { n: "négoce",          max: 15, base: 250.0,  mult: 2.1, desc: "l'art de la marge : +8% aux prix de vente par niveau." },
    LabDef { n: "horlogerie",      max: 11, base: 500.0,  mult: 2.0, desc: "progression hors-ligne simulée au retour : 2 h de base, +2 h par niveau. les pièges, eux, ne s'usent jamais." },
    LabDef { n: "chasse nocturne", max: 10, base: 1000.0, mult: 2.3, desc: "sortir aux bonnes heures : +15% de chance de shiny par niveau." },
    LabDef { n: "auto-vente",      max: 1,  base: 5000.0, mult: 1.0, desc: "revend les doublons dès la capture, selon vos filtres (à la boutique)." },
    LabDef { n: "conservation",    max: 10, base: 2000.0, mult: 1.9, desc: "de meilleures vitrines : la cagnotte du musée accumule +2 h par niveau (4 h de base)." },
    LabDef { n: "ailes du musée",  max: 6,  base: 5000.0, mult: 2.2, desc: "on pousse les murs : +1 salle d'exposition par niveau." },
    LabDef { n: "grands enclos",   max: 3,  base: 4000.0, mult: 2.5, desc: "plus de place pour les couples : +1 enclos par niveau." },
    LabDef { n: "lignées",         max: 5,  base: 3000.0, mult: 2.2, desc: "registres d'élevage : +5% de chance qu'une naissance monte d'un rang (35% de base)." },
    LabDef { n: "traqueur",        max: 5,  base: 1500.0, mult: 2.0, desc: "meilleure endurance : le repos entre deux battues diminue de 30 s par niveau (5 min de base)." },
    LabDef { n: "courtage",        max: 5,  base: 2500.0, mult: 2.1, desc: "carnet d'adresses : les primes de contrats augmentent de 20% par niveau." },
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

struct AchDef { n: &'static str, d: &'static str, r: f64 }
const ACHS: [AchDef; 26] = [
    AchDef { n: "première prise",         d: "capturer une créature",                r: 50.0 },
    AchDef { n: "braconnier du dimanche", d: "capturer 100 créatures",               r: 500.0 },
    AchDef { n: "main verte",             d: "capturer 1 000 créatures",             r: 5000.0 },
    AchDef { n: "force de la nature",     d: "capturer 10 000 créatures",            r: 50000.0 },
    AchDef { n: "ça brille",              d: "capturer un shiny",                    r: 1000.0 },
    AchDef { n: "aimant à paillettes",    d: "capturer 25 shinies",                  r: 25000.0 },
    AchDef { n: "chasseur de mythes",     d: "capturer une créature légendaire",     r: 3000.0 },
    AchDef { n: "carnet de terrain",      d: "découvrir 10 espèces",                 r: 300.0 },
    AchDef { n: "encyclopédiste",         d: "découvrir 30 espèces",                 r: 3000.0 },
    AchDef { n: "bestiaire complet",      d: "découvrir les 60 espèces",             r: 100000.0 },
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
];

const SHINY_BASE: f64 = 1.0 / 128.0;

const SAISONS: [&str; 4] = ["printemps", "été", "automne", "hiver"];
const METEOS: [&str; 6] = ["ciel clair", "pluie", "brume", "canicule", "tempête", "nuit étoilée"];
/* espèces qui ne sortent que la nuit (21 h – 7 h) */
const NOCTURNES: [usize; 6] = [7, 17, 24, 36, 47, 55]; // lucioleau, feufollet, cristalpin, mirageon, aurorelle, nocturnix
const RANK_NAMES: [&str; 4] = ["C", "B", "A", "S"];
const RANK_MULT: [f64; 4] = [1.0, 1.6, 2.8, 6.0];
/* position de la légende errante dans chaque biome */
const LEGEND_SPOTS: [(usize, usize); 6] = [(16, 9), (16, 33), (56, 4), (95, 20), (95, 5), (95, 38)];
/* durée de couvaison à l'enclos, par rareté (minutes) */
const PEN_MIN: [f64; 5] = [10.0, 20.0, 45.0, 120.0, 360.0];

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

/* réserve par espèce : compteurs par rang (C,B,A,S), par sexe, normaux et shinies.
   n/s sont les anciens compteurs sans sexe (migrés puis à zéro). */
#[derive(Serialize, Deserialize, Clone, Default)]
struct InvE {
    n: [u64; 4],
    s: [u64; 4],
    #[serde(default)]
    m: [u64; 4],
    #[serde(default)]
    f: [u64; 4],
    #[serde(default)]
    sm: [u64; 4],
    #[serde(default)]
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
    inv: Vec<(u64, u64)>, // hérité (anciennes sauvegardes), migré vers inv2
    dex: Vec<(u64, u64)>, // hérité, migré vers dex2
    #[serde(default)]
    inv2: Vec<InvE>,
    #[serde(default)]
    dex2: Vec<DexE>,
    #[serde(default)]
    contracts_window: u64,
    #[serde(default)]
    contracts_done: Vec<bool>,
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
    #[serde(default)]
    pen_born: u64,
    lab: Vec<u32>,
    autosell: Vec<bool>,
    ach: Vec<bool>,
    last_seen: f64,
}
impl Default for State {
    fn default() -> Self {
        let mut biomes = vec![None; 6];
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
            inv: vec![],
            dex: vec![],
            inv2: vec![InvE::default(); 60],
            dex2: vec![DexE::default(); 60],
            contracts_window: 0,
            contracts_done: vec![false; 3],
            museum: vec![None; 6],
            museum_at: 0.0,
            museum_pool: 0.0,
            pens: vec![None; 3],
            legends_tried: vec![],
            hunts_done: 0,
            contracts_delivered: 0,
            legends_caught: 0,
            pen_born: 0,
            lab: vec![0; LABS.len()],
            autosell: vec![false; 5],
            ach: vec![false; ACHS.len()],
            last_seen: now_ms(),
        }
    }
}
impl State {
    fn normalize(&mut self) {
        self.traps.resize(6, 0);
        self.baits.resize(5, 0);
        self.biomes.resize(6, None);
        self.lab.resize(LABS.len(), 0);
        self.autosell.resize(5, false);
        self.ach.resize(ACHS.len(), false);
        self.inv2.resize(60, InvE::default());
        self.dex2.resize(60, DexE::default());
        self.contracts_done.resize(3, false);
        self.museum.resize(12, None);
        self.pens.resize(6, None);
        // migration des sauvegardes d'avant les rangs : tout passe en rang C
        if self.dex2.iter().all(|d| d.n == 0) && self.dex.iter().any(|d| d.0 > 0) {
            for (ci, &(n, s)) in self.dex.iter().enumerate().take(60) {
                self.dex2[ci] = DexE { n, s, best: if n > 0 { 1 } else { 0 }, bests: if s > 0 { 1 } else { 0 }, mf: 0 };
            }
            for (ci, &(n, s)) in self.inv.iter().enumerate().take(60) {
                self.inv2[ci].n[0] = n;
                self.inv2[ci].s[0] = s;
            }
        }
        self.inv = vec![];
        self.dex = vec![];
        // migration v2 -> sexes : répartir les anciens compteurs sans sexe ~50/50
        let no_sex = self.inv2.iter().all(|e| e.m.iter().all(|&x| x == 0) && e.f.iter().all(|&x| x == 0) && e.sm.iter().all(|&x| x == 0) && e.sf.iter().all(|&x| x == 0));
        let has_old = self.inv2.iter().any(|e| e.n.iter().any(|&x| x > 0) || e.s.iter().any(|&x| x > 0));
        if no_sex && has_old {
            for e in self.inv2.iter_mut() {
                for r in 0..4 {
                    e.m[r] = (e.n[r] + 1) / 2;
                    e.f[r] = e.n[r] / 2;
                    e.sm[r] = (e.s[r] + 1) / 2;
                    e.sf[r] = e.s[r] / 2;
                    e.n[r] = 0;
                    e.s[r] = 0;
                }
            }
            for d in self.dex2.iter_mut() {
                if d.n >= 2 {
                    d.mf = 3;
                } else if d.n == 1 {
                    d.mf = 1;
                }
            }
        }
    }
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as f64
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}
fn is_night_at(ms: f64) -> bool {
    use chrono::{Timelike, TimeZone};
    match chrono::Local.timestamp_millis_opt(ms as i64).single() {
        Some(d) => {
            let h = d.hour();
            h >= 21 || h < 7
        }
        None => false,
    }
}
fn season_at(ms: f64) -> usize {
    ((ms / 86_400_000.0) as u64 % 4) as usize
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
        Some(p) => std::path::Path::new(&home).join(format!(".affutsh-{}.json", p)),
        None => std::path::Path::new(&home).join(".affutsh.json"),
    }
}
fn legacy_save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".traquesh.json")
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
        let ct = std::env::var("COLORTERM").unwrap_or_default();
        Theme { truecolor: ct.contains("truecolor") || ct.contains("24bit") }
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
const MAPH: usize = 46;

#[derive(Clone, Copy)]
struct Cell { ch: char, c: C, solid: bool }

struct WorldMap {
    cells: Vec<Vec<Cell>>,
    doors: Vec<(usize, usize, Zone)>,
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
}

const ZONE_RECTS: [(usize, usize, usize, usize, usize); 6] = [
    (1, 2, 33, 18, 0),   // forêt
    (1, 24, 33, 21, 1),  // marais
    (37, 1, 39, 9, 2),   // montagne
    (79, 15, 34, 13, 3), // désert
    (79, 1, 34, 11, 4),  // glacier
    (79, 31, 34, 14, 5), // abysses
];
const LABEL_POS: [(usize, usize); 6] = [(12, 3), (12, 25), (50, 2), (90, 16), (90, 2), (90, 32)];

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
        };
        let mut rng = StdRng::seed_from_u64(1337);

        // village : sol
        for y in 12..=33 {
            for x in 37..=76 {
                if rng.gen::<f64>() < 0.5 {
                    w.put(x, y, '░', C::Dimmer, false);
                }
            }
        }
        // chemins
        let mut path = |w: &mut WorldMap, x1: usize, y1: usize, x2: usize, y2: usize| {
            let (mut x, mut y) = (x1 as i32, y1 as i32);
            while x != x2 as i32 {
                w.put(x as usize, y as usize, '░', C::Dimmer, false);
                x += (x2 as i32 - x).signum();
            }
            while y != y2 as i32 {
                w.put(x as usize, y as usize, '░', C::Dimmer, false);
                y += (y2 as i32 - y).signum();
            }
        };
        path(&mut w, 37, 22, 20, 12);
        path(&mut w, 37, 28, 20, 34);
        path(&mut w, 56, 12, 56, 6);
        path(&mut w, 76, 20, 95, 6);
        path(&mut w, 76, 24, 95, 21);
        path(&mut w, 76, 30, 95, 37);
        // grand corridor du village : traversée est-ouest garantie
        for x in 37..=76 {
            w.put(x, 22, '░', C::Dimmer, false);
        }

        // biomes
        w.scatter(1, 2, 33, 18, &['♣', '♣', '♠', '.'], C::Green, 0.16, true, &mut rng);
        w.scatter(37, 1, 39, 9, &['^', '▲', '/', '∆'], C::Dim, 0.13, true, &mut rng);
        w.scatter(79, 1, 34, 11, &['*', '▲', '·'], C::Ice, 0.12, true, &mut rng);
        w.scatter(1, 24, 33, 21, &['~', '~', 'o', '"'], C::Marsh, 0.14, false, &mut rng);
        w.scatter(79, 15, 34, 13, &['∙', '≈', '·'], C::GoldDark, 0.12, false, &mut rng);
        w.scatter(79, 31, 34, 14, &['▓', '▒', '●', '·'], C::Abyss, 0.13, true, &mut rng);

        // plantes du village : purement décoratives, jamais bloquantes
        w.scatter(37, 12, 40, 22, &['♣'], C::Green, 0.012, false, &mut rng);

        // sol du quartier sud + chemins d'accès
        for y in 34..=42 {
            for x in 39..=76 {
                if rng.gen::<f64>() < 0.4 {
                    w.put(x, y, '░', C::Dimmer, false);
                }
            }
        }
        path(&mut w, 49, 30, 49, 36);
        path(&mut w, 67, 30, 67, 36);

        // bâtiments
        w.building(39, 14, 16, 6, "boutique", Zone::Boutique, 'o');
        w.building(59, 14, 16, 6, "labo", Zone::Labo, 'l');
        w.building(43, 25, 13, 5, "bestiaire", Zone::Bestiaire, 'b');
        w.building(61, 25, 13, 5, "trophées", Zone::Succes, 't');
        w.building(43, 37, 13, 5, "musée", Zone::Musee, 'm');
        w.building(61, 37, 13, 5, "enclos", Zone::Enclos, 'e');

        // fontaine (décalée sous le corridor)
        w.text(55, 23, "╭─╮", C::Blue, true);
        w.text(55, 24, "╰─╯", C::Blue, true);

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
    BuyBait(usize, u64),
    BuyLab(usize),
    Unlock(usize),
    BuySlot(usize),
    Place(usize, usize, usize),
    SetBait(usize, usize, Option<usize>),
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
    PenStart(usize, usize),
    PenCollect(usize),
    LegendTry(usize, u64, Option<usize>),
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
}

struct Game {
    s: State,
    world: WorldMap,
    px: i32,
    py: i32,
    panels: Vec<Panel>,
    logs: VecDeque<LogLine>,
    toasts: Vec<(String, Instant)>,
    quit: bool,
    panel_w: usize,
    legend_seen: u64,
}

impl Game {
    fn new() -> (Game, bool) {
        // le repli vers l'ancien nom de fichier ne vaut que pour le propriétaire :
        // un joueur invité (AFFUT_PLAYER) démarre toujours son propre monde
        let raw = std::fs::read_to_string(save_path()).or_else(|e| {
            if std::env::var("AFFUT_PLAYER").is_err() {
                std::fs::read_to_string(legacy_save_path())
            } else {
                Err(e)
            }
        });
        let (mut s, fresh) = match raw {
            Ok(raw) => (serde_json::from_str::<State>(&raw).unwrap_or_default(), false),
            Err(_) => (State::default(), true),
        };
        s.normalize();
        let game = Game {
            s,
            world: WorldMap::build(),
            px: 50,
            py: 22,
            panels: vec![],
            logs: VecDeque::new(),
            toasts: vec![],
            quit: false,
            panel_w: 74,
            legend_seen: 0,
        };
        (game, fresh)
    }

    fn log(&mut self, segs: Vec<(String, C)>) {
        let t = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push_front(LogLine { t, segs });
        self.logs.truncate(80);
    }
    fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), Instant::now()));
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }
    fn save(&mut self) {
        self.s.last_seen = now_ms();
        if let Ok(json) = serde_json::to_string(&self.s) {
            let _ = std::fs::write(save_path(), json);
        }
    }

    /* ------------------------------------------------------------- bonus */

    fn completed_biomes(&self) -> usize {
        (0..6).filter(|&b| biome_creatures(b).all(|i| self.s.dex2[i].n > 0)).count()
    }
    fn shiny_completed_biomes(&self) -> usize {
        (0..6).filter(|&b| biome_creatures(b).all(|i| self.s.dex2[i].s > 0)).count()
    }
    fn global_luck(&self) -> f64 {
        self.s.lab[LAB_FLAIR] as f64 * 0.06 + self.s.trophies as f64 * 0.04 + self.completed_biomes() as f64 * 0.10
    }
    fn sell_mult(&self) -> f64 {
        (1.0 + self.s.lab[LAB_NEGOCE] as f64 * 0.08)
            * (1.0 + self.s.trophies as f64 * 0.04)
            * (1.0 + self.shiny_completed_biomes() as f64 * 0.10)
    }
    fn speed_mult(&self) -> f64 {
        1.0 + self.s.lab[LAB_AFFUTAGE] as f64 * 0.06
    }
    fn shiny_chance(&self, bait: Option<usize>, at: f64) -> f64 {
        let mut c = SHINY_BASE * (1.0 + self.s.lab[LAB_ECLAT] as f64 * 0.15) * weather_shiny_mult(weather_at(at));
        if bait == Some(BAIT_ESSENCE) {
            c *= 4.0;
        }
        c.min(0.25)
    }    fn offline_cap_ms(&self) -> f64 {
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
        let boost = 1.0 + luck * 0.35;
        let ps = 0.03 * boost;
        let pa = 0.12 * boost;
        if r < ps { 3 } else if r < ps + pa { 2 } else if r < ps + pa + 0.30 { 1 } else { 0 }
    }
    /* retire jusqu'à q spécimens en commençant par les rangs les plus bas ; renvoie la valeur */
    /* retire jusqu'à q spécimens : rangs les plus bas d'abord, sexe majoritaire d'abord
       (pour préserver les couples) ; renvoie la valeur */
    fn take_lowest(&mut self, ci: usize, shiny: bool, mut q: u64) -> f64 {
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
    fn slot_cost(&self, b: usize) -> f64 {
        let base = BIOMES[b].cost.max(250.0);
        if self.s.biomes[b].as_ref().map(|x| x.slots).unwrap_or(2) == 2 {
            (base * 0.5).floor()
        } else {
            (base * 2.0).floor()
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
        (self.s.run_earned / 25000.0).sqrt().floor() as u32
    }
    fn gain(&mut self, n: f64) {
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
                let mut w = RAR_W[r] * (1.0 + luck).powi(r as i32);
                if bait == Some(BAIT_TRUFFE) && r >= 2 {
                    w *= 2.5;
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
            luck += 0.35;
        }
        if bait == Some(BAIT_NECTAR) {
            luck += 0.2;
        }
        if bait == Some(BAIT_ESSENCE) {
            luck += 0.3;
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
            let keep = 2 + self.contract_need(ci);
            if self.s.inv2[ci].tn() > keep {
                let (_, v) = self.sell_surplus(ci);
                sold = (v * if bait == Some(BAIT_NECTAR) { 1.5 } else { 1.0 }).floor();
                self.gain(sold);
            }
        }
        Some((ci, shiny, is_new, new_shiny, sold, rank + sex as usize * 10))
    }    fn report_catch(&mut self, biome: usize, res: Option<(usize, bool, bool, bool, f64, usize)>) {
        let Some((ci, shiny, is_new, new_shiny, sold, rank_sex)) = res else { return };
        let (rank, sex) = (rank_sex % 10, rank_sex / 10);
        let c = &CREATURES[ci];
        let rank_c = match rank { 3 => C::Gold, 2 => C::Blue, 1 => C::Text, _ => C::Dimmer };
        let mut segs = vec![
            (format!("{} → ", BIOMES[biome].name), C::Dim),
            (format!("{}{}", c.n, if shiny { " ✦" } else { "" }), if shiny { C::Shiny } else { rarity_color(c.r) }),
            (format!(" {}[{}]", if sex == 0 { "♂" } else { "♀" }, RANK_NAMES[rank]), rank_c),
            (format!(" ({}{})", RAR_LABEL[c.r], if shiny { " · shiny" } else { "" }), C::Dimmer),
        ];
        if sold > 0.0 {
            segs.push((format!(" · auto-vente +{} écus", fmt(sold)), C::GoldDark));
        }
        self.log(segs);
        if is_new {
            self.log(vec![
                ("nouvelle espèce découverte : ".into(), C::Green),
                (c.n.to_string(), rarity_color(c.r)),
                (" !".into(), C::Green),
            ]);
            self.toast(format!("nouvelle espèce : {}", c.n));
        }
        if new_shiny && !is_new {
            self.toast(format!("shiny obtenu : {} ✦", c.n));
        }
        if rank == 3 {
            self.toast(format!("rang S : {} !", c.n));
        }
        if c.r == 4 {
            self.toast(format!("légendaire capturé : {}", c.n));
        }
    }    fn tick(&mut self) {
        let now = now_ms();
        for b in 0..6 {
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
        if self.s.contracts_window != cw {
            self.s.contracts_window = cw;
            self.s.contracts_done = vec![false; 3];
            self.log(vec![("de nouveaux contrats sont affichés à la boutique.".into(), C::Blue)]);
        }
        // légende errante : annoncer son apparition (une fois par fenêtre)
        if let Some((w, b, _)) = self.legend_now() {
            if w != self.legend_seen {
                self.legend_seen = w;
                self.log(vec![
                    ("✧ une silhouette immense rôde ".into(), C::Gold),
                    (format!("en {} — trouvez-la sur la carte avant qu'elle ne disparaisse !", BIOMES[b].name), C::Gold),
                ]);
                self.toast(format!("✧ légende errante : {}", BIOMES[b].name));
            }
        }
        // musée : le revenu s'accumule (plafond extensible au labo)
        self.museum_accrue(now);
        self.check_achievements();
        self.s.last_seen = now;
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
        0.35 + self.s.lab[LAB_LIGNEES] as f64 * 0.05
    }
    fn hunt_cooldown_ms(&self) -> f64 {
        (300.0 - self.s.lab[LAB_TRAQUEUR] as f64 * 30.0) * 1000.0
    }
    /* écus par milliseconde générés par les salles occupées */
    fn museum_rate(&self) -> f64 {
        self.s.museum.iter().flatten().map(|m| self.creature_value_r(m.ci, m.shiny, m.rank) * 0.003 / 60_000.0).sum()
    }    fn run_offline(&mut self) {
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
        for b in 0..6 {
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
        for b in 0..6 {
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
            6 => (0..60).any(|c| CREATURES[c].r == 4 && s.dex2[c].n > 0),
            7 => (0..60).filter(|&c| s.dex2[c].n > 0).count() >= 10,
            8 => (0..60).filter(|&c| s.dex2[c].n > 0).count() >= 30,
            9 => (0..60).all(|c| s.dex2[c].n > 0),
            10 => s.biomes[1].is_some(),
            11 => s.biomes[5].is_some(),
            12 => s.total_earned >= 10000.0,
            13 => s.total_earned >= 1000000.0,
            14 => s.traps[5] >= 1,
            15 => s.migrations >= 1,
            16 => s.migrations >= 5,
            17 => biome_creatures(0).all(|c| s.dex2[c].n > 0),
            18 => (0..60).any(|c| s.dex2[c].best >= 4),
            19 => (0..60).filter(|&c| s.dex2[c].best >= 4).count() >= 10,
            20 => s.hunts_done >= 1,
            21 => s.contracts_delivered >= 5,
            22 => s.legends_caught >= 1,
            23 => s.pen_born >= 1,
            24 => s.museum.iter().flatten().count() >= 6,
            25 => NOCTURNES.iter().any(|&c| s.dex2[c].n > 0),
            _ => false,
        }
    }    fn check_achievements(&mut self) {
        for i in 0..ACHS.len() {
            if !self.s.ach[i] && self.ach_done(i) {
                self.s.ach[i] = true;
                if ACHS[i].r > 0.0 {
                    self.gain(ACHS[i].r);
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
                        (format!("vendu : {}× {}{}", q, CREATURES[ci].n, if shiny { " ✦" } else { "" }), C::Text),
                        (format!(" (+{} écus)", fmt(v)), C::GoldDark),
                    ]);
                }
            }
            Action::SellDupes => {
                let mut total = 0.0;
                let mut n = 0u64;
                for ci in 0..60 {
                    if self.s.inv2[ci].tn() > 2 + self.contract_need(ci) {
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
            Action::Migrate => {
                let g = self.trophy_gain();
                if g >= 1 {
                    self.s.trophies += g;
                    self.s.migrations += 1;
                    self.s.ecus = 30.0;
                    self.s.run_earned = 0.0;
                    self.s.traps = vec![0; 6];
                    self.s.traps[0] = 1;
                    self.s.baits = vec![0; 5];
                    self.s.inv2 = vec![InvE::default(); 60];
                    self.s.museum = vec![None; 6];
                    self.s.museum_pool = 0.0;
                    self.s.pens = vec![None; 3];
                    self.s.contracts_done = vec![false; 3];
                    self.s.lab = vec![0; 6];
                    self.s.autosell = vec![false; 5];
                    let mut biomes = vec![None; 6];
                    biomes[0] = Some(BiomeState { slots: 2, pl: vec![None, None], hunt_at: 0.0 });
                    self.s.biomes = biomes;
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
                    let mut hits = 0;
                    for i in 0..slots {
                        if self.s.biomes[b].as_ref().unwrap().pl[i].is_some() {
                            let res = self.attempt(b, i, now, 0.5, false);
                            if res.is_some() {
                                hits += 1;
                            }
                            self.report_catch(b, res);
                        }
                    }
                    let cd = self.hunt_cooldown_ms();
                    self.s.biomes[b].as_mut().unwrap().hunt_at = now + cd;
                    self.s.hunts_done += 1;
                    self.log(vec![(format!("battue en {} : {} prise{}", BIOMES[b].name, hits, if hits > 1 { "s" } else { "" }), C::Green)]);
                    self.check_achievements();
                }
            }
            Action::Deliver(idx) => {
                let (w, contracts) = self.contracts_now();
                if w == self.s.contracts_window && idx < contracts.len() && !self.s.contracts_done[idx] {
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
                        self.log(vec![(format!("exposé au musée : {}{} [{}]", CREATURES[ci].n, if shiny { " ✦" } else { "" }, RANK_NAMES[rank]), C::Blue)]);
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
            Action::MuseumCollect => {
                self.museum_accrue(now_ms());
                let v = self.s.museum_pool.floor();
                if v >= 1.0 {
                    self.s.museum_pool -= v;
                    self.gain(v);
                    self.log(vec![("recette du musée : ".into(), C::Blue), (format!("+{} écus", fmt(v)), C::GoldDark)]);
                }
            }
            Action::PenStart(slot, ci) => {
                let iv = &self.s.inv2[ci];
                if slot < self.pen_slots() && self.s.pens[slot].is_none() && iv.tm() >= 1 && iv.tf() >= 1 {
                    // consomme le mâle et la femelle de plus bas rang
                    let rm = (0..4).find(|&r| self.s.inv2[ci].m[r] > 0).unwrap();
                    let rf = (0..4).find(|&r| self.s.inv2[ci].f[r] > 0).unwrap();
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
                        let shiny = rand::thread_rng().gen::<f64>() < self.shiny_chance(None, now) * 3.0;
                        let (is_new, _, sex) = self.add_specimen(pen.ci, shiny, rank);
                        self.s.pen_born += 1;
                        self.log(vec![
                            ("naissance à l'enclos : ".into(), C::Green),
                            (format!("{}{} {}[{}]", CREATURES[pen.ci].n, if shiny { " ✦" } else { "" }, if sex == 0 { "♂" } else { "♀" }, RANK_NAMES[rank]),
                             if shiny { C::Shiny } else { rarity_color(CREATURES[pen.ci].r) }),
                        ]);
                        self.toast(format!("naissance : {}{}", CREATURES[pen.ci].n, if shiny { " ✦" } else { "" }));
                        let _ = is_new;
                        self.check_achievements();
                    }
                }
            }
            Action::LegendTry(biome, window, bait) => {
                if self.s.legends_tried.contains(&window) {
                    self.panels.pop();
                } else {
                    self.s.legends_tried.push(window);
                    if self.s.legends_tried.len() > 24 {
                        self.s.legends_tried.remove(0);
                    }
                    let mut p = 0.30 + (self.global_luck() * 0.15).min(0.30);
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
                        let pool: Vec<usize> = biome_creatures(biome).filter(|&i| CREATURES[i].r >= 3).collect();
                        let ci = pool[rand::thread_rng().gen_range(0..pool.len())];
                        let rank = self.roll_rank(self.global_luck() + 1.0).max(2);
                        let shiny = rand::thread_rng().gen::<f64>() < (SHINY_BASE * shiny_mult).min(0.30);
                        self.add_specimen(ci, shiny, rank);
                        self.s.legends_caught += 1;
                        self.log(vec![
                            ("légende errante capturée : ".into(), C::Gold),
                            (format!("{}{} [{}] !", CREATURES[ci].n, if shiny { " ✦" } else { "" }, RANK_NAMES[rank]),
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
    fn contract_need(&self, ci: usize) -> u64 {
        let (w, cs) = self.contracts_now();
        let mut n = 0;
        for (i, c) in cs.iter().enumerate() {
            if c.ci == ci {
                let done = w == self.s.contracts_window && self.s.contracts_done.get(i).copied().unwrap_or(false);
                if !done {
                    n += c.qty;
                }
            }
        }
        n
    }
    /* vend le surplus au-delà du meilleur couple ♂♀ ET des commandes en cours */
    fn sell_surplus(&mut self, ci: usize) -> (u64, f64) {
        let need = self.contract_need(ci);
        let bm = self.take_best_sex(ci, false, 0);
        let bf = self.take_best_sex(ci, false, 1);
        let q = self.s.inv2[ci].tn().saturating_sub(need);
        let v = self.take_lowest(ci, false, q);
        if let Some(r) = bm {
            self.give_back(ci, false, 0, r);
        }
        if let Some(r) = bf {
            self.give_back(ci, false, 1, r);
        }
        (q, v)
    }
    /* contrats du créneau courant (déterministes, 3 à la fois, renouvelés toutes les 2 h) */
    fn contracts_now(&self) -> (u64, Vec<Contract>) {
        let w = (now_ms() / 7_200_000.0) as u64;
        let mut rng = StdRng::seed_from_u64(splitmix(w ^ 0xC0117AC7));
        let unlocked: Vec<usize> = (0..6).filter(|&b| self.s.biomes[b].is_some()).collect();
        let mut out = vec![];
        for _ in 0..3 {
            let b = unlocked[rng.gen_range(0..unlocked.len())];
            let pool: Vec<usize> = biome_creatures(b).filter(|&i| CREATURES[i].r <= 2).collect();
            let ci = pool[rng.gen_range(0..pool.len())];
            let qty = match CREATURES[ci].r {
                0 => rng.gen_range(4..=8),
                1 => rng.gen_range(3..=5),
                _ => rng.gen_range(2..=3),
            };
            let reward = ((qty as f64 * self.creature_value(ci, false) * 2.5 + 50.0) * (1.0 + self.s.lab[LAB_COURTAGE] as f64 * 0.2)).floor();
            out.push(Contract { ci, qty, reward });
        }
        (w, out)
    }
    /* légende errante : fenêtre de 30 min, 30% de chance, position fixe par biome */
    fn legend_now(&self) -> Option<(u64, usize, (usize, usize))> {
        let w = (now_ms() / 1_800_000.0) as u64;
        if splitmix(w ^ 0x1E9E17D) % 100 >= 30 {
            return None;
        }
        let unlocked: Vec<usize> = (0..6).filter(|&b| self.s.biomes[b].is_some()).collect();
        let b = unlocked[(splitmix(w ^ 0xB10) % unlocked.len() as u64) as usize];
        if self.s.legends_tried.contains(&w) {
            return None;
        }
        Some((w, b, LEGEND_SPOTS[b]))
    }

    fn interact(&mut self) {
        if let Some((w, b, (lx, ly))) = self.legend_now() {
            if (lx as i32 - self.px).abs() <= 1 && (ly as i32 - self.py).abs() <= 1 {
                self.panels.push(Panel::new(PanelKind::Legend(b, w)));
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
        match self.world.zone_at(self.px as usize, self.py as usize) {
            None => ("promenez-vous · Entrée pour interagir près d'un lieu".into(), C::Dimmer),
            Some(Zone::Biome(b)) => {
                if self.s.biomes[b].is_none() {
                    (format!("{} · verrouillé — Entrée : débloquer pour {} écus", BIOMES[b].name, fmt(BIOMES[b].cost)), C::Red)
                } else {
                    let bs = self.s.biomes[b].as_ref().unwrap();
                    let placed = bs.pl.iter().flatten().count();
                    (format!("{} · {}/{} pièges posés — Entrée : gérer", BIOMES[b].name, placed, bs.slots), C::Green)
                }
            }
            Some(Zone::Boutique) => ("boutique — Entrée : acheter et vendre".into(), C::Gold),
            Some(Zone::Labo) => ("labo — Entrée : recherches et migration".into(), C::Gold),
            Some(Zone::Bestiaire) => ("bestiaire — Entrée : consulter".into(), C::Gold),
            Some(Zone::Succes) => ("trophées — Entrée : succès".into(), C::Gold),
            Some(Zone::Musee) => ("musée — Entrée : exposer vos plus belles prises".into(), C::Gold),
            Some(Zone::Enclos) => ("enclos — Entrée : faire reproduire vos créatures".into(), C::Gold),
        }
    }

    /* ---------------------------------------------- construction panneaux */

    fn build_rows(&self, kind: &PanelKind) -> (String, Vec<Row>) {
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
        for b in 0..6 {
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
                        per_rank += &format!("✦{}:{} ", RANK_NAMES[r], iv.sr(r));
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

    fn rows_contracts(&self) -> (String, Vec<Row>) {
        let (w, contracts) = self.contracts_now();
        let left_ms = ((w + 1) as f64 * 7_200_000.0) - now_ms();
        let mut rows = vec![
            Row::text("le comptoir affiche trois commandes ; livrez depuis votre réserve.", C::Dim),
            Row::text("jamais les shinies, jamais votre meilleur couple ♂♀ (« livrables » = stock hors couple).", C::Dimmer),
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
                        (format!("+{} écus", fmt(c.reward)), C::GoldDark),
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
            Row::text("", C::Dim),
        ];
        for slot in 0..self.museum_slots() {
            match &self.s.museum[slot] {
                None => rows.push(Row {
                    segs: vec![(format!("├─ salle {} : ", slot + 1), C::Dimmer), ("vide".into(), C::Dim)],
                    btns: vec![("exposer une créature".into(), C::Green, Action::Open(PanelKind::MuseumPick(slot)))],
                    act: None,
                    indent: 0,
                }),
                Some(m) => {
                    let c = &CREATURES[m.ci];
                    rows.push(Row {
                        segs: vec![
                            (format!("├─ salle {} : ", slot + 1), C::Dimmer),
                            (format!("{} {}{} [{}]", c.g, c.n, if m.shiny { " ✦" } else { "" }, RANK_NAMES[m.rank]),
                             if m.shiny { C::Shiny } else { rarity_color(c.r) }),
                            (format!("  {} écus/min", fmt2(self.creature_value_r(m.ci, m.shiny, m.rank) * 0.003)), C::GoldDark),
                        ],
                        btns: vec![("retirer".into(), C::Red, Action::MuseumRemove(slot))],
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
        for ci in 0..60 {
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
                btns.push((format!("exposer ✦ [{}]", RANK_NAMES[r]), C::Blue, Action::MuseumAdd(slot, ci, true)));
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
            Row::text("les parents (les plus bas rangs de chaque sexe) sont consommés.", C::Dimmer),
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

    fn rows_pen_pick(&self, slot: usize) -> (String, Vec<Row>) {
        let mut rows = vec![
            Row::text("il faut un couple : au moins un mâle et une femelle de l'espèce.", C::Dimmer),
            Row::text("les shinies ne se reproduisent pas (trop précieux, trop susceptibles).", C::Dimmer),
            Row::text("", C::Dim),
        ];
        let mut any = false;
        for ci in 0..60 {
            let iv = &self.s.inv2[ci];
            if iv.tn() == 0 {
                continue;
            }
            let ok = iv.tm() >= 1 && iv.tf() >= 1;
            if iv.tn() < 2 && !ok {
                continue;
            }
            any = true;
            let c = &CREATURES[ci];
            rows.push(Row {
                segs: vec![
                    (pad(&format!("{} {}", c.g, c.n), 26), rarity_color(c.r)),
                    (pad(&format!("♂{} ♀{}", iv.tm(), iv.tf()), 9), if ok { C::Green } else { C::Red }),
                    (if ok { format!("couvaison {} min", PEN_MIN[c.r] as u64) } else { format!("il manque un{}", if iv.tm() == 0 { " ♂" } else { "e ♀" }) }, C::Dimmer),
                ],
                btns: vec![("installer".into(), if ok { C::Green } else { C::Dimmer }, if ok { Action::PenStart(slot, ci) } else { Action::Nothing })],
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
        rows.push(Row::text(format!("créature épique ou légendaire — {} · rang A minimum", BIOMES[b].name), C::Gold));
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
                (if is_night_at(now) { "nuit ☽ (espèces nocturnes de sortie)".into() } else { "jour".to_string() }, C::Text),
            ],
            btns: vec![],
            act: None,
            indent: 0,
        });
        rows.push(Row::text(format!("météo : {} — change dans {} min", weather_desc(w), next_w.ceil() as u64), C::Dim));
        rows.push(Row::text(format!("saison : {}", season_desc(sea)), C::Dim));
        if let Some((wid, b, _)) = self.legend_now() {
            let left = (((wid + 1) as f64 * 1_800_000.0 - now) / 60_000.0).ceil() as u64;
            rows.push(Row::text(
                format!("✧ une légende errante rôde en {} — encore {} min pour la trouver !", BIOMES[b].name, left),
                C::Gold,
            ));
        }
        rows.push(Row::text("", C::Dim));

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
        for b in 0..6 {
            if let Some(bs) = &self.s.biomes[b] {
                for pl in bs.pl.iter().flatten() {
                    let bait_ok = pl.bait.filter(|&bt| self.s.baits[bt] > 0);
                    cpm += (TRAPS[pl.trap].succ + weather_succ_mod(w, b)).clamp(0.05, 0.99) * 60000.0 / self.trap_interval(pl.trap, bait_ok, b, now);
                }
            }
        }
        rows.push(Row::text(format!("rendement estimé : {} captures/min", fmt2(cpm)), C::Green));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("biomes — Entrée sur une ligne pour y aller"));
        for b in 0..6 {
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
                        (pad(&format!("{}/{} pièges", placed, bs.slots), 12), if placed == 0 { C::Red } else { C::Dim }),
                        (pad(&format!("bestiaire {}/10{}", found, if found == 10 { " ✓" } else { "" }), 19), C::Blue),
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
        let hunts: Vec<&str> = (0..6)
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
        for ci in 0..60 {
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
                if shiny_inv > 0 { format!(" dont {} ✦", shiny_inv) } else { String::new() },
                fmt(dupes as f64), fmt(dval)),
            if dval > 0.0 { C::Gold } else { C::Dim },
        ));
        let found_total = (0..60).filter(|&i| self.s.dex2[i].n > 0).count();
        let shiny_total = (0..60).filter(|&i| self.s.dex2[i].s > 0).count();
        let s_total = (0..60).filter(|&i| self.s.dex2[i].best >= 4).count();
        rows.push(Row::text(
            format!("bestiaire : {}/60 espèces · {}/60 shinies · {}/60 en rang S · {} biome(s) complet(s)",
                found_total, shiny_total, s_total, self.completed_biomes()),
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
            format!("bestiaire {}/10{} · valeur des prises ×{}", found, if found == 10 { " ✓" } else { "" }, bio.mult),
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
        // battue : déclenche immédiatement chaque piège posé avec +0,5 chance
        rows.push(Row::text("", C::Dim));
        let placed = bs.pl.iter().flatten().count();
        let ready = bs.hunt_at <= now;
        let left = ((bs.hunt_at - now).max(0.0) / 1000.0).ceil() as u64;
        rows.push(Row {
            segs: vec![(
                if ready { "battre les fourrés vous-même (chance +0,5) :".into() } else { format!("prochaine battue possible dans {} s", left) },
                if ready { C::Text } else { C::Dimmer },
            )],
            btns: vec![(
                "battue !".into(),
                if ready && placed > 0 { C::Gold } else { C::Dimmer },
                if ready && placed > 0 { Action::Hunt(b) } else { Action::Nothing },
            )],
            act: None,
            indent: 0,
        });
        (bio.name.to_string(), rows)
    }    fn rows_trap_pick(&self, b: usize, i: usize) -> (String, Vec<Row>) {
        let mut rows = vec![];
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
                btns: vec![("poser".into(), C::Green, Action::Place(b, i, t))],
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
        rows.push(Row::text(format!("valeur des prises ×{} · 10 espèces à découvrir", bio.mult), C::Dimmer));
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
        let mut rows = vec![Row::header(&format!("commandes du comptoir — renouvelées dans {} min", (left_ms / 60_000.0).ceil() as u64))];
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
                        (format!("+{} écus", fmt(c.reward)), C::GoldDark),
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
                btns: vec![(format!("{} écus", fmt(TRAPS[t].cost)), if ok { C::Gold } else { C::Dimmer }, Action::BuyTrap(t))],
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
        let any = (0..60).any(|ci| self.s.inv2[ci].tn() + self.s.inv2[ci].ts() > 0);
        if !any {
            rows.push(Row::text("réserve vide. les pièges y remédieront.", C::Dim));
        } else {
            rows.push(Row {
                segs: vec![],
                btns: vec![("vendre tous les doublons".into(), C::Green, Action::SellDupes)],
                act: None,
                indent: 0,
            });
            for r in wrap_rows(
                "garde le meilleur couple ♂♀ de chaque espèce, met de côté ce qu'exigent les commandes du comptoir, jamais les shinies. la vente écoule d'abord les rangs les plus bas.",
                self.panel_w, C::Dimmer,
            ) {
                rows.push(r);
            }
            for b in 0..6 {
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
                                (pad(&format!("[{}-{}]", RANK_NAMES[low], RANK_NAMES[best]), 6), if best >= 3 { C::Gold } else { C::Dim }),
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
                                (pad(&format!("{} {} ✦", c.g, c.n), 24), C::Shiny),
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
            rows.push(Row::header("auto-vente — garde le couple ♂♀ et le stock des commandes en cours"));
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
                LAB_FLAIR => format!("chance +{}", fmt2(lv as f64 * 0.06)),
                LAB_NEGOCE => format!("prix de vente +{}%", lv * 8),
                LAB_HORLOGE => format!("hors-ligne : {} h max", 2 + lv * 2),
                LAB_ECLAT => format!("chance de shiny +{}%", lv * 15),
                LAB_AUTOVENTE => if lv > 0 { "filtres débloqués".into() } else { "non débloquée".into() },
                LAB_CONSERVATION => format!("cagnotte du musée : {} h max", 4 + lv * 2),
                LAB_AILES => format!("{} salles au musée", 6 + lv),
                LAB_ENCLOS => format!("{} enclos", 3 + lv),
                LAB_LIGNEES => format!("montée de rang : {}%", 35 + lv * 5),
                LAB_TRAQUEUR => format!("battue toutes les {} s", 300 - lv * 30),
                _ => format!("primes de contrats +{}%", lv * 20),
            };
            rows.push(Row {
                segs: vec![
                    (pad(LABS[k].n, 17), C::Text),
                    (pad(&format!("niv {}/{}", lv, LABS[k].max), 11), C::Dimmer),
                    (pad(&fx, 26), C::Green),
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
        for r in wrap_rows("repartir de zéro vers des terres plus giboyeuses. le bestiaire et les succès sont conservés ; écus, pièges, labo et réserve sont perdus. chaque trophée offre définitivement +4% de chance et +4% aux prix de vente.", self.panel_w, C::Dim) {
            rows.push(r);
        }
        let g = self.trophy_gain();
        rows.push(Row::text(
            format!("écus gagnés cette expédition : {} · trophées actuels : {}", fmt(self.s.run_earned), self.s.trophies),
            C::Dimmer,
        ));
        rows.push(Row {
            segs: vec![
                ("trophées à la migration : ".into(), C::Text),
                (format!("+{}", g), if g > 0 { C::Gold } else { C::Dimmer }),
                (if g < 1 { "  (25 000 écus gagnés = 1er trophée)".into() } else { String::new() }, C::Dimmer),
            ],
            btns: vec![("migrer".into(), if g > 0 { C::Red } else { C::Dimmer }, if g > 0 { Action::Open(PanelKind::MigrConfirm) } else { Action::Nothing })],
            act: None,
            indent: 0,
        });
        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("bonus actifs"));
        rows.push(Row::text(
            format!("chance globale +{} · vente ×{} · vitesse ×{}", fmt2(self.global_luck()), fmt2(self.sell_mult()), fmt2(self.speed_mult())),
            C::Dim,
        ));
        rows.push(Row::text(
            format!("shiny 1/{} · hors-ligne {} h max", (1.0 / self.shiny_chance(None, now_ms())).round() as u64, (self.offline_cap_ms() / 3600000.0) as u64),
            C::Dim,
        ));
        (format!("labo — {} écus", fmt(self.s.ecus)), rows)
    }

    fn rows_migr_confirm(&self) -> (String, Vec<Row>) {
        let g = self.trophy_gain();
        let mut rows = wrap_rows(
            &format!("vos écus, pièges, appâts, améliorations et créatures en réserve disparaissent. le bestiaire et les succès restent. vous gagnez {} trophée{} permanents.", g, if g > 1 { "s" } else { "" }),
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
        let total = (0..60).filter(|&i| self.s.dex2[i].n > 0).count();
        let shiny_total = (0..60).filter(|&i| self.s.dex2[i].s > 0).count();
        let s_total = (0..60).filter(|&i| self.s.dex2[i].best >= 4).count();
        let mut rows = vec![
            Row::text(
                format!("espèces {}/60 {}  shinies {}/60 {}  rang S {}/60", total, ascii_bar(total as f64 / 60.0, 14), shiny_total, ascii_bar(shiny_total as f64 / 60.0, 14), s_total),
                C::Text,
            ),
            Row::text("biome complet : +10% chance · biome 100% shiny : +10% vente — pour toujours", C::Dimmer),
        ];
        for b in 0..6 {
            let found = biome_creatures(b).filter(|&i| self.s.dex2[i].n > 0).count();
            rows.push(Row::text("", C::Dim));
            rows.push(Row::header(&format!("{} — {}/10{}", BIOMES[b].name, found, if found == 10 { " ✓" } else { "" })));
            for ci in biome_creatures(b) {
                let c = &CREATURES[ci];
                let d = &self.s.dex2[ci];
                if d.n == 0 {
                    rows.push(Row {
                        segs: vec![
                            ("├─ ".into(), C::Dimmer),
                            (pad("???", 7), C::Dimmer),
                            (pad("— inconnu —", 23), C::Dimmer),
                            (pad(RAR_LABEL[c.r], 13), C::Dimmer),
                            (if NOCTURNES.contains(&ci) { "☽ nocturne".into() } else { String::new() }, C::Abyss),
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
                            (pad(c.g, 7), if d.s > 0 { C::Shiny } else { rarity_color(c.r) }),
                            (pad(c.n, 23), rarity_color(c.r)),
                            (pad(&format!("{} · pris ×{}{}", RAR_LABEL[c.r], fmt(d.n as f64), if d.s > 0 { format!(" ✦{}", d.s) } else { String::new() }), 26), C::Dim),
                            (pad(&format!("[{}]", best), 5), if d.best >= 4 { C::Gold } else { C::Dim }),
                            (pad(&format!("{}{}", if d.mf & 1 != 0 { "♂" } else { "·" }, if d.mf & 2 != 0 { "♀" } else { "·" }), 4),
                             if d.mf == 3 { C::Green } else { C::Dim }),
                            (format!("stock {}{}", iv.tn(), if iv.ts() > 0 { format!("+{}✦", iv.ts()) } else { String::new() }),
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
                (if NOCTURNES.contains(&ci) { "  ☽ nocturne (21 h – 7 h)".into() } else { String::new() }, C::Abyss),
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
            format!("capturés (total) : {}{}", fmt(d.n as f64), if d.s > 0 { format!(" · shinies : {} ✦", fmt(d.s as f64)) } else { String::new() }),
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
                per_rank += &format!("✦{}:{} ", RANK_NAMES[r], iv.sr(r));
            }
        }
        rows.push(Row::text(
            format!("en réserve : {}{}{}", iv.tn(), if iv.ts() > 0 { format!(" + {} ✦", iv.ts()) } else { String::new() },
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
        let done = (0..18).filter(|&i| self.s.ach[i]).count();
        let mut rows = vec![Row::text(format!("{}/{} débloqués", done, ACHS.len()), C::Dim), Row::text("", C::Dim)];
        for i in 0..18 {
            let ok = self.s.ach[i];
            rows.push(Row {
                segs: vec![
                    (format!("{} {}", if ok { "■" } else { "□" }, pad(ACHS[i].n, 26)), if ok { C::Green } else { C::Dimmer }),
                    (format!("{}{}", ACHS[i].d, if ACHS[i].r > 0.0 { format!(" (+{} écus)", fmt(ACHS[i].r)) } else { String::new() }), C::Dimmer),
                ],
                btns: vec![],
                act: None,
                indent: 0,
            });
        }
        ("succès".into(), rows)
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
            "l'objectif de fond : compléter le bestiaire — 60 espèces, leurs shinies ✦, et un rang S partout.",
            "compléter un biome donne +10% de chance pour toujours ; le compléter en shiny, +10% à la vente.",
            "les pièges ne s'usent jamais : posés une fois, ils travaillent indéfiniment. l'horlogerie (labo) ne limite que la progression simulée hors-ligne (2 h de base).",
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
        rows.extend(bullet_rows("· ", "la vente « sauf couple » et l'auto-vente protègent le meilleur ♂ et la meilleure ♀ ; l'auto-vente et la vente groupée réservent aussi le stock des commandes en cours.", w, C::Dim));
        rows.extend(bullet_rows("· ", "le bestiaire retient le meilleur rang obtenu par espèce, à vie.", w, C::Dim));
        rows.extend(bullet_rows("· ", "la vente écoule toujours les rangs les plus bas d'abord : vos beaux spécimens restent.", w, C::Dim));
        rows.push(Row {
            segs: vec![
                ("· ".into(), C::Dimmer),
                ("shiny ✦".into(), C::Shiny),
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
            "la nuit (21 h – 7 h), six espèces nocturnes ☽ sortent, introuvables le jour : {}.",
            noct_names.join(", ")), w, C::Dim));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("sur le terrain"));
        for t in [
            "battue : dans un biome, déclenchez vous-même tous vos pièges avec +0,5 chance (repos 5 min).",
            "appâts : consommés à chaque tentative du piège équipé ; effets décrits à la boutique.",
            "légende errante : une silhouette ✧ apparaît parfois sur la carte. approchez-la et tentez votre chance — une seule fois. créature épique ou légendaire, rang A minimum.",
            "contrats [c] : trois commandes toutes les 2 h, payées bien au-dessus du marché. la livraison ne prend jamais les shinies ni votre meilleur couple ♂♀.",
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
            "musée ╡ m ╞ : exposez vos plus beaux spécimens ; chacun génère des écus en continu (cagnotte plafonnée à 4 h de base, extensible au labo). le spécimen exposé quitte la réserve, récupérable à tout moment.",
        ] {
            rows.extend(bullet_rows("· ", t, w, C::Dim));
        }
        rows.extend(bullet_rows("· ", &format!(
            "enclos ╡ e ╞ : un couple ♂+♀ d'une même espèce donne une naissance ({}). 35% de chance de monter d'un rang, shiny ×3. les parents (plus bas rangs de chaque sexe) sont consommés.",
            (0..5).map(|r| format!("{} {} min", RAR_LABEL[r], PEN_MIN[r] as u64)).collect::<Vec<_>>().join(" · ")), w, C::Dim));

        rows.push(Row::text("", C::Dim));
        rows.push(Row::header("la migration"));
        for t in [
            "au labo, quand une expédition a bien rapporté : repartez de zéro contre des trophées permanents (+4% chance et +4% vente chacun).",
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
        rows.push(Row::text(format!("automatique dans {}", save_path().display()), C::Dimmer));
        rows.push(Row::text("copiez ce fichier pour changer de machine.", C::Dimmer));
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
            rows.push(Row::text(format!("├─ shinies : {} ✦", sum.shinies), C::Blue));
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

fn fmt(n: f64) -> String {
    let n = n.floor();
    if n >= 1e9 {
        return format!("{:.2} Md", n / 1e9).replace('.', ",");
    }
    if n >= 1e6 {
        return format!("{:.2} M", n / 1e6).replace('.', ",");
    }
    if n >= 10000.0 {
        return format!("{:.1} k", n / 1e3).replace(",0", "").replace('.', ",");
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
    // étiquettes de biomes
    for b in 0..6 {
        let (lx, ly) = LABEL_POS[b];
        let sx = 1 + lx as i32 - cam_x + off_x;
        let sy = vy0 + ly as i32 - cam_y + off_y;
        if sy < vy0 || sy > vy1 {
            continue;
        }
        let owned = game.s.biomes[b].is_some();
        let lbl = if owned { format!("╡ {} ╞", BIOMES[b].name) } else { format!("╡ {} × ╞", BIOMES[b].name) };
        draw_str(buf, area, sx, sy, &lbl, theme.style(if owned { C::White } else { C::Red }, false));
        if owned {
            let placed = game.s.biomes[b].as_ref().unwrap().pl.iter().flatten().count();
            if placed > 0 && sy + 1 <= vy1 {
                draw_str(buf, area, sx + 1, sy + 1, &format!("{} piège{}", placed, if placed > 1 { "s" } else { "" }), theme.style(C::GoldDark, false));
            }
        }
    }
    // légende errante
    if let Some((_, _, (lx, ly))) = game.legend_now() {
        let sx = 1 + lx as i32 - cam_x + off_x;
        let sy = vy0 + ly as i32 - cam_y + off_y;
        if sy >= vy0 && sy <= vy1 {
            draw_str(buf, area, sx, sy, "✧", theme.style(C::Shiny, false));
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
    let cond = format!(" {} · {} · {} ", SAISONS[season_at(nowc)], METEOS[weather_at(nowc)], if is_night_at(nowc) { "nuit ☽" } else { "jour" });
    draw_str(buf, area, cols - cond.chars().count() as i32 - 3, sep_y, &cond, theme.style(C::Ice, false));

    // ---- journal ----
    for i in 0..3usize {
        if let Some(l) = game.logs.get(i) {
            let y = rows_n - 4 + i as i32;
            let mut x = 2;
            draw_str(buf, area, x, y, &format!("[{}] ", l.t), theme.style(C::Dimmer, false));
            x += 11;
            for (txt, c) in &l.segs {
                if x >= cols - 2 {
                    break;
                }
                let avail = (cols - 2 - x) as usize;
                let t: String = txt.chars().take(avail).collect();
                draw_str(buf, area, x, y, &t, theme.style(*c, false));
                x += t.chars().count() as i32;
            }
        }
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
    let dex = (0..60).filter(|&i| game.s.dex2[i].n > 0).count();
    let stats: Vec<(String, C)> = vec![
        (format!(" {} écus ", fmt(game.s.ecus)), C::Gold),
        (format!("· {} captures ", fmt(game.s.captures as f64)), C::Dim),
        (format!("· bestiaire {}% ", (dex as f64 / 60.0 * 100.0).round() as u64), C::Blue),
        (if game.s.trophies > 0 { format!("· {} trophées ", game.s.trophies) } else { String::new() }, C::GoldDark),
    ];
    for (txt, c) in stats {
        draw_str(buf, area, tx, 0, &txt, theme.style(c, false));
        tx += txt.chars().count() as i32;
    }
    // raccourcis
    let kb = " zqsd/←↑↓→ · Entrée · [v]ue [i]nventaire [b]estiaire [o] boutique [c]ontrats [l]abo [m]usée [e]nclos [t]rophées [j]ournal [?] aide ";
    let kbt: String = kb.chars().take((cols - 4) as usize).collect();
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
    if rows.len() > inner {
        let info = format!(" {}-{}/{} ", scroll + 1, (scroll + inner).min(rows.len()), rows.len());
        draw_str(buf, area, px + pw - info.chars().count() as i32 - 2, py + ph - 1, &info, theme.style(C::Dimmer, true));
    }

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

fn panel_key(game: &mut Game, code: KeyCode) {
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
        KeyCode::Esc | KeyCode::Char('q') => {
            game.panels.pop();
        }
        KeyCode::PageDown => {
            let p = game.panels.last_mut().unwrap();
            p.scroll = (p.scroll + inner).min(max_scroll);
        }
        KeyCode::PageUp => {
            let p = game.panels.last_mut().unwrap();
            p.scroll = p.scroll.saturating_sub(inner);
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => {
            if sels.is_empty() {
                let p = game.panels.last_mut().unwrap();
                p.scroll = (p.scroll + 1).min(max_scroll);
            } else if let Some(&(r0, _)) = sels.get(sel) {
                if let Some(n) = (sel + 1..sels.len()).find(|&i| sels[i].0 > r0) {
                    let p = game.panels.last_mut().unwrap();
                    p.sel = n;
                    snap(p, sels[n].0);
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('z') => {
            if sels.is_empty() {
                let p = game.panels.last_mut().unwrap();
                p.scroll = p.scroll.saturating_sub(1);
            } else if let Some(&(r0, _)) = sels.get(sel) {
                if let Some(n) = (0..sel).rev().find(|&i| sels[i].0 < r0) {
                    let rt = sels[n].0;
                    let first = (0..=n).rev().take_while(|&i| sels[i].0 == rt).last().unwrap();
                    let p = game.panels.last_mut().unwrap();
                    p.sel = first;
                    snap(p, rt);
                }
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('d') => {
            if sel + 1 < sels.len() && sels[sel + 1].0 == sels[sel].0 {
                game.panels.last_mut().unwrap().sel = sel + 1;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if sel > 0 && sels[sel - 1].0 == sels[sel].0 {
                game.panels.last_mut().unwrap().sel = sel - 1;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
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

fn world_key(game: &mut Game, code: KeyCode) {
    let (mut dx, mut dy) = (0i32, 0i32);
    match code {
        KeyCode::Up | KeyCode::Char('z') | KeyCode::Char('w') => dy = -1,
        KeyCode::Down | KeyCode::Char('s') => dy = 1,
        KeyCode::Left | KeyCode::Char('q') | KeyCode::Char('a') => dx = -1,
        KeyCode::Right | KeyCode::Char('d') => dx = 1,
        KeyCode::Enter | KeyCode::Char(' ') => {
            game.interact();
            return;
        }
        KeyCode::Char('v') => {
            game.panels.push(Panel::new(PanelKind::Dashboard));
            return;
        }
        KeyCode::Char('i') => {
            game.panels.push(Panel::new(PanelKind::Inventory));
            return;
        }
        KeyCode::Char('c') => {
            game.panels.push(Panel::new(PanelKind::Contracts));
            return;
        }
        KeyCode::Char('m') => {
            game.panels.push(Panel::new(PanelKind::Museum));
            return;
        }
        KeyCode::Char('e') => {
            game.panels.push(Panel::new(PanelKind::Pens));
            return;
        }
        KeyCode::Char('b') => {
            game.panels.push(Panel::new(PanelKind::Dex));
            return;
        }
        KeyCode::Char('o') => {
            game.panels.push(Panel::new(PanelKind::Shop));
            return;
        }
        KeyCode::Char('l') => {
            game.panels.push(Panel::new(PanelKind::Lab));
            return;
        }
        KeyCode::Char('t') => {
            game.panels.push(Panel::new(PanelKind::Achs));
            return;
        }
        KeyCode::Char('j') => {
            game.panels.push(Panel::new(PanelKind::Journal));
            return;
        }
        KeyCode::Char('?') | KeyCode::Char('/') => {
            game.panels.push(Panel::new(PanelKind::Help));
            return;
        }
        KeyCode::Esc => {
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

fn main() -> std::io::Result<()> {
    let (mut game, fresh) = Game::new();
    game.run_offline();
    game.log(vec![(
        "bienvenue. un piège en bois vous attend en réserve — la forêt est à l'ouest.".into(),
        C::Green,
    )]);
    if fresh {
        // première partie : ouvrir le guide directement
        game.panels.push(Panel::new(PanelKind::Help));
    }

    let theme = Theme::detect();
    let mut terminal = ratatui::init();
    let mut last_tick = Instant::now();
    let mut last_save = Instant::now();

    while !game.quit {
        if last_tick.elapsed() >= Duration::from_millis(500) {
            game.tick();
            last_tick = Instant::now();
        }
        if last_save.elapsed() >= Duration::from_secs(10) {
            game.save();
            last_save = Instant::now();
        }
        game.toasts.retain(|(_, t)| t.elapsed() < Duration::from_millis(3800));

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
                    } else if game.panels.is_empty() {
                        world_key(&mut game, k.code);
                    } else {
                        panel_key(&mut game, k.code);
                    }
                }
                _ => {}
            }
        }
    }

    game.save();
    ratatui::restore();
    println!("partie sauvegardée dans {}. à bientôt.", save_path().display());
    Ok(())
}
