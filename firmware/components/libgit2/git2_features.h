/*
 * git2_features.h — hand-written feature config for building libgit2 as an
 * ESP-IDF component (Spike 7, Path 2).
 *
 * libgit2's CMake normally generates this from git2_features.h.in. We write it
 * by hand so the build is driven by ESP-IDF's component CMake instead. The one
 * substantive choice vs. the libgit2-sys vendored build: TLS + hashing use
 * *mbedTLS* (which ESP-IDF already ships and uses for its own TLS), not
 * OpenSSL/SecureTransport. That backend exists in the C library but the
 * libgit2-sys Rust wrapper never exposes it — the reason for Path 2.
 */
#ifndef INCLUDE_features_h__
#define INCLUDE_features_h__

#define GIT_THREADS 1
#define GIT_TRACE 1
#define GIT_ARCH_32 1

/* Bundled backends: no system deps to hunt for on the device. */
#define GIT_REGEX_BUILTIN 1
#define GIT_HTTPPARSER_BUILTIN 1
#define GIT_COMPRESSION_BUILTIN 1

/* TLS via ESP-IDF's mbedTLS (streams/mbedtls.c). */
#define GIT_HTTPS 1
#define GIT_MBEDTLS 1

/* Hashing via mbedTLS too (util/hash/mbedtls.c) — one crypto backend, and it
 * lets us skip the sha1dc collision-detection sources. */
#define GIT_SHA1_MBEDTLS 1
#define GIT_SHA256_MBEDTLS 1

/* Socket readiness via poll()/select() (both provided by lwip + vfs). */
#define GIT_IO_POLL 1
#define GIT_IO_SELECT 1

#endif
