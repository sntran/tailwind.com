#!/bin/sh
# The runner part of .github/workflows/remote-ssh.yml: the OpenSSH server
# of the runner on 127.0.0.1, a quick tunnel of Cloudflare to it, and the
# branch $BRANCH with the files url, user and known_hosts, so that
# scripts/remote_ssh_client.sh can find the server. The branch holds one
# commit with no parent, and this script deletes it at its end.
#
# The server runs as the user of the job, with its own host key and its
# own configuration: it does not use the SSH service of the system. Only
# the keys of $KEYS can log in. A session starts in $WORK.
#
# Environment: BRANCH, KEYS (a file in the format of authorized_keys),
# MINUTES (60, at most 120), WORK (the directory of a session),
# PROBE_TRIES (60).
#
# The job ends after MINUTES, or when the file $REMOTE_SSH_STOP exists
# (a session has this variable).
set -u
: "${BRANCH:?}" "${KEYS:?}" "${MINUTES:=60}" "${PROBE_TRIES:=60}"
[ "$MINUTES" -gt 120 ] && MINUTES=120
TMP=${RUNNER_TEMP:-/tmp}
WORK=${WORK:-$(pwd)}
# Windows: the shell and the sshd of Git (MSYS2) use paths such as /d/a.
windows=0
case "$(uname -s)" in
    MINGW*|MSYS*)
        windows=1
        TMP=$(cygpath -u "$TMP")
        WORK=$(cygpath -u "$WORK")
        KEYS=$(cygpath -u "$KEYS") ;;
esac
PORT=${REMOTE_SSH_PORT:-8022}
dir=$TMP/remote-ssh
mkdir -p "$dir"
log=$dir/sshd.log
tunnel_log=$dir/cloudflared.log
stop=$dir/stop

# cloudflared 2026.10.7, with the SHA-256 of each file.
CF_VERSION=2026.10.7
cloudflared() {
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) f=cloudflared-linux-amd64
            s=1f99fb518b83dec2ba8d870a7cc991d82a77e6196295a3d2754ba17ab9f91872 ;;
        Linux-aarch64|Linux-arm64) f=cloudflared-linux-arm64
            s=3df43aedc9ff291d1735b5cb576cb97e0b157488796db2e32b28aecabb445775 ;;
        Darwin-x86_64) f=cloudflared-darwin-amd64.tgz
            s=02f3edfd87684dd5b705607688e4eb4b323f6e777b81e7b3a85af556aa356848 ;;
        Darwin-arm64) f=cloudflared-darwin-arm64.tgz
            s=3586ccf397e98e5f437e60e7c307ca6ac463e110580503af8bb8e6f9ebf831bd ;;
        MINGW*-x86_64|MSYS*-x86_64) f=cloudflared-windows-amd64.exe
            s=2493dbc168923bb3e59e5e7716f71deced92992eea9acd6e9d74f84ac7751343 ;;
        *) echo "remote ssh: no cloudflared for $(uname -s) $(uname -m)" >&2; return 1 ;;
    esac
    curl -fsSL -o "$dir/$f" \
        "https://github.com/cloudflare/cloudflared/releases/download/$CF_VERSION/$f" || return 1
    # From standard input: with a file name that has a backslash (Windows),
    # sha256sum puts a backslash before the hash.
    got=$( (sha256sum < "$dir/$f" 2>/dev/null || shasum -a 256 < "$dir/$f") | cut -d' ' -f1)
    if [ "$got" != "$s" ]; then
        echo "remote ssh: the SHA-256 of $f is $got, not $s" >&2
        return 1
    fi
    case $f in
        *.tgz) tar -xzf "$dir/$f" -C "$dir" cloudflared && CLOUDFLARED=$dir/cloudflared ;;
        *.exe) CLOUDFLARED=$dir/$f ;;
        *) chmod +x "$dir/$f" && CLOUDFLARED=$dir/$f ;;
    esac
}

export GIT_AUTHOR_NAME="remote ssh" GIT_AUTHOR_EMAIL="remote-ssh@users.noreply.github.com"
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"

