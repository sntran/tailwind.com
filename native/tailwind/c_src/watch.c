/* The file events of the watch of tailwind.com (--watch).
 *
 * This is the method of the file watcher of BEAM.com
 * (c_src/cosmo/beam_com_watch.c), as a small library: the program gives
 * the directories and the files to watch, and waits for a change or for
 * the end of an interval. The program then compares the files itself
 * (native/tailwind/src/cli/mod.rs), so an event only wakes it: a lost or
 * a false event changes the time of a build, not its result.
 *
 * On Linux it uses inotify on the directories. Cosmopolitan has no
 * wrappers for inotify, so the calls are direct system calls. On macOS
 * and the BSDs it uses kqueue on the directories and, while there are
 * descriptors for them, on the files. Cosmopolitan has the system calls
 * of kqueue but no declarations, and struct kevent is not the same on
 * the four systems (C29 in docs/UPSTREAM.md of BEAM.com). On Windows it
 * uses ReadDirectoryChangesW() with one I/O completion port: see the
 * Windows part below.
 *
 * When the events of a system do not start, tw_watch_start() gives 0.
 * When they stop, tw_watch_wait() gives -1. The program then polls.
 *
 * With TW_WATCH_INOTIFY_ONLY (the native NIF object for BEAM.com,
 * scripts/nif.sh), the file has inotify only, and the other systems poll.
 * The object can then use only the functions of the C library that
 * beam.com gives to an object, and these do not have the kqueue and
 * Windows functions.
 */
#include <cosmo.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>
#if defined(__x86_64__) && !defined(TW_WATCH_INOTIFY_ONLY)
#include "libc/nt/createfile.h"
#include "libc/nt/dll.h"
#include "libc/nt/enum/accessmask.h"
#include "libc/nt/enum/creationdisposition.h"
#include "libc/nt/enum/fileflagandattributes.h"
#include "libc/nt/enum/filesharemode.h"
#include "libc/nt/enum/wait.h"
#include "libc/nt/errors.h"
#include "libc/nt/iocp.h"
#include "libc/nt/runtime.h"
#include "libc/nt/struct/overlapped.h"
#endif

/* The kinds of a path of tw_watch_add(). */
#define TW_FILE 0
#define TW_DIR 1
#define TW_TREE 2

/* --- Linux: inotify ---------------------------------------------------- */

static long linux_syscall(long n, long a, long b, long c)
{
    long r;
#if defined(__x86_64__)
    __asm__ volatile("syscall"
                     : "=a"(r)
                     : "0"(n), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
#elif defined(__aarch64__)
    register long x8 __asm__("x8") = n;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    r = x0;
#else
    r = -ENOSYS;
#endif
    if (r < 0 && r > -4096) {
        errno = -r;
        return -1;
    }
    return r;
}

#if defined(__x86_64__)
#define NR_INOTIFY_INIT1 294
#define NR_INOTIFY_ADD_WATCH 254
#else
#define NR_INOTIFY_INIT1 26
#define NR_INOTIFY_ADD_WATCH 27
#endif

/* The values of Linux: IN_NONBLOCK and IN_CLOEXEC, and the events. */
#define W_NONBLOCK 04000
#define W_CLOEXEC 02000000
#define W_MODIFY 0x002
#define W_ATTRIB 0x004
#define W_CLOSE_WRITE 0x008
#define W_MOVED_FROM 0x040
#define W_MOVED_TO 0x080
#define W_CREATE 0x100
#define W_DELETE 0x200
#define W_DELETE_SELF 0x400
#define W_MOVE_SELF 0x800
#define W_EVENTS (W_MODIFY | W_ATTRIB | W_CLOSE_WRITE | W_MOVED_FROM | W_MOVED_TO | \
                  W_CREATE | W_DELETE | W_DELETE_SELF | W_MOVE_SELF)

static int ifd = -1;

/* The value of POLLIN on Linux. In Cosmopolitan, POLLIN is a variable
 * with the value of the system, and beam.com does not give it to an
 * object. */
#ifdef TW_WATCH_INOTIFY_ONLY
#define W_POLLIN 0x001
#else
#define W_POLLIN POLLIN
#endif

#ifndef TW_WATCH_INOTIFY_ONLY

/* --- macOS and the BSDs: kqueue ---------------------------------------- */

