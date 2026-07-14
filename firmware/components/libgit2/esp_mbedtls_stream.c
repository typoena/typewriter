/*
 * Copyright (C) the libgit2 contributors. All rights reserved.
 *
 * This file is part of libgit2, distributed under the GNU GPL v2 with
 * a Linking Exception. For full terms see the included COPYING file.
 */

/*
 * esp_mbedtls_stream.c — verbatim copy of the vendored
 * src/libgit2/streams/mbedtls.c (v1.9.4) with ONE fix; it replaces the
 * vendored file in CMakeLists.txt (same pattern as esp_map.c).
 *
 * THE FIX (2026-07-13): mbedtls_stream_wrap()'s `out_err` path closed and
 * freed `st->io` — the caller's socket stream — but every caller frees that
 * stream on error too (git_mbedtls_stream_new does close+free right after),
 * and wrap's OTHER error paths (calloc/strdup failures) do NOT free it, so
 * the caller cannot compensate either way. When mbedtls_ssl_setup failed on
 * the device (internal-RAM exhaustion during the first real-repo push), the
 * double git__free tripped tlsf ("block already marked as free") and reset
 * the chip instead of surfacing a clean error. Delta from vendor: the
 * `out_err` label no longer touches st->io — on error, ownership of `in`
 * stays with the caller, consistently — and it frees the git__malloc'd
 * st->ssl struct the vendored path leaked.
 *
 * SECOND DELTA (2026-07-14): TLS session resumption. Every git operation
 * (push, fetch, ls-refs — and a rejected push's reconcile+retry runs three)
 * opens a fresh HTTPS connection, and a full handshake on this 160 MHz core
 * costs seconds each time. libgit2 gives the stream no connection reuse, but
 * mbedTLS can resume: cache the session after a successful handshake
 * (mbedtls_ssl_get_session at stream close) and offer it on the next connect
 * to the same host (mbedtls_ssl_set_session before the handshake) — the
 * server then skips the certificate exchange and most of the key exchange.
 * Single git thread, so plain statics like the rest of this file. Best
 * effort: any failure just falls back to a full handshake.
 *
 * Keep this file in lockstep with the vendored one on submodule bumps (diff
 * against it; the deltas must stay these two hunks).
 */

#include "streams/mbedtls.h"

#ifdef GIT_MBEDTLS

#include <ctype.h>

#include "runtime.h"
#include "stream.h"
#include "streams/socket.h"
#include "git2/transport.h"
#include "util.h"

#ifndef GIT_DEFAULT_CERT_LOCATION
#define GIT_DEFAULT_CERT_LOCATION NULL
#endif

/* Work around C90-conformance issues */
#if !defined(__STDC_VERSION__) || (__STDC_VERSION__ < 199901L)
# if defined(_MSC_VER)
#  define inline __inline
# elif defined(__GNUC__)
#  define inline __inline__
# else
#  define inline
# endif
#endif

#include <mbedtls/ssl.h>
#include <mbedtls/error.h>
#include <mbedtls/entropy.h>
#include <mbedtls/ctr_drbg.h>

#undef inline

#define GIT_SSL_DEFAULT_CIPHERS "TLS1-3-AES-128-GCM-SHA256:TLS1-3-AES-256-GCM-SHA384:TLS1-3-CHACHA20-POLY1305-SHA256:TLS-ECDHE-ECDSA-WITH-AES-128-GCM-SHA256:TLS-ECDHE-RSA-WITH-AES-128-GCM-SHA256:TLS-ECDHE-ECDSA-WITH-AES-256-GCM-SHA384:TLS-ECDHE-RSA-WITH-AES-256-GCM-SHA384:TLS-ECDHE-ECDSA-WITH-CHACHA20-POLY1305-SHA256:TLS-ECDHE-RSA-WITH-CHACHA20-POLY1305-SHA256:TLS-DHE-RSA-WITH-AES-128-GCM-SHA256:TLS-DHE-RSA-WITH-AES-256-GCM-SHA384:TLS-DHE-RSA-WITH-CHACHA20-POLY1305-SHA256:TLS-ECDHE-ECDSA-WITH-AES-128-CBC-SHA256:TLS-ECDHE-RSA-WITH-AES-128-CBC-SHA256:TLS-ECDHE-ECDSA-WITH-AES-128-CBC-SHA:TLS-ECDHE-RSA-WITH-AES-128-CBC-SHA:TLS-ECDHE-ECDSA-WITH-AES-256-CBC-SHA384:TLS-ECDHE-RSA-WITH-AES-256-CBC-SHA384:TLS-ECDHE-ECDSA-WITH-AES-256-CBC-SHA:TLS-ECDHE-RSA-WITH-AES-256-CBC-SHA:TLS-DHE-RSA-WITH-AES-128-CBC-SHA256:TLS-DHE-RSA-WITH-AES-256-CBC-SHA256:TLS-RSA-WITH-AES-128-GCM-SHA256:TLS-RSA-WITH-AES-256-GCM-SHA384:TLS-RSA-WITH-AES-128-CBC-SHA256:TLS-RSA-WITH-AES-256-CBC-SHA256:TLS-RSA-WITH-AES-128-CBC-SHA:TLS-RSA-WITH-AES-256-CBC-SHA"
#define GIT_SSL_DEFAULT_CIPHERS_COUNT 28