server_pid= tunnel_pid=
cleanup() {
    [ -n "$tunnel_pid" ] && kill "$tunnel_pid" 2>/dev/null
    [ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null
    git push -q origin --delete "$BRANCH" 2>/dev/null
    echo "remote ssh: the branch $BRANCH is deleted"
}
trap cleanup EXIT

# Wait at most $2 seconds for the pattern $1 in the file $3, while the
# process $4 runs.
wait_for() {
    n=0
    while [ $n -lt $(( $2 * 5 )) ]; do
        m=$(grep -E -o "$1" "$3" 2>/dev/null | head -n 1)
        if [ -n "$m" ]; then echo "$m"; return 0; fi
        kill -0 "$4" 2>/dev/null || return 1
        sleep 0.2
        n=$((n + 1))
    done
    return 1
}

# The server program. sshd needs its full path. On Windows, the sshd of
# Git (MSYS2) runs as the user of the job, and does not check the access
# lists of Windows for its key files, as the sshd of the system does.
if [ $windows = 1 ]; then
    sshd=/usr/bin/sshd
    keygen=/usr/bin/ssh-keygen
else
    sshd=$(command -v sshd 2>/dev/null || true)
    for candidate in /usr/sbin/sshd /usr/bin/sshd; do
        [ -n "$sshd" ] && break
        [ -x "$candidate" ] && sshd=$candidate
    done
    keygen=$(command -v ssh-keygen 2>/dev/null || echo "$(dirname "${sshd:-/usr/bin/sshd}")/ssh-keygen")
fi
if [ -z "$sshd" ] || [ ! -x "$sshd" ]; then
    echo "remote ssh: no sshd on this runner" >&2
    exit 1
fi

# The directory for privilege separation, where sshd of Linux needs it.
if [ "$(uname -s)" = Linux ] && [ ! -d /run/sshd ]; then
    sudo mkdir -p /run/sshd 2>/dev/null || true
fi

"$keygen" -q -t ed25519 -N '' -f "$dir/host_key" || exit 1
cp "$KEYS" "$dir/authorized_keys"
chmod 600 "$dir/authorized_keys" "$dir/host_key"
user=$(id -un)

# The environment of a session: the token of the job, the stop file, and
# the work directory.
{
    echo "export GH_TOKEN='${GH_TOKEN:-}'"
    echo "export REMOTE_SSH_STOP='$stop'"
    echo "export GITHUB_REPOSITORY='${GITHUB_REPOSITORY:-}'"
    echo "export PATH=\"$PATH\""
    echo "cd '$WORK'"
} > "$dir/session.sh"
chmod 600 "$dir/session.sh"

# A command of the client runs with bash, after the file session.sh. With
# no command, the session is a shell.
cat > "$dir/command.sh" <<EOF
#!/bin/sh
. '$dir/session.sh'
if [ -n "\${SSH_ORIGINAL_COMMAND:-}" ]; then
    case "\$SSH_ORIGINAL_COMMAND" in
        internal-sftp*|*sftp-server*) exec \$SSH_ORIGINAL_COMMAND ;;
    esac
    exec bash -c "\$SSH_ORIGINAL_COMMAND"
fi
exec bash -l
EOF
chmod 700 "$dir/command.sh"

sftp=internal-sftp
cat > "$dir/sshd_config" <<EOF
Port $PORT
ListenAddress 127.0.0.1
HostKey $dir/host_key
PidFile $dir/sshd.pid
AuthorizedKeysFile $dir/authorized_keys
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
StrictModes no
AllowTcpForwarding yes
ClientAliveInterval 30
Subsystem sftp $sftp
ForceCommand $dir/command.sh
EOF
# PAM is not in each build of sshd: use the option only where it is valid.
if "$sshd" -t -f "$dir/sshd_config" -o UsePAM=no 2>/dev/null; then
    echo "UsePAM no" >> "$dir/sshd_config"
fi
if ! "$sshd" -t -f "$dir/sshd_config"; then
    echo "remote ssh: the configuration of sshd is not valid" >&2
    exit 1
fi

"$sshd" -D -e -f "$dir/sshd_config" > "$log" 2>&1 &
server_pid=$!
if ! wait_for 'Server listening on' 60 "$log" "$server_pid" > /dev/null; then
    cat "$log"
    echo "remote ssh: the server did not start" >&2
    exit 1
fi
fp=$("$keygen" -l -f "$dir/host_key.pub" | cut -d' ' -f2)

# CLOUDFLARED: a cloudflared that is there already, for a test.
[ -n "${CLOUDFLARED:-}" ] || cloudflared || exit 1
"$CLOUDFLARED" tunnel --no-autoupdate --url "tcp://127.0.0.1:$PORT" > "$tunnel_log" 2>&1 &
tunnel_pid=$!
if ! host=$(wait_for '[a-z0-9-]+\.trycloudflare\.com' 120 "$tunnel_log" "$tunnel_pid"); then
    cat "$tunnel_log"
    echo "remote ssh: the tunnel did not start" >&2
    exit 1
fi
# The name of the tunnel needs some seconds in DNS.
n=0
until curl -s -o /dev/null "https://$host/"; do
    n=$((n + 1))
    if [ $n -ge "$PROBE_TRIES" ]; then
        echo "remote ssh: https://$host/ does not answer" >&2
        break
    fi
    sleep 2
done

printf 'wss://%s/\n' "$host" > "$dir/url"
printf '%s\n' "$user" > "$dir/user"
# The host key, for the alias that the client uses.
printf 'remote-ssh %s\n' "$(cut -d' ' -f1,2 "$dir/host_key.pub")" > "$dir/known_hosts"
tree=$(printf '100644 blob %s\turl\n100644 blob %s\tuser\n100644 blob %s\tknown_hosts\n' \
    "$(git hash-object -w "$dir/url")" "$(git hash-object -w "$dir/user")" \
    "$(git hash-object -w "$dir/known_hosts")" | git mktree)
commit=$(echo "Remote SSH: wss://$host/" | git commit-tree "$tree")
git push -q -f origin "$commit:refs/heads/$BRANCH" || exit 1
echo "remote ssh: wss://$host/ $fp, the user $user, for $MINUTES minutes"
echo "remote ssh: scripts/remote_ssh_client.sh $BRANCH KEY exec 'uname -a'"

# Until the limit of minutes, the stop file, or the end of the server.
end=$(( $(date +%s) + MINUTES * 60 ))
while [ "$(date +%s)" -lt "$end" ] && [ ! -e "$stop" ] && kill -0 "$server_pid" 2>/dev/null; do
    sleep 5
done
tail -n 5 "$log"
echo "remote ssh: the end"