int sys_kqueue(void);
int sys_kevent(int, const void *, int, void *, int, const struct timespec *);

/* macOS: sys_kevent is kevent64(kq, changes, nchanges, events, nevents,
 * flags, timeout). With KEVENT_FLAG_IMMEDIATE the kernel does not read
 * the timeout, and the wait is a poll() on the kqueue. */
typedef int kevent64_f(int, const void *, int, void *, int, unsigned);
#define KQ_KEVENT_FLAG_IMMEDIATE 0x1

/* struct kevent of OpenBSD, and the first fields of the other layouts. */
struct kev {
    uintptr_t ident;
    int16_t filter;
    uint16_t flags;
    uint32_t fflags;
    int64_t data;
    void *udata;
};

/* FreeBSD 12 and later. */
struct kev_freebsd {
    struct kev k;
    uint64_t ext[4];
};

/* struct kevent64_s of macOS. */
struct kev_xnu {
    struct kev k;
    uint64_t ext[2];
};

/* NetBSD: the filter and the flags are 32 bits. */
struct kev_netbsd {
    uintptr_t ident;
    uint32_t filter;
    uint32_t flags;
    uint32_t fflags;
    int64_t data;
    void *udata;
};

_Static_assert(sizeof(struct kev) == 32, "struct kevent (OpenBSD)");
_Static_assert(sizeof(struct kev_freebsd) == 64, "struct kevent (FreeBSD)");
_Static_assert(sizeof(struct kev_xnu) == 48, "struct kevent64_s (macOS)");
_Static_assert(sizeof(struct kev_netbsd) == 40, "struct kevent (NetBSD)");

#define KQ_EV_ADD 0x0001
#define KQ_EV_CLEAR 0x0020
#define KQ_NOTE_DELETE 0x0001
#define KQ_NOTE_WRITE 0x0002
#define KQ_NOTE_EXTEND 0x0004
#define KQ_NOTE_ATTRIB 0x0008
#define KQ_NOTE_LINK 0x0010
#define KQ_NOTE_RENAME 0x0020
#define KQ_NOTES (KQ_NOTE_DELETE | KQ_NOTE_WRITE | KQ_NOTE_EXTEND | \
                  KQ_NOTE_ATTRIB | KQ_NOTE_LINK | KQ_NOTE_RENAME)
#define KQ_FILTER_VNODE (IsNetbsd() ? 3 : -4)
#define KQ_MAX_EVENTS 64
#define KQ_MAX_FILES 4096

static int kq = -1, kq_files, kq_max_files;

/* The watched paths of kqueue: a hash table of the paths, and the slot of
 * each descriptor. A file that is removed or renamed leaves the table,
 * so the next tw_watch_add() of its path watches the new file. */
struct slot {
    char *path; /* NULL: free. */
    int fd;     /* -1: a removed entry. */
};
static struct slot *slots;
static size_t nslots, used_slots; /* nslots: 0 or a power of 2 */
static int *fd_slot;
static size_t nfd_slot;

static size_t kev_size(void)
{
    return IsNetbsd() ? sizeof(struct kev_netbsd)
         : IsFreebsd() ? sizeof(struct kev_freebsd)
         : IsXnu() ? sizeof(struct kev_xnu) : sizeof(struct kev);
}

static int kev_now(const void *changes, int nchanges, void *events, int nevents)
{
    static const struct timespec zero = {0, 0};
    kevent64_f *kevent64 = (kevent64_f *)(void *)sys_kevent;

    if (IsXnu())
        return kevent64(kq, changes, nchanges, events, nevents, KQ_KEVENT_FLAG_IMMEDIATE);
    return sys_kevent(kq, changes, nchanges, events, nevents, &zero);
}

static int kev_wait(void *events, int nevents, int ms)
{
    struct timespec ts = {ms / 1000, (ms % 1000) * 1000000L};
    struct pollfd pfd = {0, POLLIN, 0};
    int n;

    if (!IsXnu())
        return sys_kevent(kq, NULL, 0, events, nevents, &ts);
    pfd.fd = kq;
    if ((n = poll(&pfd, 1, ms)) <= 0)
        return n;
    return kev_now(NULL, 0, events, nevents);
}