static int ciphers_list[GIT_SSL_DEFAULT_CIPHERS_COUNT];

static bool initialized = false;
static mbedtls_ssl_config mbedtls_config;
static mbedtls_ctr_drbg_context mbedtls_rng;
static mbedtls_entropy_context mbedtls_entropy;

static bool has_ca_chain = false;
static mbedtls_x509_crt mbedtls_ca_chain;

/* TLS session cache for resumption (see SECOND DELTA in the header).
 * Valid only on the git thread; keyed by host so a config change can't
 * resume against the wrong server. */
static bool session_valid = false;
static char session_host[64];
static mbedtls_ssl_session saved_session;

/**
 * This function aims to clean-up the SSL context which
 * we allocated.
 */
static void shutdown_ssl(void)
{
	if (session_valid) {
		mbedtls_ssl_session_free(&saved_session);
		session_valid = false;
	}

	if (has_ca_chain) {
		mbedtls_x509_crt_free(&mbedtls_ca_chain);
		has_ca_chain = false;
	}

	if (initialized) {
		mbedtls_ctr_drbg_free(&mbedtls_rng);
		mbedtls_ssl_config_free(&mbedtls_config);
		mbedtls_entropy_free(&mbedtls_entropy);
		initialized = false;
	}
}

int git_mbedtls_stream_global_init(void)
{
	int loaded = 0;
	char *crtpath = GIT_DEFAULT_CERT_LOCATION;
	struct stat statbuf;

	size_t ciphers_known = 0;
	char *cipher_name = NULL;
	char *cipher_string = NULL;
	char *cipher_string_tmp = NULL;

	mbedtls_ssl_config_init(&mbedtls_config);
	mbedtls_entropy_init(&mbedtls_entropy);
	mbedtls_ctr_drbg_init(&mbedtls_rng);

	if (mbedtls_ssl_config_defaults(&mbedtls_config,
	                                MBEDTLS_SSL_IS_CLIENT,
	                                MBEDTLS_SSL_TRANSPORT_STREAM,
	                                MBEDTLS_SSL_PRESET_DEFAULT) != 0) {
		git_error_set(GIT_ERROR_SSL, "failed to initialize mbedTLS");
		goto cleanup;
	}

	/* configure TLSv1.1 or better */
#ifdef MBEDTLS_SSL_MINOR_VERSION_2
	mbedtls_ssl_conf_min_version(&mbedtls_config, MBEDTLS_SSL_MAJOR_VERSION_3, MBEDTLS_SSL_MINOR_VERSION_2);
#endif

	/* verify_server_cert is responsible for making the check.
	 * OPTIONAL because REQUIRED drops the certificate as soon as the check
	 * is made, so we can never see the certificate and override it. */
	mbedtls_ssl_conf_authmode(&mbedtls_config, MBEDTLS_SSL_VERIFY_OPTIONAL);

	/* set the list of allowed ciphersuites */
	ciphers_known = 0;
	cipher_string = cipher_string_tmp = git__strdup(GIT_SSL_DEFAULT_CIPHERS);
	GIT_ERROR_CHECK_ALLOC(cipher_string);

	while ((cipher_name = git__strtok(&cipher_string_tmp, ":")) != NULL) {
		int cipherid = mbedtls_ssl_get_ciphersuite_id(cipher_name);
		if (cipherid == 0) continue;

		if (ciphers_known >= ARRAY_SIZE(ciphers_list)) {
			git_error_set(GIT_ERROR_SSL, "out of cipher list space");
			goto cleanup;
		}

		ciphers_list[ciphers_known++] = cipherid;
	}
	git__free(cipher_string);

	if (!ciphers_known) {
		git_error_set(GIT_ERROR_SSL, "no cipher could be enabled");
		goto cleanup;
	}
	mbedtls_ssl_conf_ciphersuites(&mbedtls_config, ciphers_list);

	/* Seeding the random number generator */

	if (mbedtls_ctr_drbg_seed(&mbedtls_rng, mbedtls_entropy_func,
			&mbedtls_entropy, NULL, 0) != 0) {
		git_error_set(GIT_ERROR_SSL, "failed to initialize mbedTLS entropy pool");
		goto cleanup;
	}

	mbedtls_ssl_conf_rng(&mbedtls_config, mbedtls_ctr_drbg_random, &mbedtls_rng);

	/* load default certificates */
	if (crtpath != NULL && stat(crtpath, &statbuf) == 0 && S_ISREG(statbuf.st_mode))
		loaded = (git_mbedtls__set_cert_location(crtpath, NULL) == 0);

	if (!loaded && crtpath != NULL && stat(crtpath, &statbuf) == 0 && S_ISDIR(statbuf.st_mode))
		loaded = (git_mbedtls__set_cert_location(NULL, crtpath) == 0);

	initialized = true;

	return git_runtime_shutdown_register(shutdown_ssl);

cleanup:
	mbedtls_ctr_drbg_free(&mbedtls_rng);
	mbedtls_ssl_config_free(&mbedtls_config);
	mbedtls_entropy_free(&mbedtls_entropy);

	return -1;
}

