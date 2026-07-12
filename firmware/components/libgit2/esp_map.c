/*
 * esp_map.c — p_mmap/p_munmap for libgit2 on esp-idf (Spike 7, Path 2).
 *
 * Replaces src/util/unix/map.c, which needs <sys/mman.h> (absent on
 * picolibc/esp-idf). libgit2 uses p_mmap read-only, to view pack files and the
 * index. We emulate it by allocating a buffer and reading the range into it.
 * Allocations go through git__malloc, so with PSRAM in the heap they land in
 * the 8 MB external RAM rather than the ~340 KB internal DRAM.
 *
 * CACHE (2026-07-12): the emulation reads the range from SD on every call, so
 * libgit2's repeated pack access — `git_odb_write` → `git_odb__freshen` →
 * `git_odb_refresh` re-reads the pack idx/windows on *every* object write —
 * turned a 45 ms object write into 500 ms–1.1 s on a small repo, and would be far
 * worse on the real 570 MB-pack clone (see
 * docs/tradeoff-curves/sync-commit-staging.md). We cache the read buffers so a
 * given file region is read from the card once and reused.
 *
 * Correctness: cache ONLY read-only mappings of files >= ESP_MAP_CACHE_MIN_FILE
 * (the FILE size, not the map length — 2026-07-12b). The hot set turned out to
 * be the SMALL maps: pack trailer probes, idx fanout reads and delta-base
 * windows repeat at the same offsets on every freshen/refresh, and a map-length
 * floor excluded exactly those (0 cache hits over three full real-repo bench
 * runs). Keying on file size caches them while still excluding the small
 * mutable working-tree files diff_file.c maps (notes.md etc.) — only the pack,
 * its idx, a midx and the commit-graph are ever that large, and all are
 * immutable on this device (only loose objects/refs/index are written, none
 * via mmap). The writable mapping the pack indexer uses (fetch/clone) is
 * excluded by the prot check. Identity is (dev, ino, size, mtime, offset, len);
 * for an immutable pack any of size/mtime already differs if the file is ever
 * replaced. NOTE: on esp-idf's FAT VFS st_dev/st_ino are constant and mtime has
 * 2 s granularity, so identity is effectively (size, mtime₂ₛ, offset, len) —
 * fine for packs (a replaced pack changes size), weak for small mutable files,
 * which is exactly why those must never be cacheable.
 *
 * Memory discipline (2026-07-12b): the first version freed released buffers
 * only lazily on the next p_mmap, so a burst (read_tree of 158 trees) pinned
 * 7.4 MB and starved zlib's git__malloc (crash at 508 KB heap) — it defeated
 * GIT_OPT_SET_MWINDOW_MAPPED_LIMIT because libgit2 believed the memory was
 * returned. Now p_munmap evicts unreferenced entries down to
 * ESP_MAP_CACHE_LOW_WATER, so a released window's memory really is returned
 * under pressure while a warm working set stays resident.
 *
 * Limitation: writable/shared mappings are not written back (and not cached).
 */

#include "git2_util.h"
#include "map.h"

#include <unistd.h>
#include <string.h>
#include <errno.h>
#include <sys/stat.h>
#include <stdint.h>

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

/* Only cache mappings of files at least this large (FILE size, not map length):
 * covers the pack, its idx, midx and commit-graph — and thus the hot small
 * windows within them — while excluding the small mutable working-tree files
 * diff_file.c maps. */
#define ESP_MAP_CACHE_MIN_FILE (1024 * 1024)
/* Cached-buffer budget in PSRAM. Entries with no live ref are LRU-evicted to
 * stay under this; pinned entries may exceed it transiently. */
#define ESP_MAP_CACHE_CAP (4 * 1024 * 1024)
/* On p_munmap, unreferenced entries are evicted down to this — the resident
 * working set a burst leaves behind. Must leave git__malloc (zlib, mwindow)
 * ample heap; the 2026-07-12 OOM run crashed with 7.4 MB resident. */
#define ESP_MAP_CACHE_LOW_WATER (2 * 1024 * 1024)
/* Small maps are numerous (trailer/fanout/delta-base probes), so more slots
 * than the window-only design needed; a slot is ~40 B. */
#define ESP_MAP_CACHE_SLOTS 48

struct map_entry {
	unsigned char *data;
	size_t len;
	off64_t offset;
	dev_t dev;
	ino_t ino;
	off_t size;
	time_t mtime;
	int refcount;
	uint32_t used; /* LRU stamp; 0 = empty slot */
};

static struct map_entry g_cache[ESP_MAP_CACHE_SLOTS];
static uint32_t g_clock;
static size_t g_cached_bytes;

/* Diagnostics, read from the bench via esp_map_stats(). */
static uint32_t g_hits, g_misses;
static uint64_t g_read_bytes;

void esp_map_stats(uint32_t *hits, uint32_t *misses, uint32_t *read_kb, uint32_t *cached_kb)
{
	if (hits) *hits = g_hits;
	if (misses) *misses = g_misses;
	if (read_kb) *read_kb = (uint32_t)(g_read_bytes / 1024);
	if (cached_kb) *cached_kb = (uint32_t)(g_cached_bytes / 1024);
}