static int kq_add(int fd)
{
    struct kev_freebsd ev; /* the largest */

    memset(&ev, 0, sizeof(ev));
    if (IsNetbsd()) {
        struct kev_netbsd *k = (struct kev_netbsd *)&ev;
        k->ident = fd;
        k->filter = KQ_FILTER_VNODE;
        k->flags = KQ_EV_ADD | KQ_EV_CLEAR;
        k->fflags = KQ_NOTES;
    } else {
        ev.k.ident = fd;
        ev.k.filter = KQ_FILTER_VNODE;
        ev.k.flags = KQ_EV_ADD | KQ_EV_CLEAR;
        ev.k.fflags = KQ_NOTES;
    }
    return kev_now(&ev, 1, NULL, 0);
}

static size_t path_hash(const char *s)
{
    size_t h = 2166136261u;
    while (*s)
        h = (h ^ (unsigned char)*s++) * 16777619u;
    return h;
}

/* The slot of a path, or the free slot for it. */
static struct slot *find_slot(const char *path)
{
    size_t i = path_hash(path) & (nslots - 1);
    while (slots[i].path && strcmp(slots[i].path, path) != 0)
        i = (i + 1) & (nslots - 1);
    return &slots[i];
}

static int grow_slots(void)
{
    struct slot *old = slots;
    size_t i, n = nslots;

    nslots = nslots ? nslots * 2 : 1024;
    if (!(slots = calloc(nslots, sizeof(*slots)))) {
        slots = old;
        nslots = n;
        return 0;
    }
    used_slots = 0;
    for (i = 0; i < n; i++) {
        if (old[i].path && old[i].fd >= 0) {
            struct slot *s = find_slot(old[i].path);
            *s = old[i];
            fd_slot[s->fd] = (int)(s - slots);
            used_slots++;
        } else {
            free(old[i].path);
        }
    }
    free(old);
    return 1;
}

static void kq_watch(const char *path)
{
    struct slot *s;
    int fd;

    if (kq_files >= kq_max_files)
        return;
    if ((used_slots + 1) * 2 > nslots && !grow_slots())
        return;
    s = find_slot(path);
    if (s->path && s->fd >= 0)
        return; /* watched */
    /* O_EVTONLY is better on macOS, but open() of Cosmopolitan cannot give
     * it (C30 in docs/UPSTREAM.md of BEAM.com). */
    if ((fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC)) < 0)
        return;
    if ((size_t)fd >= nfd_slot) {
        size_t n = nfd_slot ? nfd_slot : 1024;
        int *p;
        while (n <= (size_t)fd)
            n *= 2;
        if (!(p = realloc(fd_slot, n * sizeof(*p)))) {
            close(fd);
            return;
        }
        memset(p + nfd_slot, 0xff, (n - nfd_slot) * sizeof(*p));
        fd_slot = p;
        nfd_slot = n;
    }
    if (kq_add(fd) != 0) {
        close(fd);
        return;
    }
    if (!s->path) {
        if (!(s->path = strdup(path))) {
            close(fd);
            return;
        }
        used_slots++;
    }
    s->fd = fd;
    fd_slot[fd] = (int)(s - slots);
    kq_files++;
}

/* A removed or renamed file: close it, so its path can be watched again. */
static void kq_events(const char *evs, int n)
{
    int i;

    for (i = 0; i < n; i++) {
        const char *p = evs + i * kev_size();
        uintptr_t ident;
        uint32_t fflags;
        if (IsNetbsd()) {
            ident = ((const struct kev_netbsd *)p)->ident;
            fflags = ((const struct kev_netbsd *)p)->fflags;
        } else {
            ident = ((const struct kev *)p)->ident;
            fflags = ((const struct kev *)p)->fflags;
        }
        if ((fflags & (KQ_NOTE_DELETE | KQ_NOTE_RENAME)) && ident < nfd_slot && fd_slot[ident] >= 0) {
            struct slot *s = &slots[fd_slot[ident]];
            close(s->fd); /* this also removes it from the kqueue */
            fd_slot[ident] = -1;
            s->fd = -1;
            kq_files--;
        }
    }
}

/* --- Windows: ReadDirectoryChangesW ------------------------------------ */

