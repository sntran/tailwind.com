/* The entry of tailwind.com: the program is in the Rust library. */
#include <cosmo.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include "libc/nt/dll.h"     /* GetModuleHandle(), GetProcAddress() */
#include "libc/nt/runtime.h" /* ExitProcess() */
#include "libc/nt/thunk/msabi.h"
#include "libc/runtime/runtime.h" /* __get_tmpdir() */

int tailwind_main(int argc, char **argv);

#ifdef __x86_64__
/* Windows on ARM64. The PE header of the file is for x86_64, so Windows
 * starts this x86_64 half, in its x64 emulation. The libc of the
 * aarch64 half has a Windows layer (the cosmocc of sntran/cosmopolitan,
 * C58 in docs/UPSTREAM.md of BEAM.com), and the zip of the file has its
 * loader, ape-arm64.exe. The loader runs the aarch64 half as native
 * code.
 *
 * This function writes the loader to the directory of the temporary
 * files, as .ape-arm64-HASH.exe (HASH: the FNV-1a hash of the loader).
 * Then this process becomes the loader (execv), with the path of this
 * file and the arguments. execv() of Cosmopolitan gives the status of
 * the loader to the parent process. Each start compares the file of the loader
 * with the loader of the zip, and writes a file that is different
 * again: a changed file does not run.
 *
 * The function returns when the system is not Windows on ARM64, and on
 * an error. Then the x86_64 half continues in the x64 emulation. */
#define LOADER "/zip/ape-arm64.exe"
#define MACHINE_ARM64 0xAA64 /* IMAGE_FILE_MACHINE_ARM64 */

typedef int(__msabi *IsWow64Process2_f)(int64_t process, uint16_t *process_machine,
                                        uint16_t *native_machine);

/* The contents of a file of less than 1 MiB, or NULL. */
static char *read_file(const char *path, size_t *size)
{
    struct stat st;
    char *data = NULL;
    int fd = open(path, O_RDONLY);

    if (fd < 0)
        return NULL;
    if (fstat(fd, &st) == 0 && st.st_size > 0 && st.st_size < (1 << 20) &&
        (data = malloc(st.st_size)) && read(fd, data, st.st_size) != st.st_size) {
        free(data);
        data = NULL;
    }
    close(fd);
    if (data)
        *size = st.st_size;
    return data;
}

/* 1 when the file at path has the contents data. */
static int same_file(const char *path, const char *data, size_t size)
{
    size_t n = 0;
    char *old = read_file(path, &n);
    int same = old && n == size && memcmp(old, data, size) == 0;

    free(old);
    return same;
}

static void start_native_arm64(int argc, char **argv)
{
    IsWow64Process2_f is_wow64;
    uint16_t process_machine = 0, native_machine = 0;
    uint64_t hash = 14695981039346656037ull;
    char *loader, *self, **args, path[PATH_MAX], tmp[PATH_MAX + 32];
    size_t size, i;
    int fd, ok;

    /* -1: the pseudo handle of this process (GetCurrentProcess()). */
    is_wow64 = (IsWow64Process2_f)GetProcAddress(GetModuleHandle("kernel32.dll"), "IsWow64Process2");
    if (!is_wow64 || !is_wow64(-1, &process_machine, &native_machine) ||
        native_machine != MACHINE_ARM64)
        return;
    if (!(loader = read_file(LOADER, &size)))
        return;
    for (i = 0; i < size; i++)
        hash = (hash ^ (unsigned char)loader[i]) * 1099511628211ull;
    snprintf(path, sizeof(path), "%s.ape-arm64-%016llx.exe", __get_tmpdir(), (unsigned long long)hash);
    if (!same_file(path, loader, size)) {
        /* A new file, then its name. Another start can do the same at
         * the same time: when the name does not change, the file of the
         * other start can be the loader. */
        snprintf(tmp, sizeof(tmp), "%s.%d.tmp", path, (int)getpid());
        fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0755);
        ok = fd >= 0 && write(fd, loader, size) == (ssize_t)size;
        if (fd >= 0 && close(fd) != 0)
            ok = 0;
        if (!ok || rename(tmp, path) != 0) {
            unlink(tmp);
            if (!same_file(path, loader, size)) {
                free(loader);
                return;
            }
        }
    }
    free(loader);
    /* The path of this file for Windows: /C/dir/file is C:/dir/file. */
    if (!(self = strdup(GetProgramExecutableName())) || !(args = calloc(argc + 2, sizeof(char *)))) {
        free(self);
        return;
    }
    if (self[0] == '/' && self[1] && self[2] == '/') {
        self[0] = self[1];
        self[1] = ':';
    }
    args[0] = path;
    args[1] = self;
    for (i = 1; i < (size_t)argc; i++)
        args[i + 1] = argv[i];
    fflush(NULL);
    execv(path, args);
    free(args);
    free(self);
}
#endif

int main(int argc, char **argv)
{
#ifdef __x86_64__
    if (IsWindows())
        start_native_arm64(argc, argv);
#endif

    int status = tailwind_main(argc, argv);

    /* On Windows, exit() of Cosmopolitan gives the system the POSIX wait
     * status (status << 8), so exit(1) is 256 for cmd.exe and PowerShell,
     * and 0 for a shell that keeps 8 bits. Mix and other programs of
     * Windows start this file, so the convention of Windows wins: give the
     * plain status. This is C21 in docs/UPSTREAM.md of BEAM.com. */
    if (IsWindows()) {
        fflush(NULL);
        ExitProcess(status);
    }
    return status;
}