static int bio_read(void *b, unsigned char *buf, size_t len)
{
	git_stream *io = (git_stream *) b;
	return (int) git_stream_read(io, buf, min(len, INT_MAX));
}

static int bio_write(void *b, const unsigned char *buf, size_t len)
{
	git_stream *io = (git_stream *) b;
	return (int) git_stream_write(io, (const char *)buf, min(len, INT_MAX), 0);
}

static int ssl_set_error(mbedtls_ssl_context *ssl, int error)
{
	char errbuf[512];
	int ret = -1;

	GIT_ASSERT(error != MBEDTLS_ERR_SSL_WANT_READ);
	GIT_ASSERT(error != MBEDTLS_ERR_SSL_WANT_WRITE);

	if (error != 0)
		mbedtls_strerror( error, errbuf, 512 );

	switch(error) {
		case 0:
		git_error_set(GIT_ERROR_SSL, "SSL error: unknown error");
		break;

	case MBEDTLS_ERR_X509_CERT_VERIFY_FAILED:
		git_error_set(GIT_ERROR_SSL, "SSL error: %#04x [%x] - %s", error, mbedtls_ssl_get_verify_result(ssl), errbuf);
		ret = GIT_ECERTIFICATE;
		break;

	default:
		git_error_set(GIT_ERROR_SSL, "SSL error: %#04x - %s", error, errbuf);
	}

	return ret;
}

static int ssl_teardown(mbedtls_ssl_context *ssl)
{
	int ret = 0;

	ret = mbedtls_ssl_close_notify(ssl);
	if (ret < 0)
		ret = ssl_set_error(ssl, ret);

	mbedtls_ssl_free(ssl);
	return ret;
}

static int verify_server_cert(mbedtls_ssl_context *ssl)
{
	int ret = -1;

	if ((ret = mbedtls_ssl_get_verify_result(ssl)) != 0) {
		char vrfy_buf[512];
		int len = mbedtls_x509_crt_verify_info(vrfy_buf, sizeof(vrfy_buf), "", ret);
		if (len >= 1) vrfy_buf[len - 1] = '\0'; /* Remove trailing \n */
		git_error_set(GIT_ERROR_SSL, "the SSL certificate is invalid: %#04x - %s", ret, vrfy_buf);
		return GIT_ECERTIFICATE;
	}

	return 0;
}