/* Each watched directory has one handle and one read of its changes
 * (overlapped I/O). All the handles are on one I/O completion port, so
 * one call waits for all the directories, with a time limit and with
 * no limit of 64 handles (WaitForMultipleObjects()).
 *
 * The program does not use the names in the buffer of a read: a
 * completion says only that something changed, and the program compares
 * the files itself. So a small buffer is sufficient: when the changes do
 * not fit, Windows completes the read with no data (or with
 * ERROR_NOTIFY_ENUM_DIR), and that is a change too. Windows keeps the
 * changes between a completion and the next read on the handle.
 *
 * A directory of files (TW_DIR) has a watch of its entries only, as
 * with inotify. A source directory (TW_TREE) has a watch of its full
 * tree (bWatchSubtree): a new directory in it has events at once, and
 * the directories below it need no handle. This is important on
 * Windows: a directory with an open handle below it cannot get a new
 * name. A path below a watched tree gets no handle of its own, and a
 * new tree closes the handles below it.
 *
 * Cosmopolitan has no ReadDirectoryChangesW(): the address comes from
 * kernel32.dll, and the call has the calling convention of Microsoft.
 * Cosmopolitan runs on Windows only on x86_64.
 */
#ifdef __x86_64__

/* The values of winnt.h: FILE_NOTIFY_CHANGE_FILE_NAME, DIR_NAME, SIZE,
 * LAST_WRITE and CREATION. */
#define NT_NOTIFY (0x001u | 0x002u | 0x008u | 0x010u | 0x040u)
#define NT_BUFFER 4096 /* bytes; a network share accepts at most 64 KB */
#define NT_MAX_DIRS 2048

typedef int(__attribute__((__ms_abi__)) *nt_ReadDirectoryChangesW)(
    int64_t dir, void *buffer, uint32_t size, int subtree, uint32_t filter,
    uint32_t *bytes, struct NtOverlapped *overlapped, void *routine);

int __mkntpathat(int, const char *, int, char16_t[PATH_MAX]);

struct nt_dir {
    struct NtOverlapped ov; /* the first field: a completion gives its address */
    int64_t handle;
    int tree;    /* watch the full tree */
    int pending; /* a read is in progress: a completion will come */
    int dead;    /* closed: free it at its completion */
    char16_t *path;
    size_t len;
    uint32_t buffer[NT_BUFFER / sizeof(uint32_t)]; /* aligned for DWORD */
};

static int64_t nt_port;
static nt_ReadDirectoryChangesW nt_read;
static struct nt_dir **nt_dirs;
static size_t nt_ndirs, nt_cap;

static int nt_start(void)
{
    int64_t kernel32;

    if (nt_port)
        return 1;
    if (!(kernel32 = LoadLibrary(u"kernel32.dll")))
        return 0;
    if (!(nt_read = (nt_ReadDirectoryChangesW)GetProcAddress(kernel32, "ReadDirectoryChangesW")))
        return 0;
    /* A new port, with no handle. A failure gives 0. */
    nt_port = CreateIoCompletionPort(kNtInvalidHandleValue, 0, 0, 1);
    return nt_port != 0;
}

/* Start the read of the changes of a directory. Windows queues one
 * completion for each read that starts. */
static int nt_arm(struct nt_dir *d)
{
    memset(&d->ov, 0, sizeof(d->ov));
    if (nt_read(d->handle, d->buffer, sizeof(d->buffer), d->tree, NT_NOTIFY, NULL, &d->ov, NULL) ||
        GetLastError() == kNtErrorIoPending) {
        d->pending = 1;
        return 1;
    }
    return 0;
}

static void nt_free(struct nt_dir *d)
{
    free(d->path);
    free(d);
}

/* Close the directory at an index. CloseHandle() completes the read in
 * progress: the memory of the read stays until that completion. */
static void nt_close(size_t i)
{
    struct nt_dir *d = nt_dirs[i];

    nt_dirs[i] = nt_dirs[--nt_ndirs];
    CloseHandle(d->handle);
    d->handle = 0;
    d->dead = 1;
    if (!d->pending)
        nt_free(d);
}

static char16_t nt_lower(char16_t c)
{
    return c >= u'A' && c <= u'Z' ? c + (u'a' - u'A') : c;
}

/* 1: the two paths are equal. 2: the path p is below the directory dir.
 * 0: neither. The comparison ignores the case of the ASCII letters. */
