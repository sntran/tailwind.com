/* The entry of tailwind.com: the program is in the Rust library. */
#include <cosmo.h>
#include <stdio.h>
#include "libc/nt/runtime.h" /* ExitProcess() */

int tailwind_main(int argc, char **argv);

int main(int argc, char **argv)
{
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
