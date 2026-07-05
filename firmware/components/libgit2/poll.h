/*
 * poll.h shim for the libgit2 esp-idf component (Spike 7, Path 2).
 *
 * libgit2's posix.h does `#include <poll.h>` when GIT_IO_POLL is set, but
 * esp-idf's newlib ships poll() under <sys/poll.h> only — there is no
 * top-level <poll.h>. This forwarding header (on the component's private
 * include path) bridges the gap without touching libgit2's sources.
 */
#ifndef LIBGIT2_ESPIDF_POLL_SHIM_H
#define LIBGIT2_ESPIDF_POLL_SHIM_H
#include <sys/poll.h>
#endif
