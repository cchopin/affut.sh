//! affut-sync — synchronisation des sauvegardes navigateur + classement.
//!
//! sauvegardes : GET/PUT /s/<jeton> — un fichier par jeton, le jeton EST le secret.
//! classement  : PUT /lb/<jeton> (pseudo + stats), GET /lb (tableau public).

/* ── plausibilité des statistiques du classement ───────────────────────────
   le client calcule son score et l'envoie : on ne le croit pas. un joueur a
   modifié sa sauvegarde pour s'attribuer 1 250 milliards d'écus et un score de
   696 969 693 123, là où la meilleure partie honnête en affichait 9 468.

   trois garde-fous, tous côté serveur puisque le client est précisément ce
   qu'on ne peut pas croire :

   1. le score est TOUJOURS recalculé ici, jamais repris du client ;
   2. des bornes de structure, vraies dans n'importe quelle partie : 114 espèces
      au plus, un rang par espèce découverte, un shiny toutes les 80 prises au plus, pas plus de
      captures, une migration par million d'écus gagnés (elle exige un trophée,
      donc au moins un million de gains sur la partie) et des trophées bornés
      par la formule du jeu, √(gains / 1 M) par migration ;
   3. un plafond de cadence : ni la partie entière ni l'intervalle depuis le
      dernier envoi ne peuvent avoir produit plus que le temps ne le permet.

   on BORNE au lieu de refuser : refuser laisserait en place l'entrée déjà
   écrite, et pourrait bloquer un joueur honnête sur une borne mal réglée. une
   entrée bornée est marquée « suspect » et disparaît du classement public ;
   la marque est recalculée à chaque envoi, si bien qu'une partie redevenue
   plausible revient d'elle-même. */

/* le jeu compte 114 espèces piégeables et quatre rangs (C, B, A, S) */
const ESPECES_MAX: f64 = 114.0;
const RANG_MAX: f64 = 4.0;
/* huit pièges au maximum, le plus rapide toutes les 10 s : 0,8 capture/s.
   on retient 5, six fois la marge, pour ne jamais gêner une partie honnête. */
const CAPTURES_PAR_S: f64 = 5.0;
/* le revenu maximal du jeu, tous pièges et tout le labo au bout, avoisine
   180 écus/s. on retient 1 000, cinq fois la marge. */
const ECUS_PAR_S: f64 = 1_000.0;
/* on ne s'inscrit au classement qu'en choisissant un pseudo, parfois après
   plusieurs jours de jeu : on crédite d'avance deux jours de partie pour ne
   pas borner ce premier envoi. */
const AVANCE_MS: f64 = 2.0 * 86_400_000.0;
/* migrer coûte 100 000 écus, prix doublé à chaque départ et plafonné au
   douzième. on ne peut donc pas avoir migré plus souvent que ses gains totaux
   ne l'ont permis — et sans migration, aucun trophée. */
const VOYAGE_BASE: f64 = 100_000.0;
const VOYAGE_PALIER_MAX: i32 = 12;
/* le jeu accorde √(gains de la partie / 1 M) trophées par migration */
const ECUS_PAR_TROPHEE: f64 = 1_000_000.0;

/* combien de migrations les gains totaux peuvent avoir payées */
fn migrations_max(ecus: f64) -> f64 {
    let (mut cumul, mut m) = (0.0, 0.0);
    while m < 100.0 {
        let cout = VOYAGE_BASE * 2f64.powi((m as i32).min(VOYAGE_PALIER_MAX));
        if cumul + cout > ecus {
            break;
        }
        cumul += cout;
        m += 1.0;
    }
    m
}
/* être au bout en peu de temps, c'est louche. la découverte des espèces sature :
   les communes tombent vite, les dernières demandent d'avoir ouvert les biomes
   du bout. simuler une partie entière donne plus de 2 000 h pour atteindre les
   ruines ; on retient une courbe bien plus indulgente, 114 × (1 − e^(−h/10)),
   qui laisse les parties observées très au large — 29 espèces en 6 h contre un
   plafond de 79 — et rend le bestiaire complet impossible avant deux jours.
   l'avance est de six heures ici et non deux jours : c'est elle qui fait mordre
   la borne sur une partie inscrite du jour et déjà complète. un joueur qui
   s'inscrirait après plusieurs jours de jeu serait borné une fois, puis rendu
   au classement de lui-même dès que son ancienneté rattrape sa collection. */
