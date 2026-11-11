#!/bin/sh
# The client of .github/workflows/remote-ssh.yml. It reads the URL and the
# fingerprint of the server from the branch BRANCH, and starts
# scripts/remote_ssh.erl with the erl of an Erlang/OTP on the PATH (or
# with beam.com, when BEAM_COM names it):
#
#   remote_ssh_client.sh keygen KEY
#   remote_ssh_client.sh BRANCH KEY exec 'CMD'
#   remote_ssh_client.sh BRANCH KEY put LOCAL REMOTE
#   remote_ssh_client.sh BRANCH KEY get REMOTE LOCAL
#   remote_ssh_client.sh BRANCH - bridge PORT    # then: ssh -p PORT runner@127.0.0.1
#
# KEY is a file of "keygen". The bridge needs no KEY, because OpenSSH uses
# its own key. The scripts come from BEAM.com.
set -eu
dir=$(cd "$(dirname "$0")" && pwd)

# run MODE ARGS...: scripts/remote_ssh.erl with these arguments.
run() {
    if [ -n "${BEAM_COM:-}" ]; then
        exec "$BEAM_COM" -a ssh "$dir/remote_ssh.erl" -- "$@"
    fi
    cache=${TMPDIR:-/tmp}/remote_ssh.$(id -u)
    mkdir -p "$cache"
    if [ ! -f "$cache/remote_ssh.beam" ] || [ "$dir/remote_ssh.erl" -nt "$cache/remote_ssh.beam" ]; then
        erlc -o "$cache" "$dir/remote_ssh.erl"
    fi
    exec erl -noshell -pa "$cache" -run remote_ssh main "$@"
}

if [ "${1:-}" = keygen ] && [ $# -eq 2 ]; then
    run keygen "$2"
fi
[ $# -ge 4 ] || { sed -n '2,14p' "$0" >&2; exit 2; }
BRANCH=$1 KEY=$2 MODE=$3
shift 3
git fetch -q origin "+refs/heads/$BRANCH:refs/remote-ssh/info"
url=$(git show refs/remote-ssh/info:url)
fp=$(git show refs/remote-ssh/info:fingerprint)
case $MODE in
    bridge) run bridge "$url" "$@" ;;
    *) run "$MODE" "$url" "$KEY" "$fp" "$@" ;;
esac