static int nt_below(const char16_t *dir, size_t ndir, const char16_t *p, size_t np)
{
    size_t i;

    if (np < ndir)
        return 0;
    for (i = 0; i < ndir; i++)
        if (nt_lower(dir[i]) != nt_lower(p[i]))
            return 0;
    if (np == ndir)
        return 1;
    /* A drive root (C:\) ends with the separator. */
    return p[ndir] == u'\\' || (ndir > 0 && dir[ndir - 1] == u'\\') ? 2 : 0;
}

static void nt_watch(const char *path, int kind)
{
    char16_t p[PATH_MAX];
    struct nt_dir *d;
    size_t i;
    int n, r;

    if (kind == TW_FILE)
        return;
    /* The path of Cosmopolitan (/c/dir or c:/dir, UTF-8) as a path of
     * Windows (c:\dir, UTF-16). */
    if ((n = __mkntpathat(AT_FDCWD, path, 0, p)) <= 0)
        return;
    while (n > 1 && p[n - 1] == u'\\' && p[n - 2] != u':')
        p[--n] = 0;
    for (i = 0; i < nt_ndirs; i++) {
        r = nt_below(nt_dirs[i]->path, nt_dirs[i]->len, p, (size_t)n);
        if (r && nt_dirs[i]->tree)
            return; /* in a watched tree */
        if (r == 1 && kind == TW_DIR)
            return; /* watched */
    }
    if (nt_ndirs >= NT_MAX_DIRS)
        return;
    if (nt_ndirs == nt_cap) {
        size_t cap = nt_cap ? nt_cap * 2 : 64;
        struct nt_dir **q = realloc(nt_dirs, cap * sizeof(*q));
        if (!q)
            return;
        nt_dirs = q;
        nt_cap = cap;
    }
    if (!(d = calloc(1, sizeof(*d))))
        return;
    if (!(d->path = malloc(((size_t)n + 1) * sizeof(char16_t)))) {
        free(d);
        return;
    }
    memcpy(d->path, p, ((size_t)n + 1) * sizeof(char16_t));
    d->len = (size_t)n;
    d->tree = kind == TW_TREE;
    /* FILE_FLAG_BACKUP_SEMANTICS opens a directory. All the share modes:
     * other programs can change, rename and delete the directory. */
    d->handle = CreateFile(p, kNtFileListDirectory,
                           kNtFileShareRead | kNtFileShareWrite | kNtFileShareDelete, NULL,
                           kNtOpenExisting, kNtFileFlagBackupSemantics | kNtFileFlagOverlapped, 0);
    if (d->handle == kNtInvalidHandleValue || d->handle == 0) {
        nt_free(d);
        return;
    }
    if (!CreateIoCompletionPort(d->handle, nt_port, 0, 0) || !nt_arm(d)) {
        /* No read started, so no completion comes. */
        CloseHandle(d->handle);
        nt_free(d);
        return;
    }
    /* A new tree: the handles below it are not necessary. */
    if (d->tree) {
        for (i = nt_ndirs; i-- > 0;)
            if (nt_below(d->path, d->len, nt_dirs[i]->path, nt_dirs[i]->len))
                nt_close(i);
    }
    nt_dirs[nt_ndirs++] = d;
}

/* One wait on the port. Gives 1 for a change, 0 for the end of the
 * time, -1 when no directory is watched or the port fails. */
static int nt_wait(int ms)
{
    struct timespec t0, t1;
    int left = ms;

    clock_gettime(CLOCK_MONOTONIC, &t0);
    for (;;) {
        struct NtOverlapped *ov = NULL;
        struct nt_dir *d;
        uint32_t bytes = 0, error = 0;
        uint64_t key = 0;
        size_t i;
        int ok;

        if (!nt_ndirs)
            return -1;
        ok = GetQueuedCompletionStatus(nt_port, &bytes, &key, &ov, left > 0 ? left : 0);
        if (!ok)
            error = GetLastError();
        if (!ov)
            return !ok && error == kNtWaitTimeout ? 0 : -1;
        d = (struct nt_dir *)ov;
        d->pending = 0;
        if (d->dead) {
            /* The last completion of a closed directory: not a change. */
            nt_free(d);
            clock_gettime(CLOCK_MONOTONIC, &t1);
            left = ms - (int)((t1.tv_sec - t0.tv_sec) * 1000 + (t1.tv_nsec - t0.tv_nsec) / 1000000);
            continue;
        }
        /* A completion with data, or with no data (the changes did not
         * fit in the buffer): read again. A completion with an error (the
         * directory is deleted, or its volume is gone), or a read that
         * does not start: close the directory. This is a change too, and
         * the program adds the directory again when it is there. */
        if ((ok || error == kNtErrorNotifyEnumDir) && nt_arm(d))
            return 1;
        for (i = 0; i < nt_ndirs; i++) {
            if (nt_dirs[i] == d) {
                nt_close(i);
                break;
            }
        }
        return 1;
    }
}