const ESPECES_TEMPS_H: f64 = 10.0;
const AVANCE_PROGRES_MS: f64 = 6.0 * 3_600_000.0;

fn lire_nb(e: &serde_json::Map<String, serde_json::Value>, k: &str) -> f64 {
    e.get(k)
        .and_then(|x| x.as_f64())
        .filter(|n| n.is_finite())
        .unwrap_or(0.0)
        .max(0.0)
        .floor()
}

/* vérifie une entrée DÉJÀ écrite, sans rien modifier sur le disque. sert de
   filet à la lecture du classement, pour deux raisons : les entrées écrites
   avant ce contrôle n'ont jamais été vérifiées, et une entrée forgée ne serait
   autrement corrigée qu'au prochain envoi de son auteur — qui peut très bien
   ne jamais revenir.

   on juge l'entrée à sa propre date d'écriture, avec son premier envoi pour
   origine : c'est la lecture la plus stricte, et la seule qui ne devienne pas
   plus indulgente à mesure que le temps passe.

   renvoie l'entrée avec son score recalculé, ou None si elle ne tient pas
   debout. un score gonflé seul est donc corrigé, pas puni : c'est seulement
   quand les statistiques elles-mêmes sont impossibles qu'on écarte la ligne. */
fn entree_verifiee(v: &serde_json::Value) -> Option<serde_json::Value> {
    let m = v.as_object()?;
    let at = m.get("at").and_then(|x| x.as_f64()).filter(|n| n.is_finite()).unwrap_or(0.0);
    let premier_at = m.get("premier_at").and_then(|x| x.as_f64()).filter(|n| n.is_finite()).unwrap_or(at);
    let mut copie = m.clone();
    if borner_stats(&mut copie, premier_at, None, at) {
        return None;
    }
    Some(serde_json::Value::Object(copie))
}

/* borne les statistiques d'une entrée et recalcule son score.
   renvoie true si quelque chose a été borné — l'entrée est alors suspecte. */
