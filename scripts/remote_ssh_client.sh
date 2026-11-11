#!/bin/sh
# The client of .github/workflows/remote-ssh.yml. It reads the URL, the
# user and the host key of the server from the branch BRANCH, and starts
# OpenSSH through the tunnel:
#
#   remote_ssh_client.sh keygen KEY
#   remote_ssh_client.sh BRANCH KEY exec 'CMD'
#   remote_ssh_client.sh BRANCH KEY put LOCAL REMOTE
#   remote_ssh_client.sh BRANCH KEY get REMOTE LOCAL
#   remote_ssh_client.sh BRANCH KEY shell
#   remote_ssh_client.sh BRANCH KEY config     # the options, for your own ssh command
#
# KEY is a private key file (keygen makes it, with KEY.pub).
#
# The tunnel carries the connection in a WebSocket. The bridge to it is
# "cloudflared access tcp" when cloudflared is on the PATH, else the small
# program tools/ws-proxy of this repository, which cargo builds at the
# first use. The bridge uses HTTPS_PROXY and SSL_CERT_FILE when they are
# set.
set -eu
dir=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$dir")

if [ "${1:-}" = keygen ] && [ $# -eq 2 ]; then
    exec ssh-keygen -q -t ed25519 -N '' -C tailwind.com-ci -f "$2"
fi
[ $# -ge 3 ] || { sed -n '2,19p' "$0" >&2; exit 2; }
BRANCH=$1 KEY=$2 MODE=$3
shift 3

git -C "$root" fetch -q origin "+refs/heads/$BRANCH:refs/remote-ssh/info"
url=$(git -C "$root" show refs/remote-ssh/info:url)
user=$(git -C "$root" show refs/remote-ssh/info:user)
state=${TMPDIR:-/tmp}/remote-ssh.$(id -u)
mkdir -p "$state"
git -C "$root" show refs/remote-ssh/info:known_hosts > "$state/known_hosts"
host=${url#wss://}
host=${host%/}

if command -v cloudflared > /dev/null 2>&1; then
    proxy="cloudflared access tcp --hostname $host"
else
    proxy=$root/tools/ws-proxy/target/release/ws-proxy
    if [ ! -x "$proxy" ]; then
        echo "remote_ssh_client.sh: building tools/ws-proxy" >&2
        (cd "$root/tools/ws-proxy" && cargo build -q --release)
    fi
    proxy="$proxy $url"
fi

# The host key is the key of the branch, for the alias remote-ssh.
set -- "$@"
options="-i $KEY -o IdentitiesOnly=yes -o UserKnownHostsFile=$state/known_hosts
-o StrictHostKeyChecking=yes -o HostKeyAlias=remote-ssh -o LogLevel=ERROR
-o ServerAliveInterval=30"

case $MODE in
    config)
        echo "$options -o ProxyCommand=\"$proxy\" $user@remote-ssh" ;;
    exec)
        [ $# -ge 1 ] || { echo "exec needs a command" >&2; exit 2; }
        # shellcheck disable=SC2086
        exec ssh $options -o ProxyCommand="$proxy" "$user@remote-ssh" "$@" ;;
    shell)
        # shellcheck disable=SC2086
        exec ssh -t $options -o ProxyCommand="$proxy" "$user@remote-ssh" ;;
    put)
        [ $# -eq 2 ] || { echo "put needs LOCAL REMOTE" >&2; exit 2; }
        # shellcheck disable=SC2086
        exec ssh $options -o ProxyCommand="$proxy" "$user@remote-ssh" "cat > '$2'" < "$1" ;;
    get)
        [ $# -eq 2 ] || { echo "get needs REMOTE LOCAL" >&2; exit 2; }
        # shellcheck disable=SC2086
        exec ssh $options -o ProxyCommand="$proxy" "$user@remote-ssh" "cat '$1'" > "$2" ;;
    *) sed -n '2,19p' "$0" >&2; exit 2 ;;
esac
