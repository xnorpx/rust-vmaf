#ifndef VMAF_MSVC_BUILTINS_H
#define VMAF_MSVC_BUILTINS_H

#include <intrin.h>

static inline int vmaf_builtin_clz(unsigned value)
{
    unsigned long index;
    return _BitScanReverse(&index, value) ? 31 - (int)index : 32;
}

static inline int vmaf_builtin_clzll(unsigned long long value)
{
#if defined(_M_X64) || defined(_M_ARM64)
    unsigned long index;
    return _BitScanReverse64(&index, value) ? 63 - (int)index : 64;
#else
    const unsigned high = (unsigned)(value >> 32);
    return high ? vmaf_builtin_clz(high) : 32 + vmaf_builtin_clz((unsigned)value);
#endif
}

#define __builtin_clz vmaf_builtin_clz
#define __builtin_clzll vmaf_builtin_clzll

#endif