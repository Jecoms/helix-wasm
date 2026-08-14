/* Endian conversions for wasm32-unknown-unknown, which is little-endian.
 * Picked up by tree-sitter's portable/endian.h via HAVE_ENDIAN_H (defined in
 * vendor/tree-house-bindings/build.rs). */
#ifndef _WASM_SYSROOT_ENDIAN_H
#define _WASM_SYSROOT_ENDIAN_H

#define __LITTLE_ENDIAN 1234
#define __BIG_ENDIAN 4321
#define __PDP_ENDIAN 3412
#define __BYTE_ORDER __LITTLE_ENDIAN

#define htole16(x) ((unsigned short)(x))
#define le16toh(x) ((unsigned short)(x))
#define htobe16(x) __builtin_bswap16(x)
#define be16toh(x) __builtin_bswap16(x)

#define htole32(x) ((unsigned int)(x))
#define le32toh(x) ((unsigned int)(x))
#define htobe32(x) __builtin_bswap32(x)
#define be32toh(x) __builtin_bswap32(x)

#define htole64(x) ((unsigned long long)(x))
#define le64toh(x) ((unsigned long long)(x))
#define htobe64(x) __builtin_bswap64(x)
#define be64toh(x) __builtin_bswap64(x)

#endif
