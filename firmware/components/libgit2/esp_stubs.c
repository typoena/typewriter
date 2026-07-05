/*
 * esp_stubs.c — POSIX identity/symlink stubs for libgit2 on esp-idf
 * (Spike 7, Path 2).
 *
 * picolibc *declares* these in <unistd.h>/<pwd.h> but does not implement them:
 * esp-idf has no users, groups, processes, or symlinks. libgit2 calls them
 * while resolving config paths, ownership checks, and temp names. We provide
 * definitions that model "a single root user, no user database, no symlinks",
 * which is the truthful shape of the device's flat filesystem.
 */

#include <sys/types.h>
#include <unistd.h>
#include <pwd.h>
#include <errno.h>
#include <sys/time.h>

/* One implicit root user/group. */
uid_t getuid(void)  { return 0; }
uid_t geteuid(void) { return 0; }
gid_t getgid(void)  { return 0; }
gid_t getegid(void) { return 0; }

/* No process hierarchy. */
pid_t getppid(void)      { return 0; }
pid_t getpgid(pid_t pid) { (void)pid; return 0; }
pid_t getsid(pid_t pid)  { (void)pid; return 0; }

/* No user database: report "no such user" so libgit2 falls back to $HOME. */
int getpwuid_r(uid_t uid, struct passwd *pwd, char *buf, size_t buflen,
               struct passwd **result)
{
	(void)uid;
	(void)pwd;
	(void)buf;
	(void)buflen;
	*result = NULL;
	return 0;
}

/* No symlinks on FAT/LittleFS: nothing is ever a symbolic link. */
ssize_t readlink(const char *path, char *buf, size_t bufsiz)
{
	(void)path;
	(void)buf;
	(void)bufsiz;
	errno = EINVAL; /* POSIX: EINVAL == "named file is not a symbolic link" */
	return -1;
}

/* VFS has no utimes(); accept and ignore (file mtime is cosmetic here). */
int utimes(const char *path, const struct timeval times[2])
{
	(void)path;
	(void)times;
	return 0;
}
