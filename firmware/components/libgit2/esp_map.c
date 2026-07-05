/*
 * esp_map.c — p_mmap/p_munmap for libgit2 on esp-idf (Spike 7, Path 2).
 *
 * Replaces src/util/unix/map.c, which needs <sys/mman.h> (absent on
 * picolibc/esp-idf). libgit2 uses p_mmap read-only, to view pack files and the
 * index. We emulate it by allocating a buffer and reading the range into it.
 * Allocations go through git__malloc, so with PSRAM in the heap they land in
 * the 8 MB external RAM rather than the ~340 KB internal DRAM.
 *
 * Limitation: writable/shared mappings are not written back (libgit2 does not
 * mmap for writing in the paths we exercise). If that ever changes it will
 * surface at runtime, not here.
 */

#include "git2_util.h"
#include "map.h"

#include <unistd.h>
#include <string.h>
#include <errno.h>

int git__page_size(size_t *page_size)
{
	*page_size = 4096;
	return 0;
}

int git__mmap_alignment(size_t *alignment)
{
	*alignment = 4096;
	return 0;
}

int p_mmap(git_map *out, size_t len, int prot, int flags, int fd, off64_t offset)
{
	unsigned char *data;
	size_t got = 0;

	GIT_UNUSED(prot);
	GIT_UNUSED(flags);
	GIT_MMAP_VALIDATE(out, len, prot, flags);

	out->data = NULL;
	out->len = 0;

	data = git__malloc(len);
	GIT_ERROR_CHECK_ALLOC(data);

	if (lseek(fd, offset, SEEK_SET) < 0) {
		git_error_set(GIT_ERROR_OS, "failed to seek for mmap emulation");
		git__free(data);
		return -1;
	}

	while (got < len) {
		ssize_t n = read(fd, data + got, len - got);
		if (n < 0) {
			if (errno == EINTR)
				continue;
			git_error_set(GIT_ERROR_OS, "failed to read for mmap emulation");
			git__free(data);
			return -1;
		}
		if (n == 0)
			break; /* short file: zero-fill the tail, like a real mapping */
		got += (size_t)n;
	}

	if (got < len)
		memset(data + got, 0, len - got);

	out->data = data;
	out->len = len;
	return 0;
}

int p_munmap(git_map *map)
{
	GIT_ASSERT_ARG(map);
	git__free(map->data);
	map->data = NULL;
	map->len = 0;
	return 0;
}
