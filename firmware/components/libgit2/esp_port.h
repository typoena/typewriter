/*
 * esp_port.h — platform shims for building libgit2 on esp-idf, force-included
 * into every libgit2 TU via `-include` (Spike 7, Path 2).
 *
 * esp-idf's C library is picolibc and its filesystem layer is the VFS, which
 * has no symlink concept. So POSIX calls libgit2 assumes on "unix" are either
 * absent or degenerate here. Shimming them in a force-included header keeps
 * libgit2's own sources untouched (important: we don't want to fork 1.9.4).
 */
#ifndef LIBGIT2_ESPIDF_PORT_H
#define LIBGIT2_ESPIDF_PORT_H

#include <sys/stat.h>

/* No symlinks on FAT/LittleFS: lstat is just stat. */
#ifndef lstat
#define lstat stat
#endif

#endif
