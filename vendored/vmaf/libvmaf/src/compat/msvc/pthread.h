#ifndef VMAF_PTHREAD_COMPAT_H
#define VMAF_PTHREAD_COMPAT_H

#include <errno.h>
#include <process.h>
#include <stdint.h>
#include <stdlib.h>
#include <windows.h>

typedef SRWLOCK pthread_mutex_t;
typedef CONDITION_VARIABLE pthread_cond_t;
typedef HANDLE pthread_t;

typedef struct VmafPthreadStartContext {
    void *(*start_routine)(void *);
    void *argument;
} VmafPthreadStartContext;

static unsigned __stdcall vmaf_pthread_start(void *raw_context)
{
    VmafPthreadStartContext *context = raw_context;
    void *(*start_routine)(void *) = context->start_routine;
    void *argument = context->argument;
    free(context);
    start_routine(argument);
    return 0;
}

static inline int pthread_mutex_init(pthread_mutex_t *mutex,
                                     const void *attributes)
{
    (void)attributes;
    InitializeSRWLock(mutex);
    return 0;
}

static inline int pthread_mutex_destroy(pthread_mutex_t *mutex)
{
    (void)mutex;
    return 0;
}

static inline int pthread_mutex_lock(pthread_mutex_t *mutex)
{
    AcquireSRWLockExclusive(mutex);
    return 0;
}

static inline int pthread_mutex_unlock(pthread_mutex_t *mutex)
{
    ReleaseSRWLockExclusive(mutex);
    return 0;
}

static inline int pthread_cond_init(pthread_cond_t *condition,
                                    const void *attributes)
{
    (void)attributes;
    InitializeConditionVariable(condition);
    return 0;
}

static inline int pthread_cond_destroy(pthread_cond_t *condition)
{
    (void)condition;
    return 0;
}

static inline int pthread_cond_wait(pthread_cond_t *condition,
                                    pthread_mutex_t *mutex)
{
    if (SleepConditionVariableSRW(condition, mutex, INFINITE, 0)) return 0;
    const DWORD error = GetLastError();
    return error ? (int)error : EINVAL;
}

static inline int pthread_cond_signal(pthread_cond_t *condition)
{
    WakeConditionVariable(condition);
    return 0;
}

static inline int pthread_cond_broadcast(pthread_cond_t *condition)
{
    WakeAllConditionVariable(condition);
    return 0;
}

static inline int pthread_create(pthread_t *thread, const void *attributes,
                                 void *(*start_routine)(void *), void *argument)
{
    (void)attributes;
    VmafPthreadStartContext *context = malloc(sizeof(*context));
    if (!context) return ENOMEM;
    context->start_routine = start_routine;
    context->argument = argument;

    const uintptr_t handle =
        _beginthreadex(NULL, 0, vmaf_pthread_start, context, 0, NULL);
    if (!handle) {
        const int error = errno ? errno : EAGAIN;
        free(context);
        return error;
    }

    *thread = (HANDLE)handle;
    return 0;
}

static inline int pthread_detach(pthread_t thread)
{
    if (CloseHandle(thread)) return 0;
    const DWORD error = GetLastError();
    return error ? (int)error : EINVAL;
}

#endif