static int read_range(int fd, off64_t offset, size_t len, unsigned char *data)
{
	size_t got = 0;

	if (lseek(fd, offset, SEEK_SET) < 0) {
		git_error_set(GIT_ERROR_OS, "failed to seek for mmap emulation");
		return -1;
	}
	while (got < len) {
		ssize_t n = read(fd, data + got, len - got);
		if (n < 0) {
			if (errno == EINTR)
				continue;
			git_error_set(GIT_ERROR_OS, "failed to read for mmap emulation");
			return -1;
		}
		if (n == 0)
			break; /* short file: zero-fill the tail, like a real mapping */
		got += (size_t)n;
	}
	if (got < len)
		memset(data + got, 0, len - got);
	return 0;
}

/* Best-effort: evict unreferenced LRU entries until total cached bytes fit
 * `budget`. Pinned entries (refcount > 0) can't be evicted, so budgets are
 * soft. */
static void evict_to(size_t budget)
{
	while (g_cached_bytes > budget) {
		int victim = -1;
		uint32_t oldest = 0xFFFFFFFFu;
		int i;

		for (i = 0; i < ESP_MAP_CACHE_SLOTS; i++) {
			if (g_cache[i].used && g_cache[i].refcount == 0 &&
			    g_cache[i].used < oldest) {
				oldest = g_cache[i].used;
				victim = i;
			}
		}
		if (victim < 0)
			break;
		git__free(g_cache[victim].data);
		g_cached_bytes -= g_cache[victim].len;
		memset(&g_cache[victim], 0, sizeof(g_cache[victim]));
	}
}

int p_mmap(git_map *out, size_t len, int prot, int flags, int fd, off64_t offset)
{
	unsigned char *data;
	struct stat st;
	int cacheable, i;

	GIT_UNUSED(flags);
	GIT_MMAP_VALIDATE(out, len, prot, flags);

	out->data = NULL;
	out->len = 0;

	/* Cache only read-only mappings of large files (pack/idx/...) that we can
	 * identify. The map itself may be small — small repeated maps ARE the hot
	 * set (see header comment). */
	cacheable = !(prot & GIT_PROT_WRITE);
	if (cacheable && (fstat(fd, &st) != 0 || st.st_size < ESP_MAP_CACHE_MIN_FILE))
		cacheable = 0;

	if (cacheable) {
		for (i = 0; i < ESP_MAP_CACHE_SLOTS; i++) {
			struct map_entry *e = &g_cache[i];
			if (e->used && e->len == len && e->offset == offset &&
			    e->dev == st.st_dev && e->ino == st.st_ino &&
			    e->size == st.st_size && e->mtime == st.st_mtime) {
				e->refcount++;
				e->used = ++g_clock;
				g_hits++;
				out->data = e->data;
				out->len = len;
				return 0;
			}
		}
	}

	data = git__malloc(len);
	GIT_ERROR_CHECK_ALLOC(data);
	if (read_range(fd, offset, len, data) < 0) {
		git__free(data);
		return -1;
	}
	g_misses++;
	g_read_bytes += len;

	if (cacheable) {
		if (len < ESP_MAP_CACHE_CAP)
			evict_to(ESP_MAP_CACHE_CAP - len);
		for (i = 0; i < ESP_MAP_CACHE_SLOTS; i++) {
			if (!g_cache[i].used) {
				g_cache[i].data = data;
				g_cache[i].len = len;
				g_cache[i].offset = offset;
				g_cache[i].dev = st.st_dev;
				g_cache[i].ino = st.st_ino;
				g_cache[i].size = st.st_size;
				g_cache[i].mtime = st.st_mtime;
				g_cache[i].refcount = 1;
				g_cache[i].used = ++g_clock;
				g_cached_bytes += len;
				out->data = data;
				out->len = len;
				return 0;
			}
		}
		/* All slots pinned: return uncached (freed directly at munmap). */
	}

	out->data = data;
	out->len = len;
	return 0;
}

int p_munmap(git_map *map)
{
	int i;

	GIT_ASSERT_ARG(map);

	/* Cached buffer: drop a ref and keep it for reuse — but return memory to
	 * git__malloc under pressure (down to the low-water mark), so a released
	 * window is really released and MWINDOW_MAPPED_LIMIT stays honest. */
	for (i = 0; i < ESP_MAP_CACHE_SLOTS; i++) {
		if (g_cache[i].used && g_cache[i].data == map->data) {
			if (g_cache[i].refcount > 0)
				g_cache[i].refcount--;
			if (g_cache[i].refcount == 0 &&
			    g_cached_bytes > ESP_MAP_CACHE_LOW_WATER)
				evict_to(ESP_MAP_CACHE_LOW_WATER);
			map->data = NULL;
			map->len = 0;
			return 0;
		}
	}
	git__free(map->data);
	map->data = NULL;
	map->len = 0;
	return 0;
}