typedef struct {
	git_stream parent;
	git_stream *io;
	int owned;
	bool connected;
	char *host;
	mbedtls_ssl_context *ssl;
	git_cert_x509 cert_info;
} mbedtls_stream;


static int mbedtls_connect(git_stream *stream)
{
	int ret;
	mbedtls_stream *st = (mbedtls_stream *) stream;

	if (st->owned && (ret = git_stream_connect(st->io)) < 0)
		return ret;

	st->connected = true;

	mbedtls_ssl_set_hostname(st->ssl, st->host);

	mbedtls_ssl_set_bio(st->ssl, st->io, bio_write, bio_read, NULL);

	/* Offer the cached session for an abbreviated handshake (best effort —
	 * a refusal by either side just runs the full handshake). */
	if (session_valid && st->host && strcmp(session_host, st->host) == 0)
		(void)mbedtls_ssl_set_session(st->ssl, &saved_session);

	if ((ret = mbedtls_ssl_handshake(st->ssl)) != 0)
		return ssl_set_error(st->ssl, ret);

	return verify_server_cert(st->ssl);
}

/* Snapshot the (possibly ticket-refreshed) session for the next connect.
 * Called at stream close — by then any TLS 1.3 NewSessionTicket sent after
 * the handshake has been processed by the reads. */
static void save_session(mbedtls_stream *st)
{
	mbedtls_ssl_session fresh;

	if (!st->connected || !st->host || strlen(st->host) >= sizeof(session_host))
		return;

	mbedtls_ssl_session_init(&fresh);
	if (mbedtls_ssl_get_session(st->ssl, &fresh) != 0) {
		mbedtls_ssl_session_free(&fresh);
		return;
	}

	if (session_valid)
		mbedtls_ssl_session_free(&saved_session);
	saved_session = fresh; /* shallow move — ownership transfers */
	session_valid = true;
	strcpy(session_host, st->host);
}

static int mbedtls_certificate(git_cert **out, git_stream *stream)
{
	unsigned char *encoded_cert;
	mbedtls_stream *st = (mbedtls_stream *) stream;

	const mbedtls_x509_crt *cert = mbedtls_ssl_get_peer_cert(st->ssl);
	if (!cert) {
		git_error_set(GIT_ERROR_SSL, "the server did not provide a certificate");
		return -1;
	}

	/* Retrieve the length of the certificate first */
	if (cert->raw.len == 0) {
		git_error_set(GIT_ERROR_NET, "failed to retrieve certificate information");
		return -1;
	}

	encoded_cert = git__malloc(cert->raw.len);
	GIT_ERROR_CHECK_ALLOC(encoded_cert);
	memcpy(encoded_cert, cert->raw.p, cert->raw.len);

	st->cert_info.parent.cert_type = GIT_CERT_X509;
	st->cert_info.data = encoded_cert;
	st->cert_info.len = cert->raw.len;

	*out = &st->cert_info.parent;

	return 0;
}

static int mbedtls_set_proxy(git_stream *stream, const git_proxy_options *proxy_options)
{
	mbedtls_stream *st = (mbedtls_stream *) stream;

	return git_stream_set_proxy(st->io, proxy_options);
}

static ssize_t mbedtls_stream_write(git_stream *stream, const char *data, size_t len, int flags)
{
	mbedtls_stream *st = (mbedtls_stream *) stream;
	int written;

	GIT_UNUSED(flags);

	/*
	 * `mbedtls_ssl_write` can only represent INT_MAX bytes
	 * written via its return value. We thus need to clamp
	 * the maximum number of bytes written.
	 */
	len = min(len, INT_MAX);

	if ((written = mbedtls_ssl_write(st->ssl, (const unsigned char *)data, len)) <= 0)
		return ssl_set_error(st->ssl, written);

	return written;
}

static ssize_t mbedtls_stream_read(git_stream *stream, void *data, size_t len)
{
	mbedtls_stream *st = (mbedtls_stream *) stream;
	int ret;

	if ((ret = mbedtls_ssl_read(st->ssl, (unsigned char *)data, len)) <= 0)
		ssl_set_error(st->ssl, ret);

	return ret;
}

static int mbedtls_stream_close(git_stream *stream)
{
	mbedtls_stream *st = (mbedtls_stream *) stream;
	int ret = 0;

	save_session(st);

	if (st->connected && (ret = ssl_teardown(st->ssl)) != 0)
		return -1;

	st->connected = false;

	return st->owned ? git_stream_close(st->io) : 0;
}

