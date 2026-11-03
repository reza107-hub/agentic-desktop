#ifndef AGENTIC_DESKTOP_H
#define AGENTIC_DESKTOP_H

/*
 * agentic-desktop C library.
 *
 * Run any command-line command and get back the same JSON line the CLI prints.
 *
 * Rules:
 *  - Pass the arguments after the program name as a JSON array of strings, for example
 *    "[\"snapshot\",\"--app\",\"Calculator\",\"-i\"]".
 *  - Free every string returned by ad_execute with ad_free_string, never with free().
 *  - Commands that move the real mouse or press real keys need "--headed" in the array.
 *  - Build the library with:  cargo build --profile release-ffi -p agentic-desktop-ffi
 *    (this profile unwinds panics, so a bug cannot abort your program).
 */

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

/* Compare with ad_abi_version() at start-up. It changes when old callers would break. */
#define AD_ABI_VERSION 1

uint32_t ad_abi_version(void);

/* Library version as a static string. Do not free it. */
const char *ad_version(void);

/* Run a command. Returns a JSON string, or NULL only when memory runs out. */
char *ad_execute(const char *args_json);

/* Free a string returned by ad_execute. NULL is allowed. */
void ad_free_string(char *text);

#ifdef __cplusplus
}
#endif

#endif /* AGENTIC_DESKTOP_H */
