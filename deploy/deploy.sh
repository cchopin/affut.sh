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

echo "[4/6] migration de l'ancien volume traque_data si présent"
$SSH 'if docker volume inspect traque_data >/dev/null 2>&1 && ! docker volume inspect affut_data >/dev/null 2>&1; then
  docker volume create affut_data >/dev/null
  docker run --rm -v traque_data:/old -v affut_data:/new alpine sh -c "cp -a /old/. /new/." && echo "  volume migré"
fi'

echo "[5/6] (re)lancement du conteneur — port 2322 PUBLIC (jeu seul, clés invitées)"
$SSH 'docker rm -f affut traque 2>/dev/null || true; docker run -d --name affut \
  --restart unless-stopped \
  -p 2322:2222 \
  -v affut_data:/home/affut \
  -v /opt/affut/authorized_keys:/etc/ssh/auth/affut:ro \
  --memory 384m --cpus 0.5 \
  affut'

echo "[6/6] vérifications"
$SSH 'sleep 1; docker ps --filter name=affut --format "{{.Status}}  {{.Ports}}"'

echo
echo "OK. connexion : ssh -p 2322 affut@$(echo "$DEST" | cut -d@ -f2)"
echo "inviter un ami : ./deploy/invite.sh <pseudo> \"<sa clé publique>\""
