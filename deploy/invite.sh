#!/bin/sh
# Invite un ami sur affut.sh : ajoute sa clé publique avec sa propre sauvegarde.
#
#   ./deploy/invite.sh --demande                                   message à envoyer à l'ami pour obtenir sa clé
#   ./deploy/invite.sh alice "ssh-ed25519 AAAA... alice@laptop"   inviter
#   ./deploy/invite.sh --list                                      lister les joueurs
#   ./deploy/invite.sh --remove alice                              révoquer
#
# Chaque clé est taguée AFFUT_PLAYER=<pseudo> : le jeu donne à chaque joueur
# son propre monde (~/.affutsh-<pseudo>.json dans le volume du conteneur).
set -eu

DEST="${AFFUT_HOST:-tely@vps.tely.info}"
GAME_HOST="$(echo "$DEST" | cut -d@ -f2)"
SSH="ssh -p 443 $DEST"
KEYS=/opt/affut/authorized_keys

case "${1:-}" in
  --demande)
    cat <<'MSG'
──────────────────── à envoyer à votre ami·e ────────────────────
pour te connecter au jeu, j'ai besoin de ta clé publique ssh.
copie-colle ceci dans un terminal (macOS, Linux, ou PowerShell
sous Windows 10+) — ça affiche ta clé, et la crée si tu n'en as pas :

    [ -f ~/.ssh/id_ed25519.pub ] || ssh-keygen -t ed25519
    cat ~/.ssh/id_ed25519.pub

(sous PowerShell : `ssh-keygen -t ed25519` puis
 `type $HOME\.ssh\id_ed25519.pub`)

envoie-moi la ligne complète qui commence par « ssh-ed25519 » —
c'est une clé PUBLIQUE, elle peut circuler sans risque.
ne m'envoie jamais le fichier sans .pub, celui-là est privé.
──────────────────────────────────────────────────────────────────
MSG
    exit 0
    ;;
  --list)
    echo "joueurs invités :"
    $SSH "grep -o 'AFFUT_PLAYER=[a-zA-Z0-9]*' $KEYS 2>/dev/null | cut -d= -f2 | sort -u" || true
    echo "(les lignes sans pseudo sont les clés du propriétaire)"
    exit 0
    ;;
  --remove)
    NAME="${2:?usage: invite.sh --remove <pseudo>}"
    $SSH "sed -i '/AFFUT_PLAYER=$NAME /d' $KEYS"
    echo "clé de « $NAME » révoquée (sa sauvegarde reste sur le serveur)."
    exit 0
    ;;
esac

NAME="${1:?usage: invite.sh <pseudo> \"<clé publique>\"}"
KEY="${2:?il manque la clé publique (contenu de id_ed25519.pub)}"

# pseudo : alphanumérique uniquement (il nomme le fichier de sauvegarde)
case "$NAME" in
  *[!a-zA-Z0-9]*) echo "pseudo invalide : lettres et chiffres uniquement" >&2; exit 1 ;;
esac
case "$KEY" in
  ssh-ed25519\ *|ssh-rsa\ *|ecdsa-*) : ;;
  *) echo "clé publique invalide (attendu : « ssh-ed25519 AAAA... »)" >&2; exit 1 ;;
esac

if $SSH "grep -q 'AFFUT_PLAYER=$NAME ' $KEYS 2>/dev/null"; then
  echo "« $NAME » est déjà invité." >&2
  exit 1
fi

printf 'environment="AFFUT_PLAYER=%s" %s\n' "$NAME" "$KEY" | $SSH "cat >> $KEYS"
echo "« $NAME » est invité ! aucune relance nécessaire, c'est immédiat."
echo
echo "────────────────────────── à envoyer à $NAME ──────────────────────────"
echo "tu es invité·e sur affut.sh, un jeu de capture dans le terminal."
echo
echo "connexion directe :"
echo "    ssh -p 2322 affut@$GAME_HOST"
echo
echo "ou ajoute ceci dans ton ~/.ssh/config puis tape juste « ssh affut » :"
echo
echo "    Host affut"
echo "        HostName $GAME_HOST"
echo "        Port 2322"
echo "        User affut"
echo
echo "déplacement : flèches ou zqsd · Entrée pour interagir · ? pour l'aide"
echo "────────────────────────────────────────────────────────────────────────"