fn borner_stats(
    e: &mut serde_json::Map<String, serde_json::Value>,
    premier_at: f64,
    prec: Option<&serde_json::Value>,
    now: f64,
) -> bool {
    let brut: Vec<f64> = ["captures", "especes", "rangs", "shinies", "ecus", "migrations", "trophees"]
        .iter()
        .map(|k| lire_nb(e, k))
        .collect();
    let (b_capt, b_esp, b_rang, b_shi, b_ecus, b_migr, b_tro) =
        (brut[0], brut[1], brut[2], brut[3], brut[4], brut[5], brut[6]);

    /* âge de la partie, avance comprise */
    let age_s = (((now - premier_at).max(0.0) + AVANCE_MS) / 1000.0).max(1.0);

    /* et, si l'on connaît l'envoi précédent, ce qu'il a pu produire depuis */
    let (capt_max, ecus_max) = match prec {
        Some(p) => {
            let pm = p.as_object();
            let lu = |k: &str| pm.and_then(|m| m.get(k)).and_then(|x| x.as_f64()).filter(|n| n.is_finite()).unwrap_or(0.0);
            let depuis_s = ((now - lu("at")).max(0.0) / 1000.0).max(1.0);
            (
                (lu("captures") + depuis_s * CAPTURES_PAR_S).min(age_s * CAPTURES_PAR_S),
                (lu("ecus") + depuis_s * ECUS_PAR_S).min(age_s * ECUS_PAR_S),
            )
        }
        None => (age_s * CAPTURES_PAR_S, age_s * ECUS_PAR_S),
    };

    let captures = b_capt.min(capt_max);
    let ecus = b_ecus.min(ecus_max);
    /* on ne découvre pas une espèce sans l'avoir capturée, ni tout un
       bestiaire en une soirée */
    let progres_h = ((now - premier_at).max(0.0) + AVANCE_PROGRES_MS) / 3_600_000.0;
    /* au supérieur : arrondi au plancher, la courbe n'atteindrait jamais 114
       et un bestiaire réellement complet resterait suspect à vie */
    let especes_temps = (ESPECES_MAX * (1.0 - (-progres_h / ESPECES_TEMPS_H).exp())).ceil();
    let especes = b_esp.min(ESPECES_MAX).min(captures).min(especes_temps);
    let rangs = b_rang.min(especes * RANG_MAX);
    /* le jeu plafonne la chance de shiny à 1/128 par capture (1/512 de base).
       on tolère largement la variance et les gros bonus — au-delà d'un shiny
       toutes les 80 prises, ce n'est plus de la chance. la constante additive
       laisse tranquilles les débutants chanceux. */
    let shinies = b_shi.min(captures).min(4.0 + captures / 80.0);
    let migrations = b_migr.min(migrations_max(ecus));
    /* à gains totaux donnés, la somme des √ est maximale quand les gains sont
       répartis également entre les migrations, soit √(migrations × gains / 1 M) */
    let trophees = b_tro.min((migrations * ecus / ECUS_PAR_TROPHEE).sqrt().floor());

    let borne = [
        (captures, b_capt), (especes, b_esp), (rangs, b_rang), (shinies, b_shi),
        (ecus, b_ecus), (migrations, b_migr), (trophees, b_tro),
    ];
    let suspect = borne.iter().any(|(apres, avant)| apres < avant);

    for (k, v) in [
        ("captures", captures), ("especes", especes), ("rangs", rangs),
        ("shinies", shinies), ("ecus", ecus), ("migrations", migrations),
        ("trophees", trophees),
    ] {
        e.insert(k.into(), serde_json::json!(v));
    }
    /* le score n'est jamais celui du client */
    let score = (especes * 100.0 + shinies * 300.0 + rangs * 40.0 + trophees * 1000.0 + ecus / 1000.0).floor();
    e.insert("score".into(), serde_json::json!(score));
    suspect
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::io::Read;
    let data_dir = std::env::var("AFFUT_SYNC_DIR").unwrap_or_else(|_| "/data".into());
    let lb_dir = format!("{}/lb", data_dir);
    std::fs::create_dir_all(&data_dir).ok();
    std::fs::create_dir_all(&lb_dir).ok();
    let server = tiny_http::Server::http("0.0.0.0:2323").expect("bind 2323");
    eprintln!("affut-sync sur :2323, stockage {}", data_dir);

    fn valid_token(t: &str) -> bool {
        t.len() >= 16 && t.len() <= 64 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }
    fn token_after(url: &str, prefix: &str) -> Option<String> {
        let t = url.strip_prefix(prefix)?;
        let t = t.split('?').next().unwrap_or(t);
        if valid_token(t) { Some(t.to_string()) } else { None }
    }
    /* pseudo : alphabet latin uniquement (chiffres, accents, - _ espace).
       le cyrillique et le grec sont refusés : leurs homoglyphes (е, о, а…)
       permettraient d'afficher un pseudo visuellement identique à un autre. */
    fn latin_ok(c: char) -> bool {
        c.is_ascii_alphanumeric()
            || ('\u{00C0}'..='\u{024F}').contains(&c) && c != '\u{00D7}' && c != '\u{00F7}'
            || c == '-'
            || c == '_'
            || c == ' '
    }
    fn clean_pseudo(raw: &str) -> Option<String> {
        /* espaces internes compressés : « te  ly » ne peut pas doubler « te ly » */
        let p: String = raw.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(16).collect();
        let ok = p.chars().count() >= 2 && p.chars().all(latin_ok) && p.chars().any(|c| c.is_alphanumeric());
        if ok { Some(p) } else { None }
    }
    /* clé de comparaison : deux pseudos qui se lisent pareil se valent.
       casse ignorée, accents dépouillés, séparateurs retirés. */
    fn norm_key(p: &str) -> String {
        p.to_lowercase()
            .chars()
            .filter_map(|c| match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => Some("a".to_string()),
                'ç' => Some("c".to_string()),
                'è' | 'é' | 'ê' | 'ë' => Some("e".to_string()),
                'ì' | 'í' | 'î' | 'ï' => Some("i".to_string()),
                'ñ' => Some("n".to_string()),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => Some("o".to_string()),
                'ù' | 'ú' | 'û' | 'ü' => Some("u".to_string()),
                'ý' | 'ÿ' => Some("y".to_string()),
                'æ' => Some("ae".to_string()),
                'œ' => Some("oe".to_string()),
                'ß' => Some("ss".to_string()),
                ' ' | '-' | '_' => None,
                c => Some(c.to_string()),
            })
            .collect()
    }
    fn now_ms() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0)
    }
    /* entrées du classement : (jeton, valeur json) */
    fn read_board(lb_dir: &str) -> Vec<(String, serde_json::Value)> {
        let Ok(dir) = std::fs::read_dir(lb_dir) else { return Vec::new() };
        dir.filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().map(|x| x != "json").unwrap_or(true) {
                return None;
            }
            let tok = p.file_stem()?.to_str()?.to_string();
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
            Some((tok, v))
        })
        .collect()
    }

    for mut req in server.incoming_requests() {
        let cors = [
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"https://cchopin.github.io"[..]).unwrap(),
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, PUT, OPTIONS"[..]).unwrap(),
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"content-type"[..]).unwrap(),
        ];
        let json_h = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap();
        let respond = |req: tiny_http::Request, code: u32, body: &str| {
            let mut r = tiny_http::Response::from_string(body).with_status_code(code);
            for h in cors.iter() {
                r.add_header(h.clone());
            }
            let _ = req.respond(r);
        };
        let respond_json = |req: tiny_http::Request, code: u32, body: &str| {
            let mut r = tiny_http::Response::from_string(body).with_status_code(code);
            for h in cors.iter() {
                r.add_header(h.clone());
            }
            r.add_header(json_h.clone());
            let _ = req.respond(r);
        };
        let method = req.method().clone();
        let url = req.url().to_string();
        if method == tiny_http::Method::Options {
            respond(req, 204, "");
            continue;
        }

        /* ── classement public ─────────────────────────────────────────── */
        if url == "/lb" || url.starts_with("/lb?") {
            if method != tiny_http::Method::Get {
                respond(req, 405, "");
                continue;
            }
            let rows: Vec<serde_json::Value> = read_board(&lb_dir)
                .into_iter()
                .map(|(_, v)| v)
                .filter(|v| !v.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false))
                .filter(|v| !v.get("suspect").and_then(|h| h.as_bool()).unwrap_or(false))
                .filter_map(|v| entree_verifiee(&v))
                .collect();
            let body = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into());
            respond_json(req, 200, &body);
            continue;
        }
        if let Some(token) = token_after(&url, "/lb/") {
            /* GET : le porteur du jeton relit SON entrée (pour retrouver son
               pseudo sur un autre PC) — rien d'autre n'est exposé */
            if method == tiny_http::Method::Get {
                match std::fs::read_to_string(format!("{}/{}.json", lb_dir, token)) {
                    Ok(s) => respond_json(req, 200, &s),
                    Err(_) => respond(req, 404, ""),
                }
                continue;
            }
            if method != tiny_http::Method::Put {
                respond(req, 405, "");
                continue;
            }
            let mut body = String::new();
            if req.as_reader().take(4_000).read_to_string(&mut body).is_err() {
                respond(req, 400, "lecture impossible");
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
                respond(req, 400, "json invalide");
                continue;
            };
            let path = format!("{}/{}.json", lb_dir, token);
            let raw_pseudo = v.get("pseudo").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let prev: Option<serde_json::Value> =
                std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok());
            let g = |k: &str| prev.as_ref().and_then(|v| v.get(k)).cloned();
            let prev_pseudo = g("pseudo").and_then(|p| p.as_str().map(String::from)).unwrap_or_default();
            let prev_pseudo_at = g("pseudo_at").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let renames = g("renames").and_then(|x| x.as_f64()).unwrap_or(0.0);
            /* pseudo vide = retrait de l'affichage. on MASQUE au lieu d'effacer :
               sinon il suffirait de se retirer puis de revenir pour contourner
               le délai de changement de pseudo. */
            if raw_pseudo.trim().is_empty() {
                if let Some(mut old) = prev {
                    old["hidden"] = serde_json::Value::Bool(true);
                    let _ = std::fs::write(&path, serde_json::to_string(&old).unwrap_or_default());
                }
                respond(req, 200, "retiré");
                continue;
            }
            let Some(pseudo) = clean_pseudo(&raw_pseudo) else {
                respond(req, 400, "pseudo invalide : 2 à 16 signes, alphabet latin, chiffres, - _ ou espace");
                continue;
            };
            /* un pseudo appartient au premier jeton qui l'a pris */
            let board = read_board(&lb_dir);
            let taken = board.iter().any(|(t, e)| {
                t != &token
                    && e.get("pseudo")
                        .and_then(|p| p.as_str())
                        .map(|p| norm_key(p) == norm_key(&pseudo))
                        .unwrap_or(false)
            });
            if taken {
                respond(req, 409, "pseudo déjà pris");
                continue;
            }
            if !std::path::Path::new(&path).exists() && board.len() >= 500 {
                respond(req, 507, "classement plein");
                continue;
            }
            /* un changement de pseudo par semaine — avec 10 minutes de grâce
               après le premier choix, pour corriger une faute de frappe */
            const SEMAINE: f64 = 7.0 * 86_400_000.0;
            const GRACE: f64 = 600_000.0;
            let now = now_ms();
            let changement = !prev_pseudo.is_empty() && norm_key(&prev_pseudo) != norm_key(&pseudo);
            let en_grace = changement && now - prev_pseudo_at < GRACE;
            if changement && !en_grace {
                let reste = SEMAINE - (now - prev_pseudo_at);
                if reste > 0.0 {
                    let jours = (reste / 86_400_000.0).ceil() as i64;
                    let msg = if jours > 1 {
                        format!("pseudo déjà changé récemment — vous pourrez le modifier dans {} jours", jours)
                    } else {
                        let heures = (reste / 3_600_000.0).ceil().max(1.0) as i64;
                        format!("pseudo déjà changé récemment — vous pourrez le modifier dans {} h", heures)
                    };
                    respond(req, 429, &msg);
                    continue;
                }
            }
            /* liste blanche : on ne stocke QUE des nombres finis et bornés,
               jamais le json brut du client (pas de champ surprise dans /lb) */
            let mut entry = serde_json::Map::new();
            entry.insert("pseudo".into(), serde_json::Value::String(pseudo));
            entry.insert("at".into(), serde_json::json!(now));
            /* ancre du délai : le premier choix, ou le dernier vrai changement */
            entry.insert(
                "pseudo_at".into(),
                serde_json::json!(if prev_pseudo.is_empty() || (changement && !en_grace) { now } else { prev_pseudo_at }),
            );
            entry.insert("renames".into(), serde_json::json!(if changement { renames + 1.0 } else { renames }));
            /* date du premier envoi : elle sert d'origine au plafond de cadence.
               une fois posée, elle ne bouge plus. */
            let premier_at = g("premier_at").and_then(|x| x.as_f64()).filter(|n| n.is_finite()).unwrap_or(now);
            entry.insert("premier_at".into(), serde_json::json!(premier_at));
            for k in ["captures", "especes", "shinies", "ecus", "trophees", "rangs", "migrations"] {
                let n = v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
                let n = if n.is_finite() { n.clamp(0.0, 1e15).floor() } else { 0.0 };
                entry.insert(k.into(), serde_json::json!(n));
            }
            /* le score est recalculé ici et les statistiques bornées : une
               sauvegarde trafiquée sort du classement au lieu de le dominer */
            let suspect = borner_stats(&mut entry, premier_at, prev.as_ref(), now);
            entry.insert("suspect".into(), serde_json::json!(suspect));
            let out = serde_json::to_string(&serde_json::Value::Object(entry)).unwrap_or_default();
            let tmp = format!("{}.tmp", path);
            if std::fs::write(&tmp, &out).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
                respond(req, 200, "ok");
            } else {
                respond(req, 500, "écriture impossible");
            }
            continue;
        }

        /* ── sauvegardes ───────────────────────────────────────────────── */
        let Some(token) = token_after(&url, "/s/") else {
            respond(req, 404, "affut-sync");
            continue;
        };
        let path = format!("{}/{}.json", data_dir, token);
        match method {
            tiny_http::Method::Get => match std::fs::read_to_string(&path) {
                Ok(s) => respond(req, 200, &s),
                Err(_) => respond(req, 404, ""),
            },
            tiny_http::Method::Put => {
                let mut body = String::new();
                let ok = req.as_reader().take(512_000).read_to_string(&mut body).is_ok();
                if !ok || body.len() < 2 || body.len() >= 512_000 || serde_json::from_str::<serde_json::Value>(&body).is_err() {
                    respond(req, 400, "json invalide ou trop gros");
                    continue;
                }
                // quota : un NOUVEAU jeton n'est accepté que sous le plafond global
                // (les jetons existants restent toujours modifiables)
                if !std::path::Path::new(&path).exists() {
                    let count = std::fs::read_dir(&data_dir)
                        .map(|d| d.filter(|e| e.as_ref().map(|e| e.path().extension().map(|x| x == "json").unwrap_or(false)).unwrap_or(false)).count())
                        .unwrap_or(0);
                    if count >= 500 {
                        respond(req, 507, "plus de place — contactez le gardien du comptoir");
                        continue;
                    }
                }
                let tmp = format!("{}.tmp", path);
                // filet : l'ancienne version survit en .bak (récupérable à la main)
                let _ = std::fs::copy(&path, format!("{}.bak", path));
                if std::fs::write(&tmp, &body).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
                    respond(req, 200, "ok");
                } else {
                    respond(req, 500, "écriture impossible");
                }
            }
            _ => respond(req, 405, ""),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn entree(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().unwrap().clone()
    }

    const JOUR: f64 = 86_400_000.0;

    /* la ligne réellement relevée sur le classement : un joueur s'était donné
       1 250 milliards d'écus et un score de 696 969 693 123. */
    #[test]
    fn la_sauvegarde_trafiquee_sort_du_classement() {
        let now = 1_787_339_565_576.0;
        let mut e = entree(serde_json::json!({
            "captures": 31232.0, "ecus": 1_250_023_123_123.0, "especes": 114.0,
            "migrations": 1.0, "rangs": 87.0, "score": 696_969_693_123.0,
            "shinies": 3.0, "trophees": 2.0
        }));
        // inscrit le jour même
        let suspect = borner_stats(&mut e, now - 3_600_000.0, None, now);
        assert!(suspect, "l'entrée doit être marquée suspecte");
        let ecus = lire_nb(&e, "ecus");
        let score = lire_nb(&e, "score");
        assert!(ecus < 1e9, "les écus doivent être bornés, obtenu {}", ecus);
        assert!(score < 1e6, "le score doit s'effondrer, obtenu {}", score);
    }

    /* un score inventé sans toucher au reste ne sert à rien non plus :
       il est recalculé à partir des composantes. */
    #[test]
    fn le_score_du_client_nest_jamais_repris() {
        let now = 1_787_339_565_576.0;
        let mut e = entree(serde_json::json!({
            "captures": 3352.0, "ecus": 88121.0, "especes": 38.0, "migrations": 0.0,
            "rangs": 117.0, "score": 999_999_999.0, "shinies": 3.0, "trophees": 0.0
        }));
        let suspect = borner_stats(&mut e, now - 2.0 * JOUR, None, now);
        assert!(!suspect, "aucune statistique n'est fausse, seul le score l'était");
        // espèces×100 + shinies×300 + rangs×40 + trophées×1000 + écus/1000
        let attendu: f64 = (38.0 * 100.0 + 3.0 * 300.0 + 117.0 * 40.0 + 88121.0 / 1000.0_f64).floor();
        assert_eq!(lire_nb(&e, "score"), attendu);
    }

    /* les cinq parties honnêtes relevées le même jour doivent passer intactes */
    #[test]
    fn les_parties_honnetes_passent_intactes() {
        let now = 1_787_339_632_825.0;
        let honnetes = [
            serde_json::json!({"captures": 1159.0, "ecus": 46151.0, "especes": 29.0, "migrations": 0.0, "rangs": 84.0, "shinies": 2.0, "trophees": 0.0}),
            serde_json::json!({"captures": 615.0, "ecus": 19150.0, "especes": 11.0, "migrations": 0.0, "rangs": 36.0, "shinies": 1.0, "trophees": 0.0}),
            serde_json::json!({"captures": 3352.0, "ecus": 88121.0, "especes": 38.0, "migrations": 0.0, "rangs": 117.0, "shinies": 3.0, "trophees": 0.0}),
            serde_json::json!({"captures": 87.0, "ecus": 275.0, "especes": 8.0, "migrations": 0.0, "rangs": 18.0, "shinies": 0.0, "trophees": 0.0}),
            serde_json::json!({"captures": 1860.0, "ecus": 58676.0, "especes": 28.0, "migrations": 0.0, "rangs": 88.0, "shinies": 3.0, "trophees": 0.0}),
        ];
        for j in honnetes {
            let avant = entree(j.clone());
            let mut e = entree(j);
            // inscrit à l'instant : c'est le cas le plus défavorable, seule
            // l'avance de deux jours protège l'entrée
            let suspect = borner_stats(&mut e, now, None, now);
            assert!(!suspect, "partie honnête marquée suspecte : {:?}", avant);
            for k in ["captures", "ecus", "especes", "rangs", "shinies"] {
                assert_eq!(lire_nb(&e, k), lire_nb(&avant, k), "{} modifié", k);
            }
        }
    }

    /* on ne peut pas non plus gonfler ses gains entre deux envois */
    #[test]
    fn un_bond_entre_deux_envois_est_borne() {
        let now = 1_787_339_565_576.0;
        let prec = serde_json::json!({"at": now - 30_000.0, "captures": 1000.0, "ecus": 50_000.0});
        let mut e = entree(serde_json::json!({
            "captures": 1001.0, "ecus": 900_000_000.0, "especes": 30.0,
            "migrations": 0.0, "rangs": 80.0, "shinies": 1.0, "trophees": 0.0
        }));
        let suspect = borner_stats(&mut e, now - 10.0 * JOUR, Some(&prec), now);
        assert!(suspect);
        // 30 s à 1 000 écus/s au plus, en partant des 50 000 précédents
        assert!(lire_nb(&e, "ecus") <= 80_000.0, "obtenu {}", lire_nb(&e, "ecus"));
    }

    /* les bornes de structure : rien de tout cela n'existe dans une partie */
    #[test]
    fn les_bornes_de_structure_tiennent() {
        let now = 1_787_339_565_576.0;
        let mut e = entree(serde_json::json!({
            "captures": 500.0, "ecus": 10_000.0, "especes": 9999.0, "migrations": 50.0,
            "rangs": 9999.0, "shinies": 9999.0, "trophees": 9999.0
        }));
        assert!(borner_stats(&mut e, now - 10.0 * JOUR, None, now));
        assert_eq!(lire_nb(&e, "especes"), 114.0, "114 espèces au maximum");
        assert_eq!(lire_nb(&e, "rangs"), 114.0 * 4.0, "un rang par espèce, quatre au plus");
        // le jeu plafonne la chance de shiny à 1/128 : 500 prises n'en donnent pas 500
        assert_eq!(lire_nb(&e, "shinies"), 10.0, "un shiny toutes les 80 prises au plus");
        assert_eq!(lire_nb(&e, "migrations"), 0.0, "10 000 écus ne paient pas le voyage à 100 000");
        assert_eq!(lire_nb(&e, "trophees"), 0.0, "sans migration, aucun trophée");
    }

    /* une espèce ne se découvre pas sans capture */
    #[test]
    fn on_ne_decouvre_pas_sans_capturer() {
        let now = 1_787_339_565_576.0;
        let mut e = entree(serde_json::json!({
            "captures": 5.0, "ecus": 100.0, "especes": 114.0, "migrations": 0.0,
            "rangs": 456.0, "shinies": 0.0, "trophees": 0.0
        }));
        assert!(borner_stats(&mut e, now - JOUR, None, now));
        assert_eq!(lire_nb(&e, "especes"), 5.0);
        assert_eq!(lire_nb(&e, "rangs"), 20.0);
    }

    /* les trophées doivent coller à la progression : ils ne s'obtiennent qu'en
       migrant, et migrer se paie — 100 000 écus, prix doublé à chaque départ. */
    #[test]
    fn les_trophees_doivent_etre_finances() {
        let now = 1_787_339_565_576.0;
        const JOUR: f64 = 86_400_000.0;

        // 50 000 écus gagnés en tout : pas même le premier voyage
        let mut e = entree(serde_json::json!({
            "captures": 900.0, "ecus": 50_000.0, "especes": 30.0, "migrations": 3.0,
            "rangs": 90.0, "shinies": 1.0, "trophees": 12.0
        }));
        assert!(borner_stats(&mut e, now - 10.0 * JOUR, None, now));
        assert_eq!(lire_nb(&e, "migrations"), 0.0);
        assert_eq!(lire_nb(&e, "trophees"), 0.0, "sans migration, aucun trophée");

        // 300 000 écus : le 1er voyage (100 k) et le 2e (200 k), pas le 3e
        let mut e = entree(serde_json::json!({
            "captures": 5000.0, "ecus": 300_000.0, "especes": 40.0, "migrations": 9.0,
            "rangs": 120.0, "shinies": 2.0, "trophees": 40.0
        }));
        assert!(borner_stats(&mut e, now - 10.0 * JOUR, None, now));
        assert_eq!(lire_nb(&e, "migrations"), 2.0, "300 000 écus paient deux voyages");
        // √(2 × 300 000 / 1 M) = √0,6 → aucun trophée entier
        assert_eq!(lire_nb(&e, "trophees"), 0.0);

        // une partie longue et honnête : 40 M gagnés, 5 migrations, 12 trophées
        let mut e = entree(serde_json::json!({
            "captures": 200_000.0, "ecus": 40_000_000.0, "especes": 90.0, "migrations": 5.0,
            "rangs": 300.0, "shinies": 60.0, "trophees": 12.0
        }));
        let suspect = borner_stats(&mut e, now - 60.0 * JOUR, None, now);
        assert!(!suspect, "une progression cohérente doit passer");
        assert_eq!(lire_nb(&e, "trophees"), 12.0);
    }

    /* le filet de lecture, éprouvé sur les six lignes réellement servies par le
       classement au moment du correctif — un seul relevé, pris d'un coup, car
       mélanger deux instantanés produit des lignes incohérentes qui n'ont
       jamais existé. aucune n'avait été écrite par la version vérifiée : elles
       ne portent donc ni premier_at ni suspect. */
    #[test]
    fn le_filet_de_lecture_trie_les_lignes_reelles() {
        let servies = [
            (serde_json::json!({"pseudo":"Foura","at":1787340532827.0f64,"captures":1236.0,"ecus":46366.0,"especes":29.0,"migrations":0.0,"rangs":87.0,"score":7326.0,"shinies":3.0,"trophees":0.0}), true),
            (serde_json::json!({"pseudo":"Smour","at":1787339876335.0f64,"captures":31232.0,"ecus":1250023123123.0,"especes":114.0,"migrations":1.0,"rangs":87.0,"score":696969693123.0,"shinies":3.0,"trophees":2.0}), false),
            (serde_json::json!({"pseudo":"ookChrome","at":1787340596179.0f64,"captures":640.0,"ecus":19150.0,"especes":11.0,"migrations":0.0,"rangs":36.0,"score":2859.0,"shinies":1.0,"trophees":0.0}), true),
            (serde_json::json!({"pseudo":"tely","at":1787340029702.0f64,"captures":3401.0,"ecus":90219.0,"especes":38.0,"migrations":0.0,"rangs":118.0,"score":9510.0,"shinies":3.0,"trophees":0.0}), true),
            (serde_json::json!({"pseudo":"Cuswel","at":1787315070221.0f64,"captures":87.0,"ecus":275.0,"especes":8.0,"migrations":0.0,"rangs":18.0,"score":1520.0,"shinies":0.0,"trophees":0.0}), true),
            (serde_json::json!({"pseudo":"ook","at":1787340479525.0f64,"captures":1903.0,"ecus":58676.0,"especes":28.0,"migrations":0.0,"rangs":88.0,"score":8178.0,"shinies":6.0,"trophees":0.0}), true),
        ];
        for (ligne, garde) in servies {
            let nom = ligne.get("pseudo").and_then(|p| p.as_str()).unwrap_or("?").to_string();
            assert_eq!(entree_verifiee(&ligne).is_some(), garde, "verdict inattendu pour {}", nom);
        }
    }

    /* un score gonflé sans toucher aux statistiques est corrigé, pas puni :
       le joueur reste au classement, avec le score que la formule donne. */
    #[test]
    fn un_score_gonfle_est_corrige_et_non_puni() {
        let mut l = serde_json::json!({
            "pseudo": "x", "at": 1787340029702.0f64, "captures": 3401.0, "ecus": 90219.0,
            "especes": 38.0, "migrations": 0.0, "rangs": 118.0, "score": 999_999.0,
            "shinies": 3.0, "trophees": 0.0
        });
        let v = entree_verifiee(&l).expect("la ligne doit rester au classement");
        let attendu: f64 = (38.0 * 100.0 + 3.0 * 300.0 + 118.0 * 40.0 + 90219.0 / 1000.0_f64).floor();
        assert_eq!(v.get("score").and_then(|x| x.as_f64()).unwrap(), attendu);

        // en revanche des statistiques impossibles écartent la ligne
        l["especes"] = serde_json::json!(114.0);
        l["premier_at"] = serde_json::json!(1787340029702.0f64);
        assert!(entree_verifiee(&l).is_none(), "114 espèces le jour même doivent sortir");
    }
}
