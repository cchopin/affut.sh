# affut.sh — conteneur de jeu accessible en SSH
FROM rust:alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM alpine:3.20
RUN apk add --no-cache openssh-server tzdata \
    && adduser -D -s /bin/sh affut \
    # compte sans mot de passe mais non verrouillé (auth par clé uniquement)
    && sed -i 's/^affut:!/affut:*/' /etc/shadow \
    && ssh-keygen -A \
    && mkdir -p /etc/ssh/auth
COPY --from=build /src/target/release/affut /usr/local/bin/affut
COPY --from=build /src/target/release/affut-sync /usr/local/bin/affut-sync
COPY deploy/sshd_affut.conf /etc/ssh/sshd_config
# les clés publiques (avec AFFUT_PLAYER=pseudo par joueur) sont fournies au run :
#   -v .../authorized_keys:/etc/ssh/auth/affut:ro
EXPOSE 2222
CMD ["/usr/sbin/sshd", "-D", "-e"]
