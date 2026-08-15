#pragma once
#define __BYTE_ORDER __LITTLE_ENDIAN
#define __LITTLE_ENDIAN 1234
#define __BIG_ENDIAN 4321
#define le16toh(x) ((unsigned short)(x))
#define le32toh(x) ((unsigned int)(x))
#define le64toh(x) ((unsigned long long)(x))
#define htole16(x) ((unsigned short)(x))
#define htole32(x) ((unsigned int)(x))
#define htole64(x) ((unsigned long long)(x))
#define be16toh(x) __builtin_bswap16(x)
#define be32toh(x) __builtin_bswap32(x)
#define be64toh(x) __builtin_bswap64(x)
