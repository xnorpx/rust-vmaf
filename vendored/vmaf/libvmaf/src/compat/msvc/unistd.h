#ifndef VMAF_MSVC_UNISTD_H
#define VMAF_MSVC_UNISTD_H

#include <direct.h>
#include <io.h>
#include <sys/types.h>

typedef _mode_t mode_t;

#define fileno _fileno
#define isatty _isatty
#define mkdir _mkdir

#endif
