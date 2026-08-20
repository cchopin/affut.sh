//! affut-sync — mini service de synchronisation des sauvegardes navigateur.
//! GET/PUT /s/<jeton> ; un fichier par jeton ; CORS ouvert (le jeton est le secret).

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::io::Read;
    let data_dir = std::env::var("AFFUT_SYNC_DIR").unwrap_or_else(|_| "/data".into());
    std::fs::create_dir_all(&data_dir).ok();
    let server = tiny_http::Server::http("0.0.0.0:2323").expect("bind 2323");
    eprintln!("affut-sync sur :2323, stockage {}", data_dir);

    fn token_of(url: &str) -> Option<String> {
        let t = url.strip_prefix("/s/")?;
        let t = t.split('?').next().unwrap_or(t);
        if t.len() >= 16 && t.len() <= 64 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            Some(t.to_string())
        } else {
            None
        }
    }

    for mut req in server.incoming_requests() {
        let cors = [
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"https://cchopin.github.io"[..]).unwrap(),
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, PUT, OPTIONS"[..]).unwrap(),
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"content-type"[..]).unwrap(),
        ];
        let respond = |req: tiny_http::Request, code: u32, body: &str| {
            let mut r = tiny_http::Response::from_string(body).with_status_code(code);
            for h in cors.iter() {
                r.add_header(h.clone());
            }
            let _ = req.respond(r);
        };
        let method = req.method().clone();
        let url = req.url().to_string();
        if method == tiny_http::Method::Options {
            respond(req, 204, "");
            continue;
        }
        let Some(token) = token_of(&url) else {
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