#else /* Windows is x86_64 only. */

static int nt_start(void)
{
    return 0;
}

#endif

#else /* TW_WATCH_INOTIFY_ONLY */

static int kq = -1;

#endif

static int nt_on;

/* --- The interface ----------------------------------------------------- */

/* Start the events. Gives 1 when this system has events, else 0. */
int tw_watch_start(void)
{
    if (IsLinux()) {
        if (ifd < 0)
            ifd = (int)linux_syscall(NR_INOTIFY_INIT1, W_NONBLOCK | W_CLOEXEC, 0, 0);
        return ifd >= 0;
    }
#ifndef TW_WATCH_INOTIFY_ONLY
    if (IsXnu() || IsFreebsd() || IsNetbsd() || IsOpenbsd()) {
        struct rlimit rl;

        if (kq < 0 && (kq = sys_kqueue()) >= 0) {
            kq_max_files = KQ_MAX_FILES;
            if (getrlimit(RLIMIT_NOFILE, &rl) == 0 && rl.rlim_cur / 2 < (rlim_t)kq_max_files)
                kq_max_files = rl.rlim_cur / 2;
        }
        return kq >= 0;
    }
    if (IsWindows())
        return nt_on = nt_start();
#endif
    return 0;
}

/* The name of the events of this system, after tw_watch_start() gave 1. */
const char *tw_watch_name(void)
{
    return ifd >= 0 ? "inotify" : kq >= 0 ? "kqueue" : nt_on ? "windows" : "none";
}

/* Watch a file (kind 0), a directory of files (kind 1), or a source
 * directory with all that is below it (kind 2). A path that is watched
 * stays watched. The program gives the directories first: when the
 * descriptors of kqueue are all in use, the files have no event, and
 * the interval of the program finds their changes. inotify needs the
 * directories of the files only. Only Windows uses kind 2: the program
 * gives these paths first, and the other systems ignore them. */
void tw_watch_add(const char *path, int kind)
{
    if (ifd >= 0) {
        if (kind == TW_DIR)
            linux_syscall(NR_INOTIFY_ADD_WATCH, ifd, (long)path, W_EVENTS);
    }
#ifndef TW_WATCH_INOTIFY_ONLY
    else if (kq >= 0) {
        if (kind != TW_TREE)
            kq_watch(path);
    }
#ifdef __x86_64__
    else if (nt_on) {
        nt_watch(path, kind);
    }
#endif
#endif
}

/* One wait. Gives 1 for an event, 0 for the end of the time, -1 for an
 * error. */
static int wait_once(int ms)
{
    if (ifd >= 0) {
        char buf[16384];
        struct pollfd pfd = {ifd, W_POLLIN, 0};
        int n = poll(&pfd, 1, ms);
        if (n <= 0)
            return n < 0 && errno != EINTR ? -1 : 0;
        while (read(ifd, buf, sizeof(buf)) > 0) {
        }
        return 1;
    }
#ifndef TW_WATCH_INOTIFY_ONLY
    if (kq >= 0) {
        char evs[KQ_MAX_EVENTS * sizeof(struct kev_freebsd)];
        int n = kev_wait(evs, KQ_MAX_EVENTS, ms);
        if (n < 0)
            return errno == EINTR ? 0 : -1;
        kq_events(evs, n);
        return n > 0;
    }
#ifdef __x86_64__
    if (nt_on)
        return nt_wait(ms);
#endif
#endif
    return -1;
}

/* Wait at most ms milliseconds for a change. After an event, wait until
 * there are no more events for 20 ms (at most 0.2 s): a program often
 * writes a file in more than one step. Gives 1 for a change, 0 for the
 * end of the time, and -1 when the events do not work: the program then
 * polls. */
int tw_watch_wait(int ms)
{
    int i, n = wait_once(ms);

    if (n <= 0)
        return n;
    for (i = 0; i < 10 && wait_once(20) > 0; i++) {
    }
    return 1;
}
