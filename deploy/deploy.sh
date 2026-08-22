#!/bin/sh
# Déploie affut.sh sur le serveur. Le jeu est exposé sur le port 2322 (clés invitées uniquement).
# Usage : ./deploy/deploy.sh <user>@<hôte>       (SSH d'administration, port 443 ici)
set -eu

DEST="${1:?usage: deploy.sh user@host}"
SSH="ssh -p 443 $DEST"
DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "[1/6] envoi des sources vers $DEST:/opt/affut/src"
$SSH 'sudo mkdir -p /opt/affut/src && sudo chown $(id -u) /opt/affut /opt/affut/src'
rsync -az -e 'ssh -p 443' --delete \
  --include='Cargo.toml' --include='Cargo.lock' --include='src/***' --include='deploy/***' \
  --exclude='*' "$DIR/" "$DEST:/opt/affut/src/"

echo "[2/6] clés autorisées (propriétaire ; les amis s'ajoutent via deploy/invite.sh)"
$SSH '[ -s /opt/affut/authorized_keys ] || true'
if ! $SSH '[ -s /opt/affut/authorized_keys ]'; then
  cat ~/.ssh/ovh.pub ~/.ssh/id_ed25519.pub 2>/dev/null | $SSH 'cat > /opt/affut/authorized_keys'
fi

echo "[3/6] build de l'image (sur le serveur)"
$SSH 'cd /opt/affut/src && docker build -t affut -f deploy/Dockerfile .'
# le cache de build enfle de quelques centaines de Mo à chaque déploiement :
# on n'en garde qu'un gigaoctet, de quoi accélérer la prochaine compilation
$SSH 'docker builder prune -f --keep-storage 1GB >/dev/null 2>&1 || true'

echo "[4/6] migration de l'ancien volume traque_data si présent"
$SSH 'if docker volume inspect traque_data >/dev/null 2>&1 && ! docker volume inspect affut_data >/dev/null 2>&1; then
  docker volume create affut_data >/dev/null
  docker run --rm -v traque_data:/old -v affut_data:/new alpine sh -c "cp -a /old/. /new/." && echo "  volume migré"
fi'

echo "[5/6] clé d'hôte persistante (l'identité SSH du jeu survit aux rebuilds)"
$SSH '[ -f /opt/affut/hostkeys/ssh_host_ed25519_key ] || { mkdir -p /opt/affut/hostkeys && ssh-keygen -t ed25519 -N "" -f /opt/affut/hostkeys/ssh_host_ed25519_key -C affut.sh -q; }'

echo "[5/6] (re)lancement du conteneur — port 2322 PUBLIC (jeu seul, clés invitées)"
$SSH 'docker rm -f affut traque 2>/dev/null || true; docker run -d --name affut \
  --restart unless-stopped \
  -e TZ=Europe/Paris \
  -p 2322:2222 \
  -v affut_data:/home/affut \
  -v /opt/affut/authorized_keys:/etc/ssh/auth/affut:ro \
  -v /opt/affut/hostkeys:/etc/ssh/keys:ro \
  --memory 384m --cpus 0.5 \
  affut'

echo "[6/8] synchronisation navigateur (conteneurs affut-sync + proxy TLS)"
cat > /tmp/affut-tls.conf <<'NGX'
limit_req_zone $binary_remote_addr zone=affutsync:1m rate=5r/s;
server {
    listen 8444 ssl;
    ssl_certificate     /etc/letsencrypt/live/vps.tely.info/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/vps.tely.info/privkey.pem;
    client_max_body_size 2m;
    location / {
        limit_req zone=affutsync burst=15;
        proxy_pass http://affut-sync:2323;
    }
}
NGX
scp -P 443 -q /tmp/affut-tls.conf "$DEST:/opt/affut/tls.conf"
$SSH 'docker network inspect affut-net >/dev/null 2>&1 || docker network create affut-net >/dev/null
docker rm -f affut-sync affut-tls 2>/dev/null || true
docker run -d --name affut-sync --restart unless-stopped --network affut-net \
  -v affut_sync:/data --memory 128m --cpus 0.25 affut /usr/local/bin/affut-sync
docker run -d --name affut-tls --restart unless-stopped --network affut-net \
  -p 8444:8444 \
  -v /opt/affut/tls.conf:/etc/nginx/conf.d/default.conf:ro \
  -v /etc/letsencrypt:/etc/letsencrypt:ro \
  --memory 64m nginx:alpine >/dev/null'

echo "[7/8] rechargement TLS quotidien (renouvellement des certificats)"
$SSH 'crontab -l 2>/dev/null | grep -q affut-tls || (crontab -l 2>/dev/null; echo "10 4 * * * docker exec affut-tls nginx -s reload >/dev/null 2>&1") | crontab -'

echo "[8/8] vérifications"
$SSH 'sleep 1; docker ps --filter name=affut --format "{{.Status}}  {{.Ports}}"'

echo
echo "OK. connexion : ssh -p 2322 affut@$(echo "$DEST" | cut -d@ -f2)"
echo "inviter un ami : ./deploy/invite.sh <pseudo> \"<sa clé publique>\""
