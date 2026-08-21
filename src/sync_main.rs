//! affut-sync — synchronisation des sauvegardes navigateur + classement.
//!
//! sauvegardes : GET/PUT /s/<jeton> — un fichier par jeton, le jeton EST le secret.
//! classement  : PUT /lb/<jeton> (pseudo + stats), GET /lb (tableau public).

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
            for k in ["captures", "especes", "shinies", "ecus", "trophees", "rangs", "score", "migrations"] {
                let n = v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
                let n = if n.is_finite() { n.clamp(0.0, 1e15).floor() } else { 0.0 };
                entry.insert(k.into(), serde_json::json!(n));
            }
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