static void mbedtls_stream_free(git_stream *stream)
{
	mbedtls_stream *st = (mbedtls_stream *) stream;

	if (st->owned)
		git_stream_free(st->io);

	git__free(st->host);
	git__free(st->cert_info.data);
	mbedtls_ssl_free(st->ssl);
	git__free(st->ssl);
	git__free(st);
}

static int mbedtls_stream_wrap(
	git_stream **out,
	git_stream *in,
	const char *host,
	int owned)
{
	mbedtls_stream *st;
	int error;

	st = git__calloc(1, sizeof(mbedtls_stream));
	GIT_ERROR_CHECK_ALLOC(st);

	st->io = in;
	st->owned = owned;

	st->ssl = git__malloc(sizeof(mbedtls_ssl_context));
	GIT_ERROR_CHECK_ALLOC(st->ssl);
	mbedtls_ssl_init(st->ssl);
	if (mbedtls_ssl_setup(st->ssl, &mbedtls_config)) {
		git_error_set(GIT_ERROR_SSL, "failed to create ssl object");
		error = -1;
		goto out_err;
	}

	st->host = git__strdup(host);
	GIT_ERROR_CHECK_ALLOC(st->host);

	st->parent.version = GIT_STREAM_VERSION;
	st->parent.encrypted = 1;
	st->parent.proxy_support = git_stream_supports_proxy(st->io);
	st->parent.connect = mbedtls_connect;
	st->parent.certificate = mbedtls_certificate;
	st->parent.set_proxy = mbedtls_set_proxy;
	st->parent.read = mbedtls_stream_read;
	st->parent.write = mbedtls_stream_write;
	st->parent.close = mbedtls_stream_close;
	st->parent.free = mbedtls_stream_free;

	*out = (git_stream *) st;
	return 0;

out_err:
	/* ESP FIX: do NOT close/free st->io here — on error the caller keeps
	 * ownership of `in` (git_mbedtls_stream_new closes+frees it right after;
	 * the vendored code freed it here too → double free → tlsf abort). */
	mbedtls_ssl_free(st->ssl);
	git__free(st->ssl);
	git__free(st);

	return error;
}

int git_mbedtls_stream_wrap(
	git_stream **out,
	git_stream *in,
	const char *host)
{
	return mbedtls_stream_wrap(out, in, host, 0);
}

int git_mbedtls_stream_new(
	git_stream **out,
	const char *host,
	const char *port)
{
	git_stream *stream;
	int error;

	GIT_ASSERT_ARG(out);
	GIT_ASSERT_ARG(host);
	GIT_ASSERT_ARG(port);

	if ((error = git_socket_stream_new(&stream, host, port)) < 0)
		return error;

	if ((error = mbedtls_stream_wrap(out, stream, host, 1)) < 0) {
		git_stream_close(stream);
		git_stream_free(stream);
	}

	return error;
}

int git_mbedtls__set_cert_location(const char *file, const char *path)
{
	int ret = 0;
	char errbuf[512];

	GIT_ASSERT_ARG(file || path);

	if (has_ca_chain)
		mbedtls_x509_crt_free(&mbedtls_ca_chain);

	mbedtls_x509_crt_init(&mbedtls_ca_chain);

	if (file)
		ret = mbedtls_x509_crt_parse_file(&mbedtls_ca_chain, file);

	if (ret >= 0 && path)
		ret = mbedtls_x509_crt_parse_path(&mbedtls_ca_chain, path);

	/* mbedtls_x509_crt_parse_path returns the number of invalid certs on success */
	if (ret < 0) {
		mbedtls_x509_crt_free(&mbedtls_ca_chain);
		mbedtls_strerror( ret, errbuf, 512 );
		git_error_set(GIT_ERROR_SSL, "failed to load CA certificates: %#04x - %s", ret, errbuf);
		return -1;
	}

	mbedtls_ssl_conf_ca_chain(&mbedtls_config, &mbedtls_ca_chain, NULL);
	has_ca_chain = true;

	return 0;
}

#else

#include "stream.h"

int git_mbedtls_stream_global_init(void)
{
	return 0;
}

#endif
